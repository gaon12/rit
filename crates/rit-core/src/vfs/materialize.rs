use crate::{ObjectId, ObjectKind, Repository, Result, RitError};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Component, Path, PathBuf};

/// Request to lazily materialize one blob into the working tree.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VfsMaterializeRequest {
    /// Repository-relative destination path.
    pub path: String,
    /// Blob object to write.
    pub object_id: ObjectId,
    /// Whether the materialized file should be executable on Unix.
    pub executable: bool,
}

/// Result of one lazy materialization request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VfsMaterializeResult {
    /// Repository-relative path.
    pub path: String,
    /// Materialization status.
    pub status: VfsMaterializeStatus,
    /// Number of blob bytes written.
    pub bytes_written: usize,
}

/// Lazy materialization status.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VfsMaterializeStatus {
    /// The target file already existed and was left untouched.
    AlreadyMaterialized,
    /// The blob was written to the working tree.
    Materialized,
}

impl Repository {
    /// Lazily materializes a blob object as a normal worktree file.
    pub fn materialize_vfs_blob(
        &self,
        request: &VfsMaterializeRequest,
    ) -> Result<VfsMaterializeResult> {
        let Some(worktree) = self.worktree() else {
            return Err(RitError::invalid_input(
                "VFS materialization requires a working tree",
            ));
        };
        let relative_path = validate_relative_path(&request.path)?;
        let destination = worktree.join(&relative_path);
        if destination.exists() {
            return Ok(VfsMaterializeResult {
                path: request.path.clone(),
                status: VfsMaterializeStatus::AlreadyMaterialized,
                bytes_written: 0,
            });
        }

        let object = self.read_object(request.object_id)?;
        if object.kind != ObjectKind::Blob {
            return Err(RitError::invalid_input(format!(
                "VFS materialization expected blob {}, got {}",
                request.object_id, object.kind
            )));
        }

        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent).map_err(|source| RitError::io(parent, source))?;
        }
        let temporary_path = destination.with_extension(format!("rit-tmp-{}", std::process::id()));
        write_temporary_file(&temporary_path, &object.data)?;
        set_executable_if_needed(&temporary_path, request.executable)?;
        fs::rename(&temporary_path, &destination)
            .map_err(|source| RitError::io(&destination, source))?;

        Ok(VfsMaterializeResult {
            path: request.path.clone(),
            status: VfsMaterializeStatus::Materialized,
            bytes_written: object.data.len(),
        })
    }
}

fn validate_relative_path(path: &str) -> Result<PathBuf> {
    let path = Path::new(path);
    if path.as_os_str().is_empty() || path.is_absolute() {
        return Err(RitError::invalid_input(
            "VFS materialization path must be repository-relative",
        ));
    }
    if path.components().any(|component| {
        matches!(
            component,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        )
    }) {
        return Err(RitError::invalid_input(
            "VFS materialization path cannot escape the working tree",
        ));
    }
    Ok(path.to_path_buf())
}

fn write_temporary_file(path: &Path, contents: &[u8]) -> Result<()> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|source| RitError::io(path, source))?;
    file.write_all(contents)
        .map_err(|source| RitError::io(path, source))?;
    file.sync_all().map_err(|source| RitError::io(path, source))
}

#[cfg(unix)]
fn set_executable_if_needed(path: &Path, executable: bool) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;

    if !executable {
        return Ok(());
    }
    let mut permissions = fs::metadata(path)
        .map_err(|source| RitError::io(path, source))?
        .permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions).map_err(|source| RitError::io(path, source))
}

#[cfg(not(unix))]
fn set_executable_if_needed(_path: &Path, _executable: bool) -> Result<()> {
    Ok(())
}
