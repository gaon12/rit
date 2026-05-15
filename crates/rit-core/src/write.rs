use crate::index::{Index, IndexEntry, IndexEntryStat, join_slash_path, relative_slash_path};
use crate::merge_conflict::write_conflict_markers;
use crate::object::{hash_object, parse_tree_entries};
use crate::{
    GitAttributes, GitConfig, ObjectId, ObjectKind, PathspecSet, Repository, Result, RitError,
    Signature, parse_commit, refs::validate_ref_short_name,
};
use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

/// Options that affect how `add` records files in the index.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct AddOptions {
    /// Optional executable-bit override matching `git add --chmod=+x|-x`.
    pub mode_override: Option<FileModeOverride>,
}

/// Dry-run plan for `rit add`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AddPlan {
    /// Existing or new worktree files that would be written to the index.
    pub paths_to_add: Vec<String>,
    /// Tracked index paths that match the pathspec but no longer exist.
    pub paths_to_remove: Vec<String>,
    /// Optional executable-bit override that would be applied to regular files.
    pub mode_override: Option<FileModeOverride>,
}

impl AddPlan {
    /// Returns true when no existing, new, or removed paths matched the add request.
    pub fn is_empty(&self) -> bool {
        self.paths_to_add.is_empty() && self.paths_to_remove.is_empty()
    }
}

impl AddOptions {
    /// Builds default add options.
    pub fn new() -> Self {
        Self::default()
    }
}

/// Explicit file mode override for regular files added to the index.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FileModeOverride {
    /// Record the file as a normal non-executable blob.
    Regular,
    /// Record the file as an executable blob.
    Executable,
}

impl FileModeOverride {
    fn index_mode(self) -> u32 {
        match self {
            Self::Regular => 0o100644,
            Self::Executable => 0o100755,
        }
    }
}

/// Result of creating a commit.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommitResult {
    /// Newly written commit ID.
    pub commit_id: ObjectId,
    /// Number of files tracked by the committed index.
    pub file_count: usize,
}

/// Dry-run plan for `rit commit`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommitPlan {
    /// First line of the commit message that would be used for summaries.
    pub message_summary: String,
    /// Current `HEAD` commit that would become the parent, if any.
    pub parent_id: Option<ObjectId>,
    /// Number of files tracked by the index that would be committed.
    pub file_count: usize,
    /// Repository-relative paths whose staged tree entries differ from `HEAD`.
    pub paths_to_commit: Vec<String>,
    /// Whether commit hooks would run when applying the commit.
    pub verify: bool,
    /// Author override that would be applied to the commit, if any.
    pub author: Option<SignatureIdentity>,
    /// Author date override that would be applied to the commit, if any.
    pub author_date: Option<SignatureTime>,
}

impl CommitPlan {
    /// Returns true when the index tree has no changes compared with `HEAD`.
    pub fn is_empty(&self) -> bool {
        self.paths_to_commit.is_empty()
    }
}

/// Dry-run plan for path-based `rit reset`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResetPlan {
    /// Paths whose index entries would be restored from `HEAD`.
    pub paths_to_restore: Vec<String>,
    /// Paths whose index entries would be removed because they do not exist in `HEAD`.
    pub paths_to_remove: Vec<String>,
}

impl ResetPlan {
    /// Returns true when no index paths would be restored or removed.
    pub fn is_empty(&self) -> bool {
        self.paths_to_restore.is_empty() && self.paths_to_remove.is_empty()
    }
}

/// Result of a merge operation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MergeResult {
    /// HEAD already points at the requested commit.
    AlreadyUpToDate { commit_id: ObjectId },
    /// HEAD moved forward without creating a merge commit.
    FastForward { old_id: ObjectId, new_id: ObjectId },
    /// A non-fast-forward merge completed by creating a merge commit.
    MergeCommit {
        /// Previous `HEAD` commit.
        old_id: ObjectId,
        /// Target commit requested by the user.
        target_id: ObjectId,
        /// Newly written merge commit.
        commit_id: ObjectId,
    },
    /// A non-fast-forward merge started and left conflict stages in the index.
    Conflicts {
        /// Target commit requested by the user.
        target_id: ObjectId,
        /// Paths with unmerged stage entries in the index.
        conflict_paths: Vec<String>,
        /// Structured descriptions of each conflict for porcelain output.
        conflict_reports: Vec<MergeConflictReport>,
    },
}

/// One merge conflict that `rit merge` left for the user to resolve.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MergeConflictReport {
    /// Repository-relative path.
    pub path: String,
    /// High-level conflict type.
    pub kind: MergeConflictKind,
}

/// High-level conflict classification for user-facing merge output.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MergeConflictKind {
    /// Both sides changed file content or shape in a way this merge slice
    /// cannot automatically combine.
    Content,
    /// Both sides changed a binary file. Git leaves the `HEAD` version in the
    /// working tree and asks the user to resolve it manually.
    BinaryContent,
    /// Both sides independently added the same path.
    AddAdd,
    /// The same path has different file types on each side, such as a regular
    /// file on one side and a symbolic link on the other side.
    DistinctTypes,
    /// One side deleted a path while the other side modified it.
    ModifyDelete {
        /// Side that deleted the path.
        deleted_side: MergeConflictSide,
        /// Side that modified the path.
        modified_side: MergeConflictSide,
        /// Version left in the working tree for the user to inspect.
        worktree_side: MergeConflictSide,
    },
}

/// One side of a two-parent merge conflict.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MergeConflictSide {
    /// Current `HEAD`.
    Head,
    /// Merge target requested by the user.
    Target,
}

/// Dry-run plan for the currently supported fast-forward merge path.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MergePlan {
    /// HEAD already points at the requested commit.
    AlreadyUpToDate { commit_id: ObjectId },
    /// HEAD could move forward without creating a merge commit.
    FastForward {
        /// Current `HEAD` commit.
        old_id: ObjectId,
        /// Target commit that would become `HEAD`.
        new_id: ObjectId,
        /// Paths that would be written or refreshed in the index and worktree.
        paths_to_update: Vec<String>,
        /// Paths that would be removed from the index and worktree.
        paths_to_remove: Vec<String>,
    },
    /// The target is not a descendant of `HEAD`; a merge commit would be needed.
    NonFastForward {
        /// Current `HEAD` commit.
        head_id: ObjectId,
        /// Target commit requested by the user.
        target_id: ObjectId,
        /// First common ancestor found by the simple graph walk.
        merge_base: Option<ObjectId>,
        /// Paths changed on the current `HEAD` side since the merge base.
        head_changed_paths: Vec<String>,
        /// Paths changed on the target side since the merge base.
        target_changed_paths: Vec<String>,
        /// Paths changed differently on both sides and likely to need conflict
        /// stages when merge commits are implemented.
        conflict_paths: Vec<String>,
        /// Candidate stage entries for each conflicting path.
        conflict_stages: Vec<MergeConflictStagePlan>,
    },
}

/// Planned conflict stages for one path in a future non-fast-forward merge.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MergeConflictStagePlan {
    /// Repository-relative path.
    pub path: String,
    /// Stage 1, the merge-base version.
    pub base: Option<MergeConflictStageEntry>,
    /// Stage 2, the current `HEAD` version.
    pub head: Option<MergeConflictStageEntry>,
    /// Stage 3, the target version.
    pub target: Option<MergeConflictStageEntry>,
}

/// One planned conflict-stage entry.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MergeConflictStageEntry {
    /// Git file mode for this stage.
    pub mode: u32,
    /// Blob object ID for this stage.
    pub object_id: ObjectId,
}

struct ConflictedMergeStart<'a> {
    target: &'a str,
    target_id: ObjectId,
    head_id: ObjectId,
    base_entries: &'a [IndexEntry],
    head_entries: &'a [IndexEntry],
    target_entries: &'a [IndexEntry],
    conflict_stages: &'a [MergeConflictStagePlan],
}

/// Options that affect commit metadata without changing the committed tree.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommitOptions {
    /// Author identity supplied by `git commit --author=<author>`.
    pub author: Option<SignatureIdentity>,
    /// Author timestamp supplied by `git commit --date=<date>`.
    pub author_date: Option<SignatureTime>,
    /// Whether `pre-commit` and `commit-msg` hooks should run.
    pub verify: bool,
}

impl CommitOptions {
    /// Builds default commit options with hook verification enabled.
    pub fn new() -> Self {
        Self::default()
    }
}

impl Default for CommitOptions {
    fn default() -> Self {
        Self {
            author: None,
            author_date: None,
            verify: true,
        }
    }
}

/// Name and e-mail pair used in a commit signature.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SignatureIdentity {
    /// Human-readable name.
    pub name: String,
    /// E-mail address without surrounding angle brackets.
    pub email: String,
}

impl SignatureIdentity {
    /// Parses Git's common `Name <email>` author override form.
    pub fn parse_author(input: &str) -> Result<Self> {
        let trimmed = input.trim();
        let Some(email_end) = trimmed.rfind('>') else {
            return Err(RitError::invalid_input(
                "author must be formatted as Name <email>",
            ));
        };
        if email_end != trimmed.len() - 1 {
            return Err(RitError::invalid_input(
                "author must be formatted as Name <email>",
            ));
        }
        let Some(email_start) = trimmed[..email_end].rfind('<') else {
            return Err(RitError::invalid_input(
                "author must be formatted as Name <email>",
            ));
        };
        let name = trimmed[..email_start].trim();
        let email = trimmed[email_start + 1..email_end].trim();
        if name.is_empty() || email.is_empty() {
            return Err(RitError::invalid_input(
                "author must include a name and email",
            ));
        }
        Ok(Self {
            name: name.to_owned(),
            email: email.to_owned(),
        })
    }
}

/// Timestamp and numeric timezone used in a commit signature.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SignatureTime {
    /// Seconds since the Unix epoch.
    pub timestamp: i64,
    /// Numeric timezone offset such as `+0000` or `-0730`.
    pub offset: String,
}

impl SignatureTime {
    /// Parses the raw Git date form `<unix-seconds> <+/-HHMM>`.
    pub fn parse_git_raw(input: &str) -> Result<Self> {
        let mut parts = input.split_whitespace();
        let Some(timestamp_text) = parts.next() else {
            return Err(RitError::invalid_input(
                "date must be formatted as '<unix-seconds> <+/-HHMM>'",
            ));
        };
        let timestamp = timestamp_text.parse::<i64>().map_err(|_| {
            RitError::invalid_input("date timestamp must be a Unix timestamp in seconds")
        })?;
        let offset = parts.next().unwrap_or("+0000");
        if parts.next().is_some() || !is_valid_timezone_offset(offset) {
            return Err(RitError::invalid_input(
                "date must be formatted as '<unix-seconds> <+/-HHMM>'",
            ));
        }
        Ok(Self {
            timestamp,
            offset: offset.to_owned(),
        })
    }
}

impl Repository {
    /// Adds files matching ordinary literal pathspecs to the index.
    pub fn add_paths(&self, paths: &[String]) -> Result<usize> {
        self.add_paths_with_options(paths, &AddOptions::default())
    }

    /// Builds a dry-run add plan without writing objects or the index.
    pub fn plan_add_paths_with_options(
        &self,
        paths: &[String],
        options: &AddOptions,
    ) -> Result<AddPlan> {
        let selection = self.add_path_selection(paths)?;
        Ok(AddPlan {
            paths_to_add: selection.files_to_add.into_iter().collect(),
            paths_to_remove: selection.paths_to_remove.into_iter().collect(),
            mode_override: options.mode_override,
        })
    }

    /// Adds files matching ordinary pathspecs to the index with explicit options.
    pub fn add_paths_with_options(&self, paths: &[String], options: &AddOptions) -> Result<usize> {
        let selection = self.add_path_selection(paths)?;
        let Some(worktree) = self.worktree() else {
            return Err(RitError::invalid_input(
                "add must be run in a repository with a working tree",
            ));
        };

        let index_path = self.git_dir().join("index");
        let mut index = Index::read(&index_path)?;
        let mut entries = index
            .entries
            .into_iter()
            .map(|entry| (entry.path.clone(), entry))
            .collect::<BTreeMap<_, _>>();
        let symlinks_enabled = self.core_symlinks_enabled()?;

        for relative_path in selection.files_to_add {
            let full_path = join_slash_path(worktree, &relative_path);
            let metadata = fs::symlink_metadata(&full_path)
                .map_err(|source| RitError::io(&full_path, source))?;
            let is_symlink = metadata.file_type().is_symlink();
            let should_store_symlink = is_symlink && symlinks_enabled;
            let data = if is_symlink {
                read_symlink_target_bytes(&full_path)?
            } else {
                fs::read(&full_path).map_err(|source| RitError::io(&full_path, source))?
            };
            let object_id = self.loose_objects().write_object(ObjectKind::Blob, &data)?;
            let mode = if should_store_symlink {
                0o120000
            } else {
                options
                    .mode_override
                    .map(FileModeOverride::index_mode)
                    .or_else(|| entries.get(&relative_path).map(|entry| entry.mode))
                    .unwrap_or_else(|| {
                        if is_symlink {
                            0o100644
                        } else {
                            file_mode_from_metadata(&metadata)
                        }
                    })
            };
            entries.insert(
                relative_path.clone(),
                IndexEntry {
                    stat: IndexEntryStat::from_metadata(&metadata),
                    mode,
                    object_id,
                    stage: 0,
                    extended_flags: 0,
                    file_size: data.len().min(u32::MAX as usize) as u32,
                    path: relative_path,
                },
            );
        }

        for path in selection.paths_to_remove {
            entries.remove(&path);
        }

        index = Index {
            entries: entries.into_values().collect(),
            extensions: Vec::new(),
        };
        let count = index.entries.len();
        index.write(&index_path)?;
        Ok(count)
    }

