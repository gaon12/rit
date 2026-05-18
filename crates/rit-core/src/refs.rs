use crate::{ObjectId, ObjectKind, Repository, Result, RitError, parse_commit};
use std::collections::{BTreeMap, HashSet};
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
        let mut targets = self
            .packed_refs_with_prefix("refs/heads/")?
            .into_iter()
            .map(|(name, target)| (name.trim_start_matches("refs/heads/").to_owned(), target))
            .collect::<BTreeMap<_, _>>();
        let heads_dir = self.common_dir().join("refs").join("heads");
        if heads_dir.exists() {
            collect_branch_refs(&heads_dir, "", &mut targets)?;
        }

        Ok(targets
            .into_iter()
            .map(|(name, target)| Branch {
                current: current.as_deref() == Some(name.as_str()),
                name,
                target,
            })
            .collect())
    }

    /// Lists local branches whose names match at least one refname pattern.
    pub fn list_branches_matching(&self, patterns: &[&str]) -> Result<Vec<Branch>> {
        let branches = self.list_branches()?;
        if patterns.is_empty() {
            return Ok(branches);
        }
        Ok(branches
            .into_iter()
            .filter(|branch| {
                patterns
                    .iter()
                    .any(|pattern| ref_name_matches_pattern(pattern, &branch.name))
            })
            .collect())
    }

    /// Creates a local branch at `HEAD`.
    pub fn create_branch(&self, name: &str) -> Result<ObjectId> {
        validate_ref_short_name(name)?;
        let target = self.resolve_head()?.ok_or_else(|| {
            RitError::invalid_input("cannot create branch because HEAD does not point at a commit")
        })?;
        self.create_branch_at(name, target)
    }

    /// Creates a local branch at a specific commit.
    pub fn create_branch_at(&self, name: &str, target: ObjectId) -> Result<ObjectId> {
        validate_ref_short_name(name)?;
        let object = self.read_object(target)?;
        if object.kind != ObjectKind::Commit {
            return Err(RitError::invalid_input(format!(
                "branch target {target} is {}, not commit",
                object.kind
            )));
        }
        let path = self.common_dir().join("refs").join("heads").join(name);
        if path.exists() {
            return Err(RitError::invalid_input(format!(
                "branch already exists: {name}"
            )));
        }
        write_ref_atomically(&path, target)?;
        self.refresh_indexdb_after_git_write();
        Ok(target)
    }

    /// Reads a local branch target.
    pub fn branch_target(&self, name: &str) -> Result<ObjectId> {
        validate_ref_short_name(name)?;
        let path = self.common_dir().join("refs").join("heads").join(name);
        if path.exists() {
            let target = fs::read_to_string(&path).map_err(|source| RitError::io(&path, source))?;
            return ObjectId::from_hex(target.trim());
        }
        self.packed_ref(&format!("refs/heads/{name}"))?
            .ok_or_else(|| RitError::invalid_input(format!("branch not found: {name}")))
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
            if self.packed_ref(&format!("refs/heads/{name}"))?.is_some() {
                return Err(RitError::invalid_input(format!(
                    "deleting packed branch is not implemented: {name}"
                )));
            }
            return Err(RitError::invalid_input(format!("branch not found: {name}")));
        }
        let target = fs::read_to_string(&path).map_err(|source| RitError::io(&path, source))?;
        let target = ObjectId::from_hex(target.trim())?;
        let head = self.resolve_head()?.ok_or_else(|| {
            RitError::invalid_input("cannot delete branch because HEAD does not point at a commit")
        })?;
        if !self.commit_is_reachable_from(target, head)? {
            return Err(RitError::invalid_input(format!(
                "branch '{name}' is not fully merged"
            )));
        }
        fs::remove_file(&path).map_err(|source| RitError::io(&path, source))?;
        self.refresh_indexdb_after_git_write();
        Ok(target)
    }

    /// Lists lightweight tags under `refs/tags`.
    pub fn list_tags(&self) -> Result<Vec<Tag>> {
        let mut targets = self
            .packed_refs_with_prefix("refs/tags/")?
            .into_iter()
            .map(|(name, target)| (name.trim_start_matches("refs/tags/").to_owned(), target))
            .collect::<BTreeMap<_, _>>();
        let tags_dir = self.common_dir().join("refs").join("tags");
        if tags_dir.exists() {
            collect_tag_refs(&tags_dir, "", &mut targets)?;
        }
        Ok(targets
            .into_iter()
            .map(|(name, target)| Tag { name, target })
            .collect())
    }

    /// Lists lightweight tags whose names match at least one refname pattern.
    pub fn list_tags_matching(&self, patterns: &[&str]) -> Result<Vec<Tag>> {
        let tags = self.list_tags()?;
        if patterns.is_empty() {
            return Ok(tags);
        }
        Ok(tags
            .into_iter()
            .filter(|tag| {
                patterns
                    .iter()
                    .any(|pattern| ref_name_matches_pattern(pattern, &tag.name))
            })
            .collect())
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
        self.refresh_indexdb_after_git_write();
        Ok(target)
    }

    /// Deletes a lightweight tag.
    pub fn delete_tag(&self, name: &str) -> Result<ObjectId> {
        validate_ref_short_name(name)?;
        let path = self.common_dir().join("refs").join("tags").join(name);
        if !path.exists() {
            if self.packed_ref(&format!("refs/tags/{name}"))?.is_some() {
                return Err(RitError::invalid_input(format!(
                    "deleting packed tag is not implemented: {name}"
                )));
            }
            return Err(RitError::invalid_input(format!("tag not found: {name}")));
        }
        let target = fs::read_to_string(&path).map_err(|source| RitError::io(&path, source))?;
        let target = ObjectId::from_hex(target.trim())?;
        fs::remove_file(&path).map_err(|source| RitError::io(&path, source))?;
        self.refresh_indexdb_after_git_write();
        Ok(target)
    }

    /// Reads one ref from `.git/packed-refs`.
    pub fn packed_ref(&self, full_name: &str) -> Result<Option<ObjectId>> {
        Ok(self.packed_refs()?.remove(full_name))
    }

    fn packed_refs_with_prefix(&self, prefix: &str) -> Result<BTreeMap<String, ObjectId>> {
        Ok(self
            .packed_refs()?
            .into_iter()
            .filter(|(name, _)| name.starts_with(prefix))
            .collect())
    }

    fn packed_refs(&self) -> Result<BTreeMap<String, ObjectId>> {
        let path = self.common_dir().join("packed-refs");
        if !path.exists() {
            return Ok(BTreeMap::new());
        }
        let contents = fs::read_to_string(&path).map_err(|source| RitError::io(&path, source))?;
        let mut refs = BTreeMap::new();
        for line in contents.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with('^') {
                continue;
            }
            let Some((object_id, name)) = trimmed.split_once(' ') else {
                return Err(RitError::invalid_input(format!(
                    "malformed packed-refs line: {trimmed}"
                )));
            };
            refs.insert(name.to_owned(), ObjectId::from_hex(object_id)?);
        }
        Ok(refs)
    }

    fn commit_is_reachable_from(&self, target: ObjectId, start: ObjectId) -> Result<bool> {
        let mut stack = vec![start];
        let mut seen = HashSet::new();

        while let Some(object_id) = stack.pop() {
            if object_id == target {
                return Ok(true);
            }
            if !seen.insert(object_id) {
                continue;
            }
            let object = self.read_object(object_id)?;
            if object.kind != ObjectKind::Commit {
                return Err(RitError::invalid_input(format!(
                    "object {object_id} is {}, not commit",
                    object.kind
                )));
            }
            let commit = parse_commit(&object.data)?;
            stack.extend(commit.parents);
        }

        Ok(false)
    }
}

