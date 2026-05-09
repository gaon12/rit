use crate::index::{Index, IndexEntry, join_slash_path, relative_slash_path};
use crate::object::{hash_object, parse_tree_entries};
use crate::{
    ObjectId, ObjectKind, PathspecSet, Repository, Result, RitError, Signature, parse_commit,
    refs::validate_ref_short_name,
};
use std::collections::{BTreeMap, BTreeSet};
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
    /// Adds files matching ordinary literal pathspecs to the index.
    pub fn add_paths(&self, paths: &[String]) -> Result<usize> {
        let Some(worktree) = self.worktree() else {
            return Err(RitError::invalid_input(
                "add must be run in a repository with a working tree",
            ));
        };
        if paths.is_empty() {
            return Err(RitError::invalid_input("add requires at least one path"));
        }
        let pathspecs = PathspecSet::from_args(paths)?;

        let index_path = self.git_dir().join("index");
        let mut index = Index::read(&index_path)?;
        let mut entries = index
            .entries
            .into_iter()
            .map(|entry| (entry.path.clone(), entry))
            .collect::<BTreeMap<_, _>>();
        let files_to_add = expand_add_pathspecs(worktree, &pathspecs, entries.keys())?;

        for relative_path in files_to_add {
            let full_path = join_slash_path(worktree, &relative_path);
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

        for path in entries.keys().cloned().collect::<Vec<_>>() {
            if pathspecs.matches(&path) && !join_slash_path(worktree, &path).exists() {
                entries.remove(&path);
            }
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

    /// Restores working tree files from the index.
    pub fn restore_worktree_paths(&self, paths: &[String]) -> Result<()> {
        let Some(worktree) = self.worktree() else {
            return Err(RitError::invalid_input(
                "restore must be run in a repository with a working tree",
            ));
        };
        if paths.is_empty() {
            return Err(RitError::invalid_input(
                "restore requires at least one path",
            ));
        }
        let index = Index::read(&self.git_dir().join("index"))?;
        let pathspecs = PathspecSet::from_args(paths)?;
        let entries = index
            .entries
            .iter()
            .filter(|entry| pathspecs.matches(&entry.path))
            .collect::<Vec<_>>();
        if entries.is_empty() {
            return Err(RitError::invalid_input(format!(
                "pathspec did not match any indexed file: {}",
                paths.join(" ")
            )));
        }

        for entry in entries {
            let object = self.read_object(entry.object_id)?;
            if object.kind != ObjectKind::Blob {
                return Err(RitError::invalid_input(format!(
                    "object {} is {}, not blob",
                    entry.object_id, object.kind
                )));
            }
            write_worktree_file_atomically(&join_slash_path(worktree, &entry.path), &object.data)?;
        }

        Ok(())
    }

    /// Restores index entries from `HEAD`, returning paths still modified in the worktree.
    pub fn restore_staged_paths_from_head(&self, paths: &[String]) -> Result<Vec<String>> {
        let Some(worktree) = self.worktree() else {
            return Err(RitError::invalid_input(
                "reset must be run in a repository with a working tree",
            ));
        };
        if paths.is_empty() {
            return Err(RitError::invalid_input("reset requires at least one path"));
        }
        let pathspecs = PathspecSet::from_args(paths)?;

        let index_path = self.git_dir().join("index");
        let index = Index::read(&index_path)?;
        let mut index_entries = index
            .entries
            .into_iter()
            .map(|entry| (entry.path.clone(), entry))
            .collect::<BTreeMap<_, _>>();
        let head_entries = self.head_blob_entries()?;
        let target_paths = index_entries
            .keys()
            .chain(head_entries.keys())
            .filter(|path| pathspecs.matches(path))
            .cloned()
            .collect::<BTreeSet<_>>();
        if target_paths.is_empty() {
            return Err(RitError::invalid_input(format!(
                "pathspec did not match any file known to git: {}",
                paths.join(" ")
            )));
        }
        let mut unstaged = Vec::new();

        for normalized in target_paths {
            match head_entries.get(&normalized) {
                Some(head_entry) => {
                    index_entries.insert(
                        normalized.clone(),
                        IndexEntry {
                            mode: head_entry.mode,
                            object_id: head_entry.object_id,
                            file_size: head_entry.file_size,
                            path: normalized.clone(),
                        },
                    );
                    let full_path = join_slash_path(worktree, &normalized);
                    if !full_path.exists() {
                        unstaged.push(format!("D\t{normalized}"));
                    } else {
                        let data = fs::read(&full_path)
                            .map_err(|source| RitError::io(&full_path, source))?;
                        if hash_object(ObjectKind::Blob, &data) != head_entry.object_id {
                            unstaged.push(format!("M\t{normalized}"));
                        }
                    }
                }
                None => {
                    index_entries.remove(&normalized);
                }
            }
        }

        Index {
            entries: index_entries.into_values().collect(),
        }
        .write(&index_path)?;
        Ok(unstaged)
    }

    /// Checks out an existing local branch into a clean working tree.
    pub fn checkout_branch(&self, name: &str) -> Result<ObjectId> {
        validate_ref_short_name(name)?;
        ensure_clean_for_checkout(self)?;
        let target = self.branch_target(name)?;
        self.checkout_commit_tree(target)?;
        write_text_atomically(
            &self.git_dir().join("HEAD"),
            &format!("ref: refs/heads/{name}\n"),
        )?;
        Ok(target)
    }

    /// Creates and checks out a new branch at `HEAD`.
    pub fn checkout_new_branch(&self, name: &str) -> Result<ObjectId> {
        validate_ref_short_name(name)?;
        ensure_clean_for_checkout(self)?;
        let target = self.create_branch(name)?;
        self.checkout_commit_tree(target)?;
        write_text_atomically(
            &self.git_dir().join("HEAD"),
            &format!("ref: refs/heads/{name}\n"),
        )?;
        Ok(target)
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

    fn head_blob_entries(&self) -> Result<BTreeMap<String, HeadBlobEntry>> {
        let Some(head_id) = self.resolve_head()? else {
            return Ok(BTreeMap::new());
        };
        let object = self.read_object(head_id)?;
        if object.kind != ObjectKind::Commit {
            return Err(RitError::invalid_input(format!(
                "HEAD points to {}, not commit",
                object.kind
            )));
        }
        let commit = parse_commit(&object.data)?;
        let mut entries = BTreeMap::new();
        self.collect_head_blob_entries("", commit.tree, &mut entries)?;
        Ok(entries)
    }

    fn collect_head_blob_entries(
        &self,
        prefix: &str,
        tree_id: ObjectId,
        output: &mut BTreeMap<String, HeadBlobEntry>,
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
                self.collect_head_blob_entries(&path, entry.object_id, output)?;
            } else {
                let object = self.read_object(entry.object_id)?;
                output.insert(
                    path,
                    HeadBlobEntry {
                        mode: 0o100644,
                        object_id: entry.object_id,
                        file_size: object.size().min(u32::MAX as usize) as u32,
                    },
                );
            }
        }
        Ok(())
    }

    fn checkout_commit_tree(&self, commit_id: ObjectId) -> Result<()> {
        let Some(worktree) = self.worktree() else {
            return Err(RitError::invalid_input(
                "checkout must be run in a repository with a working tree",
            ));
        };
        let target_entries = self.commit_index_entries(commit_id)?;
        let current_index = Index::read(&self.git_dir().join("index"))?;
        let current_paths = current_index
            .entries
            .iter()
            .map(|entry| entry.path.as_str())
            .collect::<std::collections::BTreeSet<_>>();
        let target_paths = target_entries
            .iter()
            .map(|entry| entry.path.as_str())
            .collect::<std::collections::BTreeSet<_>>();

        for entry in &current_index.entries {
            if !target_paths.contains(entry.path.as_str()) {
                let path = join_slash_path(worktree, &entry.path);
                if path.exists() {
                    fs::remove_file(&path).map_err(|source| RitError::io(&path, source))?;
                }
            }
        }

        for entry in &target_entries {
            let worktree_path = join_slash_path(worktree, &entry.path);
            if !current_paths.contains(entry.path.as_str()) && worktree_path.exists() {
                return Err(RitError::invalid_input(format!(
                    "untracked working tree file would be overwritten by checkout: {}",
                    entry.path
                )));
            }
            let object = self.read_object(entry.object_id)?;
            if object.kind != ObjectKind::Blob {
                return Err(RitError::invalid_input(format!(
                    "object {} is {}, not blob",
                    entry.object_id, object.kind
                )));
            }
            write_worktree_file_atomically(&worktree_path, &object.data)?;
        }

        Index {
            entries: target_entries,
        }
        .write(&self.git_dir().join("index"))
    }

    fn commit_index_entries(&self, commit_id: ObjectId) -> Result<Vec<IndexEntry>> {
        let object = self.read_object(commit_id)?;
        if object.kind != ObjectKind::Commit {
            return Err(RitError::invalid_input(format!(
                "object {commit_id} is {}, not commit",
                object.kind
            )));
        }
        let commit = parse_commit(&object.data)?;
        let mut entries = BTreeMap::new();
        self.collect_head_blob_entries("", commit.tree, &mut entries)?;
        Ok(entries
            .into_iter()
            .map(|(path, entry)| IndexEntry {
                mode: entry.mode,
                object_id: entry.object_id,
                file_size: entry.file_size,
                path,
            })
            .collect())
    }
}

fn expand_add_pathspecs<'a>(
    worktree: &Path,
    pathspecs: &PathspecSet,
    indexed_paths: impl Iterator<Item = &'a String>,
) -> Result<BTreeSet<String>> {
    let mut files = BTreeSet::new();

    if pathspecs.is_all() {
        collect_regular_files(worktree, worktree, &mut files)?;
        return Ok(files);
    }

    let indexed_paths = indexed_paths.cloned().collect::<Vec<_>>();
    for pattern in pathspecs.patterns() {
        let full_path = join_slash_path(worktree, pattern);
        if full_path.is_file() {
            files.insert(relative_slash_path(worktree, &full_path)?);
        } else if full_path.is_dir() {
            collect_regular_files(worktree, &full_path, &mut files)?;
        } else if indexed_paths
            .iter()
            .any(|path| path == pattern || path.starts_with(&format!("{pattern}/")))
        {
            continue;
        } else {
            return Err(RitError::invalid_input(format!(
                "pathspec did not match any files: {pattern}"
            )));
        }
    }

    Ok(files)
}

