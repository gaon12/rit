use crate::{ObjectId, Result, RitError, object::sha1_bytes};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// Parsed Git index.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Index {
    /// Tracked entries in index order.
    pub entries: Vec<IndexEntry>,
    /// Raw optional index extensions preserved when entries are only refreshed.
    pub extensions: Vec<u8>,
}

/// One optional extension record stored after index entries.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IndexExtension {
    /// Four-byte extension signature.
    pub signature: [u8; 4],
    /// Recognized extension kind, or `Unknown`.
    pub kind: IndexExtensionKind,
    /// Raw payload bytes for callers that need extension-specific parsing.
    pub data: Vec<u8>,
}

impl IndexExtension {
    /// Returns the extension signature as readable ASCII when possible.
    pub fn signature_text(&self) -> String {
        String::from_utf8_lossy(&self.signature).into_owned()
    }
}

/// Known Git index extension signatures.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum IndexExtensionKind {
    /// Cache-tree extension (`TREE`).
    CacheTree,
    /// Resolve-undo extension (`REUC`).
    ResolveUndo,
    /// Untracked-cache extension (`UNTR`).
    UntrackedCache,
    /// File-system monitor extension (`FSMN`).
    FsMonitor,
    /// Split-index link extension (`link`).
    SplitIndexLink,
    /// Sparse-directory extension (`sdir`).
    SparseDirectory,
    /// Extension not classified by this version of rit.
    Unknown([u8; 4]),
}

/// One tracked path in the Git index.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IndexEntry {
    /// Cached filesystem stat fields stored in the index.
    pub stat: IndexEntryStat,
    /// Git file mode stored in the index.
    pub mode: u32,
    /// Object ID stored for this path.
    pub object_id: ObjectId,
    /// File size stored in the index.
    pub file_size: u32,
    /// Repository-relative path using `/` separators.
    pub path: String,
}

/// Cached filesystem stat fields for one index entry.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct IndexEntryStat {
    /// Change-time seconds since the Unix epoch.
    pub ctime_seconds: u32,
    /// Change-time nanoseconds.
    pub ctime_nanoseconds: u32,
    /// Modification-time seconds since the Unix epoch.
    pub mtime_seconds: u32,
    /// Modification-time nanoseconds.
    pub mtime_nanoseconds: u32,
    /// Device number. Usually zero on Windows Git.
    pub device: u32,
    /// Inode number. Usually zero on Windows Git.
    pub inode: u32,
    /// Owner user ID.
    pub uid: u32,
    /// Owner group ID.
    pub gid: u32,
}

impl IndexEntryStat {
    /// Builds portable stat metadata for a regular worktree file.
    pub fn from_metadata(metadata: &fs::Metadata) -> Self {
        let modified = system_time_parts(metadata.modified().ok()).unwrap_or_default();
        let created = system_time_parts(metadata.created().ok()).unwrap_or(modified);

        Self {
            ctime_seconds: created.0,
            ctime_nanoseconds: created.1,
            mtime_seconds: modified.0,
            mtime_nanoseconds: modified.1,
            device: 0,
            inode: 0,
            uid: 0,
            gid: 0,
        }
    }

    /// Returns a copy with mtime and size-sensitive metadata refreshed.
    pub fn with_mtime_from_metadata(mut self, metadata: &fs::Metadata) -> Self {
        if let Some((seconds, nanoseconds)) = system_time_parts(metadata.modified().ok()) {
            self.mtime_seconds = seconds;
            self.mtime_nanoseconds = nanoseconds;
        }
        self
    }
}

impl Index {
    /// Reads `.git/index`. Missing indexes are treated as empty.
    pub fn read(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self {
                entries: Vec::new(),
                extensions: Vec::new(),
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
            bytes.extend_from_slice(&entry.stat.ctime_seconds.to_be_bytes());
            bytes.extend_from_slice(&entry.stat.ctime_nanoseconds.to_be_bytes());
            bytes.extend_from_slice(&entry.stat.mtime_seconds.to_be_bytes());
            bytes.extend_from_slice(&entry.stat.mtime_nanoseconds.to_be_bytes());
            bytes.extend_from_slice(&entry.stat.device.to_be_bytes());
            bytes.extend_from_slice(&entry.stat.inode.to_be_bytes());
            bytes.extend_from_slice(&entry.mode.to_be_bytes());
            bytes.extend_from_slice(&entry.stat.uid.to_be_bytes());
            bytes.extend_from_slice(&entry.stat.gid.to_be_bytes());
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

        bytes.extend_from_slice(&self.extensions);
        let checksum = sha1_bytes(&bytes);
        bytes.extend_from_slice(&checksum);
        write_file_atomically(path, &bytes)
    }

    /// Parses raw extension bytes into structured extension records.
    pub fn parsed_extensions(&self) -> Result<Vec<IndexExtension>> {
        parse_index_extensions(&self.extensions)
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

        let stat = IndexEntryStat {
            ctime_seconds: read_u32(bytes, offset)?,
            ctime_nanoseconds: read_u32(bytes, offset + 4)?,
            mtime_seconds: read_u32(bytes, offset + 8)?,
            mtime_nanoseconds: read_u32(bytes, offset + 12)?,
            device: read_u32(bytes, offset + 16)?,
            inode: read_u32(bytes, offset + 20)?,
            uid: read_u32(bytes, offset + 28)?,
            gid: read_u32(bytes, offset + 32)?,
        };
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
            stat,
            mode,
            object_id: ObjectId::from_bytes(object_id),
            file_size,
            path,
        });
    }

    if bytes.len().saturating_sub(offset) < 20 {
        return Err(RitError::invalid_input("index checksum is missing"));
    }
    let checksum_start = bytes.len() - 20;
    if offset > checksum_start {
        return Err(RitError::invalid_input("index entries overlap checksum"));
    }
    let extensions = bytes[offset..checksum_start].to_vec();

    Ok(Index {
        entries,
        extensions,
    })
}

