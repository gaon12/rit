use crate::{ObjectId, Repository, Result, RitError};
use std::thread::JoinHandle;

/// One object requested by VFS prefetch.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VfsPrefetchObject {
    /// Repository-relative path that may need the object.
    pub path: String,
    /// Object to read into the local object cache.
    pub object_id: ObjectId,
}

/// Background prefetch request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VfsPrefetchRequest {
    /// Objects to prefetch or verify locally.
    pub objects: Vec<VfsPrefetchObject>,
}

/// Object successfully observed during prefetch.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VfsPrefetchedObject {
    /// Repository-relative path.
    pub path: String,
    /// Object ID that was available.
    pub object_id: ObjectId,
    /// Number of object payload bytes read.
    pub bytes_read: usize,
}

/// Result of a VFS prefetch run.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VfsPrefetchResult {
    /// Objects found in the local object database.
    pub available: Vec<VfsPrefetchedObject>,
    /// Objects still missing locally.
    pub missing: Vec<VfsPrefetchObject>,
}

impl Repository {
    /// Prefetches VFS objects from the local object database.
    pub fn prefetch_vfs_objects(&self, request: &VfsPrefetchRequest) -> Result<VfsPrefetchResult> {
        let mut available = Vec::new();
        let mut missing = Vec::new();

        for object in &request.objects {
            match self.read_object(object.object_id) {
                Ok(git_object) => available.push(VfsPrefetchedObject {
                    path: object.path.clone(),
                    object_id: object.object_id,
                    bytes_read: git_object.data.len(),
                }),
                Err(RitError::ObjectNotFound { .. }) => missing.push(object.clone()),
                Err(error) => return Err(error),
            }
        }

        Ok(VfsPrefetchResult { available, missing })
    }

    /// Starts a background VFS prefetch worker.
    pub fn spawn_vfs_prefetch(
        &self,
        request: VfsPrefetchRequest,
    ) -> JoinHandle<Result<VfsPrefetchResult>> {
        let repository = self.clone();
        std::thread::spawn(move || repository.prefetch_vfs_objects(&request))
    }
}
