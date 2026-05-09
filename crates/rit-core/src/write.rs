use crate::index::{Index, IndexEntry, join_slash_path, relative_slash_path};
use crate::{ObjectId, ObjectKind, Repository, Result, RitError, Signature, parse_commit};
use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

/// Result of creating a commit.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommitResult {
    /// Newly written commit ID.
    pub commit_id: ObjectId,
    /// Number of files tracked by the committed index.
    pub file_count: usize,
}

impl Repository {
    /// Adds explicit files to the index and writes their blob objects.
    pub fn add_paths(&self, paths: &[String]) -> Result<usize> {
        let Some(worktree) = self.worktree() else {
            return Err(RitError::invalid_input(
                "add must be run in a repository with a working tree",
            ));
        };
        if paths.is_empty() {
            return Err(RitError::invalid_input("add requires at least one path"));
        }

        let index_path = self.git_dir().join("index");
        let mut index = Index::read(&index_path)?;
        let mut entries = index
            .entries
            .into_iter()
            .map(|entry| (entry.path.clone(), entry))
            .collect::<BTreeMap<_, _>>();

        for path_text in paths {
            let full_path = join_slash_path(worktree, &path_text.replace('\\', "/"));
            let relative_path = relative_slash_path(worktree, &full_path)?;
            if !full_path.exists() {
                entries.remove(&relative_path);
                continue;
            }
            if !full_path.is_file() {
                return Err(RitError::invalid_input(format!(
                    "only regular files can be added for now: {}",
                    full_path.display()
                )));
            }

            let data = fs::read(&full_path).map_err(|source| RitError::io(&full_path, source))?;
            let object_id = self.loose_objects().write_object(ObjectKind::Blob, &data)?;
            entries.insert(
                relative_path.clone(),
                IndexEntry {
                    mode: 0o100644,
                    object_id,
                    file_size: data.len().min(u32::MAX as usize) as u32,
                    path: relative_path,
                },
            );
        }

        index = Index {
            entries: entries.into_values().collect(),
        };
        let count = index.entries.len();
        index.write(&index_path)?;
        Ok(count)
    }

    /// Creates a commit from the current index and advances `HEAD`.
    pub fn commit_index(&self, message: &str) -> Result<CommitResult> {
        let index = Index::read(&self.git_dir().join("index"))?;
        if index.entries.is_empty() {
            return Err(RitError::invalid_input("nothing to commit"));
        }
        let tree_id = self.write_tree_from_index(&index)?;
        let parent = self.resolve_head()?;
        if let Some(parent_id) = parent {
            let parent_object = self.read_object(parent_id)?;
            let parent_commit = parse_commit(&parent_object.data)?;
            if parent_commit.tree == tree_id {
                return Err(RitError::invalid_input("nothing to commit"));
            }
        }

        let signature = read_signature(self)?;
        let mut commit = Vec::new();
        commit.extend_from_slice(format!("tree {tree_id}\n").as_bytes());
        if let Some(parent_id) = parent {
            commit.extend_from_slice(format!("parent {parent_id}\n").as_bytes());
        }
        let identity = format_signature(&signature);
        commit.extend_from_slice(format!("author {identity}\n").as_bytes());
        commit.extend_from_slice(format!("committer {identity}\n\n").as_bytes());
        commit.extend_from_slice(message.trim_end_matches('\n').as_bytes());
        commit.push(b'\n');

        let commit_id = self
            .loose_objects()
            .write_object(ObjectKind::Commit, &commit)?;
        self.update_head(commit_id)?;
        Ok(CommitResult {
            commit_id,
            file_count: index.entries.len(),
        })
    }

    fn write_tree_from_index(&self, index: &Index) -> Result<ObjectId> {
        let mut root = TreeNode::default();
        for entry in &index.entries {
            root.insert(&entry.path, entry.object_id)?;
        }
        self.write_tree_node(root)
    }

    fn write_tree_node(&self, node: TreeNode) -> Result<ObjectId> {
        let mut data = Vec::new();
        for (name, entry) in node.entries {
            match entry {
                TreeNodeEntry::Blob(object_id) => {
                    data.extend_from_slice(b"100644 ");
                    data.extend_from_slice(name.as_bytes());
                    data.push(0);
                    data.extend_from_slice(object_id.as_bytes());
                }
                TreeNodeEntry::Tree(child) => {
                    let object_id = self.write_tree_node(child)?;
                    data.extend_from_slice(b"40000 ");
                    data.extend_from_slice(name.as_bytes());
                    data.push(0);
                    data.extend_from_slice(object_id.as_bytes());
                }
            }
        }
        self.loose_objects().write_object(ObjectKind::Tree, &data)
    }

