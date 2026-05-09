use crate::{ObjectId, Repository, Result, RitError};
use std::fs;
use std::io::Write;

/// One local branch reference.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Branch {
    /// Short branch name under `refs/heads`.
    pub name: String,
    /// Commit ID stored in the branch ref.
    pub target: ObjectId,
    /// Whether this branch is currently checked out.
    pub current: bool,
}

/// One lightweight tag reference.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Tag {
    /// Short tag name under `refs/tags`.
    pub name: String,
    /// Object ID stored in the tag ref.
    pub target: ObjectId,
}

impl Repository {
    /// Returns the current symbolic branch name, if `HEAD` points at `refs/heads`.
    pub fn current_branch_name(&self) -> Result<Option<String>> {
        let head_path = self.git_dir().join("HEAD");
        let contents =
            fs::read_to_string(&head_path).map_err(|source| RitError::io(&head_path, source))?;
        let Some(reference_name) = contents.trim().strip_prefix("ref: ") else {
            return Ok(None);
        };
        Ok(reference_name
            .strip_prefix("refs/heads/")
            .map(ToOwned::to_owned))
    }

    /// Lists local branches under `refs/heads`.
    pub fn list_branches(&self) -> Result<Vec<Branch>> {
        let current = self.current_branch_name()?;
        let mut branches = Vec::new();
        let heads_dir = self.common_dir().join("refs").join("heads");
        if !heads_dir.exists() {
            return Ok(branches);
        }
        collect_branch_refs(&heads_dir, "", current.as_deref(), &mut branches)?;
        branches.sort_by(|left, right| left.name.cmp(&right.name));
        Ok(branches)
    }

    /// Creates a local branch at `HEAD`.
    pub fn create_branch(&self, name: &str) -> Result<ObjectId> {
        validate_ref_short_name(name)?;
        let target = self.resolve_head()?.ok_or_else(|| {
            RitError::invalid_input("cannot create branch because HEAD does not point at a commit")
        })?;
        let path = self.common_dir().join("refs").join("heads").join(name);
        if path.exists() {
            return Err(RitError::invalid_input(format!(
                "branch already exists: {name}"
            )));
        }
        write_ref_atomically(&path, target)?;
        Ok(target)
    }

    /// Reads a local branch target.
    pub fn branch_target(&self, name: &str) -> Result<ObjectId> {
        validate_ref_short_name(name)?;
        let path = self.common_dir().join("refs").join("heads").join(name);
        if !path.exists() {
            return Err(RitError::invalid_input(format!("branch not found: {name}")));
        }
        let target = fs::read_to_string(&path).map_err(|source| RitError::io(&path, source))?;
        ObjectId::from_hex(target.trim())
    }

    /// Deletes a local branch ref.
    pub fn delete_branch(&self, name: &str) -> Result<ObjectId> {
        validate_ref_short_name(name)?;
        if self.current_branch_name()?.as_deref() == Some(name) {
            return Err(RitError::invalid_input(format!(
                "cannot delete branch '{name}' checked out at current worktree"
            )));
        }
        let path = self.common_dir().join("refs").join("heads").join(name);
        if !path.exists() {
            return Err(RitError::invalid_input(format!("branch not found: {name}")));
        }
        let target = fs::read_to_string(&path).map_err(|source| RitError::io(&path, source))?;
        let target = ObjectId::from_hex(target.trim())?;
        fs::remove_file(&path).map_err(|source| RitError::io(&path, source))?;
        Ok(target)
    }

    /// Lists lightweight tags under `refs/tags`.
    pub fn list_tags(&self) -> Result<Vec<Tag>> {
        let tags_dir = self.common_dir().join("refs").join("tags");
        let mut tags = Vec::new();
        if !tags_dir.exists() {
            return Ok(tags);
        }
        collect_tag_refs(&tags_dir, "", &mut tags)?;
        tags.sort_by(|left, right| left.name.cmp(&right.name));
        Ok(tags)
    }