    fn add_path_selection(&self, paths: &[String]) -> Result<AddPathSelection> {
        let Some(worktree) = self.worktree() else {
            return Err(RitError::invalid_input(
                "add must be run in a repository with a working tree",
            ));
        };
        if paths.is_empty() {
            return Err(RitError::invalid_input("add requires at least one path"));
        }
        let pathspecs = PathspecSet::from_args(paths)?;
        let index = Index::read(&self.git_dir().join("index"))?;
        let entries = index
            .entries
            .into_iter()
            .map(|entry| (entry.path.clone(), entry))
            .collect::<BTreeMap<_, _>>();
        let ignore_case = self.core_ignorecase_enabled()?;
        let attributes = self.root_attributes()?;
        let files_to_add = expand_add_pathspecs(
            worktree,
            &pathspecs,
            entries.keys(),
            ignore_case,
            &attributes,
        )?;
        let paths_to_remove = entries
            .keys()
            .filter(|path| {
                pathspecs.matches_with_attributes(path, Some(&attributes))
                    && !join_slash_path(worktree, path).exists()
            })
            .cloned()
            .collect();
        Ok(AddPathSelection {
            files_to_add,
            paths_to_remove,
        })
    }

    /// Creates a commit from the current index and advances `HEAD`.
    pub fn commit_index(&self, message: &str) -> Result<CommitResult> {
        self.commit_index_with_options(message, &CommitOptions::default())
    }

    /// Builds a dry-run commit plan without writing tree or commit objects.
    pub fn plan_commit_index_with_options(
        &self,
        message: &str,
        options: &CommitOptions,
    ) -> Result<CommitPlan> {
        let index = Index::read(&self.git_dir().join("index"))?;
        if index.entries.is_empty() {
            return Err(RitError::invalid_input("nothing to commit"));
        }
        let parent_id = self.resolve_head()?;
        let parent_entries = match parent_id {
            Some(commit_id) => self.commit_index_entries(commit_id)?,
            None => Vec::new(),
        };
        let paths_to_commit = staged_paths_different_from_head(&index.entries, &parent_entries);
        if paths_to_commit.is_empty() {
            return Err(RitError::invalid_input("nothing to commit"));
        }
        Ok(CommitPlan {
            message_summary: message.lines().next().unwrap_or("").to_owned(),
            parent_id,
            file_count: index.entries.len(),
            paths_to_commit,
            verify: options.verify,
            author: options.author.clone(),
            author_date: options.author_date.clone(),
        })
    }

    /// Creates a commit from the current index with explicit metadata options.
    pub fn commit_index_with_options(
        &self,
        message: &str,
        options: &CommitOptions,
    ) -> Result<CommitResult> {
        let parents = self.resolve_head()?.into_iter().collect::<Vec<_>>();
        self.commit_index_with_parents(message, options, &parents, false)
    }

    fn commit_index_with_parents(
        &self,
        message: &str,
        options: &CommitOptions,
        parents: &[ObjectId],
        allow_same_tree: bool,
    ) -> Result<CommitResult> {
        let index = Index::read(&self.git_dir().join("index"))?;
        if index.entries.is_empty() {
            return Err(RitError::invalid_input("nothing to commit"));
        }
        let tree_id = self.write_tree_from_index(&index)?;
        if !allow_same_tree && parents.len() == 1 {
            let parent_id = parents[0];
            let parent_object = self.read_object(parent_id)?;
            let parent_commit = parse_commit(&parent_object.data)?;
            if parent_commit.tree == tree_id {
                return Err(RitError::invalid_input("nothing to commit"));
            }
        }

        let mut commit_message = message.trim_end_matches('\n').to_owned();
        run_commit_hooks(self, options, &mut commit_message)?;
        let signatures = read_commit_signatures(self, options)?;
        let mut commit = Vec::new();
        commit.extend_from_slice(format!("tree {tree_id}\n").as_bytes());
        for parent_id in parents {
            commit.extend_from_slice(format!("parent {parent_id}\n").as_bytes());
        }
        commit.extend_from_slice(
            format!("author {}\n", format_signature(&signatures.author)).as_bytes(),
        );
        commit.extend_from_slice(
            format!("committer {}\n\n", format_signature(&signatures.committer)).as_bytes(),
        );
        commit.extend_from_slice(commit_message.trim_end_matches('\n').as_bytes());
        commit.push(b'\n');

        let commit_id = self
            .loose_objects()
            .write_object(ObjectKind::Commit, &commit)?;
        self.update_head(commit_id)?;
        run_post_commit_hook(self);
        self.refresh_indexdb_after_git_write();
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
        let symlinks_enabled = self.core_symlinks_enabled()?;
        let attributes = self.root_attributes()?;
        let entries = index
            .entries
            .iter()
            .filter(|entry| pathspecs.matches_with_attributes(&entry.path, Some(&attributes)))
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
            write_worktree_entry_atomically(
                &join_slash_path(worktree, &entry.path),
                &object.data,
                entry.mode,
                symlinks_enabled,
            )?;
        }

