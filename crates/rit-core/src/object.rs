use crate::{Result, RitError};
use std::fmt::{Display, Formatter};

/// Git object kind stored in an object header.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ObjectKind {
    /// File contents.
    Blob,
    /// Directory listing.
    Tree,
    /// Commit metadata and message.
    Commit,
    /// Annotated tag metadata and message.
    Tag,
}

impl ObjectKind {
    /// Parses a Git object kind name.
    pub fn parse(name: &str) -> Result<Self> {
        match name {
            "blob" => Ok(Self::Blob),
            "tree" => Ok(Self::Tree),
            "commit" => Ok(Self::Commit),
            "tag" => Ok(Self::Tag),
            _ => Err(RitError::invalid_input(format!(
                "unsupported object type: {name}"
            ))),
        }
    }

    /// Infers the printed entry type from a tree mode.
    pub fn from_tree_mode(mode: &str) -> Self {
        match mode {
            "40000" => Self::Tree,
            "160000" => Self::Commit,
            _ => Self::Blob,
        }
    }

    fn as_bytes(self) -> &'static [u8] {
        self.to_str().as_bytes()
    }

    /// Returns the canonical Git object type name.
    pub fn to_str(self) -> &'static str {
        match self {
            Self::Blob => "blob",
            Self::Tree => "tree",
            Self::Commit => "commit",
            Self::Tag => "tag",
        }
    }
}

impl Display for ObjectKind {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.to_str())
    }
}

/// A 20-byte SHA-1 Git object ID.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ObjectId([u8; 20]);

impl ObjectId {
    /// Parses a full 40-character hexadecimal object ID.
    pub fn from_hex(hex: &str) -> Result<Self> {
        if hex.len() != 40 {
            return Err(RitError::invalid_input(format!(
                "object id must be 40 hex characters: {hex}"
            )));
        }

        let mut bytes = [0_u8; 20];
        for (index, byte) in bytes.iter_mut().enumerate() {
            let high = decode_hex_digit(hex.as_bytes()[index * 2])?;
            let low = decode_hex_digit(hex.as_bytes()[index * 2 + 1])?;
            *byte = (high << 4) | low;
        }

        Ok(Self(bytes))
    }

    /// Builds an object ID from raw SHA-1 bytes.
    pub fn from_bytes(bytes: [u8; 20]) -> Self {
        Self(bytes)
    }

    /// Returns the raw SHA-1 bytes.
    pub fn as_bytes(&self) -> &[u8; 20] {
        &self.0
    }

    /// Formats this object ID as lowercase hexadecimal.
    pub fn to_hex(self) -> String {
        let mut output = String::with_capacity(40);
        for byte in self.0 {
            output.push(hex_digit(byte >> 4));
            output.push(hex_digit(byte & 0x0f));
        }
        output
    }
}

impl Display for ObjectId {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.to_hex())
    }
}

/// A decompressed Git object.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GitObject {
    /// Object kind from the loose object header.
    pub kind: ObjectKind,
    /// Raw object payload after the header NUL byte.
    pub data: Vec<u8>,
}

impl GitObject {
    /// Returns the object payload size.
    pub fn size(&self) -> usize {
        self.data.len()
    }
}

/// One entry inside a tree object.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TreeEntry {
    /// Git file mode as stored in the tree object.
    pub mode: String,
    /// Printed object kind inferred from `mode`.
    pub kind: ObjectKind,
    /// Child object ID.
    pub object_id: ObjectId,
    /// Raw path bytes for this entry.
    pub name: Vec<u8>,
}

impl TreeEntry {
    /// Returns the entry name using replacement characters for invalid UTF-8.
    pub fn name_lossy(&self) -> String {
        String::from_utf8_lossy(&self.name).into_owned()
    }
}

/// Parses a decompressed loose object buffer.
pub fn parse_loose_object(raw: &[u8]) -> Result<GitObject> {
    let Some(header_end) = raw.iter().position(|byte| *byte == 0) else {
        return Err(RitError::invalid_input(
            "loose object is missing header terminator",
        ));
    };
    let header = std::str::from_utf8(&raw[..header_end])
        .map_err(|_| RitError::invalid_input("loose object header is not UTF-8"))?;
    let Some((kind_name, size_text)) = header.split_once(' ') else {
        return Err(RitError::invalid_input("loose object header is malformed"));
    };

    let kind = ObjectKind::parse(kind_name)?;
    let expected_size = size_text
        .parse::<usize>()
        .map_err(|_| RitError::invalid_input("loose object size is not a number"))?;
    let data = raw[header_end + 1..].to_vec();
    if data.len() != expected_size {
        return Err(RitError::invalid_input(format!(
            "loose object size mismatch: header says {expected_size}, payload is {}",
            data.len()
        )));
    }

    Ok(GitObject { kind, data })
}

