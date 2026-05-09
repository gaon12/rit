use crate::{
    GitObject, ObjectId, ObjectKind, Result, RitError, hash_object, object::parse_loose_object,
};
use flate2::Compression;
use flate2::read::ZlibDecoder;
use flate2::write::ZlibEncoder;
use std::fs;
use std::io::{Read, Write};
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