        Ok(())
    }

    /// Builds a dry-run path reset plan without writing the index.
    pub fn plan_restore_staged_paths_from_head(&self, paths: &[String]) -> Result<ResetPlan> {
        let selection = self.reset_path_selection_from_head(paths)?;
        let mut paths_to_restore = Vec::new();
        let mut paths_to_remove = Vec::new();
        for path in selection.target_paths {
            if selection.head_entries.contains_key(&path) {
                paths_to_restore.push(path);
            } else {
                paths_to_remove.push(path);
            }
        }
        Ok(ResetPlan {
            paths_to_restore,
            paths_to_remove,
        })
    }

    /// Restores index entries from `HEAD`, returning paths still modified in the worktree.
    pub fn restore_staged_paths_from_head(&self, paths: &[String]) -> Result<Vec<String>> {
        let Some(worktree) = self.worktree() else {
            return Err(RitError::invalid_input(
                "reset must be run in a repository with a working tree",
            ));
        };
        let selection = self.reset_path_selection_from_head(paths)?;
        let index_path = self.git_dir().join("index");
        let index = Index::read(&index_path)?;
        let mut index_entries = index
            .entries
            .into_iter()
            .map(|entry| (entry.path.clone(), entry))
            .collect::<BTreeMap<_, _>>();
        let mut unstaged = Vec::new();

        for normalized in selection.target_paths {
            match selection.head_entries.get(&normalized) {
                Some(head_entry) => {
                    index_entries.insert(
                        normalized.clone(),
                        IndexEntry {
                            stat: head_entry.stat,
                            mode: head_entry.mode,
                            object_id: head_entry.object_id,
                            stage: 0,
                            extended_flags: 0,
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
            extensions: Vec::new(),
        }
        .write(&index_path)?;
        Ok(unstaged)
    }

    fn reset_path_selection_from_head(&self, paths: &[String]) -> Result<ResetPathSelection> {
        if self.worktree().is_none() {
            return Err(RitError::invalid_input(
                "reset must be run in a repository with a working tree",
            ));
        }
        if paths.is_empty() {
            return Err(RitError::invalid_input("reset requires at least one path"));
        }
        let pathspecs = PathspecSet::from_args(paths)?;
        let attributes = self.root_attributes()?;
        let index = Index::read(&self.git_dir().join("index"))?;
        let index_entries = index
            .entries
            .into_iter()
            .map(|entry| (entry.path.clone(), entry))
            .collect::<BTreeMap<_, _>>();
        let head_entries = self.head_blob_entries()?;
        let target_paths = index_entries
            .keys()
            .chain(head_entries.keys())
            .filter(|path| pathspecs.matches_with_attributes(path, Some(&attributes)))
            .cloned()
            .collect::<BTreeSet<_>>();
        if target_paths.is_empty() {
            return Err(RitError::invalid_input(format!(
                "pathspec did not match any file known to git: {}",
                paths.join(" ")
            )));
        }
        Ok(ResetPathSelection {
            head_entries,
            target_paths,
        })
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
        self.refresh_indexdb_after_git_write();
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
        self.refresh_indexdb_after_git_write();
        Ok(target)
    }

    /// Checks out a commit and leaves `HEAD` detached at that commit.
    pub fn checkout_detached(&self, revision: &str) -> Result<ObjectId> {
        ensure_clean_for_checkout(self)?;
        let target = self.resolve_revision(revision)?;
        let object = self.read_object(target)?;
        if object.kind != ObjectKind::Commit {
            return Err(RitError::invalid_input(format!(
                "object {target} is {}, not commit",
                object.kind
            )));
        }
        self.checkout_commit_tree(target)?;
        write_text_atomically(&self.git_dir().join("HEAD"), &format!("{target}\n"))?;
        self.refresh_indexdb_after_git_write();
        Ok(target)
    }

    /// Fast-forwards the current branch to `target` when possible.
    pub fn plan_merge_ff_only(&self, target: &str) -> Result<MergePlan> {
        let old_id = self
            .resolve_head()?
            .ok_or_else(|| RitError::invalid_input("merge requires an existing HEAD"))?;
        let new_id = self.resolve_merge_target(target)?;
        if old_id == new_id {
            return Ok(MergePlan::AlreadyUpToDate { commit_id: old_id });
        }
        if !self.commit_is_ancestor(old_id, new_id)? {
            let merge_base = self.find_merge_base(old_id, new_id)?;
            let old_entries = self.commit_index_entries(old_id)?;
            let new_entries = self.commit_index_entries(new_id)?;
            let base_entries = match merge_base {
                Some(base_id) => self.commit_index_entries(base_id)?,
                None => Vec::new(),
            };
            let merge_workflow =
                non_fast_forward_merge_workflow_plan(&base_entries, &old_entries, &new_entries);
            return Ok(MergePlan::NonFastForward {
                head_id: old_id,
                target_id: new_id,
                merge_base,
                head_changed_paths: merge_workflow.head_changed_paths,
                target_changed_paths: merge_workflow.target_changed_paths,
                conflict_paths: merge_workflow.conflict_paths,
                conflict_stages: merge_workflow.conflict_stages,
            });
        }
        let old_entries = self.commit_index_entries(old_id)?;
        let new_entries = self.commit_index_entries(new_id)?;
        let (paths_to_update, paths_to_remove) = tree_update_plan(&old_entries, &new_entries);
        Ok(MergePlan::FastForward {
            old_id,
            new_id,
            paths_to_update,
            paths_to_remove,
        })
    }

    /// Merges `target` into the current branch for the currently supported cases.
    pub fn merge(&self, target: &str) -> Result<MergeResult> {
        ensure_clean_for_checkout(self)?;
        let old_id = self
            .resolve_head()?
            .ok_or_else(|| RitError::invalid_input("merge requires an existing HEAD"))?;
        let new_id = self.resolve_merge_target(target)?;
        if old_id == new_id {
            return Ok(MergeResult::AlreadyUpToDate { commit_id: old_id });
        }
        if !self.commit_is_ancestor(old_id, new_id)? {
            let merge_base = self.find_merge_base(old_id, new_id)?;
            let head_entries = self.commit_index_entries(old_id)?;
            let target_entries = self.commit_index_entries(new_id)?;
            let base_entries = match merge_base {
                Some(base_id) => self.commit_index_entries(base_id)?,
                None => Vec::new(),
            };
            let merge_workflow =
                non_fast_forward_merge_workflow_plan(&base_entries, &head_entries, &target_entries);
            if merge_workflow.conflict_stages.is_empty() {
                let commit_id = self.create_clean_merge_commit(
                    target,
                    old_id,
                    new_id,
                    &base_entries,
                    &head_entries,
                    &target_entries,
                )?;
                return Ok(MergeResult::MergeCommit {
                    old_id,
                    target_id: new_id,
                    commit_id,
                });
            }
            self.start_conflicted_merge(ConflictedMergeStart {
                target,
                target_id: new_id,
                head_id: old_id,
                base_entries: &base_entries,
                head_entries: &head_entries,
                target_entries: &target_entries,
                conflict_stages: &merge_workflow.conflict_stages,
            })?;
            self.refresh_indexdb_after_git_write();
            return Ok(MergeResult::Conflicts {
                target_id: new_id,
                conflict_paths: merge_workflow.conflict_paths,
                conflict_reports: self.merge_conflict_reports(&merge_workflow.conflict_stages)?,
            });
        }

        self.checkout_commit_tree(new_id)?;
        self.update_head(new_id)?;
        self.refresh_indexdb_after_git_write();
        Ok(MergeResult::FastForward { old_id, new_id })
    }

    fn create_clean_merge_commit(
        &self,
        target: &str,
        old_id: ObjectId,
        target_id: ObjectId,
        base_entries: &[IndexEntry],
        head_entries: &[IndexEntry],
        target_entries: &[IndexEntry],
    ) -> Result<ObjectId> {
        let Some(worktree) = self.worktree() else {
            return Err(RitError::invalid_input(
                "merge must be run in a repository with a working tree",
            ));
        };
        let symlinks_enabled = self.core_symlinks_enabled()?;
        let merged_entries = self.non_fast_forward_merge_index_entries(
            base_entries,
            head_entries,
            target_entries,
            &[],
            target,
        )?;
        let conflict_paths = BTreeSet::new();
        write_non_conflicting_merge_worktree_changes(
            self,
            worktree,
            head_entries,
            &merged_entries,
            &conflict_paths,
            symlinks_enabled,
        )?;
        write_text_atomically(&self.git_dir().join("ORIG_HEAD"), &format!("{old_id}\n"))?;
        Index {
            entries: merged_entries,
            extensions: Vec::new(),
        }
        .write(&self.git_dir().join("index"))?;
        let message = format!("Merge commit '{target}'");
        let result = self.commit_index_with_parents(
            &message,
            &CommitOptions::default(),
            &[old_id, target_id],
            true,
        )?;
        Ok(result.commit_id)
    }

    /// Fast-forwards the current branch to `target` when possible.
    pub fn merge_ff_only(&self, target: &str) -> Result<MergeResult> {
        ensure_clean_for_checkout(self)?;
        let old_id = self
            .resolve_head()?
            .ok_or_else(|| RitError::invalid_input("merge requires an existing HEAD"))?;
        let new_id = self.resolve_merge_target(target)?;
        if old_id == new_id {
            return Ok(MergeResult::AlreadyUpToDate { commit_id: old_id });
        }
        if !self.commit_is_ancestor(old_id, new_id)? {
            return Err(RitError::invalid_input(
                "not possible to fast-forward; merge requires a merge commit",
            ));
        }

        self.checkout_commit_tree(new_id)?;
        self.update_head(new_id)?;
        self.refresh_indexdb_after_git_write();
        Ok(MergeResult::FastForward { old_id, new_id })
    }

    fn start_conflicted_merge(&self, input: ConflictedMergeStart<'_>) -> Result<()> {
        let Some(worktree) = self.worktree() else {
            return Err(RitError::invalid_input(
                "merge must be run in a repository with a working tree",
            ));
        };
        let symlinks_enabled = self.core_symlinks_enabled()?;
        let merged_entries = self.non_fast_forward_merge_index_entries(
            input.base_entries,
            input.head_entries,
            input.target_entries,
            input.conflict_stages,
            input.target,
        )?;
        let conflict_paths = input
            .conflict_stages
            .iter()
            .map(|stage| stage.path.as_str())
            .collect::<BTreeSet<_>>();
        write_non_conflicting_merge_worktree_changes(
            self,
            worktree,
            input.head_entries,
            &merged_entries,
            &conflict_paths,
            symlinks_enabled,
        )?;
        write_conflict_markers(
            self,
            worktree,
            input.conflict_stages,
            input.target,
            symlinks_enabled,
        )?;

        write_text_atomically(
            &self.git_dir().join("ORIG_HEAD"),
            &format!("{}\n", input.head_id),
        )?;
        Index {
            entries: merged_entries,
            extensions: Vec::new(),
        }
        .write(&self.git_dir().join("index"))?;
        write_text_atomically(
            &self.git_dir().join("MERGE_HEAD"),
            &format!("{}\n", input.target_id),
        )?;
        write_text_atomically(
            &self.git_dir().join("MERGE_MSG"),
            &merge_message_with_conflicts(input.target, &conflict_paths),
        )
    }

    /// Aborts an in-progress merge and restores the tree saved in `ORIG_HEAD`.
    pub fn abort_merge(&self) -> Result<ObjectId> {
        let merge_head_path = self.git_dir().join("MERGE_HEAD");
        if !merge_head_path.exists() {
            return Err(RitError::invalid_input("no merge to abort"));
        }
        let original_head_path = self.git_dir().join("ORIG_HEAD");
        let original_head_text = fs::read_to_string(&original_head_path)
            .map_err(|source| RitError::io(&original_head_path, source))?;
        let original_head = ObjectId::from_hex(original_head_text.trim())?;

        self.checkout_commit_tree(original_head)?;
        self.update_head(original_head)?;
        remove_file_if_exists(&merge_head_path)?;
        remove_file_if_exists(&self.git_dir().join("MERGE_MSG"))?;
        remove_file_if_exists(&self.git_dir().join("MERGE_MODE"))?;
        self.refresh_indexdb_after_git_write();
        Ok(original_head)
    }

    /// Forgets an in-progress merge while leaving the index and worktree alone.
    pub fn quit_merge(&self) -> Result<()> {
        remove_file_if_exists(&self.git_dir().join("MERGE_HEAD"))?;
        remove_file_if_exists(&self.git_dir().join("MERGE_MSG"))?;
        remove_file_if_exists(&self.git_dir().join("MERGE_MODE"))
    }

    /// Commits a resolved in-progress merge.
    pub fn continue_merge(&self, options: &CommitOptions) -> Result<CommitResult> {
        let state = self.merge_state()?;
        if state.merge_heads.is_empty() {
            return Err(RitError::invalid_input("no merge to continue"));
        }
        let head_id = self
            .resolve_head()?
            .ok_or_else(|| RitError::invalid_input("merge continue requires an existing HEAD"))?;
        let Some(message) = state.merge_message.as_deref() else {
            return Err(RitError::invalid_input("merge message is missing"));
        };
        let mut parents = vec![head_id];
        parents.extend(state.merge_heads);
        let result = self.commit_index_with_parents(message, options, &parents, true)?;
        remove_file_if_exists(&self.git_dir().join("MERGE_HEAD"))?;
        remove_file_if_exists(&self.git_dir().join("MERGE_MSG"))?;
        remove_file_if_exists(&self.git_dir().join("MERGE_MODE"))?;
        self.refresh_indexdb_after_git_write();
        Ok(result)
    }

    fn non_fast_forward_merge_index_entries(
        &self,
        base_entries: &[IndexEntry],
        head_entries: &[IndexEntry],
        target_entries: &[IndexEntry],
        conflict_stages: &[MergeConflictStagePlan],
        target_label: &str,
    ) -> Result<Vec<IndexEntry>> {
        let base_by_path = index_entries_by_path(base_entries);
        let head_by_path = index_entries_by_path(head_entries);
        let target_by_path = index_entries_by_path(target_entries);
        let conflict_paths = conflict_stages
            .iter()
            .map(|stage| stage.path.as_str())
            .collect::<BTreeSet<_>>();
        let all_paths = base_by_path
            .keys()
            .chain(head_by_path.keys())
            .chain(target_by_path.keys())
            .copied()
            .collect::<BTreeSet<_>>();
        let mut entries = Vec::new();

        for path in all_paths {
            if conflict_paths.contains(path) {
                continue;
            }
            let base_entry = base_by_path.get(path).copied();
            let head_entry = head_by_path.get(path).copied();
            let target_entry = target_by_path.get(path).copied();
            let selected = if let Some(entry) =
                clean_mode_content_merge_entry(base_entry, head_entry, target_entry)
            {
                Some(entry)
            } else if tree_entries_equal(head_entry, target_entry) {
                head_entry.cloned()
            } else if tree_entries_equal(base_entry, head_entry) {
                target_entry.cloned()
            } else {
                head_entry.cloned()
            };
            if let Some(entry) = selected {
                let mut entry = entry;
                entry.stage = 0;
                entries.push(entry);
            }
        }

        for conflict in conflict_stages {
            if self.write_distinct_type_conflict_index_entries(
                &mut entries,
                conflict,
                target_label,
            )? {
                continue;
            }
            if let Some(base) = conflict.base {
                entries.push(self.conflict_index_entry(&conflict.path, 1, base)?);
            }
            if let Some(head) = conflict.head {
                entries.push(self.conflict_index_entry(&conflict.path, 2, head)?);
            }
            if let Some(target) = conflict.target {
                entries.push(self.conflict_index_entry(&conflict.path, 3, target)?);
            }
        }

        Ok(entries)
    }

    fn write_distinct_type_conflict_index_entries(
        &self,
        entries: &mut Vec<IndexEntry>,
        conflict: &MergeConflictStagePlan,
        target_label: &str,
    ) -> Result<bool> {
        let (Some(head), Some(target)) = (conflict.head, conflict.target) else {
            return Ok(false);
        };
        if !entry_modes_have_distinct_file_types(head.mode, target.mode) {
            return Ok(false);
        }

        if mode_is_regular_file(head.mode) && !mode_is_regular_file(target.mode) {
            if let Some(base) = conflict.base {
                entries.push(self.conflict_index_entry(
                    &head_side_conflict_path(&conflict.path),
                    1,
                    base,
                )?);
            }
            entries.push(self.conflict_index_entry(
                &head_side_conflict_path(&conflict.path),
                2,
                head,
            )?);
            entries.push(self.conflict_index_entry(&conflict.path, 3, target)?);
            return Ok(true);
        }

        if !mode_is_regular_file(head.mode) && mode_is_regular_file(target.mode) {
            let target_path = target_side_conflict_path(&conflict.path, target_label);
            entries.push(self.conflict_index_entry(&conflict.path, 2, head)?);
            if let Some(base) = conflict.base {
                entries.push(self.conflict_index_entry(&target_path, 1, base)?);
            }
            entries.push(self.conflict_index_entry(&target_path, 3, target)?);
            return Ok(true);
        }

        Ok(false)
    }

    fn conflict_index_entry(
        &self,
        path: &str,
        stage: u8,
        entry: MergeConflictStageEntry,
    ) -> Result<IndexEntry> {
        let object = self.read_object(entry.object_id)?;
        Ok(IndexEntry {
            stat: IndexEntryStat::default(),
            mode: entry.mode,
            object_id: entry.object_id,
            stage,
            extended_flags: 0,
            file_size: object.size().min(u32::MAX as usize) as u32,
            path: path.to_owned(),
        })
    }

    fn merge_conflict_reports(
        &self,
        conflict_stages: &[MergeConflictStagePlan],
    ) -> Result<Vec<MergeConflictReport>> {
        conflict_stages
            .iter()
            .map(|stage| {
                Ok(MergeConflictReport {
                    path: stage.path.clone(),
                    kind: self.merge_conflict_kind(stage)?,
                })
            })
            .collect()
    }

    fn merge_conflict_kind(&self, stage: &MergeConflictStagePlan) -> Result<MergeConflictKind> {
        match (stage.base, stage.head, stage.target) {
            (Some(_), None, Some(_)) => Ok(MergeConflictKind::ModifyDelete {
                deleted_side: MergeConflictSide::Head,
                modified_side: MergeConflictSide::Target,
                worktree_side: MergeConflictSide::Target,
            }),
            (Some(_), Some(_), None) => Ok(MergeConflictKind::ModifyDelete {
                deleted_side: MergeConflictSide::Target,
                modified_side: MergeConflictSide::Head,
                worktree_side: MergeConflictSide::Head,
            }),
            (_, Some(head), Some(target)) => {
                if entry_modes_have_distinct_file_types(head.mode, target.mode) {
                    Ok(MergeConflictKind::DistinctTypes)
                } else if self.conflict_stage_is_binary(head)?
                    || self.conflict_stage_is_binary(target)?
                {
                    Ok(MergeConflictKind::BinaryContent)
                } else if stage.base.is_none() {
                    Ok(MergeConflictKind::AddAdd)
                } else {
                    Ok(MergeConflictKind::Content)
                }
            }
            _ => Ok(MergeConflictKind::Content),
        }
    }

    fn conflict_stage_is_binary(&self, entry: MergeConflictStageEntry) -> Result<bool> {
        if !matches!(entry.mode, 0o100644 | 0o100755) {
            return Ok(false);
        }
        let object = self.read_object(entry.object_id)?;
        if object.kind != ObjectKind::Blob {
            return Err(RitError::invalid_input(format!(
                "object {} is {}, not blob",
                entry.object_id, object.kind
            )));
        }
        Ok(object.data.contains(&0) || std::str::from_utf8(&object.data).is_err())
    }

    fn resolve_merge_target(&self, target: &str) -> Result<ObjectId> {
        match self.branch_target(target) {
            Ok(object_id) => Ok(object_id),
            Err(_) => self.resolve_revision(target),
        }
    }

    fn commit_is_ancestor(&self, ancestor: ObjectId, descendant: ObjectId) -> Result<bool> {
        let mut stack = vec![descendant];
        let mut seen = HashSet::new();
        while let Some(commit_id) = stack.pop() {
            if !seen.insert(commit_id) {
                continue;
            }
            if commit_id == ancestor {
                return Ok(true);
            }
            let object = self.read_object(commit_id)?;
            if object.kind != ObjectKind::Commit {
                continue;
            }
            let commit = parse_commit(&object.data)?;
            stack.extend(commit.parents);
        }
        Ok(false)
    }

    fn find_merge_base(&self, left: ObjectId, right: ObjectId) -> Result<Option<ObjectId>> {
        let left_ancestors = self.commit_ancestor_set(left)?;
        let mut stack = vec![right];
        let mut seen = HashSet::new();
        while let Some(commit_id) = stack.pop() {
            if !seen.insert(commit_id) {
                continue;
            }
            if left_ancestors.contains(&commit_id) {
                return Ok(Some(commit_id));
            }
            let object = self.read_object(commit_id)?;
            if object.kind != ObjectKind::Commit {
                continue;
            }
            let commit = parse_commit(&object.data)?;
            stack.extend(commit.parents);
        }
        Ok(None)
    }

    fn commit_ancestor_set(&self, start: ObjectId) -> Result<HashSet<ObjectId>> {
        let mut ancestors = HashSet::new();
        let mut stack = vec![start];
        while let Some(commit_id) = stack.pop() {
            if !ancestors.insert(commit_id) {
                continue;
            }
            let object = self.read_object(commit_id)?;
            if object.kind != ObjectKind::Commit {
                continue;
            }
            let commit = parse_commit(&object.data)?;
            stack.extend(commit.parents);
        }
        Ok(ancestors)
    }

    fn write_tree_from_index(&self, index: &Index) -> Result<ObjectId> {
        let mut root = TreeNode::default();
        for entry in &index.entries {
            if entry.stage != 0 {
                return Err(RitError::invalid_input(format!(
                    "cannot write tree with unmerged index entry: {}",
                    entry.path
                )));
            }
            root.insert(&entry.path, entry.object_id, entry.mode)?;
        }
        self.write_tree_node(root)
    }

    fn write_tree_node(&self, node: TreeNode) -> Result<ObjectId> {
        let mut data = Vec::new();
        for (name, entry) in node.entries {
            match entry {
                TreeNodeEntry::Blob { object_id, mode } => {
                    data.extend_from_slice(format!("{mode:o} ").as_bytes());
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
                let mode = parse_index_mode(&entry.mode)?;
                output.insert(
                    path,
                    HeadBlobEntry {
                        stat: IndexEntryStat::default(),
                        mode,
                        object_id: entry.object_id,
                        file_size: object.size().min(u32::MAX as usize) as u32,
                    },
                );
            }
        }
        Ok(())
    }

    pub(crate) fn checkout_commit_tree(&self, commit_id: ObjectId) -> Result<()> {
        let Some(worktree) = self.worktree() else {
            return Err(RitError::invalid_input(
                "checkout must be run in a repository with a working tree",
            ));
        };
        let symlinks_enabled = self.core_symlinks_enabled()?;
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
            write_worktree_entry_atomically(
                &worktree_path,
                &object.data,
                entry.mode,
                symlinks_enabled,
            )?;
        }

        Index {
            entries: target_entries,
            extensions: Vec::new(),
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
                stat: entry.stat,
                mode: entry.mode,
                object_id: entry.object_id,
                stage: 0,
                extended_flags: 0,
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
    ignore_case: bool,
    attributes: &GitAttributes,
) -> Result<BTreeSet<String>> {
    let mut files = BTreeSet::new();

    let positive_patterns = pathspecs
        .patterns()
        .iter()
        .filter(|pattern| !pattern.is_exclude())
        .collect::<Vec<_>>();

    if pathspecs.is_all() || positive_patterns.is_empty() {
        collect_regular_files(worktree, worktree, &mut files)?;
        files.retain(|path| pathspecs.matches_with_attributes(path, Some(attributes)));
        return Ok(files);
    }

    let indexed_paths = indexed_paths.cloned().collect::<Vec<_>>();
    let mut worktree_files = None;
    for pattern in positive_patterns {
        if pattern.has_wildcard() || pattern.ignore_case() {
            if worktree_files.is_none() {
                let mut files = BTreeSet::new();
                collect_regular_files(worktree, worktree, &mut files)?;
                worktree_files = Some(files);
            }
            let all_worktree_files = worktree_files
                .as_ref()
                .expect("wildcard expansion should collect worktree files");
            let mut matched = false;
            for path in all_worktree_files {
                if pattern.matches_with_attributes(path, Some(attributes))
                    && pathspecs.matches_with_attributes(path, Some(attributes))
                {
                    files.insert(path.clone());
                    matched = true;
                }
            }
            if indexed_paths.iter().any(|path| {
                pattern.matches_with_attributes(path, Some(attributes))
                    && pathspecs.matches_with_attributes(path, Some(attributes))
            }) {
                matched = true;
            }
            if !matched {
                return Err(RitError::invalid_input(format!(
                    "pathspec did not match any files: {}",
                    pattern.pattern()
                )));
            }
        } else {
            let exact_case_exists = worktree_path_matches_exact_case(worktree, pattern.pattern());
            if !exact_case_exists
                && ignore_case
                && (indexed_paths
                    .iter()
                    .any(|path| path.eq_ignore_ascii_case(pattern.pattern()))
                    || worktree_path_exists_ignore_case(worktree, pattern.pattern()))
            {
                continue;
            }
            let full_path = join_slash_path(worktree, pattern.pattern());
            if exact_case_exists && full_path.is_file() {
                files.insert(relative_slash_path(worktree, &full_path)?);
            } else if exact_case_exists && full_path.is_dir() {
                collect_regular_files(worktree, &full_path, &mut files)?;
            } else if indexed_paths.iter().any(|path| {
                pattern.matches_with_attributes(path, Some(attributes))
                    && pathspecs.matches_with_attributes(path, Some(attributes))
            }) {
                continue;
            } else {
                return Err(RitError::invalid_input(format!(
                    "pathspec did not match any files: {}",
                    pattern.pattern()
                )));
            }
        }
    }

    files.retain(|path| pathspecs.matches_with_attributes(path, Some(attributes)));

    Ok(files)
}

struct AddPathSelection {
    files_to_add: BTreeSet<String>,
    paths_to_remove: BTreeSet<String>,
}

struct ResetPathSelection {
    head_entries: BTreeMap<String, HeadBlobEntry>,
    target_paths: BTreeSet<String>,
}

fn staged_paths_different_from_head(
    index_entries: &[IndexEntry],
    parent_entries: &[IndexEntry],
) -> Vec<String> {
    let index_by_path = index_entries
        .iter()
        .map(|entry| (entry.path.as_str(), entry))
        .collect::<BTreeMap<_, _>>();
    let parent_by_path = parent_entries
        .iter()
        .map(|entry| (entry.path.as_str(), entry))
        .collect::<BTreeMap<_, _>>();
    let paths = index_by_path
        .keys()
        .chain(parent_by_path.keys())
        .copied()
        .collect::<BTreeSet<_>>();
    paths
        .into_iter()
        .filter(
            |path| match (index_by_path.get(path), parent_by_path.get(path)) {
                (Some(index_entry), Some(parent_entry)) => {
                    index_entry.mode != parent_entry.mode
                        || index_entry.object_id != parent_entry.object_id
                }
                (Some(_), None) | (None, Some(_)) => true,
                (None, None) => false,
            },
        )
        .map(str::to_owned)
        .collect()
}

fn tree_update_plan(
    old_entries: &[IndexEntry],
    new_entries: &[IndexEntry],
) -> (Vec<String>, Vec<String>) {
    let old_by_path = old_entries
        .iter()
        .map(|entry| (entry.path.as_str(), entry))
        .collect::<BTreeMap<_, _>>();
    let new_by_path = new_entries
        .iter()
        .map(|entry| (entry.path.as_str(), entry))
        .collect::<BTreeMap<_, _>>();
    let paths = old_by_path
        .keys()
        .chain(new_by_path.keys())
        .copied()
        .collect::<BTreeSet<_>>();
    let mut paths_to_update = Vec::new();
    let mut paths_to_remove = Vec::new();
    for path in paths {
        match (old_by_path.get(path), new_by_path.get(path)) {
            (Some(old_entry), Some(new_entry)) => {
                if old_entry.mode != new_entry.mode || old_entry.object_id != new_entry.object_id {
                    paths_to_update.push(path.to_owned());
                }
            }
            (None, Some(_)) => paths_to_update.push(path.to_owned()),
            (Some(_), None) => paths_to_remove.push(path.to_owned()),
            (None, None) => {}
        }
    }
    (paths_to_update, paths_to_remove)
}

struct NonFastForwardMergeWorkflowPlan {
    head_changed_paths: Vec<String>,
    target_changed_paths: Vec<String>,
    conflict_paths: Vec<String>,
    conflict_stages: Vec<MergeConflictStagePlan>,
}

fn non_fast_forward_merge_workflow_plan(
    base_entries: &[IndexEntry],
    head_entries: &[IndexEntry],
    target_entries: &[IndexEntry],
) -> NonFastForwardMergeWorkflowPlan {
    let base_by_path = index_entries_by_path(base_entries);
    let head_by_path = index_entries_by_path(head_entries);
    let target_by_path = index_entries_by_path(target_entries);
    let paths = base_by_path
        .keys()
        .chain(head_by_path.keys())
        .chain(target_by_path.keys())
        .copied()
        .collect::<BTreeSet<_>>();
    let mut head_changed_paths = Vec::new();
    let mut target_changed_paths = Vec::new();
    let mut conflict_paths = Vec::new();
    let mut conflict_stages = Vec::new();
    for path in paths {
        let base_entry = base_by_path.get(path).copied();
        let head_entry = head_by_path.get(path).copied();
        let target_entry = target_by_path.get(path).copied();
        let head_changed = tree_entry_changed(base_entry, head_entry);
        let target_changed = tree_entry_changed(base_entry, target_entry);
        if head_changed {
            head_changed_paths.push(path.to_owned());
        }
        if target_changed {
            target_changed_paths.push(path.to_owned());
        }
        if head_changed
            && target_changed
            && !tree_entries_equal(head_entry, target_entry)
            && clean_mode_content_merge_entry(base_entry, head_entry, target_entry).is_none()
        {
            conflict_paths.push(path.to_owned());
            conflict_stages.push(MergeConflictStagePlan {
                path: path.to_owned(),
                base: conflict_stage_entry(base_entry),
                head: conflict_stage_entry(head_entry),
                target: conflict_stage_entry(target_entry),
            });
        }
    }
    NonFastForwardMergeWorkflowPlan {
        head_changed_paths,
        target_changed_paths,
        conflict_paths,
        conflict_stages,
    }
}

fn index_entries_by_path(entries: &[IndexEntry]) -> BTreeMap<&str, &IndexEntry> {
    entries
        .iter()
        .map(|entry| (entry.path.as_str(), entry))
        .collect()
}

fn tree_entry_changed(base: Option<&IndexEntry>, other: Option<&IndexEntry>) -> bool {
    !tree_entries_equal(base, other)
}

fn tree_entries_equal(left: Option<&IndexEntry>, right: Option<&IndexEntry>) -> bool {
    match (left, right) {
        (Some(left), Some(right)) => left.mode == right.mode && left.object_id == right.object_id,
        (None, None) => true,
        (Some(_), None) | (None, Some(_)) => false,
    }
}

fn entry_modes_have_distinct_file_types(left_mode: u32, right_mode: u32) -> bool {
    file_type_from_mode(left_mode) != file_type_from_mode(right_mode)
}

fn file_type_from_mode(mode: u32) -> FileTypeFromMode {
    if mode_is_regular_file(mode) {
        FileTypeFromMode::Regular
    } else if mode == 0o120000 {
        FileTypeFromMode::Symlink
    } else {
        FileTypeFromMode::Other
    }
}

fn mode_is_regular_file(mode: u32) -> bool {
    matches!(mode, 0o100644 | 0o100755)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FileTypeFromMode {
    Regular,
    Symlink,
    Other,
}

fn head_side_conflict_path(path: &str) -> String {
    format!("{path}~HEAD")
}

fn target_side_conflict_path(path: &str, target_label: &str) -> String {
    format!("{path}~{}", conflict_path_label(target_label))
}

fn conflict_path_label(label: &str) -> String {
    label
        .chars()
        .map(|character| match character {
            '/' | '\\' => '_',
            _ => character,
        })
        .collect()
}

fn clean_mode_content_merge_entry(
    base: Option<&IndexEntry>,
    head: Option<&IndexEntry>,
    target: Option<&IndexEntry>,
) -> Option<IndexEntry> {
    let (base, head, target) = (base?, head?, target?);
    if entry_changed_mode_only(base, head) && entry_changed_content_only(base, target) {
        return Some(entry_with_mode(target, head.mode));
    }
    if entry_changed_content_only(base, head) && entry_changed_mode_only(base, target) {
        return Some(entry_with_mode(head, target.mode));
    }
    None
}

fn entry_changed_mode_only(base: &IndexEntry, other: &IndexEntry) -> bool {
    base.object_id == other.object_id && base.mode != other.mode
}

fn entry_changed_content_only(base: &IndexEntry, other: &IndexEntry) -> bool {
    base.object_id != other.object_id && base.mode == other.mode
}

fn entry_with_mode(entry: &IndexEntry, mode: u32) -> IndexEntry {
    let mut entry = entry.clone();
    entry.mode = mode;
    entry.stage = 0;
    entry
}

fn conflict_stage_entry(entry: Option<&IndexEntry>) -> Option<MergeConflictStageEntry> {
    entry.map(|entry| MergeConflictStageEntry {
        mode: entry.mode,
        object_id: entry.object_id,
    })
}

fn write_non_conflicting_merge_worktree_changes(
    repository: &Repository,
    worktree: &Path,
    head_entries: &[IndexEntry],
    merged_entries: &[IndexEntry],
    conflict_paths: &BTreeSet<&str>,
    symlinks_enabled: bool,
) -> Result<()> {
    let head_by_path = index_entries_by_path(head_entries);
    let merged_by_path = merged_entries
        .iter()
        .filter(|entry| entry.stage == 0)
        .map(|entry| (entry.path.as_str(), entry))
        .collect::<BTreeMap<_, _>>();
    let paths = head_by_path
        .keys()
        .chain(merged_by_path.keys())
        .copied()
        .collect::<BTreeSet<_>>();

    for path in paths {
        if conflict_paths.contains(path) {
            continue;
        }
        match (
            head_by_path.get(path).copied(),
            merged_by_path.get(path).copied(),
        ) {
            (Some(head), Some(merged)) if tree_entries_equal(Some(head), Some(merged)) => {}
            (_, Some(merged)) => {
                let object = repository.read_object(merged.object_id)?;
                if object.kind != ObjectKind::Blob {
                    return Err(RitError::invalid_input(format!(
                        "object {} is {}, not blob",
                        merged.object_id, object.kind
                    )));
                }
                write_worktree_entry_atomically(
                    &join_slash_path(worktree, &merged.path),
                    &object.data,
                    merged.mode,
                    symlinks_enabled,
                )?;
            }
            (Some(head), None) => {
                let path = join_slash_path(worktree, &head.path);
                if path.exists() || fs::symlink_metadata(&path).is_ok() {
                    fs::remove_file(&path).map_err(|source| RitError::io(&path, source))?;
                }
            }
            (None, None) => {}
        }
    }

    Ok(())
}

fn merge_message_with_conflicts(target: &str, conflict_paths: &BTreeSet<&str>) -> String {
    let mut message = format!("Merge commit '{target}'\n\nConflicts:\n");
    for path in conflict_paths {
        message.push('\t');
        message.push_str(path);
        message.push('\n');
    }
    message
}

fn remove_file_if_exists(path: &Path) -> Result<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(source) => Err(RitError::io(path, source)),
    }
}

fn worktree_path_matches_exact_case(root: &Path, slash_path: &str) -> bool {
    find_worktree_path_case_insensitive(root, slash_path, true).is_some()
}

fn worktree_path_exists_ignore_case(root: &Path, slash_path: &str) -> bool {
    find_worktree_path_case_insensitive(root, slash_path, false).is_some()
}

fn find_worktree_path_case_insensitive(
    root: &Path,
    slash_path: &str,
    require_exact_case: bool,
) -> Option<PathBuf> {
    let mut current = root.to_path_buf();
    for component in slash_path.split('/').filter(|part| !part.is_empty()) {
        let mut matched = None;
        for entry in fs::read_dir(&current).ok()? {
            let entry = entry.ok()?;
            let name = entry.file_name();
            let name = name.to_string_lossy();
            let is_match = if require_exact_case {
                name == component
            } else {
                name.eq_ignore_ascii_case(component)
            };
            if is_match {
                matched = Some(entry.path());
                break;
            }
        }
        current = matched?;
    }
    Some(current)
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
        } else if file_type.is_file() || file_type.is_symlink() {
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
    stat: IndexEntryStat,
    mode: u32,
    object_id: ObjectId,
    file_size: u32,
}

#[derive(Default)]
struct TreeNode {
    entries: BTreeMap<String, TreeNodeEntry>,
}

enum TreeNodeEntry {
    Blob { object_id: ObjectId, mode: u32 },
    Tree(TreeNode),
}

impl TreeNode {
    fn insert(&mut self, path: &str, object_id: ObjectId, mode: u32) -> Result<()> {
        let mut parts = path.split('/').collect::<Vec<_>>();
        if parts.is_empty() {
            return Err(RitError::invalid_input("empty index path"));
        }
        self.insert_parts(&mut parts, object_id, mode)
    }

    fn insert_parts(
        &mut self,
        parts: &mut Vec<&str>,
        object_id: ObjectId,
        mode: u32,
    ) -> Result<()> {
        let name = parts.remove(0);
        if parts.is_empty() {
            self.entries
                .insert(name.to_owned(), TreeNodeEntry::Blob { object_id, mode });
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
        child.insert_parts(parts, object_id, mode)
    }
}

fn parse_index_mode(mode: &str) -> Result<u32> {
    u32::from_str_radix(mode, 8)
        .map_err(|_| RitError::invalid_input(format!("invalid tree mode: {mode}")))
}

#[cfg(unix)]
fn file_mode_from_metadata(metadata: &fs::Metadata) -> u32 {
    use std::os::unix::fs::PermissionsExt;
    if metadata.permissions().mode() & 0o111 != 0 {
        0o100755
    } else {
        0o100644
    }
}

#[cfg(not(unix))]
fn file_mode_from_metadata(_metadata: &fs::Metadata) -> u32 {
    0o100644
}

fn run_commit_hooks(
    repository: &Repository,
    options: &CommitOptions,
    message: &mut String,
) -> Result<()> {
    let message_path = repository.git_dir().join("COMMIT_EDITMSG");
    write_text_atomically(
        &message_path,
        &format!("{}\n", message.trim_end_matches('\n')),
    )?;
    if !options.verify {
        run_hook(
            repository,
            "prepare-commit-msg",
            &[message_path.clone(), PathBuf::from("message")],
        )?;
        *message = fs::read_to_string(&message_path)
            .map_err(|source| RitError::io(&message_path, source))?;
        return Ok(());
    }

    run_hook(repository, "pre-commit", &[])?;
    run_hook(
        repository,
        "prepare-commit-msg",
        &[message_path.clone(), PathBuf::from("message")],
    )?;
    run_hook(
        repository,
        "commit-msg",
        std::slice::from_ref(&message_path),
    )?;
    *message =
        fs::read_to_string(&message_path).map_err(|source| RitError::io(&message_path, source))?;
    Ok(())
}

fn run_hook(repository: &Repository, name: &str, args: &[PathBuf]) -> Result<()> {
    let hook_path = repository.common_dir().join("hooks").join(name);
    if !hook_should_run(&hook_path)? {
        return Ok(());
    }

    let mut command = hook_command(&hook_path)?;
    let child_args = args
        .iter()
        .map(|argument| child_path(argument))
        .collect::<Vec<_>>();
    let current_dir = child_path(repository.worktree().unwrap_or(repository.common_dir()));
    command
        .args(&child_args)
        .current_dir(current_dir)
        .env("GIT_DIR", child_path(repository.git_dir()))
        .env(
            "GIT_INDEX_FILE",
            child_path(&repository.git_dir().join("index")),
        );
    let output = command
        .output()
        .map_err(|source| RitError::io(&hook_path, source))?;
    if output.status.success() {
        return Ok(());
    }

    let detail = String::from_utf8_lossy(if output.stderr.is_empty() {
        &output.stdout
    } else {
        &output.stderr
    })
    .trim()
    .to_owned();
    let status = output
        .status
        .code()
        .map_or_else(|| "signal".to_owned(), |code| code.to_string());
    if detail.is_empty() {
        Err(RitError::invalid_input(format!(
            "hook '{name}' failed with status {status}"
        )))
    } else {
        Err(RitError::invalid_input(format!(
            "hook '{name}' failed with status {status}: {detail}"
        )))
    }
}

fn run_post_commit_hook(repository: &Repository) {
    let hook_path = repository.common_dir().join("hooks").join("post-commit");
    if !matches!(hook_should_run(&hook_path), Ok(true)) {
        return;
    }
    let Ok(mut command) = hook_command(&hook_path) else {
        return;
    };
    let current_dir = child_path(repository.worktree().unwrap_or(repository.common_dir()));
    let _ = command
        .current_dir(current_dir)
        .env("GIT_DIR", child_path(repository.git_dir()))
        .env(
            "GIT_INDEX_FILE",
            child_path(&repository.git_dir().join("index")),
        )
        .output();
}

fn hook_should_run(path: &Path) -> Result<bool> {
    let metadata = match fs::metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(RitError::io(path, error)),
    };
    if !metadata.is_file() {
        return Ok(false);
    }
    Ok(hook_is_executable(&metadata))
}

#[cfg(unix)]
fn hook_is_executable(metadata: &fs::Metadata) -> bool {
    use std::os::unix::fs::PermissionsExt;
    metadata.permissions().mode() & 0o111 != 0
}

#[cfg(not(unix))]
fn hook_is_executable(_metadata: &fs::Metadata) -> bool {
    true
}

fn hook_command(path: &Path) -> Result<Command> {
    #[cfg(windows)]
    {
        let shell = windows_shell_path().ok_or_else(|| {
            RitError::invalid_input(
                "cannot run hook script because no Git-compatible sh.exe was found",
            )
        })?;
        let mut command = Command::new(shell);
        command.arg(child_path(path));
        Ok(command)
    }

    #[cfg(not(windows))]
    {
        Ok(Command::new(path))
    }
}

fn child_path(path: &Path) -> PathBuf {
    #[cfg(windows)]
    {
        let text = path.as_os_str().to_string_lossy();
        if let Some(stripped) = text.strip_prefix(r"\\?\") {
            return PathBuf::from(stripped);
        }
    }
    path.to_path_buf()
}

#[cfg(windows)]
fn windows_shell_path() -> Option<PathBuf> {
    std::env::var_os("SHELL")
        .map(PathBuf::from)
        .filter(|path| path.exists() && is_sh_like_shell(path))
        .or_else(|| {
            [
                r"C:\Program Files\Git\bin\sh.exe",
                r"C:\Program Files\Git\usr\bin\sh.exe",
                r"C:\Program Files (x86)\Git\bin\sh.exe",
                r"C:\Program Files (x86)\Git\usr\bin\sh.exe",
            ]
            .into_iter()
            .map(PathBuf::from)
            .find(|path| path.exists())
        })
}

#[cfg(windows)]
fn is_sh_like_shell(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(|name| {
            let lower = name.to_ascii_lowercase();
            lower == "sh.exe" || lower == "bash.exe" || lower == "sh" || lower == "bash"
        })
        .unwrap_or(false)
}

struct CommitSignatures {
    author: Signature,
    committer: Signature,
}

fn read_commit_signatures(
    repository: &Repository,
    options: &CommitOptions,
) -> Result<CommitSignatures> {
    let config_path = repository.common_dir().join("config");
    let default_time = current_signature_time()?;
    let author_identity = match &options.author {
        Some(identity) => identity.clone(),
        None => read_identity(
            &config_path,
            "GIT_AUTHOR_NAME",
            "GIT_AUTHOR_EMAIL",
            "author",
        )?,
    };
    let committer_identity = read_identity(
        &config_path,
        "GIT_COMMITTER_NAME",
        "GIT_COMMITTER_EMAIL",
        "committer",
    )?;
    let author_time = match &options.author_date {
        Some(time) => time.clone(),
        None => read_env_signature_time("GIT_AUTHOR_DATE")?.unwrap_or_else(|| default_time.clone()),
    };
    let committer_time = read_env_signature_time("GIT_COMMITTER_DATE")?.unwrap_or(default_time);

    Ok(CommitSignatures {
        author: Signature {
            name: author_identity.name,
            email: author_identity.email,
            timestamp: author_time.timestamp,
            offset: author_time.offset,
        },
        committer: Signature {
            name: committer_identity.name,
            email: committer_identity.email,
            timestamp: committer_time.timestamp,
            offset: committer_time.offset,
        },
    })
}

fn read_identity(
    config_path: &Path,
    name_env: &str,
    email_env: &str,
    role: &str,
) -> Result<SignatureIdentity> {
    let name = std::env::var(name_env)
        .ok()
        .or_else(|| read_config_value(config_path, "user", "name"));
    let email = std::env::var(email_env)
        .ok()
        .or_else(|| read_config_value(config_path, "user", "email"));

    Ok(SignatureIdentity {
        name: name.ok_or_else(|| {
            RitError::invalid_input(format!(
                "{role} identity unknown; set user.name or {name_env}"
            ))
        })?,
        email: email.ok_or_else(|| {
            RitError::invalid_input(format!(
                "{role} identity unknown; set user.email or {email_env}"
            ))
        })?,
    })
}

fn read_env_signature_time(name: &str) -> Result<Option<SignatureTime>> {
    match std::env::var(name) {
        Ok(value) => SignatureTime::parse_git_raw(&value).map(Some),
        Err(_) => Ok(None),
    }
}

fn current_signature_time() -> Result<SignatureTime> {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| RitError::invalid_input("system time is before Unix epoch"))?
        .as_secs() as i64;

    Ok(SignatureTime {
        timestamp,
        offset: "+0000".to_owned(),
    })
}

fn is_valid_timezone_offset(offset: &str) -> bool {
    if offset.len() != 5 {
        return false;
    }
    let sign = &offset[..1];
    if sign != "+" && sign != "-" {
        return false;
    }
    let Ok(hours) = offset[1..3].parse::<u8>() else {
        return false;
    };
    let Ok(minutes) = offset[3..5].parse::<u8>() else {
        return false;
    };
    hours <= 23 && minutes <= 59
}

fn read_config_value(path: &Path, section: &str, key: &str) -> Option<String> {
    GitConfig::read(path)
        .ok()
        .and_then(|config| config.get(section, key).map(ToOwned::to_owned))
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

pub(crate) fn write_worktree_entry_atomically(
    path: &Path,
    contents: &[u8],
    mode: u32,
    symlinks_enabled: bool,
) -> Result<()> {
    if mode == 0o120000 && symlinks_enabled {
        return write_worktree_symlink_atomically(path, contents);
    }
    let file_mode = if mode == 0o120000 { 0o100644 } else { mode };
    write_worktree_file_atomically(path, contents, file_mode)
}

fn write_worktree_file_atomically(path: &Path, contents: &[u8], mode: u32) -> Result<()> {
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
    if path.exists() || fs::symlink_metadata(path).is_ok() {
        fs::remove_file(path).map_err(|source| RitError::io(path, source))?;
    }
    fs::rename(&temp_path, path).map_err(|source| RitError::io(path, source))?;
    set_worktree_file_mode(path, mode)?;
    Ok(())
}

#[cfg(unix)]
fn set_worktree_file_mode(path: &Path, mode: u32) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let permission_mode = match mode {
        0o100755 => 0o755,
        _ => 0o644,
    };
    let permissions = fs::Permissions::from_mode(permission_mode);
    fs::set_permissions(path, permissions).map_err(|source| RitError::io(path, source))
}

#[cfg(not(unix))]
fn set_worktree_file_mode(_path: &Path, _mode: u32) -> Result<()> {
    Ok(())
}

fn read_symlink_target_bytes(path: &Path) -> Result<Vec<u8>> {
    let target = fs::read_link(path).map_err(|source| RitError::io(path, source))?;
    Ok(target.to_string_lossy().replace('\\', "/").into_bytes())
}

#[cfg(unix)]
fn write_worktree_symlink_atomically(path: &Path, target: &[u8]) -> Result<()> {
    use std::os::unix::fs::symlink;

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| RitError::io(parent, source))?;
    }
    let target = String::from_utf8_lossy(target);
    let temp_path = path.with_extension(format!("rit-tmp-{}", std::process::id()));
    if temp_path.exists() {
        fs::remove_file(&temp_path).map_err(|source| RitError::io(&temp_path, source))?;
    }
    symlink(target.as_ref(), &temp_path).map_err(|source| RitError::io(&temp_path, source))?;
    if path.exists() || fs::symlink_metadata(path).is_ok() {
        fs::remove_file(path).map_err(|source| RitError::io(path, source))?;
    }
    fs::rename(&temp_path, path).map_err(|source| RitError::io(path, source))
}

#[cfg(not(unix))]
fn write_worktree_symlink_atomically(path: &Path, target: &[u8]) -> Result<()> {
    write_worktree_file_atomically(path, target, 0o100644)
}

#[cfg(test)]
mod tests {
    use super::{
        AddOptions, FileModeOverride, MergeConflictKind, MergeConflictReport, MergeConflictSide,
        SignatureIdentity, SignatureTime, parse_tree_entries, read_config_value,
    };
    use crate::{
        Index, IndexEntry, InitOptions, ObjectKind, Repository, index::IndexEntryStat, parse_commit,
    };
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
    fn parses_author_override() {
        assert_eq!(
            SignatureIdentity::parse_author("A U Thor <a@example.test>")
                .expect("author should parse"),
            SignatureIdentity {
                name: "A U Thor".to_owned(),
                email: "a@example.test".to_owned(),
            }
        );
    }

    #[test]
    fn rejects_author_override_without_email() {
        let error =
            SignatureIdentity::parse_author("A U Thor").expect_err("author should be rejected");
        assert_eq!(
            error.to_string(),
            "author must be formatted as Name <email>"
        );
    }

    #[test]
    fn parses_raw_git_signature_time() {
        assert_eq!(
            SignatureTime::parse_git_raw("1700000000 +0900").expect("date should parse"),
            SignatureTime {
                timestamp: 1_700_000_000,
                offset: "+0900".to_owned(),
            }
        );
    }

    #[test]
    fn raw_git_signature_time_defaults_to_utc() {
        assert_eq!(
            SignatureTime::parse_git_raw("1700000000").expect("date should parse"),
            SignatureTime {
                timestamp: 1_700_000_000,
                offset: "+0000".to_owned(),
            }
        );
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
    fn add_plan_reports_paths_without_writing_index() {
        let temp = temp_path("add-plan");
        let repository = Repository::init(&InitOptions::new(&temp)).expect("init should work");
        fs::create_dir_all(temp.join("nested")).expect("nested directory should be written");
        fs::write(temp.join("nested").join("a.txt"), "one\n").expect("file should be written");

        let plan = repository
            .plan_add_paths_with_options(
                &["nested".to_owned()],
                &AddOptions {
                    mode_override: Some(FileModeOverride::Executable),
                },
            )
            .expect("add plan should work");

        assert_eq!(plan.paths_to_add, vec!["nested/a.txt"]);
        assert_eq!(plan.paths_to_remove, Vec::<String>::new());
        assert_eq!(plan.mode_override, Some(FileModeOverride::Executable));
        let index = Index::read(&repository.git_dir().join("index")).expect("index should read");
        assert!(index.entries.is_empty());
        remove_dir_all(&temp);
    }

    #[test]
    fn commit_plan_reports_staged_paths_without_writing_objects() {
        let temp = temp_path("commit-plan");
        let repository = Repository::init(&InitOptions::new(&temp)).expect("init should work");
        fs::write(temp.join("tracked.txt"), "one\n").expect("file should be written");
        repository
            .add_paths(&["tracked.txt".to_owned()])
            .expect("add should work");
        let object_count_before = count_object_files(repository.common_dir().join("objects"));

        let plan = repository
            .plan_commit_index_with_options("initial\nbody", &super::CommitOptions::default())
            .expect("commit plan should work");

        assert_eq!(plan.message_summary, "initial");
        assert_eq!(plan.parent_id, None);
        assert_eq!(plan.file_count, 1);
        assert_eq!(plan.paths_to_commit, vec!["tracked.txt"]);
        assert!(plan.verify);
        assert_eq!(
            count_object_files(repository.common_dir().join("objects")),
            object_count_before
        );
        assert_eq!(
            repository.resolve_head().expect("HEAD should resolve"),
            None
        );
        remove_dir_all(&temp);
    }

    #[test]
    fn add_chmod_executable_records_index_mode_and_tree_mode() {
        let temp = temp_path("add-chmod-executable");
        let repository = Repository::init(&InitOptions::new(&temp)).expect("init should work");
        fs::write(temp.join("script.sh"), "#!/bin/sh\n").expect("file should be written");
        fs::write(
            repository.git_dir().join("config"),
            "[core]\n\trepositoryformatversion = 0\n\tbare = false\n[user]\n\tname = Rit Test\n\temail = rit@example.test\n",
        )
        .expect("config should be written");

        repository
            .add_paths_with_options(
                &["script.sh".to_owned()],
                &AddOptions {
                    mode_override: Some(FileModeOverride::Executable),
                },
            )
            .expect("file should be added with executable mode");

        let index = Index::read(&repository.git_dir().join("index")).expect("index should read");
        assert_eq!(index.entries[0].mode, 0o100755);

        let commit_id = repository
            .commit_index("add executable")
            .expect("commit should be created")
            .commit_id;
        let commit = repository
            .read_object(commit_id)
            .expect("commit object should read");
        let parsed_commit = parse_commit(&commit.data).expect("commit should parse");
        let tree = repository
            .read_object(parsed_commit.tree)
            .expect("tree object should read");
        assert_eq!(tree.kind, ObjectKind::Tree);
        let entries = parse_tree_entries(&tree.data).expect("tree should parse");
        assert_eq!(entries[0].mode, "100755");
        remove_dir_all(&temp);
    }

    #[cfg(unix)]
    #[test]
    fn restore_worktree_paths_materializes_executable_mode() {
        let temp = temp_path("restore-executable-mode");
        let repository = Repository::init(&InitOptions::new(&temp)).expect("init should work");
        let script_path = temp.join("script.sh");
        fs::write(&script_path, "#!/bin/sh\n").expect("file should be written");
        fs::write(
            repository.git_dir().join("config"),
            "[core]\n\trepositoryformatversion = 0\n\tbare = false\n[user]\n\tname = Rit Test\n\temail = rit@example.test\n",
        )
        .expect("config should be written");
        repository
            .add_paths_with_options(
                &["script.sh".to_owned()],
                &AddOptions {
                    mode_override: Some(FileModeOverride::Executable),
                },
            )
            .expect("file should be added with executable mode");
        repository
            .commit_index("add executable")
            .expect("commit should be created");

        set_test_permissions(&script_path, 0o644);
        repository
            .restore_worktree_paths(&["script.sh".to_owned()])
            .expect("restore should write executable mode");

        assert!(is_test_executable(&script_path));
        remove_dir_all(&temp);
    }

    #[cfg(unix)]
    #[test]
    fn add_and_restore_symlink_entries() {
        use std::os::unix::fs::symlink;

        let temp = temp_path("add-symlink");
        let repository = Repository::init(&InitOptions::new(&temp)).expect("init should work");
        fs::write(temp.join("target.txt"), "target\n").expect("target should be written");
        symlink("target.txt", temp.join("link.txt")).expect("symlink should be written");
        fs::write(
            repository.git_dir().join("config"),
            "[core]\n\trepositoryformatversion = 0\n\tbare = false\n[user]\n\tname = Rit Test\n\temail = rit@example.test\n",
        )
        .expect("config should be written");

        repository
            .add_paths(&["link.txt".to_owned()])
            .expect("symlink should be added");
        let index = Index::read(&repository.git_dir().join("index")).expect("index should read");
        assert_eq!(index.entries[0].mode, 0o120000);
        let link_object = repository
            .read_object(index.entries[0].object_id)
            .expect("link object should read");
        assert_eq!(link_object.data, b"target.txt");

        let commit_id = repository
            .commit_index("add symlink")
            .expect("commit should be created")
            .commit_id;
        fs::remove_file(temp.join("link.txt")).expect("link should be removed");
        repository
            .restore_worktree_paths(&["link.txt".to_owned()])
            .expect("restore should recreate symlink");
        assert_eq!(
            fs::read_link(temp.join("link.txt")).expect("link target should read"),
            PathBuf::from("target.txt")
        );

        let commit = repository
            .read_object(commit_id)
            .expect("commit object should read");
        let parsed_commit = parse_commit(&commit.data).expect("commit should parse");
        let tree = repository
            .read_object(parsed_commit.tree)
            .expect("tree object should read");
        let entries = parse_tree_entries(&tree.data).expect("tree should parse");
        assert_eq!(entries[0].mode, "120000");
        remove_dir_all(&temp);
    }

    #[cfg(unix)]
    #[test]
    fn core_symlinks_false_adds_symlink_as_regular_file() {
        use std::os::unix::fs::symlink;

        let temp = temp_path("core-symlinks-false-add");
        let repository = Repository::init(&InitOptions::new(&temp)).expect("init should work");
        fs::write(temp.join("target.txt"), "target contents\n").expect("target should be written");
        symlink("target.txt", temp.join("link.txt")).expect("symlink should be written");
        fs::write(
            repository.git_dir().join("config"),
            "[core]\n\trepositoryformatversion = 0\n\tbare = false\n\tsymlinks = false\n[user]\n\tname = Rit Test\n\temail = rit@example.test\n",
        )
        .expect("config should be written");

        repository
            .add_paths(&["link.txt".to_owned()])
            .expect("symlink should be added as regular file");

        let index = Index::read(&repository.git_dir().join("index")).expect("index should read");
        assert_eq!(index.entries[0].mode, 0o100644);
        let link_object = repository
            .read_object(index.entries[0].object_id)
            .expect("link object should read");
        assert_eq!(link_object.data, b"target.txt");
        remove_dir_all(&temp);
    }

    #[cfg(unix)]
    #[test]
    fn core_symlinks_false_restores_symlink_entry_as_plain_file() {
        use std::os::unix::fs::symlink;

        let temp = temp_path("core-symlinks-false-restore");
        let repository = Repository::init(&InitOptions::new(&temp)).expect("init should work");
        fs::write(temp.join("target.txt"), "target contents\n").expect("target should be written");
        symlink("target.txt", temp.join("link.txt")).expect("symlink should be written");
        fs::write(
            repository.git_dir().join("config"),
            "[core]\n\trepositoryformatversion = 0\n\tbare = false\n[user]\n\tname = Rit Test\n\temail = rit@example.test\n",
        )
        .expect("config should be written");
        repository
            .add_paths(&["link.txt".to_owned()])
            .expect("symlink should be added");
        repository
            .commit_index("add symlink")
            .expect("commit should be created");
        fs::write(
            repository.git_dir().join("config"),
            "[core]\n\trepositoryformatversion = 0\n\tbare = false\n\tsymlinks = false\n[user]\n\tname = Rit Test\n\temail = rit@example.test\n",
        )
        .expect("config should be rewritten");
        fs::remove_file(temp.join("link.txt")).expect("link should be removed");

        repository
            .restore_worktree_paths(&["link.txt".to_owned()])
            .expect("restore should write a plain link file");

        let metadata = fs::symlink_metadata(temp.join("link.txt"))
            .expect("restored path metadata should read");
        assert!(!metadata.file_type().is_symlink());
        assert_eq!(
            fs::read_to_string(temp.join("link.txt")).expect("plain link file should read"),
            "target.txt"
        );
        let status = repository
            .status_porcelain_v1()
            .expect("status should be clean");
        assert_eq!(status.to_porcelain_v1(), "");
        remove_dir_all(&temp);
    }

    #[test]
    fn core_symlinks_false_restores_index_symlink_entry_as_plain_file() {
        let temp = temp_path("core-symlinks-false-index-restore");
        let repository = Repository::init(&InitOptions::new(&temp)).expect("init should work");
        fs::write(
            repository.git_dir().join("config"),
            "[core]\n\trepositoryformatversion = 0\n\tbare = false\n\tsymlinks = false\n[user]\n\tname = Rit Test\n\temail = rit@example.test\n",
        )
        .expect("config should be written");
        let object_id = repository
            .loose_objects()
            .write_object(ObjectKind::Blob, b"target.txt")
            .expect("link target blob should be written");
        Index {
            entries: vec![IndexEntry {
                stat: IndexEntryStat::default(),
                mode: 0o120000,
                object_id,
                stage: 0,
                extended_flags: 0,
                file_size: b"target.txt".len() as u32,
                path: "link.txt".to_owned(),
            }],
            extensions: Vec::new(),
        }
        .write(&repository.git_dir().join("index"))
        .expect("index should be written");
        repository
            .commit_index("add symlink entry")
            .expect("commit should be created");

        repository
            .restore_worktree_paths(&["link.txt".to_owned()])
            .expect("restore should write a plain link file");

        let metadata = fs::symlink_metadata(temp.join("link.txt"))
            .expect("restored path metadata should read");
        assert!(!metadata.file_type().is_symlink());
        assert_eq!(
            fs::read_to_string(temp.join("link.txt")).expect("plain link file should read"),
            "target.txt"
        );
        let status = repository
            .status_porcelain_v1()
            .expect("status should be clean");
        assert_eq!(status.to_porcelain_v1(), "");
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

    #[test]
    fn reset_plan_reports_index_changes_without_writing_index() {
        let temp = temp_path("reset-plan");
        let repository = committed_nested_repository(&temp);
        fs::write(temp.join("nested").join("a.txt"), "changed\n").expect("file should be changed");
        fs::write(temp.join("nested").join("new.txt"), "new\n").expect("file should be written");
        repository
            .add_paths(&["nested".to_owned()])
            .expect("add should work");

        let plan = repository
            .plan_restore_staged_paths_from_head(&["nested".to_owned()])
            .expect("reset plan should work");

        assert_eq!(plan.paths_to_restore, vec!["nested/a.txt"]);
        assert_eq!(plan.paths_to_remove, vec!["nested/new.txt"]);
        let index = Index::read(&repository.git_dir().join("index")).expect("index should read");
        let paths = index
            .entries
            .iter()
            .map(|entry| entry.path.as_str())
            .collect::<Vec<_>>();
        assert_eq!(paths, vec!["nested/a.txt", "nested/new.txt"]);
        remove_dir_all(&temp);
    }

    #[test]
    fn merge_plan_reports_fast_forward_without_changing_head() {
        let temp = temp_path("merge-plan");
        let repository = committed_nested_repository(&temp);
        let base = repository
            .resolve_head()
            .expect("HEAD should resolve")
            .expect("HEAD should exist");
        repository
            .checkout_new_branch("topic")
            .expect("topic branch should be created");
        fs::write(temp.join("nested").join("a.txt"), "topic\n").expect("file should be changed");
        repository
            .add_paths(&["nested/a.txt".to_owned()])
            .expect("add should work");
        let topic = repository
            .commit_index("topic")
            .expect("topic commit should work")
            .commit_id;
        repository
            .checkout_branch("master")
            .expect("master checkout should work");

        let plan = repository
            .plan_merge_ff_only("topic")
            .expect("merge plan should work");

        assert_eq!(
            plan,
            super::MergePlan::FastForward {
                old_id: base,
                new_id: topic,
                paths_to_update: vec!["nested/a.txt".to_owned()],
                paths_to_remove: Vec::new(),
            }
        );
        assert_eq!(
            repository
                .resolve_head()
                .expect("HEAD should resolve")
                .expect("HEAD should exist"),
            base
        );
        assert_eq!(
            fs::read_to_string(temp.join("nested").join("a.txt")).expect("file should read"),
            "base\n"
        );
        remove_dir_all(&temp);
    }

    #[test]
    fn merge_plan_reports_non_fast_forward_without_changing_head() {
        let temp = temp_path("merge-plan-non-ff");
        let repository = committed_nested_repository(&temp);
        let base = repository
            .resolve_head()
            .expect("HEAD should resolve")
            .expect("HEAD should exist");
        repository
            .checkout_new_branch("topic")
            .expect("topic branch should be created");
        fs::write(temp.join("nested").join("a.txt"), "topic\n").expect("file should be changed");
        repository
            .add_paths(&["nested/a.txt".to_owned()])
            .expect("topic add should work");
        let topic = repository
            .commit_index("topic")
            .expect("topic commit should work")
            .commit_id;
        repository
            .checkout_branch("master")
            .expect("master checkout should work");
        fs::write(temp.join("nested").join("a.txt"), "master\n").expect("file should be changed");
        repository
            .add_paths(&["nested/a.txt".to_owned()])
            .expect("master add should work");
        let master = repository
            .commit_index("master")
            .expect("master commit should work")
            .commit_id;

        let plan = repository
            .plan_merge_ff_only("topic")
            .expect("merge plan should work");

        let super::MergePlan::NonFastForward {
            head_id,
            target_id,
            merge_base,
            head_changed_paths,
            target_changed_paths,
            conflict_paths,
            conflict_stages,
        } = plan
        else {
            panic!("expected non-fast-forward merge plan");
        };
        assert_eq!(head_id, master);
        assert_eq!(target_id, topic);
        assert_eq!(merge_base, Some(base));
        assert_eq!(head_changed_paths, vec!["nested/a.txt"]);
        assert_eq!(target_changed_paths, vec!["nested/a.txt"]);
        assert_eq!(conflict_paths, vec!["nested/a.txt"]);
        assert_eq!(conflict_stages.len(), 1);
        assert_eq!(conflict_stages[0].path, "nested/a.txt");
        assert!(conflict_stages[0].base.is_some());
        assert!(conflict_stages[0].head.is_some());
        assert!(conflict_stages[0].target.is_some());
        assert_eq!(
            repository
                .resolve_head()
                .expect("HEAD should resolve")
                .expect("HEAD should exist"),
            master
        );
        assert_eq!(
            fs::read_to_string(temp.join("nested").join("a.txt")).expect("file should read"),
            "master\n"
        );
        remove_dir_all(&temp);
    }

    #[test]
    fn merge_writes_conflict_stages_for_non_fast_forward_conflict() {
        let temp = temp_path("merge-conflict-stages");
        let repository = committed_nested_repository(&temp);
        repository
            .checkout_new_branch("topic")
            .expect("topic branch should be created");
        fs::write(temp.join("nested").join("a.txt"), "topic\n").expect("file should be changed");
        repository
            .add_paths(&["nested/a.txt".to_owned()])
            .expect("topic add should work");
        let topic = repository
            .commit_index("topic")
            .expect("topic commit should work")
            .commit_id;
        repository
            .checkout_branch("master")
            .expect("master checkout should work");
        fs::write(temp.join("nested").join("a.txt"), "master\n").expect("file should be changed");
        repository
            .add_paths(&["nested/a.txt".to_owned()])
            .expect("master add should work");
        let master = repository
            .commit_index("master")
            .expect("master commit should work")
            .commit_id;

        let result = repository
            .merge("topic")
            .expect("conflicted merge should write stages");

        assert_eq!(
            result,
            super::MergeResult::Conflicts {
                target_id: topic,
                conflict_paths: vec!["nested/a.txt".to_owned()],
                conflict_reports: vec![MergeConflictReport {
                    path: "nested/a.txt".to_owned(),
                    kind: MergeConflictKind::Content,
                }],
            }
        );
        let index = Index::read(&repository.git_dir().join("index")).expect("index should read");
        let stages = index
            .entries
            .iter()
            .filter(|entry| entry.path == "nested/a.txt")
            .map(|entry| entry.stage)
            .collect::<Vec<_>>();
        assert_eq!(stages, vec![1, 2, 3]);
        assert_eq!(
            fs::read_to_string(repository.git_dir().join("MERGE_HEAD"))
                .expect("MERGE_HEAD should read"),
            format!("{topic}\n")
        );
        assert!(
            fs::read_to_string(repository.git_dir().join("MERGE_MSG"))
                .expect("MERGE_MSG should read")
                .contains("nested/a.txt")
        );
        let conflict_text =
            fs::read_to_string(temp.join("nested").join("a.txt")).expect("file should read");
        assert!(conflict_text.contains("<<<<<<< HEAD\nmaster\n=======\ntopic\n>>>>>>> topic\n"));
        assert_eq!(
            repository
                .commit_index("should fail")
                .expect_err("unmerged index should not commit")
                .to_string(),
            "cannot write tree with unmerged index entry: nested/a.txt"
        );

        let restored = repository.abort_merge().expect("abort should restore");

        assert_eq!(restored, master);
        assert!(!repository.git_dir().join("MERGE_HEAD").exists());
        assert!(!repository.git_dir().join("MERGE_MSG").exists());
        let index = Index::read(&repository.git_dir().join("index")).expect("index should read");
        let stages = index
            .entries
            .iter()
            .filter(|entry| entry.path == "nested/a.txt")
            .map(|entry| entry.stage)
            .collect::<Vec<_>>();
        assert_eq!(stages, vec![0]);
        assert_eq!(
            fs::read_to_string(temp.join("nested").join("a.txt")).expect("file should read"),
            "master\n"
        );
        assert_eq!(
            repository
                .status_porcelain_v1()
                .expect("status should read")
                .to_porcelain_v1(),
            ""
        );
        remove_dir_all(&temp);
    }

    #[test]
    fn merge_materializes_target_for_delete_modify_conflict_when_head_deleted() {
        let temp = temp_path("merge-delete-modify-target");
        let repository = committed_nested_repository(&temp);
        fs::write(temp.join("nested").join("b.txt"), "kept\n").expect("file should be written");
        repository
            .add_paths(&["nested/b.txt".to_owned()])
            .expect("kept add should work");
        repository
            .commit_index("kept")
            .expect("kept commit should work");
        repository
            .checkout_new_branch("topic")
            .expect("topic branch should be created");
        fs::write(temp.join("nested").join("a.txt"), "topic\n").expect("file should be changed");
        repository
            .add_paths(&["nested/a.txt".to_owned()])
            .expect("topic add should work");
        repository
            .commit_index("topic")
            .expect("topic commit should work");
        repository
            .checkout_branch("master")
            .expect("master checkout should work");
        fs::remove_file(temp.join("nested").join("a.txt")).expect("file should be removed");
        let mut index =
            Index::read(&repository.git_dir().join("index")).expect("index should read");
        index.entries.retain(|entry| entry.path != "nested/a.txt");
        index
            .write(&repository.git_dir().join("index"))
            .expect("delete should be staged");
        let parent = repository
            .resolve_head()
            .expect("head should resolve")
            .expect("head should exist");
        repository
            .commit_index_with_parents("delete", &super::CommitOptions::default(), &[parent], false)
            .expect("delete commit should work");

        let result = repository
            .merge("topic")
            .expect("delete/modify merge should start");

        let super::MergeResult::Conflicts {
            conflict_paths,
            conflict_reports,
            ..
        } = result
        else {
            panic!("expected conflicted merge");
        };
        assert_eq!(conflict_paths, vec!["nested/a.txt".to_owned()]);
        assert_eq!(
            conflict_reports,
            vec![MergeConflictReport {
                path: "nested/a.txt".to_owned(),
                kind: MergeConflictKind::ModifyDelete {
                    deleted_side: MergeConflictSide::Head,
                    modified_side: MergeConflictSide::Target,
                    worktree_side: MergeConflictSide::Target,
                },
            }]
        );
        assert_eq!(
            fs::read_to_string(temp.join("nested").join("a.txt")).expect("file should read"),
            "topic\n"
        );
        let index = Index::read(&repository.git_dir().join("index")).expect("index should read");
        let stages = index
            .entries
            .iter()
            .filter(|entry| entry.path == "nested/a.txt")
            .map(|entry| entry.stage)
            .collect::<Vec<_>>();
        assert_eq!(stages, vec![1, 3]);
        remove_dir_all(&temp);
    }

    #[test]
    fn merge_keeps_head_for_delete_modify_conflict_when_target_deleted() {
        let temp = temp_path("merge-delete-modify-head");
        let repository = committed_nested_repository(&temp);
        fs::write(temp.join("nested").join("b.txt"), "kept\n").expect("file should be written");
        repository
            .add_paths(&["nested/b.txt".to_owned()])
            .expect("kept add should work");
        repository
            .commit_index("kept")
            .expect("kept commit should work");
        repository
            .checkout_new_branch("topic")
            .expect("topic branch should be created");
        fs::remove_file(temp.join("nested").join("a.txt")).expect("file should be removed");
        let mut index =
            Index::read(&repository.git_dir().join("index")).expect("index should read");
        index.entries.retain(|entry| entry.path != "nested/a.txt");
        index
            .write(&repository.git_dir().join("index"))
            .expect("delete should be staged");
        let parent = repository
            .resolve_head()
            .expect("head should resolve")
            .expect("head should exist");
        repository
            .commit_index_with_parents("delete", &super::CommitOptions::default(), &[parent], false)
            .expect("delete commit should work");
        repository
            .checkout_branch("master")
            .expect("master checkout should work");
        fs::write(temp.join("nested").join("a.txt"), "head\n").expect("file should be changed");
        repository
            .add_paths(&["nested/a.txt".to_owned()])
            .expect("head add should work");
        repository
            .commit_index("head")
            .expect("head commit should work");

        let result = repository
            .merge("topic")
            .expect("delete/modify merge should start");

        let super::MergeResult::Conflicts {
            conflict_paths,
            conflict_reports,
            ..
        } = result
        else {
            panic!("expected conflicted merge");
        };
        assert_eq!(conflict_paths, vec!["nested/a.txt".to_owned()]);
        assert_eq!(
            conflict_reports,
            vec![MergeConflictReport {
                path: "nested/a.txt".to_owned(),
                kind: MergeConflictKind::ModifyDelete {
                    deleted_side: MergeConflictSide::Target,
                    modified_side: MergeConflictSide::Head,
                    worktree_side: MergeConflictSide::Head,
                },
            }]
        );
        assert_eq!(
            fs::read_to_string(temp.join("nested").join("a.txt")).expect("file should read"),
            "head\n"
        );
        let index = Index::read(&repository.git_dir().join("index")).expect("index should read");
        let stages = index
            .entries
            .iter()
            .filter(|entry| entry.path == "nested/a.txt")
            .map(|entry| entry.stage)
            .collect::<Vec<_>>();
        assert_eq!(stages, vec![1, 2]);
        remove_dir_all(&temp);
    }

    #[test]
    fn merge_reports_binary_content_conflict_and_keeps_head_bytes() {
        let temp = temp_path("merge-binary-conflict");
        let repository = committed_nested_repository(&temp);
        fs::write(temp.join("nested").join("bin.dat"), [0, 1, 2, 3, 4])
            .expect("binary file should be written");
        repository
            .add_paths(&["nested/bin.dat".to_owned()])
            .expect("binary add should work");
        repository
            .commit_index("binary base")
            .expect("binary base commit should work");
        repository
            .checkout_new_branch("topic")
            .expect("topic branch should be created");
        fs::write(temp.join("nested").join("bin.dat"), [0, 1, 2, 9, 4])
            .expect("topic binary should be written");
        repository
            .add_paths(&["nested/bin.dat".to_owned()])
            .expect("topic binary add should work");
        repository
            .commit_index("topic")
            .expect("topic commit should work");
        repository
            .checkout_branch("master")
            .expect("master checkout should work");
        fs::write(temp.join("nested").join("bin.dat"), [0, 1, 2, 8, 4])
            .expect("head binary should be written");
        repository
            .add_paths(&["nested/bin.dat".to_owned()])
            .expect("head binary add should work");
        repository
            .commit_index("master")
            .expect("master commit should work");

        let result = repository
            .merge("topic")
            .expect("binary merge should start");

        let super::MergeResult::Conflicts {
            conflict_paths,
            conflict_reports,
            ..
        } = result
        else {
            panic!("expected conflicted merge");
        };
        assert_eq!(conflict_paths, vec!["nested/bin.dat".to_owned()]);
        assert_eq!(
            conflict_reports,
            vec![MergeConflictReport {
                path: "nested/bin.dat".to_owned(),
                kind: MergeConflictKind::BinaryContent,
            }]
        );
        assert_eq!(
            fs::read(temp.join("nested").join("bin.dat")).expect("binary should read"),
            vec![0, 1, 2, 8, 4]
        );
        remove_dir_all(&temp);
    }

    #[test]
    fn merge_preserves_mode_stages_for_content_conflict_with_mode_change() {
        let temp = temp_path("merge-content-mode-conflict");
        let repository = committed_nested_repository(&temp);
        repository
            .checkout_new_branch("topic")
            .expect("topic branch should be created");
        fs::write(temp.join("nested").join("a.txt"), "topic\n")
            .expect("topic file should be written");
        repository
            .add_paths_with_options(
                &["nested/a.txt".to_owned()],
                &AddOptions {
                    mode_override: Some(FileModeOverride::Executable),
                },
            )
            .expect("topic add should work");
        repository
            .commit_index("topic")
            .expect("topic commit should work");
        repository
            .checkout_branch("master")
            .expect("master checkout should work");
        fs::write(temp.join("nested").join("a.txt"), "head\n")
            .expect("head file should be written");
        repository
            .add_paths(&["nested/a.txt".to_owned()])
            .expect("head add should work");
        repository
            .commit_index("head")
            .expect("head commit should work");

        let result = repository
            .merge("topic")
            .expect("content/mode merge should start");

        let super::MergeResult::Conflicts {
            conflict_paths,
            conflict_reports,
            ..
        } = result
        else {
            panic!("expected conflicted merge");
        };
        assert_eq!(conflict_paths, vec!["nested/a.txt".to_owned()]);
        assert_eq!(
            conflict_reports,
            vec![MergeConflictReport {
                path: "nested/a.txt".to_owned(),
                kind: MergeConflictKind::Content,
            }]
        );
        let index = Index::read(&repository.git_dir().join("index")).expect("index should read");
        let stages = index
            .entries
            .iter()
            .filter(|entry| entry.path == "nested/a.txt")
            .map(|entry| (entry.stage, entry.mode))
            .collect::<Vec<_>>();
        assert_eq!(stages, vec![(1, 0o100644), (2, 0o100644), (3, 0o100755)]);
        assert_eq!(
            fs::read_to_string(temp.join("nested").join("a.txt"))
                .expect("conflict file should read"),
            "<<<<<<< HEAD\nhead\n=======\ntopic\n>>>>>>> topic\n"
        );
        remove_dir_all(&temp);
    }

    #[test]
    fn merge_reports_add_add_conflict_for_independent_adds() {
        let temp = temp_path("merge-add-add-conflict");
        let repository = committed_nested_repository(&temp);
        repository
            .checkout_new_branch("topic")
            .expect("topic branch should be created");
        fs::write(temp.join("nested").join("c.txt"), "topic\n").expect("topic file should write");
        repository
            .add_paths(&["nested/c.txt".to_owned()])
            .expect("topic add should work");
        repository
            .commit_index("topic")
            .expect("topic commit should work");
        repository
            .checkout_branch("master")
            .expect("master checkout should work");
        fs::write(temp.join("nested").join("c.txt"), "head\n").expect("head file should write");
        repository
            .add_paths(&["nested/c.txt".to_owned()])
            .expect("head add should work");
        repository
            .commit_index("head")
            .expect("head commit should work");

        let result = repository
            .merge("topic")
            .expect("add/add merge should start");

        let super::MergeResult::Conflicts {
            conflict_paths,
            conflict_reports,
            ..
        } = result
        else {
            panic!("expected conflicted merge");
        };
        assert_eq!(conflict_paths, vec!["nested/c.txt".to_owned()]);
        assert_eq!(
            conflict_reports,
            vec![MergeConflictReport {
                path: "nested/c.txt".to_owned(),
                kind: MergeConflictKind::AddAdd,
            }]
        );
        let index = Index::read(&repository.git_dir().join("index")).expect("index should read");
        let stages = index
            .entries
            .iter()
            .filter(|entry| entry.path == "nested/c.txt")
            .map(|entry| entry.stage)
            .collect::<Vec<_>>();
        assert_eq!(stages, vec![2, 3]);
        assert!(
            fs::read_to_string(temp.join("nested").join("c.txt"))
                .expect("conflict file should read")
                .contains("<<<<<<< HEAD\nhead\n=======\ntopic\n>>>>>>> topic\n")
        );
        remove_dir_all(&temp);
    }

    #[test]
    fn merge_continue_creates_merge_commit_after_resolution() {
        let temp = temp_path("merge-continue");
        let repository = committed_nested_repository(&temp);
        repository
            .checkout_new_branch("topic")
            .expect("topic branch should be created");
        fs::write(temp.join("nested").join("a.txt"), "topic\n").expect("file should be changed");
        repository
            .add_paths(&["nested/a.txt".to_owned()])
            .expect("topic add should work");
        let topic = repository
            .commit_index("topic")
            .expect("topic commit should work")
            .commit_id;
        repository
            .checkout_branch("master")
            .expect("master checkout should work");
        fs::write(temp.join("nested").join("a.txt"), "master\n").expect("file should be changed");
        repository
            .add_paths(&["nested/a.txt".to_owned()])
            .expect("master add should work");
        let master = repository
            .commit_index("master")
            .expect("master commit should work")
            .commit_id;
        repository
            .merge("topic")
            .expect("conflicted merge should start");
        fs::write(temp.join("nested").join("a.txt"), "resolved\n")
            .expect("resolution should be written");
        repository
            .add_paths(&["nested/a.txt".to_owned()])
            .expect("resolution should be staged");

        let result = repository
            .continue_merge(&super::CommitOptions::default())
            .expect("merge continue should commit");

        let object = repository
            .read_object(result.commit_id)
            .expect("merge commit should read");
        let commit = parse_commit(&object.data).expect("merge commit should parse");
        assert_eq!(commit.parents, vec![master, topic]);
        assert!(!repository.git_dir().join("MERGE_HEAD").exists());
        assert!(!repository.git_dir().join("MERGE_MSG").exists());
        assert_eq!(
            repository
                .status_porcelain_v1()
                .expect("status should read")
                .to_porcelain_v1(),
            ""
        );
        assert_eq!(
            fs::read_to_string(temp.join("nested").join("a.txt")).expect("file should read"),
            "resolved\n"
        );
        remove_dir_all(&temp);
    }

    #[test]
    fn merge_creates_clean_non_fast_forward_merge_commit() {
        let temp = temp_path("merge-clean-non-ff");
        let repository = committed_nested_repository(&temp);
        repository
            .checkout_new_branch("topic")
            .expect("topic branch should be created");
        fs::write(temp.join("nested").join("a.txt"), "topic\n").expect("file should be changed");
        repository
            .add_paths(&["nested/a.txt".to_owned()])
            .expect("topic add should work");
        let topic = repository
            .commit_index("topic")
            .expect("topic commit should work")
            .commit_id;
        repository
            .checkout_branch("master")
            .expect("master checkout should work");
        fs::write(temp.join("nested").join("b.txt"), "master\n").expect("file should be written");
        repository
            .add_paths(&["nested/b.txt".to_owned()])
            .expect("master add should work");
        let master = repository
            .commit_index("master")
            .expect("master commit should work")
            .commit_id;

        let result = repository
            .merge("topic")
            .expect("clean non-fast-forward merge should commit");

        let super::MergeResult::MergeCommit {
            old_id,
            target_id,
            commit_id,
        } = result
        else {
            panic!("expected clean merge commit result");
        };
        assert_eq!(old_id, master);
        assert_eq!(target_id, topic);
        let object = repository
            .read_object(commit_id)
            .expect("merge commit should read");
        let commit = parse_commit(&object.data).expect("merge commit should parse");
        assert_eq!(commit.parents, vec![master, topic]);
        assert_eq!(
            fs::read_to_string(temp.join("nested").join("a.txt")).expect("file should read"),
            "topic\n"
        );
        assert_eq!(
            fs::read_to_string(temp.join("nested").join("b.txt")).expect("file should read"),
            "master\n"
        );
        assert!(!repository.git_dir().join("MERGE_HEAD").exists());
        assert_eq!(
            repository
                .status_porcelain_v1()
                .expect("status should read")
                .to_porcelain_v1(),
            ""
        );
        remove_dir_all(&temp);
    }

    #[test]
    fn merge_combines_mode_only_and_content_only_changes_cleanly() {
        let temp = temp_path("merge-mode-content-clean");
        let repository = committed_nested_repository(&temp);
        repository
            .checkout_new_branch("topic")
            .expect("topic branch should be created");
        repository
            .add_paths_with_options(
                &["nested/a.txt".to_owned()],
                &AddOptions {
                    mode_override: Some(FileModeOverride::Executable),
                },
            )
            .expect("mode-only add should work");
        let topic = repository
            .commit_index("mode")
            .expect("topic commit should work")
            .commit_id;
        repository
            .checkout_branch("master")
            .expect("master checkout should work");
        fs::write(temp.join("nested").join("a.txt"), "head\n").expect("file should be changed");
        repository
            .add_paths(&["nested/a.txt".to_owned()])
            .expect("content add should work");
        let master = repository
            .commit_index("content")
            .expect("master commit should work")
            .commit_id;

        let result = repository
            .merge("topic")
            .expect("mode/content merge should commit");

        let super::MergeResult::MergeCommit {
            old_id,
            target_id,
            commit_id,
        } = result
        else {
            panic!("expected clean merge commit result");
        };
        assert_eq!(old_id, master);
        assert_eq!(target_id, topic);
        let entries = repository
            .commit_index_entries(commit_id)
            .expect("merge tree should read");
        let entry = entries
            .iter()
            .find(|entry| entry.path == "nested/a.txt")
            .expect("merged file should exist");
        assert_eq!(entry.mode, 0o100755);
        let object = repository
            .read_object(entry.object_id)
            .expect("merged blob should read");
        assert_eq!(object.data, b"head\n");
        assert!(!repository.git_dir().join("MERGE_HEAD").exists());
        remove_dir_all(&temp);
    }

    #[test]
    fn merge_reports_distinct_type_conflict_for_regular_file_and_symlink() {
        let temp = temp_path("merge-distinct-type-conflict");
        let repository = committed_nested_repository(&temp);
        fs::write(
            repository.git_dir().join("config"),
            "[core]\n\trepositoryformatversion = 0\n\tbare = false\n\tsymlinks = false\n[user]\n\tname = Rit Test\n\temail = rit@example.test\n",
        )
        .expect("config should be written");
        repository
            .checkout_new_branch("topic")
            .expect("topic branch should be created");
        let target_object_id = repository
            .loose_objects()
            .write_object(ObjectKind::Blob, b"target")
            .expect("symlink target blob should be written");
        let mut index =
            Index::read(&repository.git_dir().join("index")).expect("index should read");
        let entry = index
            .entries
            .iter_mut()
            .find(|entry| entry.path == "nested/a.txt")
            .expect("tracked entry should exist");
        entry.mode = 0o120000;
        entry.object_id = target_object_id;
        entry.file_size = b"target".len() as u32;
        index
            .write(&repository.git_dir().join("index"))
            .expect("index should write");
        repository
            .commit_index("symlink")
            .expect("topic commit should work");
        fs::write(temp.join("nested").join("a.txt"), "target")
            .expect("plain symlink target should be materialized");
        repository
            .checkout_branch("master")
            .expect("master checkout should work");
        fs::write(temp.join("nested").join("a.txt"), "head\n").expect("file should be changed");
        repository
            .add_paths(&["nested/a.txt".to_owned()])
            .expect("content add should work");
        repository
            .commit_index("content")
            .expect("master commit should work");

        let result = repository
            .merge("topic")
            .expect("distinct type merge should start");

        let super::MergeResult::Conflicts {
            conflict_reports, ..
        } = result
        else {
            panic!("expected distinct type conflict result");
        };
        assert_eq!(
            conflict_reports,
            vec![MergeConflictReport {
                path: "nested/a.txt".to_owned(),
                kind: MergeConflictKind::DistinctTypes,
            }]
        );
        let index = Index::read(&repository.git_dir().join("index")).expect("index should read");
        assert!(index.entries.iter().any(|entry| {
            entry.path == "nested/a.txt" && entry.stage == 3 && entry.mode == 0o120000
        }));
        assert!(index.entries.iter().any(|entry| {
            entry.path == "nested/a.txt~HEAD" && entry.stage == 1 && entry.mode == 0o100644
        }));
        assert!(index.entries.iter().any(|entry| {
            entry.path == "nested/a.txt~HEAD" && entry.stage == 2 && entry.mode == 0o100644
        }));
        assert_eq!(
            fs::read_to_string(temp.join("nested").join("a.txt"))
                .expect("target side should be written"),
            "target"
        );
        assert_eq!(
            fs::read_to_string(temp.join("nested").join("a.txt~HEAD"))
                .expect("head side should be written"),
            "head\n"
        );
        remove_dir_all(&temp);
    }

    #[test]
    fn merge_reports_distinct_type_conflict_for_symlink_and_regular_file() {
        let temp = temp_path("merge-distinct-type-inverse");
        let repository = committed_nested_repository(&temp);
        fs::write(
            repository.git_dir().join("config"),
            "[core]\n\trepositoryformatversion = 0\n\tbare = false\n\tsymlinks = false\n[user]\n\tname = Rit Test\n\temail = rit@example.test\n",
        )
        .expect("config should be written");
        repository
            .checkout_new_branch("topic")
            .expect("topic branch should be created");
        fs::write(temp.join("nested").join("a.txt"), "target\n")
            .expect("target file should be changed");
        repository
            .add_paths(&["nested/a.txt".to_owned()])
            .expect("target add should work");
        repository
            .commit_index("content")
            .expect("topic commit should work");
        repository
            .checkout_branch("master")
            .expect("master checkout should work");
        let head_object_id = repository
            .loose_objects()
            .write_object(ObjectKind::Blob, b"head-target")
            .expect("symlink target blob should be written");
        let mut index =
            Index::read(&repository.git_dir().join("index")).expect("index should read");
        let entry = index
            .entries
            .iter_mut()
            .find(|entry| entry.path == "nested/a.txt")
            .expect("tracked entry should exist");
        entry.mode = 0o120000;
        entry.object_id = head_object_id;
        entry.file_size = b"head-target".len() as u32;
        index
            .write(&repository.git_dir().join("index"))
            .expect("index should write");
        repository
            .commit_index("symlink")
            .expect("master commit should work");
        fs::write(temp.join("nested").join("a.txt"), "head-target")
            .expect("plain symlink target should be materialized");

        let result = repository
            .merge("topic")
            .expect("distinct type merge should start");

        let super::MergeResult::Conflicts {
            conflict_reports, ..
        } = result
        else {
            panic!("expected distinct type conflict result");
        };
        assert_eq!(
            conflict_reports,
            vec![MergeConflictReport {
                path: "nested/a.txt".to_owned(),
                kind: MergeConflictKind::DistinctTypes,
            }]
        );
        let index = Index::read(&repository.git_dir().join("index")).expect("index should read");
        assert!(index.entries.iter().any(|entry| {
            entry.path == "nested/a.txt" && entry.stage == 2 && entry.mode == 0o120000
        }));
        assert!(index.entries.iter().any(|entry| {
            entry.path == "nested/a.txt~topic" && entry.stage == 1 && entry.mode == 0o100644
        }));
        assert!(index.entries.iter().any(|entry| {
            entry.path == "nested/a.txt~topic" && entry.stage == 3 && entry.mode == 0o100644
        }));
        assert_eq!(
            fs::read_to_string(temp.join("nested").join("a.txt"))
                .expect("head side should be written"),
            "head-target"
        );
        assert_eq!(
            fs::read_to_string(temp.join("nested").join("a.txt~topic"))
                .expect("target side should be written"),
            "target\n"
        );
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

    fn count_object_files(path: impl AsRef<Path>) -> usize {
        let path = path.as_ref();
        if !path.exists() {
            return 0;
        }
        let mut count = 0;
        for entry in fs::read_dir(path).expect("object directory should read") {
            let entry = entry.expect("object entry should read");
            let path = entry.path();
            if path.is_dir() {
                count += count_object_files(&path);
            } else if path.is_file() {
                count += 1;
            }
        }
        count
    }

    #[cfg(unix)]
    fn set_test_permissions(path: &Path, mode: u32) {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(mode))
            .expect("test permissions should be set");
    }

    #[cfg(unix)]
    fn is_test_executable(path: &Path) -> bool {
        use std::os::unix::fs::PermissionsExt;
        fs::metadata(path)
            .expect("metadata should read")
            .permissions()
            .mode()
            & 0o111
            != 0
    }
}
