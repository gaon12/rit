use crate::{
    GitObject, ObjectId, ObjectKind, Result, RitError, hash_object, object::parse_loose_object,
};
use flate2::Compression;
use flate2::read::ZlibDecoder;
use flate2::write::ZlibEncoder;
use std::fs;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

/// Reader for Git loose objects under `.git/objects`.
#[derive(Clone, Debug)]
pub struct LooseObjectDb {
    objects_dir: PathBuf,
}

impl LooseObjectDb {
    /// Creates a loose object reader rooted at an `objects` directory.
    pub fn new(objects_dir: impl Into<PathBuf>) -> Self {
        Self {
            objects_dir: objects_dir.into(),
        }
    }

    /// Reads and validates one loose object.
    pub fn read_object(&self, object_id: ObjectId) -> Result<GitObject> {
        let path = self.loose_object_path(object_id);
        if path.is_file() {
            let compressed = fs::read(&path).map_err(|source| RitError::io(&path, source))?;
            let mut decoder = ZlibDecoder::new(compressed.as_slice());
            let mut raw = Vec::new();
            decoder
                .read_to_end(&mut raw)
                .map_err(|source| RitError::io(&path, source))?;
            return parse_loose_object(&raw);
        }

        self.read_packed_object(object_id)?
            .ok_or_else(|| RitError::ObjectNotFound {
                object_id: object_id.to_hex(),
            })
    }

    /// Writes a loose object if it is not already present.
    pub fn write_object(&self, kind: ObjectKind, data: &[u8]) -> Result<ObjectId> {
        let object_id = hash_object(kind, data);
        let path = self.loose_object_path(object_id);
        if path.exists() {
            return Ok(object_id);
        }

        let parent = path
            .parent()
            .ok_or_else(|| RitError::invalid_input("loose object path has no parent"))?;
        fs::create_dir_all(parent).map_err(|source| RitError::io(parent, source))?;

        let mut raw = Vec::new();
        raw.extend_from_slice(kind.to_str().as_bytes());
        raw.push(b' ');
        raw.extend_from_slice(data.len().to_string().as_bytes());
        raw.push(0);
        raw.extend_from_slice(data);

        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
        encoder
            .write_all(&raw)
            .map_err(|source| RitError::io(&path, source))?;
        let compressed = encoder
            .finish()
            .map_err(|source| RitError::io(&path, source))?;
        write_new_file_atomically(&path, &compressed)
            .or_else(|error| if path.exists() { Ok(()) } else { Err(error) })?;
        Ok(object_id)
    }

    /// Returns the path where a loose object should live.
    pub fn loose_object_path(&self, object_id: ObjectId) -> PathBuf {
        let hex = object_id.to_hex();
        self.objects_dir.join(&hex[..2]).join(&hex[2..])
    }

    /// Returns the root objects directory.
    pub fn objects_dir(&self) -> &Path {
        &self.objects_dir
    }

    fn read_packed_object(&self, object_id: ObjectId) -> Result<Option<GitObject>> {
        let pack_dir = self.objects_dir.join("pack");
        if !pack_dir.exists() {
            return Ok(None);
        }

        for entry in fs::read_dir(&pack_dir).map_err(|source| RitError::io(&pack_dir, source))? {
            let entry = entry.map_err(|source| RitError::io(&pack_dir, source))?;
            let index_path = entry.path();
            if index_path
                .extension()
                .and_then(|extension| extension.to_str())
                != Some("idx")
            {
                continue;
            }
            let Some(offset) = find_pack_offset(&index_path, object_id)? else {
                continue;
            };
            let pack_path = index_path.with_extension("pack");
            return read_pack_object_at(&pack_path, offset).map(Some);
        }

        Ok(None)
    }
}

fn find_pack_offset(index_path: &Path, object_id: ObjectId) -> Result<Option<u64>> {
    let bytes = fs::read(index_path).map_err(|source| RitError::io(index_path, source))?;
    if bytes.len() < 8 || &bytes[..4] != b"\xfftOc" {
        return Err(RitError::invalid_input(format!(
            "unsupported pack index format: {}",
            index_path.display()
        )));
    }
    let version = read_u32(&bytes, 4)?;
    if version != 2 {
        return Err(RitError::invalid_input(format!(
            "unsupported pack index version: {version}"
        )));
    }

    let fanout_start = 8;
    let object_count = read_u32(&bytes, fanout_start + 255 * 4)? as usize;
    let names_start = fanout_start + 256 * 4;
    let crc_start = names_start + object_count * 20;
    let offsets_start = crc_start + object_count * 4;
    let large_offsets_start = offsets_start + object_count * 4;
    if bytes.len() < large_offsets_start {
        return Err(RitError::invalid_input("pack index is truncated"));
    }

    let first_byte = object_id.as_bytes()[0] as usize;
    let low = if first_byte == 0 {
        0
    } else {
        read_u32(&bytes, fanout_start + (first_byte - 1) * 4)? as usize
    };
    let high = read_u32(&bytes, fanout_start + first_byte * 4)? as usize;
    let mut left = low;
    let mut right = high;

    while left < right {
        let middle = (left + right) / 2;
        let name_start = names_start + middle * 20;
        let name = bytes
            .get(name_start..name_start + 20)
            .ok_or_else(|| RitError::invalid_input("pack index object name table is truncated"))?;
        match name.cmp(object_id.as_bytes()) {
            std::cmp::Ordering::Less => left = middle + 1,
            std::cmp::Ordering::Greater => right = middle,
            std::cmp::Ordering::Equal => {
                let offset_value = read_u32(&bytes, offsets_start + middle * 4)?;
                if offset_value & 0x8000_0000 == 0 {
                    return Ok(Some(offset_value as u64));
                }
                let large_index = (offset_value & 0x7fff_ffff) as usize;
                return Ok(Some(read_u64(
                    &bytes,
                    large_offsets_start + large_index * 8,
                )?));
            }
        }
    }

    Ok(None)
}

