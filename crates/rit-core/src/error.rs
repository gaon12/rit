use std::fmt::{Display, Formatter};
use std::path::PathBuf;

/// Result type used by public `rit-core` APIs.
pub type Result<T> = std::result::Result<T, RitError>;

/// Explicit error type for recoverable repository and object failures.
#[derive(Debug)]
pub enum RitError {
    /// No Git repository could be found while walking upward from `path`.
    RepositoryNotFound { path: PathBuf },
    /// A requested object ID is not present in the object database.
    ObjectNotFound { object_id: String },
    /// A repository uses a format version that this build will not write to.
    UnsupportedRepositoryFormat { version: u32 },
    /// A repository declares an extension that this build does not understand.
    UnsupportedRepositoryExtension { name: String },
    /// A path-related I/O operation failed.
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    /// The caller supplied invalid command or object input.
    InvalidInput { message: String },
}

impl RitError {
    /// Wraps an I/O error with the path that caused it.
    pub fn io(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        Self::Io {
            path: path.into(),
            source,
        }
    }

    /// Builds a clear invalid-input error.
    pub fn invalid_input(message: impl Into<String>) -> Self {
        Self::InvalidInput {
            message: message.into(),
        }
    }
}

impl Display for RitError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::RepositoryNotFound { path } => {
                write!(
                    formatter,
                    "repository not found from path: {}",
                    path.display()
                )
            }
            Self::ObjectNotFound { object_id } => {
                write!(formatter, "object not found: {object_id}")
            }
            Self::UnsupportedRepositoryFormat { version } => {
                write!(
                    formatter,
                    "unsupported repository format version: {version}"
                )
            }
            Self::UnsupportedRepositoryExtension { name } => {
                write!(formatter, "unsupported repository extension: {name}")
            }
            Self::Io { path, source } => {
                write!(
                    formatter,
                    "I/O error while accessing {}: {source}",
                    path.display()
                )
            }
            Self::InvalidInput { message } => formatter.write_str(message),
        }
    }
}

impl std::error::Error for RitError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::RitError;
    use std::path::PathBuf;

    #[test]
    fn repository_not_found_error_names_start_path() {
        let error = RitError::RepositoryNotFound {
            path: PathBuf::from("missing"),
        };

        assert_eq!(error.to_string(), "repository not found from path: missing");
    }

    #[test]
    fn object_not_found_error_names_object_id() {
        let error = RitError::ObjectNotFound {
            object_id: "abc123".to_owned(),
        };

        assert_eq!(error.to_string(), "object not found: abc123");
    }
}
