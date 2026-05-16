use std::fs;
use std::io::Write;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use std::collections::{BTreeMap, BTreeSet};

use crate::index::{Index, IndexEntry, IndexEntryStat, join_slash_path};
use crate::write::write_worktree_entry_atomically;
use crate::{
    DiffPatch, DiffSummary, GitConfig, ObjectId, ObjectKind, PathspecSet, Repository, Result,
    RitError, Signature, parse_commit,
};

const ZERO_OBJECT_ID: &str = "0000000000000000000000000000000000000000";

/// One entry from `refs/stash` reflog, ordered as Git displays it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StashListEntry {
    /// Display index, where the newest stash is `stash@{0}`.
    pub index: usize,
    /// Reflog message displayed after `stash@{n}: `.
    pub message: String,
}

/// Result of dropping one stash entry.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StashDropResult {
    /// Display name of the dropped stash, such as `refs/stash@{0}`.
    pub name: String,
    /// Commit ID that was stored by the dropped reflog entry.
    pub object_id: ObjectId,
}

/// Result of applying one stash entry.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StashApplyResult {
    /// Stash commit that was applied.
    pub object_id: ObjectId,
    /// Repository-relative paths written or removed in the working tree.
    pub paths: Vec<String>,
}

/// Result of saving the current tracked changes with `stash push`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StashPushResult {
    /// No tracked index or working-tree changes were present.
    NoLocalChanges,
    /// A new stash commit was written and `refs/stash` now points to it.
    Saved {
        /// Commit ID stored in `refs/stash`.
        object_id: ObjectId,
        /// Reflog and commit message shown by `stash list`.
        message: String,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct StashReflogEntry {
    old_id: ObjectId,
    new_id: ObjectId,
    rest: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CreatedStashCommit {
    object_id: ObjectId,
    message: String,
}

impl Repository {
    /// Creates a stash commit for tracked changes without storing it in `refs/stash`.
    ///
    /// This mirrors `git stash create`: the index and working tree are left as-is,
    /// and clean repositories return `None`.
    pub fn stash_create(&self, message: Option<&str>) -> Result<Option<ObjectId>> {
        Ok(self
            .create_tracked_stash_commit(message)?
            .map(|created| created.object_id))
    }

    /// Saves tracked index and working-tree changes as a loose `refs/stash` entry.
    ///
    /// This implements the default `git stash push` shape for tracked paths.
    /// Untracked files, pathspec filtering, `--keep-index`, and apply/pop are
    /// intentionally left to later milestone slices.
    pub fn stash_push(&self, message: Option<&str>) -> Result<StashPushResult> {
        let Some(created) = self.create_tracked_stash_commit(message)? else {
            return Ok(StashPushResult::NoLocalChanges);
        };
        let head_id = self
            .resolve_head()?
            .ok_or_else(|| RitError::invalid_input("stash push requires an existing HEAD"))?;

        self.stash_store(created.object_id, Some(&created.message))?;
        self.checkout_commit_tree(head_id)?;
        Ok(StashPushResult::Saved {
            object_id: created.object_id,
            message: created.message,
        })
    }

    /// Applies tracked working-tree changes from one loose stash entry.
    ///
    /// This first apply slice requires a clean tracked index/worktree and a
    /// current `HEAD` that still matches the stash base. It restores worktree
    /// files only; `--index`, conflict handling, and cross-branch applies are
    /// later milestone work.
    pub fn stash_apply(&self, display_index: usize) -> Result<StashApplyResult> {
        let Some(worktree) = self.worktree() else {
            return Err(RitError::invalid_input(
                "stash apply must be run in a repository with a working tree",
            ));
        };
        ensure_clean_tracked_state_for_stash_apply(self)?;

        let stash_id = self.stash_id_at(display_index)?;
        let stash_object = self.read_object(stash_id)?;
        if stash_object.kind != ObjectKind::Commit {
            return Err(RitError::invalid_input(format!(
                "stash entry {stash_id} is {}, not commit",
                stash_object.kind
            )));
        }
        let stash_commit = parse_commit(&stash_object.data)?;
        let base_id = stash_commit
            .parents
            .first()
            .copied()
            .ok_or_else(|| RitError::invalid_input("stash commit has no parent"))?;
        let head_id = self
            .resolve_head()?
            .ok_or_else(|| RitError::invalid_input("stash apply requires an existing HEAD"))?;
        if head_id != base_id {
            return Err(RitError::invalid_input(
                "stash apply currently requires HEAD to match the stash base",
            ));
        }

        let base_entries = index_entries_by_path(self.commit_index_entries(base_id)?);
        let stash_entries = index_entries_by_path(self.commit_index_entries(stash_id)?);
        let changed_paths = changed_stash_paths(&base_entries, &stash_entries);
        let symlinks_enabled = self.core_symlinks_enabled()?;
        for path in &changed_paths {
            let full_path = join_slash_path(worktree, path);
            match stash_entries.get(path) {
                Some(entry) => {
                    let object = self.read_object(entry.object_id)?;
                    if object.kind != ObjectKind::Blob {
                        return Err(RitError::invalid_input(format!(
                            "object {} is {}, not blob",
                            entry.object_id, object.kind
                        )));
                    }
                    write_worktree_entry_atomically(
                        &full_path,
                        &object.data,
                        entry.mode,
                        symlinks_enabled,
                    )?;
                }
                None => remove_file_if_exists(&full_path)?,
            }
        }

        Ok(StashApplyResult {
            object_id: stash_id,
            paths: changed_paths.into_iter().collect(),
        })
    }

    /// Applies one tracked stash entry and drops it from the loose stash reflog.
    ///
    /// This uses the same intentionally small apply implementation as
    /// [`Repository::stash_apply`], so it currently requires the stash base to
    /// match `HEAD` and does not restore the index.
    pub fn stash_pop(&self, display_index: usize, name: String) -> Result<StashDropResult> {
        self.stash_apply(display_index)?;
        self.stash_drop(display_index, name)
    }

    /// Lists stashes by reading the Git-compatible `refs/stash` reflog.
    pub fn stash_list(&self) -> Result<Vec<StashListEntry>> {
        let mut messages = self
            .read_stash_reflog()?
            .into_iter()
            .filter_map(|entry| {
                entry
                    .rest
                    .split_once('\t')
                    .map(|(_, message)| message.to_owned())
            })
            .collect::<Vec<_>>();
        messages.reverse();

        Ok(messages
            .into_iter()
            .enumerate()
            .map(|(index, message)| StashListEntry { index, message })
            .collect())
    }

    /// Clears the loose `refs/stash` ref and its reflog.
    pub fn stash_clear(&self) -> Result<()> {
        remove_file_if_exists(&self.common_dir().join("refs").join("stash"))?;
        remove_file_if_exists(&self.common_dir().join("logs").join("refs").join("stash"))
    }

    /// Drops one stash reflog entry and updates loose `refs/stash`.
    pub fn stash_drop(&self, display_index: usize, name: String) -> Result<StashDropResult> {
        let mut entries = self.read_stash_reflog()?;
        if entries.is_empty() {
            return Err(RitError::invalid_input("No stash entries found."));
        }
        if display_index >= entries.len() {
            return Err(RitError::invalid_input(format!(
                "log for 'stash' only has {} entries",
                entries.len()
            )));
        }

        let storage_index = entries.len() - 1 - display_index;
        let dropped = entries.remove(storage_index);
        if entries.is_empty() {
            self.stash_clear()?;
        } else {
            relink_stash_reflog_entries(&mut entries)?;
            self.write_stash_reflog(&entries)?;
            self.write_stash_ref(entries.last().expect("entries not empty").new_id)?;
        }

        Ok(StashDropResult {
            name,
            object_id: dropped.new_id,
        })
    }

    /// Stores an existing commit as the newest loose stash entry.
    pub fn stash_store(&self, target: ObjectId, message: Option<&str>) -> Result<()> {
        let object = self.read_object(target)?;
        if object.kind != ObjectKind::Commit {
            return Err(RitError::invalid_input(format!(
                "stash store target {target} is {}, not commit",
                object.kind
            )));
        }

        let mut entries = self.read_stash_reflog()?;
        let old_id = entries
            .last()
            .map(|entry| entry.new_id)
            .unwrap_or(zero_object_id()?);
        entries.push(StashReflogEntry {
            old_id,
            new_id: target,
            rest: format!(
                "{}\t{}",
                format_reflog_signature(&self.reflog_committer()?),
                message.unwrap_or("Created via \"git stash store\".")
            ),
        });
        self.write_stash_reflog(&entries)?;
        self.write_stash_ref(target)
    }

    /// Shows the changes recorded by one stash against its first parent.
    pub fn stash_show(&self, display_index: usize, pathspecs: &PathspecSet) -> Result<DiffSummary> {
        let (base_id, stash_id) = self.stash_diff_pair(display_index)?;
        self.diff_commits_with_pathspecs(base_id, stash_id, pathspecs)
    }

    /// Shows patch output for the changes recorded by one stash.
    pub fn stash_show_patch(
        &self,
        display_index: usize,
        pathspecs: &PathspecSet,
    ) -> Result<DiffPatch> {
        let (base_id, stash_id) = self.stash_diff_pair(display_index)?;
        self.diff_commits_patch_with_pathspecs(base_id, stash_id, pathspecs)
    }

    fn stash_diff_pair(&self, display_index: usize) -> Result<(ObjectId, ObjectId)> {
        let stash_id = self.stash_id_at(display_index)?;
        let stash_object = self.read_object(stash_id)?;
        if stash_object.kind != ObjectKind::Commit {
            return Err(RitError::invalid_input(format!(
                "stash entry {stash_id} is {}, not commit",
                stash_object.kind
            )));
        }
        let stash_commit = crate::parse_commit(&stash_object.data)?;
        let base_id = stash_commit
            .parents
            .first()
            .copied()
            .ok_or_else(|| RitError::invalid_input("stash commit has no parent"))?;
        Ok((base_id, stash_id))
    }

    fn read_stash_reflog(&self) -> Result<Vec<StashReflogEntry>> {
        let path = self.common_dir().join("logs").join("refs").join("stash");
        let text = match fs::read_to_string(&path) {
            Ok(text) => text,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(source) => return Err(RitError::io(&path, source)),
        };

        text.lines().map(parse_stash_reflog_entry).collect()
    }

    fn stash_id_at(&self, display_index: usize) -> Result<ObjectId> {
        let entries = self.read_stash_reflog()?;
        if entries.is_empty() {
            return Err(RitError::invalid_input("No stash entries found."));
        }
        if display_index >= entries.len() {
            return Err(RitError::invalid_input(format!(
                "log for 'stash' only has {} entries",
                entries.len()
            )));
        }
        Ok(entries[entries.len() - 1 - display_index].new_id)
    }

    fn write_stash_reflog(&self, entries: &[StashReflogEntry]) -> Result<()> {
        let path = self.common_dir().join("logs").join("refs").join("stash");
        write_file_atomically(&path, |file| {
            for entry in entries {
                writeln!(file, "{} {} {}", entry.old_id, entry.new_id, entry.rest)?;
            }
            Ok(())
        })
    }

    fn write_stash_ref(&self, target: ObjectId) -> Result<()> {
        let path = self.common_dir().join("refs").join("stash");
        write_file_atomically(&path, |file| writeln!(file, "{target}"))
    }

    fn create_tracked_stash_commit(
        &self,
        message: Option<&str>,
    ) -> Result<Option<CreatedStashCommit>> {
        let head_id = self
            .resolve_head()?
            .ok_or_else(|| RitError::invalid_input("stash create requires an existing HEAD"))?;
        if !self.has_tracked_stash_changes()? {
            return Ok(None);
        }

        let index = Index::read(&self.git_dir().join("index"))?;
        ensure_stashable_index(&index)?;
        let index_tree_id = self.write_tree_from_index(&index)?;
        let stash_message = self.stash_push_message(head_id, message)?;
        let index_commit_id = self.write_stash_commit(
            index_tree_id,
            &[head_id],
            &index_commit_message(&stash_message),
        )?;
        let worktree_index = self.worktree_stash_index(&index)?;
        let worktree_tree_id = self.write_tree_from_index(&worktree_index)?;
        let stash_id = self.write_stash_commit(
            worktree_tree_id,
            &[head_id, index_commit_id],
            &stash_message,
        )?;
        Ok(Some(CreatedStashCommit {
            object_id: stash_id,
            message: stash_message,
        }))
    }

    fn has_tracked_stash_changes(&self) -> Result<bool> {
        Ok(self.status_porcelain_v1()?.entries.iter().any(|entry| {
            entry.index_status != '?' && (entry.index_status != ' ' || entry.worktree_status != ' ')
        }))
    }

    fn worktree_stash_index(&self, index: &Index) -> Result<Index> {
        let Some(worktree) = self.worktree() else {
            return Err(RitError::invalid_input(
                "stash push must be run in a repository with a working tree",
            ));
        };
        let symlinks_enabled = self.core_symlinks_enabled()?;
        let mut entries = Vec::new();
        for entry in index.entries.iter().filter(|entry| entry.stage == 0) {
            let full_path = join_slash_path(worktree, &entry.path);
            let metadata = match fs::symlink_metadata(&full_path) {
                Ok(metadata) => metadata,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
                Err(source) => return Err(RitError::io(&full_path, source)),
            };
            let is_symlink = metadata.file_type().is_symlink();
            let store_symlink = is_symlink && symlinks_enabled;
            let data = if store_symlink {
                read_symlink_target_bytes(&full_path)?
            } else {
                fs::read(&full_path).map_err(|source| RitError::io(&full_path, source))?
            };
            let mode = if store_symlink { 0o120000 } else { entry.mode };
            let object_id = self.loose_objects().write_object(ObjectKind::Blob, &data)?;
            entries.push(IndexEntry {
                stat: IndexEntryStat::from_metadata(&metadata),
                mode,
                object_id,
                stage: 0,
                extended_flags: 0,
                file_size: data.len().min(u32::MAX as usize) as u32,
                path: entry.path.clone(),
            });
        }
        Ok(Index {
            entries,
            extensions: Vec::new(),
        })
    }

    fn stash_push_message(&self, head_id: ObjectId, message: Option<&str>) -> Result<String> {
        let branch = self
            .current_branch_name()?
            .unwrap_or_else(|| "(no branch)".to_owned());
        if let Some(message) = message {
            return Ok(format!("On {branch}: {message}"));
        }

        let object = self.read_object(head_id)?;
        if object.kind != ObjectKind::Commit {
            return Err(RitError::invalid_input(format!(
                "HEAD points to {}, not commit",
                object.kind
            )));
        }
        let commit = parse_commit(&object.data)?;
        let subject = commit.message.lines().next().unwrap_or("");
        Ok(format!("WIP on {branch}: {} {subject}", short_id(head_id)))
    }

    fn write_stash_commit(
        &self,
        tree_id: ObjectId,
        parents: &[ObjectId],
        message: &str,
    ) -> Result<ObjectId> {
        let signature = self.reflog_committer()?;
        let mut commit = Vec::new();
        commit.extend_from_slice(format!("tree {tree_id}\n").as_bytes());
        for parent_id in parents {
            commit.extend_from_slice(format!("parent {parent_id}\n").as_bytes());
        }
        commit.extend_from_slice(
            format!("author {}\n", format_reflog_signature(&signature)).as_bytes(),
        );
        commit.extend_from_slice(
            format!("committer {}\n\n", format_reflog_signature(&signature)).as_bytes(),
        );
        commit.extend_from_slice(message.trim_end_matches('\n').as_bytes());
        commit.push(b'\n');
        self.loose_objects()
            .write_object(ObjectKind::Commit, &commit)
    }

    fn reflog_committer(&self) -> Result<Signature> {
        let config_path = self.common_dir().join("config");
        let name = std::env::var("GIT_COMMITTER_NAME")
            .ok()
            .or_else(|| read_config_value(&config_path, "user", "name"))
            .ok_or_else(|| {
                RitError::invalid_input(
                    "committer identity unknown; set user.name or GIT_COMMITTER_NAME",
                )
            })?;
        let email = std::env::var("GIT_COMMITTER_EMAIL")
            .ok()
            .or_else(|| read_config_value(&config_path, "user", "email"))
            .ok_or_else(|| {
                RitError::invalid_input(
                    "committer identity unknown; set user.email or GIT_COMMITTER_EMAIL",
                )
            })?;
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| RitError::invalid_input("system time is before Unix epoch"))?
            .as_secs() as i64;
        Ok(Signature {
            name,
            email,
            timestamp,
            offset: "+0000".to_owned(),
        })
    }
}

fn parse_stash_reflog_entry(line: &str) -> Result<StashReflogEntry> {
    let mut parts = line.splitn(3, ' ');
    let old_id = parts
        .next()
        .ok_or_else(|| RitError::invalid_input("malformed stash reflog entry"))?;
    let new_id = parts
        .next()
        .ok_or_else(|| RitError::invalid_input("malformed stash reflog entry"))?;
    let rest = parts
        .next()
        .ok_or_else(|| RitError::invalid_input("malformed stash reflog entry"))?;

    Ok(StashReflogEntry {
        old_id: ObjectId::from_hex(old_id)?,
        new_id: ObjectId::from_hex(new_id)?,
        rest: rest.to_owned(),
    })
}

fn relink_stash_reflog_entries(entries: &mut [StashReflogEntry]) -> Result<()> {
    let mut previous = zero_object_id()?;
    for entry in entries {
        entry.old_id = previous;
        previous = entry.new_id;
    }
    Ok(())
}

fn zero_object_id() -> Result<ObjectId> {
    ObjectId::from_hex(ZERO_OBJECT_ID)
}

fn read_config_value(path: &Path, section: &str, key: &str) -> Option<String> {
    GitConfig::read(path)
        .ok()
        .and_then(|config| config.get(section, key).map(ToOwned::to_owned))
}

fn format_reflog_signature(signature: &Signature) -> String {
    format!(
        "{} <{}> {} {}",
        signature.name, signature.email, signature.timestamp, signature.offset
    )
}

fn remove_file_if_exists(path: &Path) -> Result<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(source) => Err(RitError::io(path, source)),
    }
}

