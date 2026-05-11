use crate::{Result, RitError};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

const GIT_LFS_POINTER_VERSION: &str = "https://git-lfs.github.com/spec/v1";

/// Well-known large-file storage backends.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LargeFileBackendKind {
    /// Git LFS compatible object storage.
    Lfs,
    /// Xet chunked storage.
    Xet,
    /// Local content-addressed storage used for tests or offline workflows.
    LocalCas,
    /// A backend supplied by an embedding application.
    Custom(String),
}

impl LargeFileBackendKind {
    /// Stable backend name for configuration and diagnostics.
    pub fn as_str(&self) -> &str {
        match self {
            Self::Lfs => "lfs",
            Self::Xet => "xet",
            Self::LocalCas => "local-cas",
            Self::Custom(name) => name,
        }
    }
}

/// A repository path pattern that should use a large-file backend.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LargeFileTrackRule {
    /// Git-style attribute/path pattern such as `*.zip`.
    pub pattern: String,
    /// Backend selected for matching paths.
    pub backend: LargeFileBackendKind,
}

impl LargeFileTrackRule {
    /// Creates a new path tracking rule.
    pub fn new(pattern: impl Into<String>, backend: LargeFileBackendKind) -> Self {
        Self {
            pattern: pattern.into(),
            backend,
        }
    }
}

/// Backend-neutral pointer metadata for materialized large files.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LargeFilePointer {
    /// Backend that owns this pointer.
    pub backend: LargeFileBackendKind,
    /// Backend-specific object ID or content hash.
    pub object_id: String,
    /// Original file size in bytes.
    pub size: u64,
}

impl LargeFilePointer {
    /// Creates backend-neutral pointer metadata.
    pub fn new(backend: LargeFileBackendKind, object_id: impl Into<String>, size: u64) -> Self {
        Self {
            backend,
            object_id: object_id.into(),
            size,
        }
    }
}

/// Git LFS local object cache rooted at `.git/lfs/objects`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LfsLocalCache {
    objects_dir: PathBuf,
}

impl LfsLocalCache {
    /// Creates a cache using the repository `.git` directory.
    pub fn new(git_dir: impl AsRef<Path>) -> Self {
        Self {
            objects_dir: git_dir.as_ref().join("lfs").join("objects"),
        }
    }

    /// Creates a cache from an explicit objects directory.
    pub fn from_objects_dir(objects_dir: impl Into<PathBuf>) -> Self {
        Self {
            objects_dir: objects_dir.into(),
        }
    }

    /// Returns the root cache directory.
    pub fn objects_dir(&self) -> &Path {
        &self.objects_dir
    }

    /// Returns the sharded cache path for a Git LFS pointer.
    pub fn path_for_pointer(&self, pointer: &LargeFilePointer) -> Result<PathBuf> {
        self.path_for_oid(&pointer.object_id)
    }

    /// Returns true when the object exists at the expected cache path.
    pub fn contains(&self, pointer: &LargeFilePointer) -> Result<bool> {
        Ok(self.path_for_pointer(pointer)?.is_file())
    }

    /// Reads an object after validating its SHA-256 and size.
    pub fn read_object(&self, pointer: &LargeFilePointer) -> Result<Vec<u8>> {
        let path = self.path_for_pointer(pointer)?;
        let data = fs::read(&path).map_err(|source| RitError::Io {
            path: path.clone(),
            source,
        })?;
        verify_lfs_object(pointer, &data)?;
        Ok(data)
    }

    /// Streams an object into the cache and verifies it against the pointer.
    pub fn write_object_from_reader(
        &self,
        pointer: &LargeFilePointer,
        mut reader: impl Read,
    ) -> Result<PathBuf> {
        let path = self.path_for_pointer(pointer)?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|source| RitError::Io {
                path: parent.to_path_buf(),
                source,
            })?;
        }

        let temp_path = path.with_extension("tmp");
        let mut file = fs::File::create(&temp_path).map_err(|source| RitError::Io {
            path: temp_path.clone(),
            source,
        })?;
        let mut hasher = Sha256::new();
        let mut size = 0_u64;
        let mut buffer = [0_u8; 8192];
        loop {
            let read = reader.read(&mut buffer).map_err(|source| RitError::Io {
                path: temp_path.clone(),
                source,
            })?;
            if read == 0 {
                break;
            }
            file.write_all(&buffer[..read])
                .map_err(|source| RitError::Io {
                    path: temp_path.clone(),
                    source,
                })?;
            hasher.update(&buffer[..read]);
            size += read as u64;
        }
        file.flush().map_err(|source| RitError::Io {
            path: temp_path.clone(),
            source,
        })?;
        drop(file);

        let actual_oid = format!("{:x}", hasher.finalize());
        if actual_oid != pointer.object_id || size != pointer.size {
            let _ = fs::remove_file(&temp_path);
            return Err(RitError::invalid_input(format!(
                "LFS object verification failed: expected sha256:{} size {}, got sha256:{actual_oid} size {size}",
                pointer.object_id, pointer.size
            )));
        }

        fs::rename(&temp_path, &path).map_err(|source| RitError::Io {
            path: path.clone(),
            source,
        })?;
        Ok(path)
    }

    fn path_for_oid(&self, object_id: &str) -> Result<PathBuf> {
        if !is_lower_hex_sha256(object_id) {
            return Err(RitError::invalid_input(format!(
                "invalid Git LFS sha256 object id: {object_id}"
            )));
        }
        Ok(self
            .objects_dir
            .join(&object_id[0..2])
            .join(&object_id[2..4])
            .join(object_id))
    }
}

