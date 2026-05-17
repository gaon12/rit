use std::fs;
use std::io::Write;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use std::collections::{BTreeMap, BTreeSet};

use crate::index::{Index, IndexEntry, IndexEntryStat, join_slash_path};
use crate::write::write_worktree_entry_atomically;
use crate::{
    DiffFileStat, DiffPatch, DiffPatchFile, DiffSummary, GitConfig, ObjectId, ObjectKind,
    PathspecSet, Repository, Result, RitError, Signature, StatusOptions, UntrackedFilesMode,
    parse_commit,
};

const ZERO_OBJECT_ID: &str = "0000000000000000000000000000000000000000";
const STASH_EXPORT_PREFIX: &str = "git stash: ";

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
    /// A new stash was stored, but cleaning the working tree failed.
    SavedCleanupFailed {
        /// Commit ID stored in `refs/stash`.
        object_id: ObjectId,
        /// Reflog and commit message shown by `stash list`.
        message: String,
        /// Cleanup failure message.
        cleanup_error: String,
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
    paths: Vec<String>,
    untracked_paths: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct StashExportItem {
    stash_id: ObjectId,
    base_id: ObjectId,
    message: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum StashCleanupMode {
    RestoreHead,
    KeepIndex,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum StashChangeMode {
    AllTracked,
    StagedOnly,
}

impl Repository {
    /// Creates a stash commit for tracked changes without storing it in `refs/stash`.
    ///
    /// This mirrors `git stash create`: the index and working tree are left as-is,
    /// and clean repositories return `None`.
    pub fn stash_create(&self, message: Option<&str>) -> Result<Option<ObjectId>> {
        Ok(self
            .create_tracked_stash_commit(
                message,
                &PathspecSet::all(),
                StashChangeMode::AllTracked,
                false,
                false,
            )?
            .map(|created| created.object_id))
    }

    /// Saves tracked index and working-tree changes as a loose `refs/stash` entry.
    ///
    /// This implements the default `git stash push` shape for tracked paths.
    /// Untracked files, staged-only mode, and pathspec file expansion are
    /// intentionally left to later milestone slices.
    pub fn stash_push(&self, message: Option<&str>) -> Result<StashPushResult> {
        self.stash_push_with_pathspecs(message, &PathspecSet::all())
    }

    /// Saves tracked index and working-tree changes matching `pathspecs`.
    pub fn stash_push_with_pathspecs(
        &self,
        message: Option<&str>,
        pathspecs: &PathspecSet,
    ) -> Result<StashPushResult> {
        self.stash_push_with_cleanup(message, pathspecs, StashCleanupMode::RestoreHead)
    }

    /// Saves tracked changes matching `pathspecs` while keeping index state.
    pub fn stash_push_keep_index_with_pathspecs(
        &self,
        message: Option<&str>,
        pathspecs: &PathspecSet,
    ) -> Result<StashPushResult> {
        self.stash_push_with_cleanup(message, pathspecs, StashCleanupMode::KeepIndex)
    }

    /// Saves staged changes matching `pathspecs` and leaves unstaged paths alone.
    pub fn stash_push_staged_with_pathspecs(
        &self,
        message: Option<&str>,
        pathspecs: &PathspecSet,
    ) -> Result<StashPushResult> {
        self.stash_push_with_mode(
            message,
            pathspecs,
            StashCleanupMode::RestoreHead,
            StashChangeMode::StagedOnly,
            false,
            false,
        )
    }

    /// Saves tracked and untracked changes matching `pathspecs`.
    pub fn stash_push_include_untracked_with_pathspecs(
        &self,
        message: Option<&str>,
        pathspecs: &PathspecSet,
    ) -> Result<StashPushResult> {
        self.stash_push_with_mode(
            message,
            pathspecs,
            StashCleanupMode::RestoreHead,
            StashChangeMode::AllTracked,
            true,
            false,
        )
    }

    /// Saves tracked, untracked, and ignored changes matching `pathspecs`.
    pub fn stash_push_all_with_pathspecs(
        &self,
        message: Option<&str>,
        pathspecs: &PathspecSet,
    ) -> Result<StashPushResult> {
        self.stash_push_with_mode(
            message,
            pathspecs,
            StashCleanupMode::RestoreHead,
            StashChangeMode::AllTracked,
            true,
            true,
        )
    }

    fn stash_push_with_cleanup(
        &self,
        message: Option<&str>,
        pathspecs: &PathspecSet,
        cleanup_mode: StashCleanupMode,
    ) -> Result<StashPushResult> {
        self.stash_push_with_mode(
            message,
            pathspecs,
            cleanup_mode,
            StashChangeMode::AllTracked,
            false,
            false,
        )
    }

    fn stash_push_with_mode(
        &self,
        message: Option<&str>,
        pathspecs: &PathspecSet,
        cleanup_mode: StashCleanupMode,
        change_mode: StashChangeMode,
        include_untracked: bool,
        include_ignored: bool,
    ) -> Result<StashPushResult> {
        let Some(created) = self.create_tracked_stash_commit(
            message,
            pathspecs,
            change_mode,
            include_untracked,
            include_ignored,
        )?
        else {
            return Ok(StashPushResult::NoLocalChanges);
        };
        let head_id = self
            .resolve_head()?
            .ok_or_else(|| RitError::invalid_input("stash push requires an existing HEAD"))?;
        let cleanup_index = if cleanup_mode == StashCleanupMode::KeepIndex {
            Some(Index::read(&self.git_dir().join("index"))?)
        } else {
            None
        };

        self.stash_store(created.object_id, Some(&created.message))?;
        if let Some(index) = cleanup_index {
            self.restore_stash_paths_to_index(&index, &created.paths)?;
        } else {
            if change_mode == StashChangeMode::StagedOnly
                && let Err(error) = self.ensure_staged_stash_cleanup_supported(pathspecs)
            {
                return Ok(StashPushResult::SavedCleanupFailed {
                    object_id: created.object_id,
                    message: created.message,
                    cleanup_error: error.to_string(),
                });
            }
            self.restore_stash_paths_to_head(head_id, &created.paths)?;
        }
        self.remove_stashed_untracked_paths(&created.untracked_paths)?;
        Ok(StashPushResult::Saved {
            object_id: created.object_id,
            message: created.message,
        })
    }

    /// Applies tracked working-tree changes from one loose stash entry.
    ///
    /// This first apply slice requires a clean tracked index/worktree and a
    /// current `HEAD` that still matches the stash base. Conflict handling and
    /// cross-branch applies are later milestone work.
    pub fn stash_apply(&self, display_index: usize) -> Result<StashApplyResult> {
        self.stash_apply_with_index(display_index, false)
    }

    /// Applies one stash entry and optionally restores its saved index state.
    pub fn stash_apply_with_index(
        &self,
        display_index: usize,
        restore_index: bool,
    ) -> Result<StashApplyResult> {
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
        let mut changed_paths = changed_stash_paths(&base_entries, &stash_entries);
        let index_parent_entries = if restore_index {
            let index_parent_id = stash_commit
                .parents
                .get(1)
                .copied()
                .ok_or_else(|| RitError::invalid_input("stash commit has no index parent"))?;
            let entries = self.commit_index_entries(index_parent_id)?;
            changed_paths.extend(changed_stash_paths(
                &base_entries,
                &index_entries_by_path(entries.clone()),
            ));
            Some(Index {
                entries,
                extensions: Vec::new(),
            })
        } else {
            None
        };
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
        if let Some(index_parent) = index_parent_entries {
            let target_paths = changed_paths.iter().cloned().collect::<Vec<_>>();
            self.replace_stash_paths_in_index(&index_parent, &target_paths)?;
        }
        if let Some(untracked_id) = stash_commit.parents.get(2).copied() {
            let untracked_entries = self.commit_index_entries(untracked_id)?;
            for entry in untracked_entries.iter().filter(|entry| entry.stage == 0) {
                let object = self.read_object(entry.object_id)?;
                if object.kind != ObjectKind::Blob {
                    return Err(RitError::invalid_input(format!(
                        "object {} is {}, not blob",
                        entry.object_id, object.kind
                    )));
                }
                write_worktree_entry_atomically(
                    &join_slash_path(worktree, &entry.path),
                    &object.data,
                    entry.mode,
                    symlinks_enabled,
                )?;
                changed_paths.insert(entry.path.clone());
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
    /// match `HEAD`.
    pub fn stash_pop(&self, display_index: usize, name: String) -> Result<StashDropResult> {
        self.stash_pop_with_index(display_index, name, false)
    }

    /// Applies one stash entry and drops it, optionally restoring its saved index state.
    pub fn stash_pop_with_index(
        &self,
        display_index: usize,
        name: String,
        restore_index: bool,
    ) -> Result<StashDropResult> {
        self.stash_apply_with_index(display_index, restore_index)?;
        self.stash_drop(display_index, name)
    }

    /// Creates a branch at the selected stash base, applies the stash, then drops it.
    ///
    /// This mirrors the currently supported clean tracked apply/pop scope. If
    /// applying the stash fails, the newly checked out branch is left in place
    /// and the stash entry is not dropped, matching Git's safety shape.
    pub fn stash_branch(
        &self,
        branch_name: &str,
        display_index: usize,
        name: String,
    ) -> Result<StashDropResult> {
        let (base_id, _) = self.stash_diff_pair(display_index)?;
        self.create_branch_at(branch_name, base_id)?;
        self.checkout_branch(branch_name)?;
        self.stash_apply(display_index)?;
        self.stash_drop(display_index, name)
    }

    /// Exports selected stash entries into Git's stash-export commit chain.
    ///
    /// An empty `display_indices` slice exports all current entries in display
    /// order, matching `git stash export --print`.
    pub fn stash_export(&self, display_indices: &[usize]) -> Result<ObjectId> {
        let entries = self.read_stash_reflog()?;
        if entries.is_empty() {
            return Err(RitError::invalid_input("No stash entries found."));
        }
        let indices = if display_indices.is_empty() {
            (0..entries.len()).collect::<Vec<_>>()
        } else {
            display_indices.to_vec()
        };
        let mut items = Vec::new();
        for display_index in indices {
            let entry = stash_reflog_entry_at_display_index(&entries, display_index)?;
            let stash_object = self.read_object(entry.new_id)?;
            if stash_object.kind != ObjectKind::Commit {
                return Err(RitError::invalid_input(format!(
                    "stash entry {} is {}, not commit",
                    entry.new_id, stash_object.kind
                )));
            }
            let stash_commit = parse_commit(&stash_object.data)?;
            let base_id = stash_commit
                .parents
                .first()
                .copied()
                .ok_or_else(|| RitError::invalid_input("stash commit has no parent"))?;
            items.push(StashExportItem {
                stash_id: entry.new_id,
                base_id,
                message: stash_reflog_message(entry)?,
            });
        }

        self.write_stash_export_chain(&items)
    }

    /// Exports selected stash entries and writes the export commit to `ref_name`.
    pub fn stash_export_to_ref(
        &self,
        display_indices: &[usize],
        ref_name: &str,
    ) -> Result<ObjectId> {
        let object_id = self.stash_export(display_indices)?;
        self.write_stash_export_ref(ref_name, object_id)?;
        Ok(object_id)
    }

    /// Imports a Git stash-export commit chain into loose `refs/stash`.
    pub fn stash_import(&self, export_id: ObjectId) -> Result<Vec<ObjectId>> {
        let mut current_id = export_id;
        let mut stash_ids = Vec::new();

        loop {
            let export_commit = self.parse_stash_export_commit(current_id)?;
            let stash_id = *export_commit.parents.get(1).ok_or_else(|| {
                RitError::invalid_input("stash export commit has no stash parent")
            })?;
            self.ensure_stash_commit(stash_id)?;
            stash_ids.push(stash_id);

            let Some(next_id) = export_commit.parents.first().copied() else {
                break;
            };
            if self.is_stash_export_commit(next_id)? {
                current_id = next_id;
            } else {
                break;
            }
        }

        for stash_id in stash_ids.iter().rev() {
            let message = self.stash_commit_subject(*stash_id)?;
            self.stash_store(*stash_id, Some(&message))?;
        }
        Ok(stash_ids)
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
        self.remove_stash_from_packed_refs()?;
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

    /// Shows tracked changes and untracked third-parent entries from one stash.
    pub fn stash_show_include_untracked(
        &self,
        display_index: usize,
        pathspecs: &PathspecSet,
    ) -> Result<DiffSummary> {
        let mut diff = self.stash_show(display_index, pathspecs)?;
        diff.files.extend(
            self.stash_show_only_untracked(display_index, pathspecs)?
                .files,
        );
        diff.files.sort_by(|left, right| left.path.cmp(&right.path));
        Ok(diff)
    }

    /// Shows only untracked third-parent entries from one stash.
    pub fn stash_show_only_untracked(
        &self,
        display_index: usize,
        pathspecs: &PathspecSet,
    ) -> Result<DiffSummary> {
        let mut files = Vec::new();
        if let Some(untracked_id) = self.stash_untracked_parent(display_index)? {
            for entry in self.stash_untracked_entries(untracked_id, pathspecs)? {
                let object = self.read_stash_blob(entry.object_id)?;
                files.push(DiffFileStat {
                    status: 'A',
                    old_path: None,
                    path: entry.path,
                    similarity_score: None,
                    insertions: count_text_lines(&object.data),
                    deletions: 0,
                    binary: object.data.contains(&0),
                    old_size: 0,
                    new_size: object.data.len(),
                });
            }
            files.sort_by(|left, right| left.path.cmp(&right.path));
        }
        Ok(DiffSummary {
            files,
            warnings: Vec::new(),
        })
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

    /// Shows tracked changes and untracked third-parent entries as patch output.
    pub fn stash_show_patch_include_untracked(
        &self,
        display_index: usize,
        pathspecs: &PathspecSet,
    ) -> Result<DiffPatch> {
        let mut patch = self.stash_show_patch(display_index, pathspecs)?;
        patch.files.extend(
            self.stash_show_patch_only_untracked(display_index, pathspecs)?
                .files,
        );
        patch
            .files
            .sort_by(|left, right| left.path.cmp(&right.path));
        Ok(patch)
    }

    /// Shows only untracked third-parent entries as patch output.
    pub fn stash_show_patch_only_untracked(
        &self,
        display_index: usize,
        pathspecs: &PathspecSet,
    ) -> Result<DiffPatch> {
        let mut files = Vec::new();
        if let Some(untracked_id) = self.stash_untracked_parent(display_index)? {
            for entry in self.stash_untracked_entries(untracked_id, pathspecs)? {
                let object = self.read_stash_blob(entry.object_id)?;
                files.push(DiffPatchFile {
                    status: 'A',
                    old_path: None,
                    path: entry.path,
                    similarity_score: None,
                    old_object_id: None,
                    new_object_id: Some(entry.object_id),
                    mode: entry.mode,
                    old_data: Vec::new(),
                    new_data: object.data,
                });
            }
            files.sort_by(|left, right| left.path.cmp(&right.path));
        }
        Ok(DiffPatch {
            files,
            warnings: Vec::new(),
        })
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

    fn stash_untracked_parent(&self, display_index: usize) -> Result<Option<ObjectId>> {
        let stash_id = self.stash_id_at(display_index)?;
        let stash_object = self.read_object(stash_id)?;
        if stash_object.kind != ObjectKind::Commit {
            return Err(RitError::invalid_input(format!(
                "stash entry {stash_id} is {}, not commit",
                stash_object.kind
            )));
        }
        let stash_commit = crate::parse_commit(&stash_object.data)?;
        Ok(stash_commit.parents.get(2).copied())
    }

    fn stash_untracked_entries(
        &self,
        untracked_id: ObjectId,
        pathspecs: &PathspecSet,
    ) -> Result<Vec<IndexEntry>> {
        let attributes = self.root_attributes()?;
        Ok(self
            .commit_index_entries(untracked_id)?
            .into_iter()
            .filter(|entry| {
                entry.stage == 0
                    && pathspecs.matches_with_attributes(&entry.path, Some(&attributes))
            })
            .collect())
    }

    fn read_stash_blob(&self, object_id: ObjectId) -> Result<crate::GitObject> {
        let object = self.read_object(object_id)?;
        if object.kind != ObjectKind::Blob {
            return Err(RitError::invalid_input(format!(
                "object {object_id} is {}, not blob",
                object.kind
            )));
        }
        Ok(object)
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

    fn write_stash_export_ref(&self, ref_name: &str, target: ObjectId) -> Result<()> {
        validate_stash_export_ref_name(ref_name)?;
        let path = self.common_dir().join(ref_name);
        write_file_atomically(&path, |file| writeln!(file, "{target}"))?;
        self.refresh_indexdb_after_git_write();
        Ok(())
    }

    fn write_stash_export_chain(&self, items: &[StashExportItem]) -> Result<ObjectId> {
        let empty_tree_id = self.loose_objects().write_object(ObjectKind::Tree, &[])?;
        let mut previous_export_id = None;
        for item in items.iter().rev() {
            let first_parent = previous_export_id.unwrap_or(item.base_id);
            let message = format!("{STASH_EXPORT_PREFIX}{}", item.message);
            previous_export_id = Some(self.write_stash_commit(
                empty_tree_id,
                &[first_parent, item.stash_id],
                &message,
            )?);
        }
        previous_export_id.ok_or_else(|| RitError::invalid_input("No stash entries found."))
    }

    fn parse_stash_export_commit(&self, object_id: ObjectId) -> Result<crate::Commit> {
        let object = self.read_object(object_id)?;
        if object.kind != ObjectKind::Commit {
            return Err(RitError::invalid_input(format!(
                "stash export target {object_id} is {}, not commit",
                object.kind
            )));
        }
        let commit = parse_commit(&object.data)?;
        if !commit.message.starts_with(STASH_EXPORT_PREFIX) || commit.parents.len() < 2 {
            return Err(RitError::invalid_input(format!(
                "commit {object_id} is not a stash export"
            )));
        }
        Ok(commit)
    }

    fn is_stash_export_commit(&self, object_id: ObjectId) -> Result<bool> {
        let object = self.read_object(object_id)?;
        if object.kind != ObjectKind::Commit {
            return Ok(false);
        }
        let commit = parse_commit(&object.data)?;
        Ok(commit.message.starts_with(STASH_EXPORT_PREFIX) && commit.parents.len() >= 2)
    }

    fn ensure_stash_commit(&self, object_id: ObjectId) -> Result<()> {
        let object = self.read_object(object_id)?;
        if object.kind != ObjectKind::Commit {
            return Err(RitError::invalid_input(format!(
                "stash parent {object_id} is {}, not commit",
                object.kind
            )));
        }
        let commit = parse_commit(&object.data)?;
        if commit.parents.len() < 2 {
            return Err(RitError::invalid_input(format!(
                "stash parent {object_id} is not a stash commit"
            )));
        }
        Ok(())
    }

    fn stash_commit_subject(&self, object_id: ObjectId) -> Result<String> {
        let object = self.read_object(object_id)?;
        if object.kind != ObjectKind::Commit {
            return Err(RitError::invalid_input(format!(
                "stash parent {object_id} is {}, not commit",
                object.kind
            )));
        }
        let commit = parse_commit(&object.data)?;
        Ok(commit.message.lines().next().unwrap_or("").to_owned())
    }

    fn remove_stash_from_packed_refs(&self) -> Result<()> {
        let path = self.common_dir().join("packed-refs");
        let contents = match fs::read_to_string(&path) {
            Ok(contents) => contents,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(source) => return Err(RitError::io(&path, source)),
        };

        let mut changed = false;
        let mut skip_peeled_line = false;
        let mut kept_lines = Vec::new();
        for line in contents.lines() {
            if skip_peeled_line && line.starts_with('^') {
                changed = true;
                continue;
            }
            skip_peeled_line = false;

            let ref_name = line.split_whitespace().nth(1);
            if ref_name == Some("refs/stash") {
                changed = true;
                skip_peeled_line = true;
                continue;
            }
            kept_lines.push(line.to_owned());
        }

        if changed {
            write_file_atomically(&path, |file| {
                for line in &kept_lines {
                    writeln!(file, "{line}")?;
                }
                Ok(())
            })?;
        }
        Ok(())
    }

    fn create_tracked_stash_commit(
        &self,
        message: Option<&str>,
        pathspecs: &PathspecSet,
        change_mode: StashChangeMode,
        include_untracked: bool,
        include_ignored: bool,
    ) -> Result<Option<CreatedStashCommit>> {
        let head_id = self
            .resolve_head()?
            .ok_or_else(|| RitError::invalid_input("stash create requires an existing HEAD"))?;
        let target_paths = self.tracked_stash_target_paths(pathspecs, change_mode)?;
        let untracked_paths = if include_untracked {
            self.untracked_stash_paths(pathspecs, include_ignored)?
        } else {
            Vec::new()
        };
        if target_paths.is_empty() && untracked_paths.is_empty() {
            return Ok(None);
        }

        let index = Index::read(&self.git_dir().join("index"))?;
        ensure_stashable_index(&index)?;
        let index_snapshot = self.selected_index_stash_index(head_id, &index, &target_paths)?;
        let index_tree_id = self.write_tree_from_index(&index_snapshot)?;
        let stash_message = self.stash_push_message(head_id, message)?;
        let index_commit_id = self.write_stash_commit(
            index_tree_id,
            &[head_id],
            &index_commit_message(&stash_message),
        )?;
        let worktree_index = match change_mode {
            StashChangeMode::AllTracked => {
                self.worktree_stash_index(head_id, &index, &target_paths)?
            }
            StashChangeMode::StagedOnly => index_snapshot.clone(),
        };
        let worktree_tree_id = self.write_tree_from_index(&worktree_index)?;
        let mut parents = vec![head_id, index_commit_id];
        if !untracked_paths.is_empty() {
            let untracked_index = self.untracked_stash_index(&untracked_paths)?;
            let untracked_tree_id = self.write_tree_from_index(&untracked_index)?;
            let untracked_commit_id = self.write_stash_commit(
                untracked_tree_id,
                &[],
                &self.untracked_commit_message(head_id)?,
            )?;
            parents.push(untracked_commit_id);
        }
        let stash_id = self.write_stash_commit(worktree_tree_id, &parents, &stash_message)?;
        Ok(Some(CreatedStashCommit {
            object_id: stash_id,
            message: stash_message,
            paths: target_paths,
            untracked_paths,
        }))
    }

    fn tracked_stash_target_paths(
        &self,
        pathspecs: &PathspecSet,
        change_mode: StashChangeMode,
    ) -> Result<Vec<String>> {
        let status = self.status_porcelain_v1_with_pathspecs(pathspecs)?;
        Ok(status
            .entries
            .into_iter()
            .filter(|entry| match change_mode {
                StashChangeMode::AllTracked => {
                    entry.index_status != '?'
                        && (entry.index_status != ' ' || entry.worktree_status != ' ')
                }
                StashChangeMode::StagedOnly => {
                    entry.index_status != '?' && entry.index_status != ' '
                }
            })
            .map(|entry| entry.path)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect())
    }

    fn ensure_staged_stash_cleanup_supported(&self, pathspecs: &PathspecSet) -> Result<()> {
        let status = self.status_porcelain_v1_with_pathspecs(pathspecs)?;
        if status.entries.iter().any(|entry| {
            entry.index_status != '?' && entry.index_status != ' ' && entry.worktree_status != ' '
        }) {
            return Err(RitError::invalid_input("Cannot remove worktree changes"));
        }
        Ok(())
    }

    fn untracked_stash_paths(
        &self,
        pathspecs: &PathspecSet,
        include_ignored: bool,
    ) -> Result<Vec<String>> {
        let status = self.status_porcelain_v1_with_options(
            pathspecs,
            StatusOptions {
                untracked_files: UntrackedFilesMode::All,
                include_branch_header: false,
                include_ignored,
            },
        )?;
        Ok(status
            .entries
            .into_iter()
            .filter(|entry| {
                (entry.index_status == '?' && entry.worktree_status == '?')
                    || (include_ignored
                        && entry.index_status == '!'
                        && entry.worktree_status == '!')
            })
            .map(|entry| entry.path)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect())
    }

    fn untracked_stash_index(&self, untracked_paths: &[String]) -> Result<Index> {
        let Some(worktree) = self.worktree() else {
            return Err(RitError::invalid_input(
                "stash push must be run in a repository with a working tree",
            ));
        };
        let symlinks_enabled = self.core_symlinks_enabled()?;
        let mut entries = Vec::new();
        for path in untracked_paths {
            let full_path = join_slash_path(worktree, path);
            let metadata = fs::symlink_metadata(&full_path)
                .map_err(|source| RitError::io(&full_path, source))?;
            let is_symlink = metadata.file_type().is_symlink();
            let store_symlink = is_symlink && symlinks_enabled;
            let data = if store_symlink {
                read_symlink_target_bytes(&full_path)?
            } else {
                fs::read(&full_path).map_err(|source| RitError::io(&full_path, source))?
            };
            let mode = if store_symlink { 0o120000 } else { 0o100644 };
            let object_id = self.loose_objects().write_object(ObjectKind::Blob, &data)?;
            entries.push(IndexEntry {
                stat: IndexEntryStat::from_metadata(&metadata),
                mode,
                object_id,
                stage: 0,
                extended_flags: 0,
                file_size: data.len().min(u32::MAX as usize) as u32,
                path: path.clone(),
            });
        }
        Ok(Index {
            entries,
            extensions: Vec::new(),
        })
    }

    fn selected_index_stash_index(
        &self,
        head_id: ObjectId,
        index: &Index,
        target_paths: &[String],
    ) -> Result<Index> {
        let mut entries = index_entries_by_path(self.commit_index_entries(head_id)?);
        let current_entries = index_entries_by_path(
            index
                .entries
                .iter()
                .filter(|entry| entry.stage == 0)
                .cloned()
                .collect(),
        );
        for path in target_paths {
            match current_entries.get(path) {
                Some(entry) => {
                    entries.insert(path.clone(), entry.clone());
                }
                None => {
                    entries.remove(path);
                }
            }
        }
        Ok(Index {
            entries: entries.into_values().collect(),
            extensions: Vec::new(),
        })
    }

    fn worktree_stash_index(
        &self,
        head_id: ObjectId,
        index: &Index,
        target_paths: &[String],
    ) -> Result<Index> {
        let Some(worktree) = self.worktree() else {
            return Err(RitError::invalid_input(
                "stash push must be run in a repository with a working tree",
            ));
        };
        let symlinks_enabled = self.core_symlinks_enabled()?;
        let mut entries = index_entries_by_path(self.commit_index_entries(head_id)?);
        let current_entries = index_entries_by_path(
            index
                .entries
                .iter()
                .filter(|entry| entry.stage == 0)
                .cloned()
                .collect(),
        );
        for path in target_paths {
            let full_path = join_slash_path(worktree, path);
            let metadata = match fs::symlink_metadata(&full_path) {
                Ok(metadata) => metadata,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    entries.remove(path);
                    continue;
                }
                Err(source) => return Err(RitError::io(&full_path, source)),
            };
            let is_symlink = metadata.file_type().is_symlink();
            let store_symlink = is_symlink && symlinks_enabled;
            let data = if store_symlink {
                read_symlink_target_bytes(&full_path)?
            } else {
                fs::read(&full_path).map_err(|source| RitError::io(&full_path, source))?
            };
            let mode = if store_symlink {
                0o120000
            } else {
                current_entries
                    .get(path)
                    .or_else(|| entries.get(path))
                    .map(|entry| entry.mode)
                    .unwrap_or(0o100644)
            };
            let object_id = self.loose_objects().write_object(ObjectKind::Blob, &data)?;
            entries.insert(
                path.clone(),
                IndexEntry {
                    stat: IndexEntryStat::from_metadata(&metadata),
                    mode,
                    object_id,
                    stage: 0,
                    extended_flags: 0,
                    file_size: data.len().min(u32::MAX as usize) as u32,
                    path: path.clone(),
                },
            );
        }
        Ok(Index {
            entries: entries.into_values().collect(),
            extensions: Vec::new(),
        })
    }

    fn restore_stash_paths_to_head(
        &self,
        head_id: ObjectId,
        target_paths: &[String],
    ) -> Result<()> {
        let head_index = Index {
            entries: self.commit_index_entries(head_id)?,
            extensions: Vec::new(),
        };
        self.restore_stash_paths_to_index(&head_index, target_paths)
    }

    fn remove_stashed_untracked_paths(&self, untracked_paths: &[String]) -> Result<()> {
        let Some(worktree) = self.worktree() else {
            return Err(RitError::invalid_input(
                "stash push must be run in a repository with a working tree",
            ));
        };
        for path in untracked_paths {
            remove_file_if_exists(&join_slash_path(worktree, path))?;
        }
        Ok(())
    }

    fn restore_stash_paths_to_index(
        &self,
        source_index: &Index,
        target_paths: &[String],
    ) -> Result<()> {
        let Some(worktree) = self.worktree() else {
            return Err(RitError::invalid_input(
                "stash push must be run in a repository with a working tree",
            ));
        };
        let source_entries = index_entries_by_path(
            source_index
                .entries
                .iter()
                .filter(|entry| entry.stage == 0)
                .cloned()
                .collect(),
        );
        let index_path = self.git_dir().join("index");
        let index = Index::read(&index_path)?;
        let mut index_entries = index_entries_by_path(index.entries);
        let symlinks_enabled = self.core_symlinks_enabled()?;

        for path in target_paths {
            let full_path = join_slash_path(worktree, path);
            match source_entries.get(path) {
                Some(entry) => {
                    index_entries.insert(path.clone(), entry.clone());
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
                None => {
                    index_entries.remove(path);
                    remove_file_if_exists(&full_path)?;
                }
            }
        }
        Index {
            entries: index_entries.into_values().collect(),
            extensions: Vec::new(),
        }
        .write(&index_path)
    }

    fn replace_stash_paths_in_index(
        &self,
        source_index: &Index,
        target_paths: &[String],
    ) -> Result<()> {
        let source_entries = index_entries_by_path(
            source_index
                .entries
                .iter()
                .filter(|entry| entry.stage == 0)
                .cloned()
                .collect(),
        );
        let index_path = self.git_dir().join("index");
        let index = Index::read(&index_path)?;
        let mut index_entries = index_entries_by_path(index.entries);

        for path in target_paths {
            match source_entries.get(path) {
                Some(entry) => {
                    index_entries.insert(path.clone(), entry.clone());
                }
                None => {
                    index_entries.remove(path);
                }
            }
        }

        Index {
            entries: index_entries.into_values().collect(),
            extensions: Vec::new(),
        }
        .write(&index_path)
    }

    fn stash_push_message(&self, head_id: ObjectId, message: Option<&str>) -> Result<String> {
        let (branch, short_id, subject) = self.head_message_parts(head_id)?;
        if let Some(message) = message {
            return Ok(format!("On {branch}: {message}"));
        }

        Ok(format!("WIP on {branch}: {short_id} {subject}"))
    }

    fn untracked_commit_message(&self, head_id: ObjectId) -> Result<String> {
        let (branch, short_id, subject) = self.head_message_parts(head_id)?;
        Ok(format!("untracked files on {branch}: {short_id} {subject}"))
    }

    fn head_message_parts(&self, head_id: ObjectId) -> Result<(String, String, String)> {
        let branch = self
            .current_branch_name()?
            .unwrap_or_else(|| "(no branch)".to_owned());
        let object = self.read_object(head_id)?;
        if object.kind != ObjectKind::Commit {
            return Err(RitError::invalid_input(format!(
                "HEAD points to {}, not commit",
                object.kind
            )));
        }
        let commit = parse_commit(&object.data)?;
        let subject = commit.message.lines().next().unwrap_or("");
        Ok((branch, short_id(head_id), subject.to_owned()))
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

fn stash_reflog_entry_at_display_index(
    entries: &[StashReflogEntry],
    display_index: usize,
) -> Result<&StashReflogEntry> {
    if display_index >= entries.len() {
        return Err(RitError::invalid_input(format!(
            "log for 'stash' only has {} entries",
            entries.len()
        )));
    }
    Ok(&entries[entries.len() - 1 - display_index])
}

fn stash_reflog_message(entry: &StashReflogEntry) -> Result<String> {
    entry
        .rest
        .split_once('\t')
        .map(|(_, message)| message.to_owned())
        .ok_or_else(|| RitError::invalid_input("malformed stash reflog entry"))
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

fn validate_stash_export_ref_name(ref_name: &str) -> Result<()> {
    if !ref_name.starts_with("refs/")
        || ref_name.ends_with('/')
        || ref_name.contains('\\')
        || ref_name.contains("..")
        || ref_name.contains("//")
        || ref_name.ends_with(".lock")
        || ref_name
            .chars()
            .any(|character| character.is_control() || character.is_whitespace())
    {
        return Err(RitError::invalid_input(format!(
            "invalid ref name: {ref_name}"
        )));
    }
    Ok(())
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

fn count_text_lines(data: &[u8]) -> usize {
    if data.is_empty() {
        0
    } else {
        data.iter().filter(|byte| **byte == b'\n').count() + usize::from(!data.ends_with(b"\n"))
    }
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