fn collect_regular_files(
    root: &Path,
    directory: &Path,
    output: &mut BTreeSet<String>,
) -> Result<()> {
    for entry in fs::read_dir(directory).map_err(|source| RitError::io(directory, source))? {
        let entry = entry.map_err(|source| RitError::io(directory, source))?;
        let path = entry.path();
        let relative = relative_slash_path(root, &path)?;
        if relative == ".git" || relative.starts_with(".git/") {
            continue;
        }

        let file_type = entry
            .file_type()
            .map_err(|source| RitError::io(&path, source))?;
        if file_type.is_dir() {
            collect_regular_files(root, &path, output)?;
        } else if file_type.is_file() {
            output.insert(relative);
        }
    }

    Ok(())
}

fn ensure_clean_for_checkout(repository: &Repository) -> Result<()> {
    let status = repository.status_porcelain_v1()?;
    if status
        .entries
        .iter()
        .any(|entry| entry.index_status != '?' || entry.worktree_status != '?')
    {
        return Err(RitError::invalid_input(
            "checkout requires a clean index and working tree",
        ));
    }
    Ok(())
}

#[derive(Clone, Copy)]
struct HeadBlobEntry {
    mode: u32,
    object_id: ObjectId,
    file_size: u32,
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

fn write_worktree_file_atomically(path: &Path, contents: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| RitError::io(parent, source))?;
    }
    let temp_path = path.with_extension(format!("rit-tmp-{}", std::process::id()));
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
    if path.exists() {
        fs::remove_file(path).map_err(|source| RitError::io(path, source))?;
    }
    fs::rename(&temp_path, path).map_err(|source| RitError::io(path, source))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::read_config_value;
    use crate::{Index, InitOptions, Repository};
    use std::fs;
    use std::path::{Path, PathBuf};
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

    #[test]
    fn add_directory_pathspec_adds_nested_files() {
        let temp = temp_path("add-directory");
        let repository = Repository::init(&InitOptions::new(&temp)).expect("init should work");
        fs::create_dir_all(temp.join("nested").join("deeper"))
            .expect("nested directory should be written");
        fs::write(temp.join("nested").join("a.txt"), "one\n").expect("file should be written");
        fs::write(temp.join("nested").join("deeper").join("b.txt"), "two\n")
            .expect("deep file should be written");

        repository
            .add_paths(&["nested".to_owned()])
            .expect("directory add should work");

        let index = Index::read(&repository.git_dir().join("index")).expect("index should read");
        let paths = index
            .entries
            .iter()
            .map(|entry| entry.path.as_str())
            .collect::<Vec<_>>();
        assert_eq!(paths, vec!["nested/a.txt", "nested/deeper/b.txt"]);
        remove_dir_all(&temp);
    }

    #[test]
    fn restore_directory_pathspec_restores_matching_worktree_files() {
        let temp = temp_path("restore-directory");
        let repository = committed_nested_repository(&temp);
        fs::write(temp.join("nested").join("a.txt"), "changed\n").expect("file should be modified");

        repository
            .restore_worktree_paths(&["nested".to_owned()])
            .expect("directory restore should work");

        let contents =
            fs::read_to_string(temp.join("nested").join("a.txt")).expect("file should read");
        assert_eq!(contents, "base\n");
        remove_dir_all(&temp);
    }

    #[test]
    fn reset_directory_pathspec_restores_matching_index_entries() {
        let temp = temp_path("reset-directory");
        let repository = committed_nested_repository(&temp);
        fs::write(temp.join("nested").join("a.txt"), "changed\n").expect("file should be modified");
        repository
            .add_paths(&["nested".to_owned()])
            .expect("modified file should be staged");

        repository
            .restore_staged_paths_from_head(&["nested".to_owned()])
            .expect("directory reset should work");

        let diff = repository
            .diff_index_to_head()
            .expect("cached diff should be readable");
        assert!(diff.files.is_empty());
        let status = repository
            .status_porcelain_v1()
            .expect("status should be readable");
        assert_eq!(status.to_porcelain_v1(), " M nested/a.txt\n");
        remove_dir_all(&temp);
    }

    fn committed_nested_repository(path: &Path) -> Repository {
        let repository = Repository::init(&InitOptions::new(path)).expect("init should work");
        fs::create_dir_all(path.join("nested")).expect("nested directory should be written");
        fs::write(path.join("nested").join("a.txt"), "base\n").expect("file should be written");
        fs::write(
            repository.git_dir().join("config"),
            "[core]\n\trepositoryformatversion = 0\n\tbare = false\n[user]\n\tname = Rit Test\n\temail = rit@example.test\n",
        )
        .expect("config should be written");
        repository
            .add_paths(&["nested".to_owned()])
            .expect("file should be added");
        repository
            .commit_index("base")
            .expect("base commit should be created");
        repository
    }

    fn temp_path(name: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time should be valid")
            .as_nanos();
        std::env::temp_dir().join(format!("rit-write-{name}-{unique}"))
    }

    fn remove_dir_all(path: &Path) {
        if path.exists() {
            fs::remove_dir_all(path).expect("temporary directory should be removed");
        }
    }
}
