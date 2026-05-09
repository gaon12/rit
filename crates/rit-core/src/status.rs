use crate::index::{Index, join_slash_path, relative_slash_path};
use crate::object::{ObjectKind, hash_object, parse_tree_entries};
use crate::{ObjectId, Repository, Result, RitError};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

/// One porcelain v1 status entry.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StatusEntry {
    /// Index status column.
    pub index_status: char,
    /// Working tree status column.
    pub worktree_status: char,
    /// Repository-relative path using `/` separators.
    pub path: String,
}

/// Status result formatted by the CLI.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PorcelainStatus {
    /// Ordered porcelain entries.
    pub entries: Vec<StatusEntry>,
}

impl PorcelainStatus {
    /// Renders porcelain v1 text.
    pub fn to_porcelain_v1(&self) -> String {
        let mut output = String::new();
        for entry in &self.entries {
            output.push(entry.index_status);
            output.push(entry.worktree_status);
            output.push(' ');
            output.push_str(&entry.path);
            output.push('\n');
        }
        output
    }
}

impl Repository {
    /// Computes a conservative porcelain v1 status.
    pub fn status_porcelain_v1(&self) -> Result<PorcelainStatus> {
        let Some(worktree) = self.worktree() else {
            return Err(RitError::invalid_input(
                "status must be run in a repository with a working tree",
            ));
        };

        let index = Index::read(&self.git_dir().join("index"))?;
        let index_entries = index
            .entries
            .iter()
            .map(|entry| (entry.path.clone(), entry.object_id))
            .collect::<BTreeMap<_, _>>();
        let head_entries = self.head_tree_entries()?;
        let ignore_rules = IgnoreRules::read(worktree, self.git_dir())?;
        let working_files = scan_working_files(worktree, &ignore_rules)?;

        let mut entries = Vec::new();
        let tracked_paths = index_entries
            .keys()
            .chain(head_entries.keys())
            .cloned()
            .collect::<BTreeSet<_>>();

        for path in tracked_paths {
            let head_object = head_entries.get(&path);
            let index_object = index_entries.get(&path);
            let index_status = match (head_object, index_object) {
                (None, Some(_)) => 'A',
                (Some(_), None) => 'D',
                (Some(head), Some(index)) if head != index => 'M',
                _ => ' ',
            };

            let worktree_status = match index_object {
                None => ' ',
                Some(index_object) => {
                    let full_path = join_slash_path(worktree, &path);
                    if !full_path.exists() {
                        'D'
                    } else if hash_worktree_file(&full_path)? != *index_object {
                        'M'
                    } else {
                        ' '
                    }
                }
            };

            if index_status != ' ' || worktree_status != ' ' {
                entries.push(StatusEntry {
                    index_status,
                    worktree_status,
                    path,
                });
            }
        }

        for path in working_files {
            if !index_entries.contains_key(&path) {
                entries.push(StatusEntry {
                    index_status: '?',
                    worktree_status: '?',
                    path,
                });
            }
        }

        Ok(PorcelainStatus { entries })
    }

    fn head_tree_entries(&self) -> Result<BTreeMap<String, ObjectId>> {
        let Some(head_object_id) = self.resolve_head()? else {
            return Ok(BTreeMap::new());
        };
        let commit = self.read_object(head_object_id)?;
        if commit.kind != ObjectKind::Commit {
            return Err(RitError::invalid_input(format!(
                "HEAD points to {}, not commit",
                commit.kind
            )));
        }
        let tree_id = parse_commit_tree(&commit.data)?;
        let mut entries = BTreeMap::new();
        self.collect_tree_entries("", tree_id, &mut entries)?;
        Ok(entries)
    }

    fn resolve_head(&self) -> Result<Option<ObjectId>> {
        let head_path = self.git_dir().join("HEAD");
        let contents =
            fs::read_to_string(&head_path).map_err(|source| RitError::io(&head_path, source))?;
        let trimmed = contents.trim();
        if let Some(reference_name) = trimmed.strip_prefix("ref: ") {
            let reference_path = self.common_dir().join(reference_name);
            if !reference_path.exists() {
                return Ok(None);
            }
            let object_id = fs::read_to_string(&reference_path)
                .map_err(|source| RitError::io(&reference_path, source))?;
            return Ok(Some(ObjectId::from_hex(object_id.trim())?));
        }

        Ok(Some(ObjectId::from_hex(trimmed)?))
    }

