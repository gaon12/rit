use crate::{ObjectId, Result, RitError, object::sha1_bytes};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

/// Parsed Git index.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Index {
    /// Tracked entries in index order.
    pub entries: Vec<IndexEntry>,
}

/// One tracked path in the Git index.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IndexEntry {
    /// Git file mode stored in the index.
    pub mode: u32,
    /// Object ID stored for this path.
    pub object_id: ObjectId,
    /// File size stored in the index.
    pub file_size: u32,
    /// Repository-relative path using `/` separators.
    pub path: String,
}

impl Index {
    /// Reads `.git/index`. Missing indexes are treated as empty.
    pub fn read(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self {
                entries: Vec::new(),
            });
        }

        let bytes = fs::read(path).map_err(|source| RitError::io(path, source))?;
        parse_index(&bytes)
    }

    /// Writes this index as Git index v2.
    pub fn write(&self, path: &Path) -> Result<()> {
        let mut entries = self.entries.clone();
        entries.sort_by(|left, right| left.path.cmp(&right.path));

        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"DIRC");
        bytes.extend_from_slice(&2_u32.to_be_bytes());
        bytes.extend_from_slice(&(entries.len() as u32).to_be_bytes());

        for entry in entries {
            let entry_start = bytes.len();
            bytes.extend_from_slice(&0_u32.to_be_bytes());
            bytes.extend_from_slice(&0_u32.to_be_bytes());
            bytes.extend_from_slice(&0_u32.to_be_bytes());
            bytes.extend_from_slice(&0_u32.to_be_bytes());
            bytes.extend_from_slice(&0_u32.to_be_bytes());
            bytes.extend_from_slice(&0_u32.to_be_bytes());
            bytes.extend_from_slice(&entry.mode.to_be_bytes());
            bytes.extend_from_slice(&0_u32.to_be_bytes());
            bytes.extend_from_slice(&0_u32.to_be_bytes());
            bytes.extend_from_slice(&entry.file_size.to_be_bytes());
            bytes.extend_from_slice(entry.object_id.as_bytes());
            let flags = entry.path.len().min(0x0fff) as u16;
            bytes.extend_from_slice(&flags.to_be_bytes());
            bytes.extend_from_slice(entry.path.as_bytes());
            bytes.push(0);
            while (bytes.len() - entry_start) % 8 != 0 {
                bytes.push(0);
            }
        }

        let checksum = sha1_bytes(&bytes);
        bytes.extend_from_slice(&checksum);
        write_file_atomically(path, &bytes)
    }
}

fn parse_index(bytes: &[u8]) -> Result<Index> {
    if bytes.len() < 12 || &bytes[..4] != b"DIRC" {
        return Err(RitError::invalid_input("index header is invalid"));
    }

    let version = read_u32(bytes, 4)?;
    if !matches!(version, 2 | 3) {
        return Err(RitError::invalid_input(format!(
            "unsupported index version: {version}"
        )));
    }
    let entry_count = read_u32(bytes, 8)? as usize;
    let mut offset = 12;
    let mut entries = Vec::with_capacity(entry_count);

    for _ in 0..entry_count {
        let entry_start = offset;
        if bytes.len().saturating_sub(offset) < 62 {
            return Err(RitError::invalid_input("index entry is truncated"));
        }

        let mode = read_u32(bytes, offset + 24)?;
        let file_size = read_u32(bytes, offset + 36)?;
        let mut object_id = [0_u8; 20];
        object_id.copy_from_slice(&bytes[offset + 40..offset + 60]);
        let flags = read_u16(bytes, offset + 60)?;
        offset += 62;

        if flags & 0x4000 != 0 {
            if bytes.len().saturating_sub(offset) < 2 {
                return Err(RitError::invalid_input(
                    "index extended flags are truncated",
                ));
            }
            offset += 2;
        }

        let path_start = offset;
        while offset < bytes.len() && bytes[offset] != 0 {
            offset += 1;
        }
        if offset == bytes.len() {
            return Err(RitError::invalid_input(
                "index entry path is not NUL terminated",
            ));
        }
        let path = std::str::from_utf8(&bytes[path_start..offset])
            .map_err(|_| RitError::invalid_input("index path is not UTF-8"))?
            .replace('\\', "/");
        offset += 1;

        let entry_len = offset - entry_start;
        let padding = (8 - (entry_len % 8)) % 8;
        if bytes.len().saturating_sub(offset) < padding {
            return Err(RitError::invalid_input("index entry padding is truncated"));
        }
        offset += padding;

        entries.push(IndexEntry {
            mode,
            object_id: ObjectId::from_bytes(object_id),
            file_size,
            path,
        });
    }

    Ok(Index { entries })
}

fn write_file_atomically(path: &Path, contents: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| RitError::io(parent, source))?;
    }
    let lock_path = path.with_extension("lock");
    {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&lock_path)
            .map_err(|source| RitError::io(&lock_path, source))?;
        file.write_all(contents)
            .map_err(|source| RitError::io(&lock_path, source))?;
        file.sync_all()
            .map_err(|source| RitError::io(&lock_path, source))?;
    }
    fs::rename(&lock_path, path).map_err(|source| RitError::io(path, source))?;
    Ok(())
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32> {
    if bytes.len().saturating_sub(offset) < 4 {
        return Err(RitError::invalid_input("index integer is truncated"));
    }
    Ok(u32::from_be_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
    ]))
}

fn read_u16(bytes: &[u8], offset: usize) -> Result<u16> {
    if bytes.len().saturating_sub(offset) < 2 {
        return Err(RitError::invalid_input("index integer is truncated"));
    }
    Ok(u16::from_be_bytes([bytes[offset], bytes[offset + 1]]))
}

/// Converts an absolute or joined path to a repository-relative slash path.
pub fn relative_slash_path(root: &Path, path: &Path) -> Result<String> {
    let relative = path.strip_prefix(root).map_err(|_| {
        RitError::invalid_input(format!(
            "path {} is outside repository root {}",
            path.display(),
            root.display()
        ))
    })?;
    Ok(relative
        .components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/"))
}

/// Converts a repository-relative slash path to a platform path under `root`.
pub fn join_slash_path(root: &Path, path: &str) -> PathBuf {
    path.split('/')
        .fold(root.to_path_buf(), |path, component| path.join(component))
}

#[cfg(test)]
mod tests {
    use super::{join_slash_path, relative_slash_path};
    use std::path::Path;

    #[test]
    fn slash_paths_round_trip() {
        let root = Path::new("repo");
        let path = root.join("src").join("main.rs");
        let slash = relative_slash_path(root, &path).expect("path should be relative");

        assert_eq!(slash, "src/main.rs");
        assert_eq!(join_slash_path(root, &slash), path);
    }
}