fn parse_index_extensions(bytes: &[u8]) -> Result<Vec<IndexExtension>> {
    let mut offset = 0;
    let mut extensions = Vec::new();

    while offset < bytes.len() {
        if bytes.len().saturating_sub(offset) < 8 {
            return Err(RitError::invalid_input(
                "index extension header is truncated",
            ));
        }
        let mut signature = [0_u8; 4];
        signature.copy_from_slice(&bytes[offset..offset + 4]);
        let length = read_u32(bytes, offset + 4)? as usize;
        offset += 8;
        if bytes.len().saturating_sub(offset) < length {
            return Err(RitError::invalid_input(
                "index extension payload is truncated",
            ));
        }
        let data = bytes[offset..offset + length].to_vec();
        offset += length;
        extensions.push(IndexExtension {
            signature,
            kind: IndexExtensionKind::from_signature(signature),
            data,
        });
    }

    Ok(extensions)
}

impl IndexExtensionKind {
    fn from_signature(signature: [u8; 4]) -> Self {
        match &signature {
            b"TREE" => Self::CacheTree,
            b"REUC" => Self::ResolveUndo,
            b"UNTR" => Self::UntrackedCache,
            b"FSMN" => Self::FsMonitor,
            b"link" => Self::SplitIndexLink,
            b"sdir" => Self::SparseDirectory,
            _ => Self::Unknown(signature),
        }
    }
}

fn system_time_parts(time: Option<SystemTime>) -> Option<(u32, u32)> {
    let duration = time?.duration_since(UNIX_EPOCH).ok()?;
    Some((
        duration.as_secs().min(u32::MAX as u64) as u32,
        duration.subsec_nanos(),
    ))
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
    use super::{
        Index, IndexEntry, IndexEntryStat, IndexExtensionKind, join_slash_path, relative_slash_path,
    };
    use crate::ObjectId;
    use std::fs;
    use std::path::Path;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn slash_paths_round_trip() {
        let root = Path::new("repo");
        let path = root.join("src").join("main.rs");
        let slash = relative_slash_path(root, &path).expect("path should be relative");

        assert_eq!(slash, "src/main.rs");
        assert_eq!(join_slash_path(root, &slash), path);
    }

    #[test]
    fn stat_metadata_refresh_keeps_non_mtime_fields() {
        let stat = IndexEntryStat {
            ctime_seconds: 1,
            ctime_nanoseconds: 2,
            mtime_seconds: 3,
            mtime_nanoseconds: 4,
            device: 5,
            inode: 6,
            uid: 7,
            gid: 8,
        };

        let refreshed = stat.with_mtime_from_metadata(
            &std::fs::metadata(".").expect("current directory metadata should read"),
        );

        assert_eq!(refreshed.ctime_seconds, 1);
        assert_eq!(refreshed.ctime_nanoseconds, 2);
        assert_eq!(refreshed.device, 5);
        assert_eq!(refreshed.inode, 6);
        assert_eq!(refreshed.uid, 7);
        assert_eq!(refreshed.gid, 8);
    }

    #[test]
    fn index_write_preserves_extensions_when_present() {
        let path = std::env::temp_dir().join(format!(
            "rit-index-extension-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system time should be after epoch")
                .as_nanos()
        ));
        let index = Index {
            entries: vec![IndexEntry {
                stat: IndexEntryStat::default(),
                mode: 0o100644,
                object_id: ObjectId::from_bytes([1; 20]),
                file_size: 1,
                path: "a.txt".to_owned(),
            }],
            extensions: b"TREE\0\0\0\x05hello".to_vec(),
        };

        index.write(&path).expect("index should write");
        let parsed = Index::read(&path).expect("index should read");
        let _ = fs::remove_file(&path);

        assert_eq!(parsed.extensions, b"TREE\0\0\0\x05hello");
    }

    #[test]
    fn parsed_extensions_classify_known_records() {
        let index = Index {
            entries: Vec::new(),
            extensions: b"TREE\0\0\0\x05helloREUC\0\0\0\x05there".to_vec(),
        };

        let extensions = index.parsed_extensions().expect("extensions should parse");

        assert_eq!(extensions.len(), 2);
        assert_eq!(extensions[0].signature_text(), "TREE");
        assert_eq!(extensions[0].kind, IndexExtensionKind::CacheTree);
        assert_eq!(extensions[0].data, b"hello");
        assert_eq!(extensions[1].signature_text(), "REUC");
        assert_eq!(extensions[1].kind, IndexExtensionKind::ResolveUndo);
        assert_eq!(extensions[1].data, b"there");
    }

    #[test]
    fn parsed_extensions_reject_truncated_payloads() {
        let index = Index {
            entries: Vec::new(),
            extensions: b"TREE\0\0\0\x05he".to_vec(),
        };

        let error = index
            .parsed_extensions()
            .expect_err("truncated extension should fail");

        assert_eq!(error.to_string(), "index extension payload is truncated");
    }
}