/// Common interface implemented by LFS, Xet, and future large-file backends.
pub trait LargeFileBackend {
    /// Returns the backend kind handled by this implementation.
    fn kind(&self) -> LargeFileBackendKind;

    /// Attempts to parse backend pointer bytes.
    fn parse_pointer(&self, data: &[u8]) -> Result<Option<LargeFilePointer>>;

    /// Encodes backend pointer metadata into Git blob bytes.
    fn encode_pointer(&self, pointer: &LargeFilePointer) -> Result<Vec<u8>>;
}

/// Git LFS pointer parser and encoder.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct GitLfsBackend;

impl LargeFileBackend for GitLfsBackend {
    fn kind(&self) -> LargeFileBackendKind {
        LargeFileBackendKind::Lfs
    }

    fn parse_pointer(&self, data: &[u8]) -> Result<Option<LargeFilePointer>> {
        parse_lfs_pointer(data)
    }

    fn encode_pointer(&self, pointer: &LargeFilePointer) -> Result<Vec<u8>> {
        encode_lfs_pointer(pointer)
    }
}

/// Parses a Git LFS v1 pointer blob.
pub fn parse_lfs_pointer(data: &[u8]) -> Result<Option<LargeFilePointer>> {
    if data.is_empty() || data.len() >= 1024 {
        return Ok(None);
    }
    let Ok(text) = std::str::from_utf8(data) else {
        return Ok(None);
    };
    let mut lines = text.lines();
    let Some(version_line) = lines.next() else {
        return Ok(None);
    };
    if version_line != format!("version {GIT_LFS_POINTER_VERSION}") {
        return Ok(None);
    }

    let mut oid = None;
    let mut size = None;
    for line in lines {
        let Some((key, value)) = line.split_once(' ') else {
            return Ok(None);
        };
        match key {
            "oid" => {
                let Some(hash) = value.strip_prefix("sha256:") else {
                    return Ok(None);
                };
                if !is_lower_hex_sha256(hash) {
                    return Ok(None);
                }
                oid = Some(hash.to_owned());
            }
            "size" => {
                let Ok(parsed_size) = value.parse::<u64>() else {
                    return Ok(None);
                };
                size = Some(parsed_size);
            }
            _ => {}
        }
    }

    let (Some(object_id), Some(size)) = (oid, size) else {
        return Ok(None);
    };
    Ok(Some(LargeFilePointer::new(
        LargeFileBackendKind::Lfs,
        object_id,
        size,
    )))
}

/// Encodes Git LFS v1 pointer metadata.
pub fn encode_lfs_pointer(pointer: &LargeFilePointer) -> Result<Vec<u8>> {
    if pointer.backend != LargeFileBackendKind::Lfs {
        return Err(crate::RitError::invalid_input(format!(
            "cannot encode {} pointer as Git LFS",
            pointer.backend.as_str()
        )));
    }
    if !is_lower_hex_sha256(&pointer.object_id) {
        return Err(crate::RitError::invalid_input(format!(
            "invalid Git LFS sha256 object id: {}",
            pointer.object_id
        )));
    }
    Ok(format!(
        "version {GIT_LFS_POINTER_VERSION}\noid sha256:{}\nsize {}\n",
        pointer.object_id, pointer.size
    )
    .into_bytes())
}