/// Parses all entries in a tree object payload.
pub fn parse_tree_entries(data: &[u8]) -> Result<Vec<TreeEntry>> {
    let mut entries = Vec::new();
    let mut index = 0;

    while index < data.len() {
        let mode_start = index;
        while index < data.len() && data[index] != b' ' {
            index += 1;
        }
        if index == data.len() {
            return Err(RitError::invalid_input(
                "tree entry is missing mode separator",
            ));
        }
        let mode = std::str::from_utf8(&data[mode_start..index])
            .map_err(|_| RitError::invalid_input("tree entry mode is not UTF-8"))?
            .to_owned();
        index += 1;

        let name_start = index;
        while index < data.len() && data[index] != 0 {
            index += 1;
        }
        if index == data.len() {
            return Err(RitError::invalid_input(
                "tree entry is missing name terminator",
            ));
        }
        let name = data[name_start..index].to_vec();
        index += 1;

        if data.len().saturating_sub(index) < 20 {
            return Err(RitError::invalid_input("tree entry is missing object id"));
        }
        let mut object_id = [0_u8; 20];
        object_id.copy_from_slice(&data[index..index + 20]);
        index += 20;

        entries.push(TreeEntry {
            kind: ObjectKind::from_tree_mode(&mode),
            mode,
            object_id: ObjectId::from_bytes(object_id),
            name,
        });
    }

    Ok(entries)
}

/// Computes a Git-compatible object ID for `kind` and `data`.
pub fn hash_object(kind: ObjectKind, data: &[u8]) -> ObjectId {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(kind.as_bytes());
    bytes.push(b' ');
    bytes.extend_from_slice(data.len().to_string().as_bytes());
    bytes.push(0);
    bytes.extend_from_slice(data);
    ObjectId::from_bytes(sha1_bytes(&bytes))
}

/// Computes raw SHA-1 bytes.
pub fn sha1_bytes(input: &[u8]) -> [u8; 20] {
    sha1(input)
}

fn decode_hex_digit(byte: u8) -> Result<u8> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        b'A'..=b'F' => Ok(byte - b'A' + 10),
        _ => Err(RitError::invalid_input(
            "object id contains a non-hex digit",
        )),
    }
}

fn hex_digit(value: u8) -> char {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    DIGITS[value as usize] as char
}

fn sha1(input: &[u8]) -> [u8; 20] {
    let mut h0 = 0x6745_2301_u32;
    let mut h1 = 0xefcd_ab89_u32;
    let mut h2 = 0x98ba_dcfe_u32;
    let mut h3 = 0x1032_5476_u32;
    let mut h4 = 0xc3d2_e1f0_u32;

    let bit_len = (input.len() as u64) * 8;
    let mut message = input.to_vec();
    message.push(0x80);
    while message.len() % 64 != 56 {
        message.push(0);
    }
    message.extend_from_slice(&bit_len.to_be_bytes());

    for chunk in message.chunks_exact(64) {
        let mut words = [0_u32; 80];
        for (index, bytes) in chunk.chunks_exact(4).enumerate().take(16) {
            words[index] = u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        }
        for index in 16..80 {
            words[index] =
                (words[index - 3] ^ words[index - 8] ^ words[index - 14] ^ words[index - 16])
                    .rotate_left(1);
        }

        let mut a = h0;
        let mut b = h1;
        let mut c = h2;
        let mut d = h3;
        let mut e = h4;

        for (index, word) in words.iter().enumerate() {
            let (function, constant) = match index {
                0..=19 => (((b & c) | ((!b) & d)), 0x5a82_7999),
                20..=39 => ((b ^ c ^ d), 0x6ed9_eba1),
                40..=59 => (((b & c) | (b & d) | (c & d)), 0x8f1b_bcdc),
                _ => ((b ^ c ^ d), 0xca62_c1d6),
            };
            let temp = a
                .rotate_left(5)
                .wrapping_add(function)
                .wrapping_add(e)
                .wrapping_add(constant)
                .wrapping_add(*word);
            e = d;
            d = c;
            c = b.rotate_left(30);
            b = a;
            a = temp;
        }

        h0 = h0.wrapping_add(a);
        h1 = h1.wrapping_add(b);
        h2 = h2.wrapping_add(c);
        h3 = h3.wrapping_add(d);
        h4 = h4.wrapping_add(e);
    }

    let mut output = [0_u8; 20];
    output[..4].copy_from_slice(&h0.to_be_bytes());
    output[4..8].copy_from_slice(&h1.to_be_bytes());
    output[8..12].copy_from_slice(&h2.to_be_bytes());
    output[12..16].copy_from_slice(&h3.to_be_bytes());
    output[16..20].copy_from_slice(&h4.to_be_bytes());
    output
}

#[cfg(test)]
mod tests {
    use super::{ObjectId, ObjectKind, hash_object, parse_loose_object, parse_tree_entries};

    #[test]
    fn parses_loose_blob() {
        let object = parse_loose_object(b"blob 5\0hello").expect("blob should parse");

        assert_eq!(object.kind, ObjectKind::Blob);
        assert_eq!(object.data, b"hello");
    }

    #[test]
    fn hashes_blob_like_git() {
        let object_id = hash_object(ObjectKind::Blob, b"hello\n");

        assert_eq!(
            object_id.to_hex(),
            "ce013625030ba8dba906f756967f9e9ca394464a"
        );
    }

    #[test]
    fn parses_tree_entries() {
        let object_id =
            ObjectId::from_hex("ce013625030ba8dba906f756967f9e9ca394464a").expect("valid oid");
        let mut data = b"100644 hello.txt\0".to_vec();
        data.extend_from_slice(object_id.as_bytes());

        let entries = parse_tree_entries(&data).expect("tree should parse");

        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].mode, "100644");
        assert_eq!(entries[0].kind, ObjectKind::Blob);
        assert_eq!(entries[0].name_lossy(), "hello.txt");
        assert_eq!(entries[0].object_id, object_id);
    }
}
