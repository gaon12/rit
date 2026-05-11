use crate::Result;

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

/// Common interface implemented by LFS, Xet, and future large-file backends.
pub trait LargeFileBackend {
    /// Returns the backend kind handled by this implementation.
    fn kind(&self) -> LargeFileBackendKind;

    /// Attempts to parse backend pointer bytes.
    fn parse_pointer(&self, data: &[u8]) -> Result<Option<LargeFilePointer>>;

    /// Encodes backend pointer metadata into Git blob bytes.
    fn encode_pointer(&self, pointer: &LargeFilePointer) -> Result<Vec<u8>>;
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