fn write_file_atomically(
    path: &Path,
    write_contents: impl FnOnce(&mut fs::File) -> std::io::Result<()>,
) -> Result<()> {
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
        write_contents(&mut file).map_err(|source| RitError::io(&lock_path, source))?;
        file.sync_all()
            .map_err(|source| RitError::io(&lock_path, source))?;
    }
    fs::rename(&lock_path, path).map_err(|source| RitError::io(path, source))
}

fn ensure_stashable_index(index: &Index) -> Result<()> {
    if let Some(entry) = index.entries.iter().find(|entry| entry.stage != 0) {
        return Err(RitError::invalid_input(format!(
            "cannot stash with unmerged index entry: {}",
            entry.path
        )));
    }
    Ok(())
}

fn ensure_clean_tracked_state_for_stash_apply(repository: &Repository) -> Result<()> {
    if repository
        .status_porcelain_v1()?
        .entries
        .iter()
        .any(|entry| entry.index_status != '?')
    {
        return Err(RitError::invalid_input(
            "stash apply requires a clean tracked index and working tree",
        ));
    }
    Ok(())
}

fn index_entries_by_path(entries: Vec<IndexEntry>) -> BTreeMap<String, IndexEntry> {
    entries
        .into_iter()
        .map(|entry| (entry.path.clone(), entry))
        .collect()
}

