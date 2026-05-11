use crate::{Result, RitError};
use std::path::{Path, PathBuf};

/// Xet hash string used for xorbs, chunks, and reconstruction terms.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct XetHash(String);

impl XetHash {
    /// Creates a lowercase hexadecimal Xet hash value.
    pub fn new(value: impl Into<String>) -> Result<Self> {
        let value = value.into();
        if value.len() != 64
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(RitError::invalid_input(format!(
                "invalid Xet hash: {value}"
            )));
        }
        Ok(Self(value))
    }

    /// Returns the hash string.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A chunk range inside one xorb.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct XetChunkRange {
    /// Offset within the xorb.
    pub offset: u64,
    /// Length in bytes.
    pub length: u64,
}

impl XetChunkRange {
    /// Creates a validated chunk range.
    pub fn new(offset: u64, length: u64) -> Result<Self> {
        if length == 0 {
            return Err(RitError::invalid_input(
                "Xet chunk range length must be > 0",
            ));
        }
        Ok(Self { offset, length })
    }
}

/// One reconstruction term: bytes from a xorb at one range.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct XetReconstructionTerm {
    /// Xorb object hash.
    pub xorb_hash: XetHash,
    /// Range within the xorb.
    pub range: XetChunkRange,
}

impl XetReconstructionTerm {
    /// Creates a reconstruction term.
    pub fn new(xorb_hash: XetHash, range: XetChunkRange) -> Self {
        Self { xorb_hash, range }
    }
}

/// Xet file reconstruction metadata.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct XetFileReconstruction {
    /// File hash used by the Xet service.
    pub file_hash: XetHash,
    /// Original file size in bytes.
    pub size: u64,
    /// Ordered reconstruction terms.
    pub terms: Vec<XetReconstructionTerm>,
}

impl XetFileReconstruction {
    /// Creates validated reconstruction metadata.
    pub fn new(file_hash: XetHash, size: u64, terms: Vec<XetReconstructionTerm>) -> Result<Self> {
        let reconstructed_size = terms
            .iter()
            .try_fold(0_u64, |total, term| total.checked_add(term.range.length))
            .ok_or_else(|| RitError::invalid_input("Xet reconstruction size overflow"))?;
        if reconstructed_size != size {
            return Err(RitError::invalid_input(format!(
                "Xet reconstruction size mismatch: expected {size}, got {reconstructed_size}"
            )));
        }
        Ok(Self {
            file_hash,
            size,
            terms,
        })
    }
}

/// Local Xet cache path model.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct XetLocalCache {
    root: PathBuf,
}

impl XetLocalCache {
    /// Creates a cache rooted at `HF_XET_CACHE`-style directory.
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// Returns the cache root.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Returns the sharded path for a cached xorb.
    pub fn xorb_path(&self, hash: &XetHash) -> PathBuf {
        self.root
            .join("xorbs")
            .join(&hash.as_str()[0..2])
            .join(&hash.as_str()[2..4])
            .join(hash.as_str())
    }

    /// Returns the sharded path for reconstruction metadata.
    pub fn reconstruction_path(&self, hash: &XetHash) -> PathBuf {
        self.root
            .join("reconstructions")
            .join(&hash.as_str()[0..2])
            .join(&hash.as_str()[2..4])
            .join(format!("{}.json", hash.as_str()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hash_with(prefix: char) -> XetHash {
        XetHash::new(prefix.to_string().repeat(64)).expect("hash should be valid")
    }

    #[test]
    fn validates_hash_and_chunk_ranges() {
        assert!(XetHash::new("a".repeat(64)).is_ok());
        assert!(XetHash::new("A".repeat(64)).is_err());
        assert!(XetChunkRange::new(0, 1).is_ok());
        assert!(XetChunkRange::new(0, 0).is_err());
    }

    #[test]
    fn validates_reconstruction_size() {
        let term = XetReconstructionTerm::new(
            hash_with('b'),
            XetChunkRange::new(10, 5).expect("range should be valid"),
        );

        assert!(XetFileReconstruction::new(hash_with('c'), 5, vec![term.clone()]).is_ok());
        assert!(XetFileReconstruction::new(hash_with('c'), 6, vec![term]).is_err());
    }

    #[test]
    fn builds_stable_cache_paths() {
        let cache = XetLocalCache::new("cache");
        let hash = hash_with('d');

        assert_eq!(
            cache.xorb_path(&hash),
            PathBuf::from("cache")
                .join("xorbs")
                .join("dd")
                .join("dd")
                .join(hash.as_str())
        );
        assert_eq!(
            cache.reconstruction_path(&hash),
            PathBuf::from("cache")
                .join("reconstructions")
                .join("dd")
                .join("dd")
                .join(format!("{}.json", hash.as_str()))
        );
    }
}