fn is_lower_hex_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn verify_lfs_object(pointer: &LargeFilePointer, data: &[u8]) -> Result<()> {
    if pointer.backend != LargeFileBackendKind::Lfs {
        return Err(RitError::invalid_input(format!(
            "cannot verify {} pointer as Git LFS",
            pointer.backend.as_str()
        )));
    }
    if data.len() as u64 != pointer.size {
        return Err(RitError::invalid_input(format!(
            "LFS object size mismatch: expected {}, got {}",
            pointer.size,
            data.len()
        )));
    }
    let actual_oid = format!("{:x}", Sha256::digest(data));
    if actual_oid != pointer.object_id {
        return Err(RitError::invalid_input(format!(
            "LFS object sha256 mismatch: expected {}, got {actual_oid}",
            pointer.object_id
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Cursor;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    struct EchoBackend;

    impl LargeFileBackend for EchoBackend {
        fn kind(&self) -> LargeFileBackendKind {
            LargeFileBackendKind::LocalCas
        }

        fn parse_pointer(&self, data: &[u8]) -> Result<Option<LargeFilePointer>> {
            let text = String::from_utf8_lossy(data);
            let Some((object_id, size)) = text.split_once(':') else {
                return Ok(None);
            };
            let Ok(size) = size.parse::<u64>() else {
                return Ok(None);
            };
            Ok(Some(LargeFilePointer::new(
                LargeFileBackendKind::LocalCas,
                object_id,
                size,
            )))
        }

        fn encode_pointer(&self, pointer: &LargeFilePointer) -> Result<Vec<u8>> {
            Ok(format!("{}:{}", pointer.object_id, pointer.size).into_bytes())
        }
    }

    #[test]
    fn backend_kind_names_are_stable() {
        assert_eq!(LargeFileBackendKind::Lfs.as_str(), "lfs");
        assert_eq!(LargeFileBackendKind::Xet.as_str(), "xet");
        assert_eq!(LargeFileBackendKind::LocalCas.as_str(), "local-cas");
        assert_eq!(
            LargeFileBackendKind::Custom("demo".to_owned()).as_str(),
            "demo"
        );
    }

    #[test]
    fn track_rule_records_pattern_and_backend() {
        let rule = LargeFileTrackRule::new("*.zip", LargeFileBackendKind::Lfs);

        assert_eq!(rule.pattern, "*.zip");
        assert_eq!(rule.backend, LargeFileBackendKind::Lfs);
    }

    #[test]
    fn backend_trait_round_trips_pointer_metadata() {
        let backend = EchoBackend;
        let pointer = LargeFilePointer::new(backend.kind(), "abc123", 42);
        let encoded = backend
            .encode_pointer(&pointer)
            .expect("pointer should encode");

        assert_eq!(
            backend
                .parse_pointer(&encoded)
                .expect("pointer should parse"),
            Some(pointer)
        );
        assert_eq!(
            backend
                .parse_pointer(b"not a pointer")
                .expect("non-pointer should parse"),
            None
        );
    }

    #[test]
    fn parses_git_lfs_pointer() {
        let pointer = parse_lfs_pointer(
            b"version https://git-lfs.github.com/spec/v1\noid sha256:4d7a214614ab2935c943f9e0ff69d22eadbb8f32b1258daaa5e2ca24d17e2393\nsize 12345\n",
        )
        .expect("pointer parse should succeed");

        assert_eq!(
            pointer,
            Some(LargeFilePointer::new(
                LargeFileBackendKind::Lfs,
                "4d7a214614ab2935c943f9e0ff69d22eadbb8f32b1258daaa5e2ca24d17e2393",
                12345,
            ))
        );
    }

    #[test]
    fn encodes_git_lfs_pointer() {
        let pointer = LargeFilePointer::new(
            LargeFileBackendKind::Lfs,
            "4d7a214614ab2935c943f9e0ff69d22eadbb8f32b1258daaa5e2ca24d17e2393",
            12345,
        );

        assert_eq!(
            encode_lfs_pointer(&pointer).expect("pointer should encode"),
            b"version https://git-lfs.github.com/spec/v1\noid sha256:4d7a214614ab2935c943f9e0ff69d22eadbb8f32b1258daaa5e2ca24d17e2393\nsize 12345\n"
        );
    }

    #[test]
    fn rejects_non_lfs_pointers() {
        assert_eq!(
            parse_lfs_pointer(b"version https://git-lfs.github.com/spec/v1\nsize 1\n")
                .expect("parse should succeed"),
            None
        );
        assert_eq!(
            parse_lfs_pointer(
                b"version https://git-lfs.github.com/spec/v1\noid sha256:ABC\nsize 1\n"
            )
            .expect("parse should succeed"),
            None
        );
    }

    #[test]
    fn lfs_cache_writes_and_reads_verified_object() {
        let temp = temp_path("lfs-cache");
        let cache = LfsLocalCache::from_objects_dir(temp.join("objects"));
        let data = b"large file contents";
        let pointer = LargeFilePointer::new(
            LargeFileBackendKind::Lfs,
            format!("{:x}", Sha256::digest(data)),
            data.len() as u64,
        );

        let path = cache
            .write_object_from_reader(&pointer, Cursor::new(data))
            .expect("object should write");

        assert_eq!(
            path,
            temp.join("objects")
                .join(&pointer.object_id[0..2])
                .join(&pointer.object_id[2..4])
                .join(&pointer.object_id)
        );
        assert!(cache.contains(&pointer).expect("contains should work"));
        assert_eq!(
            cache.read_object(&pointer).expect("object should read"),
            data
        );
        remove_dir_all(&temp);
    }

    #[test]
    fn lfs_cache_rejects_bad_hash_or_size() {
        let temp = temp_path("lfs-cache-bad");
        let cache = LfsLocalCache::from_objects_dir(temp.join("objects"));
        let pointer = LargeFilePointer::new(
            LargeFileBackendKind::Lfs,
            "4d7a214614ab2935c943f9e0ff69d22eadbb8f32b1258daaa5e2ca24d17e2393",
            12345,
        );

        let result = cache.write_object_from_reader(&pointer, Cursor::new(b"wrong"));

        assert!(result.is_err());
        assert!(
            !cache
                .path_for_pointer(&pointer)
                .expect("path should build")
                .exists()
        );
        remove_dir_all(&temp);
    }

    fn temp_path(name: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("rit-{name}-{unique}"))
    }

    fn remove_dir_all(path: &Path) {
        let _ = fs::remove_dir_all(path);
    }
}