    fn collect_tree_entries(
        &self,
        prefix: &str,
        tree_id: ObjectId,
        output: &mut BTreeMap<String, ObjectId>,
    ) -> Result<()> {
        let tree = self.read_object(tree_id)?;
        if tree.kind != ObjectKind::Tree {
            return Err(RitError::invalid_input(format!(
                "object {tree_id} is {}, not tree",
                tree.kind
            )));
        }

        for entry in parse_tree_entries(&tree.data)? {
            let path = if prefix.is_empty() {
                entry.name_lossy()
            } else {
                format!("{prefix}/{}", entry.name_lossy())
            };
            if entry.kind == ObjectKind::Tree {
                self.collect_tree_entries(&path, entry.object_id, output)?;
            } else {
                output.insert(path, entry.object_id);
            }
        }

        Ok(())
    }
}

fn parse_commit_tree(data: &[u8]) -> Result<ObjectId> {
    let text = std::str::from_utf8(data)
        .map_err(|_| RitError::invalid_input("commit object is not UTF-8"))?;
    for line in text.lines() {
        if let Some(tree_id) = line.strip_prefix("tree ") {
            return ObjectId::from_hex(tree_id);
        }
    }

    Err(RitError::invalid_input(
        "commit object is missing tree line",
    ))
}

fn hash_worktree_file(path: &Path) -> Result<ObjectId> {
    let bytes = fs::read(path).map_err(|source| RitError::io(path, source))?;
    Ok(hash_object(ObjectKind::Blob, &bytes))
}

fn scan_working_files(root: &Path, ignore_rules: &IgnoreRules) -> Result<BTreeSet<String>> {
    let mut files = BTreeSet::new();
    scan_directory(root, root, ignore_rules, &mut files)?;
    Ok(files)
}

fn scan_directory(
    root: &Path,
    directory: &Path,
    ignore_rules: &IgnoreRules,
    output: &mut BTreeSet<String>,
) -> Result<()> {
    for entry in fs::read_dir(directory).map_err(|source| RitError::io(directory, source))? {
        let entry = entry.map_err(|source| RitError::io(directory, source))?;
        let path = entry.path();
        let relative = relative_slash_path(root, &path)?;
        if relative == ".git" || relative.starts_with(".git/") || ignore_rules.matches(&relative) {
            continue;
        }

        let file_type = entry
            .file_type()
            .map_err(|source| RitError::io(&path, source))?;
        if file_type.is_dir() {
            scan_directory(root, &path, ignore_rules, output)?;
        } else if file_type.is_file() {
            output.insert(relative);
        }
    }

    Ok(())
}

#[derive(Clone, Debug, Default)]
struct IgnoreRules {
    patterns: Vec<String>,
}

impl IgnoreRules {
    fn read(worktree: &Path, git_dir: &Path) -> Result<Self> {
        let mut patterns = Vec::new();
        read_ignore_file(&worktree.join(".gitignore"), &mut patterns)?;
        read_ignore_file(&git_dir.join("info").join("exclude"), &mut patterns)?;
        Ok(Self { patterns })
    }

    fn matches(&self, path: &str) -> bool {
        self.patterns.iter().any(|pattern| {
            let normalized = pattern.trim_start_matches('/');
            if let Some(directory) = normalized.strip_suffix('/') {
                path == directory || path.starts_with(&format!("{directory}/"))
            } else {
                path == normalized || path.ends_with(&format!("/{normalized}"))
            }
        })
    }
}

fn read_ignore_file(path: &PathBuf, patterns: &mut Vec<String>) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }

    let contents = fs::read_to_string(path).map_err(|source| RitError::io(path, source))?;
    for line in contents.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with('!') {
            continue;
        }
        patterns.push(trimmed.replace('\\', "/"));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{IgnoreRules, PorcelainStatus, StatusEntry};

    #[test]
    fn porcelain_v1_renders_entries() {
        let status = PorcelainStatus {
            entries: vec![StatusEntry {
                index_status: '?',
                worktree_status: '?',
                path: "new.txt".to_owned(),
            }],
        };

        assert_eq!(status.to_porcelain_v1(), "?? new.txt\n");
    }

    #[test]
    fn ignore_rules_match_rooted_directory() {
        let rules = IgnoreRules {
            patterns: vec!["/target/".to_owned()],
        };

        assert!(rules.matches("target/debug/app"));
        assert!(!rules.matches("src/target.rs"));
    }
}