fn collect_branch_refs(
    directory: &std::path::Path,
    prefix: &str,
    output: &mut BTreeMap<String, ObjectId>,
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
            collect_branch_refs(&path, &full_name, output)?;
        } else if file_type.is_file() {
            let target = fs::read_to_string(&path).map_err(|source| RitError::io(&path, source))?;
            output.insert(full_name, ObjectId::from_hex(target.trim())?);
        }
    }
    Ok(())
}

fn collect_tag_refs(
    directory: &std::path::Path,
    prefix: &str,
    output: &mut BTreeMap<String, ObjectId>,
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
            output.insert(full_name, ObjectId::from_hex(target.trim())?);
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

fn ref_name_matches_pattern(pattern: &str, ref_name: &str) -> bool {
    if ref_pattern_has_wildcard(pattern) {
        wildcard_matches(pattern, ref_name)
    } else {
        pattern == ref_name
    }
}

fn ref_pattern_has_wildcard(pattern: &str) -> bool {
    pattern.contains('*') || pattern.contains('?') || pattern.contains('[')
}

fn wildcard_matches(pattern: &str, value: &str) -> bool {
    let pattern = pattern.as_bytes();
    let value = value.as_bytes();
    let mut pattern_index = 0;
    let mut value_index = 0;
    let mut last_star = None;
    let mut value_after_star = 0;

    while value_index < value.len() {
        if let Some(next_pattern_index) =
            match_single_pattern_item(pattern, pattern_index, value[value_index])
        {
            pattern_index = next_pattern_index;
            value_index += 1;
        } else if pattern_index < pattern.len() && pattern[pattern_index] == b'*' {
            last_star = Some(pattern_index);
            pattern_index += 1;
            value_after_star = value_index;
        } else if let Some(star_index) = last_star {
            pattern_index = star_index + 1;
            value_after_star += 1;
            value_index = value_after_star;
        } else {
            return false;
        }
    }

    pattern[pattern_index..]
        .iter()
        .all(|character| *character == b'*')
}

fn match_single_pattern_item(pattern: &[u8], index: usize, value_byte: u8) -> Option<usize> {
    let pattern_byte = *pattern.get(index)?;
    match pattern_byte {
        b'?' => Some(index + 1),
        b'[' => match_bracket_class(pattern, index, value_byte),
        literal if literal == value_byte => Some(index + 1),
        _ => None,
    }
}

fn match_bracket_class(pattern: &[u8], index: usize, value_byte: u8) -> Option<usize> {
    let mut cursor = index + 1;
    let negated = matches!(pattern.get(cursor), Some(b'!' | b'^'));
    if negated {
        cursor += 1;
    }

    let class_start = cursor;
    let mut matched = false;
    while cursor < pattern.len() {
        if pattern[cursor] == b']' && cursor > class_start {
            return if matched != negated {
                Some(cursor + 1)
            } else {
                None
            };
        }

        if cursor + 2 < pattern.len() && pattern[cursor + 1] == b'-' && pattern[cursor + 2] != b']'
        {
            let start = pattern[cursor];
            let end = pattern[cursor + 2];
            if start <= value_byte && value_byte <= end {
                matched = true;
            }
            cursor += 3;
        } else {
            if pattern[cursor] == value_byte {
                matched = true;
            }
            cursor += 1;
        }
    }

    if value_byte == b'[' {
        Some(index + 1)
    } else {
        None
    }
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
    use super::{ref_name_matches_pattern, validate_ref_short_name};

    #[test]
    fn validates_basic_ref_names() {
        assert!(validate_ref_short_name("feature/demo").is_ok());
        assert!(validate_ref_short_name("bad name").is_err());
        assert!(validate_ref_short_name("bad..name").is_err());
        assert!(validate_ref_short_name("-bad").is_err());
    }

    #[test]
    fn ref_patterns_match_git_style_globs() {
        assert!(ref_name_matches_pattern("topic*", "topic/one"));
        assert!(ref_name_matches_pattern("*/one", "feature/one"));
        assert!(ref_name_matches_pattern("release", "release"));
        assert!(ref_name_matches_pattern("topic-[ot]ne", "topic-one"));
        assert!(!ref_name_matches_pattern("topic/*", "topic-one"));
        assert!(!ref_name_matches_pattern("release", "release/v1"));
    }
}