    fn update_head(&self, commit_id: ObjectId) -> Result<()> {
        let head_path = self.git_dir().join("HEAD");
        let contents =
            fs::read_to_string(&head_path).map_err(|source| RitError::io(&head_path, source))?;
        if let Some(reference_name) = contents.trim().strip_prefix("ref: ") {
            let reference_path = self.common_dir().join(reference_name);
            write_text_atomically(&reference_path, &format!("{commit_id}\n"))
        } else {
            write_text_atomically(&head_path, &format!("{commit_id}\n"))
        }
    }
}

#[derive(Default)]
struct TreeNode {
    entries: BTreeMap<String, TreeNodeEntry>,
}

enum TreeNodeEntry {
    Blob(ObjectId),
    Tree(TreeNode),
}

impl TreeNode {
    fn insert(&mut self, path: &str, object_id: ObjectId) -> Result<()> {
        let mut parts = path.split('/').collect::<Vec<_>>();
        if parts.is_empty() {
            return Err(RitError::invalid_input("empty index path"));
        }
        self.insert_parts(&mut parts, object_id)
    }

    fn insert_parts(&mut self, parts: &mut Vec<&str>, object_id: ObjectId) -> Result<()> {
        let name = parts.remove(0);
        if parts.is_empty() {
            self.entries
                .insert(name.to_owned(), TreeNodeEntry::Blob(object_id));
            return Ok(());
        }

        let entry = self
            .entries
            .entry(name.to_owned())
            .or_insert_with(|| TreeNodeEntry::Tree(TreeNode::default()));
        let TreeNodeEntry::Tree(child) = entry else {
            return Err(RitError::invalid_input(format!(
                "path conflicts with file: {name}"
            )));
        };
        child.insert_parts(parts, object_id)
    }
}

fn read_signature(repository: &Repository) -> Result<Signature> {
    let name = std::env::var("GIT_AUTHOR_NAME")
        .or_else(|_| std::env::var("GIT_COMMITTER_NAME"))
        .ok()
        .or_else(|| read_config_value(&repository.git_dir().join("config"), "user", "name"));
    let email = std::env::var("GIT_AUTHOR_EMAIL")
        .or_else(|_| std::env::var("GIT_COMMITTER_EMAIL"))
        .ok()
        .or_else(|| read_config_value(&repository.git_dir().join("config"), "user", "email"));
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| RitError::invalid_input("system time is before Unix epoch"))?
        .as_secs() as i64;

    Ok(Signature {
        name: name.ok_or_else(|| {
            RitError::invalid_input("author identity unknown; set user.name or GIT_AUTHOR_NAME")
        })?,
        email: email.ok_or_else(|| {
            RitError::invalid_input("author identity unknown; set user.email or GIT_AUTHOR_EMAIL")
        })?,
        timestamp,
        offset: "+0000".to_owned(),
    })
}

fn read_config_value(path: &Path, section: &str, key: &str) -> Option<String> {
    let contents = fs::read_to_string(path).ok()?;
    let mut current_section = None;
    for line in contents.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            current_section = Some(trimmed.trim_matches(&['[', ']'][..]).to_owned());
            continue;
        }
        if current_section.as_deref() == Some(section) {
            let Some((left, right)) = trimmed.split_once('=') else {
                continue;
            };
            if left.trim() == key {
                return Some(right.trim().to_owned());
            }
        }
    }
    None
}

fn format_signature(signature: &Signature) -> String {
    format!(
        "{} <{}> {} {}",
        signature.name, signature.email, signature.timestamp, signature.offset
    )
}

fn write_text_atomically(path: &Path, contents: &str) -> Result<()> {
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
        file.write_all(contents.as_bytes())
            .map_err(|source| RitError::io(&lock_path, source))?;
        file.sync_all()
            .map_err(|source| RitError::io(&lock_path, source))?;
    }
    fs::rename(&lock_path, path).map_err(|source| RitError::io(path, source))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::read_config_value;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn reads_user_config_values() {
        let path = std::env::temp_dir().join(format!(
            "rit-config-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("time should be valid")
                .as_nanos()
        ));
        fs::write(
            &path,
            "[user]\n\tname = Rit Test\n\temail = rit@example.test\n",
        )
        .expect("config should be written");

        assert_eq!(
            read_config_value(&path, "user", "name"),
            Some("Rit Test".to_owned())
        );
        assert_eq!(
            read_config_value(&path, "user", "email"),
            Some("rit@example.test".to_owned())
        );

        fs::remove_file(path).expect("config should be removed");
    }
}