fn changed_stash_paths(
    base_entries: &BTreeMap<String, IndexEntry>,
    stash_entries: &BTreeMap<String, IndexEntry>,
) -> BTreeSet<String> {
    base_entries
        .keys()
        .chain(stash_entries.keys())
        .filter(|path| {
            let base = base_entries.get(*path);
            let stash = stash_entries.get(*path);
            !same_stash_tree_entry(base, stash)
        })
        .cloned()
        .collect()
}

fn same_stash_tree_entry(left: Option<&IndexEntry>, right: Option<&IndexEntry>) -> bool {
    match (left, right) {
        (Some(left), Some(right)) => left.object_id == right.object_id && left.mode == right.mode,
        (None, None) => true,
        _ => false,
    }
}

fn index_commit_message(stash_message: &str) -> String {
    match stash_message.strip_prefix("WIP on ") {
        Some(rest) => format!("index on {rest}"),
        None => format!("index {stash_message}"),
    }
}

fn short_id(object_id: ObjectId) -> String {
    object_id.to_hex().chars().take(7).collect()
}

#[cfg(unix)]
fn read_symlink_target_bytes(path: &Path) -> Result<Vec<u8>> {
    use std::os::unix::ffi::OsStrExt;
    let target = fs::read_link(path).map_err(|source| RitError::io(path, source))?;
    Ok(target.as_os_str().as_bytes().to_vec())
}

#[cfg(not(unix))]
fn read_symlink_target_bytes(path: &Path) -> Result<Vec<u8>> {
    fs::read(path).map_err(|source| RitError::io(path, source))
}