    /// Creates a lightweight tag at `HEAD`.
    pub fn create_tag(&self, name: &str) -> Result<ObjectId> {
        validate_ref_short_name(name)?;
        let target = self.resolve_head()?.ok_or_else(|| {
            RitError::invalid_input("cannot create tag because HEAD does not point at a commit")
        })?;
        let path = self.common_dir().join("refs").join("tags").join(name);
        if path.exists() {
            return Err(RitError::invalid_input(format!(
                "tag already exists: {name}"
            )));
        }
        write_ref_atomically(&path, target)?;
        Ok(target)
    }

    /// Deletes a lightweight tag.
    pub fn delete_tag(&self, name: &str) -> Result<ObjectId> {
        validate_ref_short_name(name)?;
        let path = self.common_dir().join("refs").join("tags").join(name);
        if !path.exists() {
            return Err(RitError::invalid_input(format!("tag not found: {name}")));
        }
        let target = fs::read_to_string(&path).map_err(|source| RitError::io(&path, source))?;
        let target = ObjectId::from_hex(target.trim())?;
        fs::remove_file(&path).map_err(|source| RitError::io(&path, source))?;
        Ok(target)
    }
}

fn collect_branch_refs(
    directory: &std::path::Path,
    prefix: &str,
    current: Option<&str>,
    output: &mut Vec<Branch>,
) -> Result<()> {
    for entry in fs::read_dir(directory).map_err(|source| RitError::io(directory, source))? {
        let entry = entry.map_err(|source| RitError::io(directory, source))?;
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        let full_name = if prefix.is_empty() {
            name
        } else {
            format!("{prefix}/{name}")
        };
        let file_type = entry
            .file_type()
            .map_err(|source| RitError::io(&path, source))?;
        if file_type.is_dir() {
            collect_branch_refs(&path, &full_name, current, output)?;
        } else if file_type.is_file() {
            let target = fs::read_to_string(&path).map_err(|source| RitError::io(&path, source))?;
            output.push(Branch {
                current: current == Some(full_name.as_str()),
                name: full_name,
                target: ObjectId::from_hex(target.trim())?,
            });
        }
    }
    Ok(())
}

fn collect_tag_refs(
    directory: &std::path::Path,
    prefix: &str,
    output: &mut Vec<Tag>,
) -> Result<()> {
    for entry in fs::read_dir(directory).map_err(|source| RitError::io(directory, source))? {
        let entry = entry.map_err(|source| RitError::io(directory, source))?;
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        let full_name = if prefix.is_empty() {
            name
        } else {
            format!("{prefix}/{name}")
        };
        let file_type = entry
            .file_type()
            .map_err(|source| RitError::io(&path, source))?;
        if file_type.is_dir() {
            collect_tag_refs(&path, &full_name, output)?;
        } else if file_type.is_file() {
            let target = fs::read_to_string(&path).map_err(|source| RitError::io(&path, source))?;
            output.push(Tag {
                name: full_name,
                target: ObjectId::from_hex(target.trim())?,
            });
        }
    }
    Ok(())
}

fn write_ref_atomically(path: &std::path::Path, target: ObjectId) -> Result<()> {
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
        writeln!(file, "{target}").map_err(|source| RitError::io(&lock_path, source))?;
        file.sync_all()
            .map_err(|source| RitError::io(&lock_path, source))?;
    }
    fs::rename(&lock_path, path).map_err(|source| RitError::io(path, source))?;
    Ok(())
}

/// Validates a conservative local ref short name.
pub fn validate_ref_short_name(name: &str) -> Result<()> {
    if name.is_empty()
        || name.starts_with('/')
        || name.ends_with('/')
        || name.starts_with('-')
        || name.contains("..")
        || name.contains("@{")
        || name.ends_with(".lock")
        || name
            .chars()
            .any(|character| character.is_control() || character.is_whitespace())
        || name
            .chars()
            .any(|character| matches!(character, '~' | '^' | ':' | '?' | '*' | '[' | '\\'))
    {
        return Err(RitError::invalid_input(format!("invalid ref name: {name}")));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::validate_ref_short_name;

    #[test]
    fn validates_basic_ref_names() {
        assert!(validate_ref_short_name("feature/demo").is_ok());
        assert!(validate_ref_short_name("bad name").is_err());
        assert!(validate_ref_short_name("bad..name").is_err());
        assert!(validate_ref_short_name("-bad").is_err());
    }
}