fn read_pack_object_at(pack_path: &Path, offset: u64) -> Result<GitObject> {
    let mut file = fs::File::open(pack_path).map_err(|source| RitError::io(pack_path, source))?;
    validate_pack_header(&mut file, pack_path)?;
    file.seek(SeekFrom::Start(offset))
        .map_err(|source| RitError::io(pack_path, source))?;

    let (kind, size) = read_pack_object_header(&mut file, pack_path)?;
    let mut decoder = ZlibDecoder::new(file);
    let mut data = Vec::new();
    decoder
        .read_to_end(&mut data)
        .map_err(|source| RitError::io(pack_path, source))?;
    if data.len() != size {
        return Err(RitError::invalid_input(format!(
            "pack object size mismatch: header says {size}, payload is {}",
            data.len()
        )));
    }
    Ok(GitObject { kind, data })
}

fn validate_pack_header(file: &mut fs::File, pack_path: &Path) -> Result<()> {
    let mut header = [0_u8; 12];
    file.read_exact(&mut header)
        .map_err(|source| RitError::io(pack_path, source))?;
    if &header[..4] != b"PACK" {
        return Err(RitError::invalid_input(format!(
            "invalid pack header: {}",
            pack_path.display()
        )));
    }
    let version = u32::from_be_bytes([header[4], header[5], header[6], header[7]]);
    if !matches!(version, 2 | 3) {
        return Err(RitError::invalid_input(format!(
            "unsupported pack version: {version}"
        )));
    }
    Ok(())
}

fn read_pack_object_header(file: &mut fs::File, pack_path: &Path) -> Result<(ObjectKind, usize)> {
    let mut byte = read_byte(file, pack_path)?;
    let object_type = (byte >> 4) & 0b111;
    let mut size = (byte & 0b1111) as usize;
    let mut shift = 4;
    while byte & 0b1000_0000 != 0 {
        byte = read_byte(file, pack_path)?;
        size |= ((byte & 0b0111_1111) as usize) << shift;
        shift += 7;
    }
    let kind = match object_type {
        1 => ObjectKind::Commit,
        2 => ObjectKind::Tree,
        3 => ObjectKind::Blob,
        4 => ObjectKind::Tag,
        6 | 7 => {
            return Err(RitError::invalid_input(
                "delta-compressed pack objects are not implemented yet",
            ));
        }
        _ => {
            return Err(RitError::invalid_input(format!(
                "unknown pack object type: {object_type}"
            )));
        }
    };
    Ok((kind, size))
}

fn read_byte(file: &mut fs::File, path: &Path) -> Result<u8> {
    let mut byte = [0_u8; 1];
    file.read_exact(&mut byte)
        .map_err(|source| RitError::io(path, source))?;
    Ok(byte[0])
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32> {
    let slice = bytes
        .get(offset..offset + 4)
        .ok_or_else(|| RitError::invalid_input("pack index integer is truncated"))?;
    Ok(u32::from_be_bytes([slice[0], slice[1], slice[2], slice[3]]))
}

fn read_u64(bytes: &[u8], offset: usize) -> Result<u64> {
    let slice = bytes
        .get(offset..offset + 8)
        .ok_or_else(|| RitError::invalid_input("pack index large offset is truncated"))?;
    Ok(u64::from_be_bytes([
        slice[0], slice[1], slice[2], slice[3], slice[4], slice[5], slice[6], slice[7],
    ]))
}

fn write_new_file_atomically(path: &Path, contents: &[u8]) -> Result<()> {
    let temp_path = path.with_extension(format!("tmp-{}", std::process::id()));
    {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp_path)
            .map_err(|source| RitError::io(&temp_path, source))?;
        file.write_all(contents)
            .map_err(|source| RitError::io(&temp_path, source))?;
        file.sync_all()
            .map_err(|source| RitError::io(&temp_path, source))?;
    }
    fs::rename(&temp_path, path).map_err(|source| RitError::io(path, source))?;
    Ok(())
}
