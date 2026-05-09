use crate::{GitObject, ObjectId, Result, RitError, object::parse_loose_object};
use flate2::read::ZlibDecoder;
use std::fs;
use std::io::Read;
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
        if !path.is_file() {
            return Err(RitError::ObjectNotFound {
                object_id: object_id.to_hex(),
            });
        }

        let compressed = fs::read(&path).map_err(|source| RitError::io(&path, source))?;
        let mut decoder = ZlibDecoder::new(compressed.as_slice());
        let mut raw = Vec::new();
        decoder
            .read_to_end(&mut raw)
            .map_err(|source| RitError::io(&path, source))?;
        parse_loose_object(&raw)
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
}
