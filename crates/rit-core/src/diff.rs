use crate::index::{Index, join_slash_path};
use crate::object::parse_tree_entries;
use crate::{
    GitConfig, ObjectId, ObjectKind, PathspecSet, Repository, Result, RitError, hash_object,
    parse_commit,
};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;

/// Summary of working tree changes compared with the index.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiffSummary {
    /// Changed file stats in stable path order.
    pub files: Vec<DiffFileStat>,
    /// Git-shaped warnings discovered while computing the diff.
    pub warnings: Vec<String>,
}

impl DiffSummary {
    /// Returns a copy containing only files accepted by a Git `--diff-filter`.
    pub fn into_filtered_by_status(mut self, filter: &DiffStatusFilter) -> Self {
        if filter.all_or_none && self.files.iter().any(|file| filter.matches(file.status)) {
            return self;
        }
        self.files.retain(|file| filter.matches(file.status));
        self
    }

    /// Returns changed path names only.
    pub fn name_only(&self) -> Vec<&str> {
        self.files.iter().map(|file| file.path.as_str()).collect()
    }

    /// Renders Git-like `-z --name-only` output.
    pub fn to_name_only_z(&self) -> Vec<u8> {
        let mut output = Vec::new();
        for file in &self.files {
            output.extend_from_slice(file.path.as_bytes());
            output.push(0);
        }
        output
    }

    /// Renders a Git-like `--name-status` summary.
    pub fn to_name_status_text(&self) -> String {
        let mut output = String::new();
        for file in &self.files {
            if file.status == 'R' || file.status == 'C' {
                output.push_str(&format!(
                    "{}{:03}\t{}\t{}\n",
                    file.status,
                    file.similarity_score.unwrap_or(100),
                    file.old_path.as_deref().unwrap_or(&file.path),
                    file.path
                ));
            } else {
                output.push(file.status);
                output.push('\t');
                output.push_str(&file.path);
                output.push('\n');
            }
        }
        output
    }

    /// Renders Git-like `-z --name-status` output.
    pub fn to_name_status_z(&self) -> Vec<u8> {
        let mut output = Vec::new();
        for file in &self.files {
            if file.status == 'R' || file.status == 'C' {
                output.extend_from_slice(
                    format!("{}{:03}", file.status, file.similarity_score.unwrap_or(100))
                        .as_bytes(),
                );
                output.push(0);
                output.extend_from_slice(file.old_path.as_deref().unwrap_or(&file.path).as_bytes());
                output.push(0);
                output.extend_from_slice(file.path.as_bytes());
                output.push(0);
            } else {
                output.push(file.status as u8);
                output.push(0);
                output.extend_from_slice(file.path.as_bytes());
                output.push(0);
            }
        }
        output
    }

    /// Renders a Git-like `--numstat` summary for text files.
    pub fn to_numstat_text(&self) -> String {
        let mut output = String::new();
        for file in &self.files {
            if file.binary {
                output.push_str("-\t-");
            } else {
                output.push_str(&file.insertions.to_string());
                output.push('\t');
                output.push_str(&file.deletions.to_string());
            }
            output.push('\t');
            output.push_str(&file.display_path());
            output.push('\n');
        }
        output
    }

    /// Renders Git-like `-z --numstat` output.
    pub fn to_numstat_z(&self) -> Vec<u8> {
        let mut output = Vec::new();
        for file in &self.files {
            if file.binary {
                output.extend_from_slice(b"-\t-");
            } else {
                output.extend_from_slice(file.insertions.to_string().as_bytes());
                output.push(b'\t');
                output.extend_from_slice(file.deletions.to_string().as_bytes());
            }
            output.push(b'\t');
            if file.status == 'R' || file.status == 'C' {
                output.push(0);
                output.extend_from_slice(file.old_path.as_deref().unwrap_or(&file.path).as_bytes());
                output.push(0);
                output.extend_from_slice(file.path.as_bytes());
            } else {
                output.extend_from_slice(file.path.as_bytes());
            }
            output.push(0);
        }
        output
    }

    /// Renders a Git-like `--shortstat` summary.
    pub fn to_shortstat_text(&self) -> String {
        if self.files.is_empty() {
            return String::new();
        }

        let mut total_insertions = 0;
        let mut total_deletions = 0;
        let has_binary_file = self.files.iter().any(|file| file.binary);
        let has_rename_or_copy = self
            .files
            .iter()
            .any(|file| file.status == 'R' || file.status == 'C');
        for file in &self.files {
            total_insertions += file.insertions;
            total_deletions += file.deletions;
        }

        let mut output = format!(" {} changed", plural(self.files.len(), "file", "files"));
        if total_insertions > 0 || has_binary_file || has_rename_or_copy {
            output.push_str(&format!(
                ", {}",
                plural(total_insertions, "insertion(+)", "insertions(+)")
            ));
        }
        if total_deletions > 0 || has_binary_file || has_rename_or_copy {
            output.push_str(&format!(
                ", {}",
                plural(total_deletions, "deletion(-)", "deletions(-)")
            ));
        }
        output.push('\n');
        output
    }

    /// Renders a Git-like `--stat` summary.
    pub fn to_stat_text(&self) -> String {
        self.to_stat_text_with_paths(DiffFileStat::display_path)
    }

    /// Renders a Git-like `--compact-summary` summary.
    pub fn to_compact_stat_text(&self) -> String {
        self.to_stat_text_with_paths(DiffFileStat::compact_display_path)
    }

    fn to_stat_text_with_paths(&self, display_path: fn(&DiffFileStat) -> String) -> String {
        if self.files.is_empty() {
            return String::new();
        }

        let max_path_width = self
            .files
            .iter()
            .map(|file| display_path(file).len())
            .max()
            .unwrap_or(0);
        let max_change_width = self
            .files
            .iter()
            .map(|file| file.changed_lines().to_string().len())
            .max()
            .unwrap_or(1);
        let mut output = String::new();
        let mut total_insertions = 0;
        let mut total_deletions = 0;
        let has_binary_file = self.files.iter().any(|file| file.binary);
        let has_rename_or_copy = self
            .files
            .iter()
            .any(|file| file.status == 'R' || file.status == 'C');

        for file in &self.files {
            let path = display_path(file);
            total_insertions += file.insertions;
            total_deletions += file.deletions;
            if file.binary {
                output.push_str(&format!(
                    " {:path_width$} | Bin {} -> {} bytes\n",
                    path,
                    file.old_size,
                    file.new_size,
                    path_width = max_path_width
                ));
            } else {
                let mut graph = String::new();
                graph.extend(std::iter::repeat_n('+', file.insertions));
                graph.extend(std::iter::repeat_n('-', file.deletions));
                if graph.is_empty() {
                    output.push_str(&format!(
                        " {path:path_width$} | {:change_width$}\n",
                        file.changed_lines(),
                        path_width = max_path_width,
                        change_width = max_change_width
                    ));
                } else {
                    output.push_str(&format!(
                        " {path:path_width$} | {:change_width$} {graph}\n",
                        file.changed_lines(),
                        path_width = max_path_width,
                        change_width = max_change_width
                    ));
                }
            }
        }

        output.push_str(&format!(
            " {} changed",
            plural(self.files.len(), "file", "files")
        ));
        if total_insertions > 0 || has_binary_file || has_rename_or_copy {
            output.push_str(&format!(
                ", {}",
                plural(total_insertions, "insertion(+)", "insertions(+)")
            ));
        }
        if total_deletions > 0 || has_binary_file || has_rename_or_copy {
            output.push_str(&format!(
                ", {}",
                plural(total_deletions, "deletion(-)", "deletions(-)")
            ));
        }
        output.push('\n');
        output
    }
}

/// Patch-form diff output.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiffPatch {
    /// Changed files in stable path order.
    pub files: Vec<DiffPatchFile>,
    /// Git-shaped warnings discovered while computing the diff.
    pub warnings: Vec<String>,
}

impl DiffPatch {
    /// Returns a copy containing only files accepted by a Git `--diff-filter`.
    pub fn into_filtered_by_status(mut self, filter: &DiffStatusFilter) -> Self {
        if filter.all_or_none && self.files.iter().any(|file| filter.matches(file.status)) {
            return self;
        }
        self.files.retain(|file| filter.matches(file.status));
        self
    }

    /// Renders a small Git-like unified patch.
    pub fn to_patch_text(&self) -> Result<String> {
        self.to_patch_text_with_options(&PatchRenderOptions::default())
    }

    /// Renders a small Git-like unified patch with explicit rendering options.
    pub fn to_patch_text_with_options(&self, options: &PatchRenderOptions) -> Result<String> {
        let mut output = String::new();
        for file in &self.files {
            output.push_str(&file.to_patch_text(options)?);
        }
        Ok(output)
    }

    /// Renders Git-like `--raw` records for the patch files.
    pub fn to_raw_text_with_options(&self, options: &PatchRenderOptions) -> String {
        let mut output = String::new();
        for file in &self.files {
            output.push_str(&file.to_raw_text(options));
        }
        output
    }

    /// Renders Git-like `--summary` records for file creation, deletion,
    /// rename, and copy changes.
    pub fn to_summary_text(&self) -> String {
        let mut output = String::new();
        for file in &self.files {
            output.push_str(&file.to_summary_text());
        }
        output
    }
}

/// Parsed Git `--diff-filter=<letters>` status selector.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DiffStatusFilter {
    included: BTreeSet<char>,
    excluded: BTreeSet<char>,
    all_or_none: bool,
}

impl DiffStatusFilter {
    /// Parses the letter form used by Git's `--diff-filter=<letters>` option.
    pub fn from_git_diff_filter(value: &str) -> Result<Self> {
        let mut filter = Self::default();
        for character in value.chars() {
            if character == '*' {
                filter.all_or_none = true;
                continue;
            }
            let status = character.to_ascii_uppercase();
            if !is_known_diff_filter_status(status) {
                return Err(RitError::invalid_input(format!(
                    "unknown change class '{character}' in --diff-filter={value}"
                )));
            }
            if character.is_ascii_lowercase() {
                filter.excluded.insert(status);
            } else {
                filter.included.insert(status);
            }
        }
        Ok(filter)
    }

    /// Returns true when `status` should be shown.
    pub fn matches(&self, status: char) -> bool {
        let status = status.to_ascii_uppercase();
        let included = self.included.is_empty() || self.included.contains(&status);
        included && !self.excluded.contains(&status)
    }
}

fn is_known_diff_filter_status(status: char) -> bool {
    matches!(status, 'A' | 'C' | 'D' | 'M' | 'R' | 'T' | 'U' | 'X' | 'B')
}

/// Options that affect patch text rendering without changing the diff itself.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PatchRenderOptions {
    /// Render full object IDs in `index` header lines instead of Git's default
    /// abbreviated IDs.
    pub full_index: bool,
    /// Number of object ID hex characters to show when `full_index` is false.
    pub abbrev: usize,
    /// Number of unchanged context lines to include around each hunk.
    pub context_lines: usize,
    /// Number of omitted context lines that may be shown to merge nearby hunks.
    pub inter_hunk_context: usize,
    /// Whether patch paths use Git's default `a/` and `b/` prefixes.
    pub default_prefixes: bool,
    /// Prefix for inserted lines in unified hunks.
    pub new_line_indicator: Option<char>,
    /// Prefix for deleted lines in unified hunks.
    pub old_line_indicator: Option<char>,
    /// Prefix for unchanged context lines in unified hunks.
    pub context_line_indicator: Option<char>,
}

impl Default for PatchRenderOptions {
    fn default() -> Self {
        Self {
            full_index: false,
            abbrev: 7,
            context_lines: 3,
            inter_hunk_context: 0,
            default_prefixes: true,
            new_line_indicator: Some('+'),
            old_line_indicator: Some('-'),
            context_line_indicator: Some(' '),
        }
    }
}

/// One file in a patch-form diff.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiffPatchFile {
    /// Git name-status code for this path.
    pub status: char,
    /// Previous repository-relative path for renames and copies.
    pub old_path: Option<String>,
    /// Repository-relative path using `/` separators.
    pub path: String,
    /// Similarity percentage for rename and copy entries.
    pub similarity_score: Option<u8>,
    /// Old blob object ID, or `None` for new files.
    pub old_object_id: Option<ObjectId>,
    /// New blob object ID, or `None` for deleted files.
    pub new_object_id: Option<ObjectId>,
    /// File mode used in patch headers.
    pub mode: u32,
    /// Old blob contents.
    pub old_data: Vec<u8>,
    /// New blob contents.
    pub new_data: Vec<u8>,
}

impl DiffPatchFile {
    fn to_summary_text(&self) -> String {
        match self.status {
            'A' => format!(" create mode {:06o} {}\n", self.mode, self.path),
            'D' => format!(" delete mode {:06o} {}\n", self.mode, self.path),
            'R' | 'C' => {
                let action = if self.status == 'R' { "rename" } else { "copy" };
                let old_path = self.old_path.as_deref().unwrap_or(&self.path);
                format!(
                    " {action} {} ({}%)\n",
                    rename_summary_path(old_path, &self.path),
                    self.similarity_score.unwrap_or(100)
                )
            }
            _ => String::new(),
        }
    }

    fn to_raw_text(&self, options: &PatchRenderOptions) -> String {
        let old_mode = if self.old_object_id.is_some() {
            self.mode
        } else {
            0
        };
        let new_mode = if self.new_object_id.is_some() {
            self.mode
        } else {
            0
        };
        let old_id = raw_object_id(self.old_object_id, options);
        let new_id = raw_object_id(self.new_object_id, options);
        let old_path = self.old_path.as_deref().unwrap_or(&self.path);
        if self.status == 'R' || self.status == 'C' {
            return format!(
                ":{old_mode:06o} {new_mode:06o} {old_id} {new_id} {}{:03}\t{old_path}\t{}\n",
                self.status,
                self.similarity_score.unwrap_or(100),
                self.path
            );
        }

        format!(
            ":{old_mode:06o} {new_mode:06o} {old_id} {new_id} {}\t{}\n",
            self.status, self.path
        )
    }

    fn to_patch_text(&self, options: &PatchRenderOptions) -> Result<String> {
        let mut output = String::new();
        let is_binary = is_binary_data(&self.old_data) || is_binary_data(&self.new_data);
        let old_path = self.old_path.as_deref().unwrap_or(&self.path);
        output.push_str(&format!(
            "diff --git {} {}\n",
            prefixed_old_path(old_path, options),
            prefixed_new_path(&self.path, options)
        ));
        match self.status {
            'R' | 'C' => {
                let action = if self.status == 'R' { "rename" } else { "copy" };
                output.push_str(&format!(
                    "similarity index {}%\n",
                    self.similarity_score.unwrap_or(100)
                ));
                output.push_str(&format!("{action} from {old_path}\n"));
                output.push_str(&format!("{action} to {}\n", self.path));
                if self.old_object_id == self.new_object_id {
                    return Ok(output);
                }
                output.push_str(&format!(
                    "index {}..{} {:06o}\n",
                    patch_object_id(self.old_object_id, self.new_object_id, options),
                    patch_object_id(self.new_object_id, self.old_object_id, options),
                    self.mode
                ));
                if !is_binary {
                    output.push_str(&format!("--- {}\n", prefixed_old_path(old_path, options)));
                    output.push_str(&format!("+++ {}\n", prefixed_new_path(&self.path, options)));
                }
            }
            'A' => {
                output.push_str(&format!("new file mode {:06o}\n", self.mode));
                output.push_str(&format!(
                    "index {}..{}\n",
                    patch_object_id(None, self.new_object_id, options),
                    patch_object_id(self.new_object_id, self.old_object_id, options)
                ));
                if !is_binary {
                    output.push_str("--- /dev/null\n");
                    output.push_str(&format!("+++ {}\n", prefixed_new_path(&self.path, options)));
                }
            }
            'D' => {
                output.push_str(&format!("deleted file mode {:06o}\n", self.mode));
                output.push_str(&format!(
                    "index {}..{}\n",
                    patch_object_id(self.old_object_id, self.new_object_id, options),
                    patch_object_id(None, self.old_object_id, options)
                ));
                if !is_binary {
                    output.push_str(&format!("--- {}\n", prefixed_old_path(&self.path, options)));
                    output.push_str("+++ /dev/null\n");
                }
            }
            _ => {
                output.push_str(&format!(
                    "index {}..{} {:06o}\n",
                    patch_object_id(self.old_object_id, self.new_object_id, options),
                    patch_object_id(self.new_object_id, self.old_object_id, options),
                    self.mode
                ));
                if !is_binary {
                    output.push_str(&format!("--- {}\n", prefixed_old_path(&self.path, options)));
                    output.push_str(&format!("+++ {}\n", prefixed_new_path(&self.path, options)));
                }
            }
        }
        if is_binary {
            output.push_str(&binary_patch_line(self, options));
        } else {
            output.push_str(&unified_hunk_with_context(
                &self.old_data,
                &self.new_data,
                options,
            )?);
        }
        Ok(output)
    }
}

fn rename_summary_path(old_path: &str, new_path: &str) -> String {
    format!("{old_path} => {new_path}")
}

fn prefixed_old_path(path: &str, options: &PatchRenderOptions) -> String {
    prefixed_path("a/", path, options)
}

fn prefixed_new_path(path: &str, options: &PatchRenderOptions) -> String {
    prefixed_path("b/", path, options)
}

fn prefixed_path(prefix: &str, path: &str, options: &PatchRenderOptions) -> String {
    if options.default_prefixes {
        format!("{prefix}{path}")
    } else {
        path.to_owned()
    }
}

fn binary_patch_line(file: &DiffPatchFile, options: &PatchRenderOptions) -> String {
    let old_path = if file.old_object_id.is_some() {
        prefixed_old_path(file.old_path.as_deref().unwrap_or(&file.path), options)
    } else {
        "/dev/null".to_owned()
    };
    let new_path = if file.new_object_id.is_some() {
        prefixed_new_path(&file.path, options)
    } else {
        "/dev/null".to_owned()
    };
    format!("Binary files {old_path} and {new_path} differ\n")
}

/// Per-file line statistics.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiffFileStat {
    /// Git name-status code for this path.
    pub status: char,
    /// Previous repository-relative path for renames and copies.
    pub old_path: Option<String>,
    /// Repository-relative path using `/` separators.
    pub path: String,
    /// Similarity percentage for rename and copy entries.
    pub similarity_score: Option<u8>,
    /// Added line count.
    pub insertions: usize,
    /// Deleted line count.
    pub deletions: usize,
    /// Whether the file was treated as binary for diff accounting.
    pub binary: bool,
    /// Old file size in bytes, used for binary stat output.
    pub old_size: usize,
    /// New file size in bytes, used for binary stat output.
    pub new_size: usize,
}

impl DiffFileStat {
    fn changed_lines(&self) -> usize {
        self.insertions + self.deletions
    }

    fn display_path(&self) -> String {
        if self.status == 'R' || self.status == 'C' {
            format!(
                "{} => {}",
                self.old_path.as_deref().unwrap_or(&self.path),
                self.path
            )
        } else {
            self.path.clone()
        }
    }

    fn compact_display_path(&self) -> String {
        match self.status {
            'A' => format!("{} (new)", self.display_path()),
            'D' => format!("{} (gone)", self.display_path()),
            _ => self.display_path(),
        }
    }
}

/// Options for repository diff calculations.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiffOptions {
    /// Detect renames between deleted and added paths.
    pub find_renames: bool,
    /// Detect copies from modified paths to added paths.
    pub find_copies: bool,
    /// Also consider unchanged HEAD paths as copy sources.
    pub find_copies_harder: bool,
    /// Whether rename/copy behavior came from an explicit CLI option.
    pub rename_detection_explicit: bool,
    /// Minimum similarity percentage for rename detection.
    pub rename_similarity_threshold: u32,
    /// Minimum similarity percentage for copy detection.
    pub copy_similarity_threshold: u32,
    /// Optional cap for rename/copy candidate paths. `0` means unlimited.
    pub rename_limit: Option<usize>,
}

impl DiffOptions {
    /// Returns options that follow Git's default rename/copy threshold.
    pub fn new() -> Self {
        Self {
            find_renames: false,
            find_copies: false,
            find_copies_harder: false,
            rename_detection_explicit: false,
            rename_similarity_threshold: 50,
            copy_similarity_threshold: 50,
            rename_limit: None,
        }
    }
}

impl Default for DiffOptions {
    fn default() -> Self {
        Self::new()
    }
}

impl Repository {
    /// Computes default `git diff` scope: working tree compared with the index.
    pub fn diff_worktree_to_index(&self) -> Result<DiffSummary> {
        self.diff_worktree_to_index_with_pathspecs(&PathspecSet::all())
    }

    /// Computes default `git diff` scope for matching paths only.
    pub fn diff_worktree_to_index_with_pathspecs(
        &self,
        pathspecs: &PathspecSet,
    ) -> Result<DiffSummary> {
        self.diff_worktree_to_index_with_options(pathspecs, &DiffOptions::default())
    }

    /// Computes default `git diff` scope with explicit diff options.
    pub fn diff_worktree_to_index_with_options(
        &self,
        pathspecs: &PathspecSet,
        options: &DiffOptions,
    ) -> Result<DiffSummary> {
        let patch = self.diff_worktree_to_index_patch_with_options(pathspecs, options)?;
        patch_files_to_summary(&patch)
    }

    /// Computes default `git diff` patch output.
    pub fn diff_worktree_to_index_patch_with_pathspecs(
        &self,
        pathspecs: &PathspecSet,
    ) -> Result<DiffPatch> {
        self.diff_worktree_to_index_patch_with_options(pathspecs, &DiffOptions::default())
    }

    /// Computes default `git diff` patch output with explicit diff options.
    pub fn diff_worktree_to_index_patch_with_options(
        &self,
        pathspecs: &PathspecSet,
        options: &DiffOptions,
    ) -> Result<DiffPatch> {
        let Some(worktree) = self.worktree() else {
            return Err(RitError::invalid_input(
                "diff must be run in a repository with a working tree",
            ));
        };
        let index = Index::read(&self.git_dir().join("index"))?;
        let attributes = self.root_attributes()?;
        let options = self.diff_options_with_config(options)?;
        let mut files = Vec::new();
        let mut copy_sources = Vec::new();

        for entry in index.entries.iter().filter(|entry| entry.stage == 0) {
            if !pathspecs.matches_with_attributes(&entry.path, Some(&attributes)) {
                continue;
            }
            let worktree_path = join_slash_path(worktree, &entry.path);
            let old_object = if entry.is_intent_to_add() {
                None
            } else {
                let object = self.read_blob(entry.object_id)?;
                Some(object)
            };

            if !worktree_path.exists() {
                if let Some(old_object) = old_object {
                    files.push(DiffPatchFile {
                        status: 'D',
                        old_path: None,
                        path: entry.path.clone(),
                        similarity_score: None,
                        old_object_id: Some(entry.object_id),
                        new_object_id: None,
                        mode: entry.mode,
                        old_data: old_object.data,
                        new_data: Vec::new(),
                    });
                }
                continue;
            }

            let new_data = read_worktree_entry_data(&worktree_path, entry.mode)?;
            let new_object_id = hash_object(ObjectKind::Blob, &new_data);
            if let Some(old_object) = old_object {
                if new_object_id == entry.object_id {
                    if options.find_copies_harder {
                        copy_sources.push(CopySource {
                            path: entry.path.clone(),
                            mode: entry.mode,
                            object_id: Some(entry.object_id),
                            data: old_object.data,
                            changed: false,
                        });
                    }
                    continue;
                }

                copy_sources.push(CopySource {
                    path: entry.path.clone(),
                    mode: entry.mode,
                    object_id: Some(entry.object_id),
                    data: old_object.data.clone(),
                    changed: true,
                });
                files.push(DiffPatchFile {
                    status: 'M',
                    old_path: None,
                    path: entry.path.clone(),
                    similarity_score: None,
                    old_object_id: Some(entry.object_id),
                    new_object_id: Some(new_object_id),
                    mode: entry.mode,
                    old_data: old_object.data,
                    new_data,
                });
                continue;
            }

            files.push(DiffPatchFile {
                status: 'A',
                old_path: None,
                path: entry.path.clone(),
                similarity_score: None,
                old_object_id: None,
                new_object_id: Some(new_object_id),
                mode: entry.mode,
                old_data: Vec::new(),
                new_data,
            });
        }

        let mut warnings = Vec::new();
        if options.find_renames {
            warnings.extend(detect_patch_renames(&mut files, &options)?);
        }
        if options.find_copies {
            warnings.extend(detect_patch_copies_from_sources(
                &mut files,
                &copy_sources,
                &options,
            )?);
        }

        Ok(DiffPatch { files, warnings })
    }

    /// Computes `git diff --cached` scope: index compared with `HEAD`.
    pub fn diff_index_to_head(&self) -> Result<DiffSummary> {
        self.diff_index_to_head_with_pathspecs(&PathspecSet::all())
    }

    /// Computes `git diff --cached` scope for matching paths only.
    pub fn diff_index_to_head_with_pathspecs(
        &self,
        pathspecs: &PathspecSet,
    ) -> Result<DiffSummary> {
        self.diff_index_to_head_with_options(pathspecs, &DiffOptions::default())
    }

    /// Computes `git diff --cached` with explicit diff options.
    pub fn diff_index_to_head_with_options(
        &self,
        pathspecs: &PathspecSet,
        options: &DiffOptions,
    ) -> Result<DiffSummary> {
        let index = Index::read(&self.git_dir().join("index"))?;
        let index_entries = index
            .entries
            .iter()
            .map(|entry| {
                (
                    entry.path.clone(),
                    DiffTreeEntry {
                        object_id: entry.object_id,
                        mode: entry.mode,
                    },
                )
            })
            .collect::<BTreeMap<_, _>>();
        let head_entries = self.head_diff_entries()?;
        let attributes = self.root_attributes()?;
        let options = self.diff_options_with_config(options)?;
        let paths = index_entries
            .keys()
            .chain(head_entries.keys())
            .cloned()
            .collect::<BTreeSet<_>>();
        let mut files = Vec::new();

        for path in paths {
            if !pathspecs.matches_with_attributes(&path, Some(&attributes)) {
                continue;
            }
            match (head_entries.get(&path), index_entries.get(&path)) {
                (None, Some(new_entry)) => {
                    let new_object = self.read_blob(new_entry.object_id)?;
                    files.push(DiffFileStat {
                        status: 'A',
                        old_path: None,
                        path,
                        similarity_score: None,
                        insertions: count_lines(&new_object.data),
                        deletions: 0,
                        binary: is_binary_data(&new_object.data),
                        old_size: 0,
                        new_size: new_object.data.len(),
                    });
                }
                (Some(old_entry), None) => {
                    let old_object = self.read_blob(old_entry.object_id)?;
                    files.push(DiffFileStat {
                        status: 'D',
                        old_path: None,
                        path,
                        similarity_score: None,
                        insertions: 0,
                        deletions: count_lines(&old_object.data),
                        binary: is_binary_data(&old_object.data),
                        old_size: old_object.data.len(),
                        new_size: 0,
                    });
                }
                (Some(old_entry), Some(new_entry)) if old_entry != new_entry => {
                    let old_object = self.read_blob(old_entry.object_id)?;
                    let new_object = self.read_blob(new_entry.object_id)?;
                    let (insertions, deletions, binary) =
                        file_delta(&old_object.data, &new_object.data)?;
                    files.push(DiffFileStat {
                        status: 'M',
                        old_path: None,
                        path,
                        similarity_score: None,
                        insertions,
                        deletions,
                        binary,
                        old_size: old_object.data.len(),
                        new_size: new_object.data.len(),
                    });
                }
                _ => {}
            }
        }

        let mut warnings = Vec::new();
        if options.find_renames {
            warnings.extend(self.detect_summary_renames(
                &mut files,
                &head_entries,
                &index_entries,
                &options,
            )?);
        }
        if options.find_copies {
            warnings.extend(self.detect_summary_copies(
                &mut files,
                &head_entries,
                &index_entries,
                &options,
            )?);
        }

        Ok(DiffSummary { files, warnings })
    }

    /// Computes a summary diff between two commit trees.
    pub fn diff_commits_with_pathspecs(
        &self,
        old_commit_id: ObjectId,
        new_commit_id: ObjectId,
        pathspecs: &PathspecSet,
    ) -> Result<DiffSummary> {
        self.diff_commits_with_options(
            old_commit_id,
            new_commit_id,
            pathspecs,
            &DiffOptions::default(),
        )
    }

    /// Computes a summary diff between two commit trees with explicit options.
    pub fn diff_commits_with_options(
        &self,
        old_commit_id: ObjectId,
        new_commit_id: ObjectId,
        pathspecs: &PathspecSet,
        options: &DiffOptions,
    ) -> Result<DiffSummary> {
        let old_entries = self.commit_diff_entries(old_commit_id)?;
        let new_entries = self.commit_diff_entries(new_commit_id)?;
        let attributes = self.root_attributes()?;
        let options = self.diff_options_with_config(options)?;
        let paths = old_entries
            .keys()
            .chain(new_entries.keys())
            .cloned()
            .collect::<BTreeSet<_>>();
        let mut files = Vec::new();

        for path in paths {
            if !pathspecs.matches_with_attributes(&path, Some(&attributes)) {
                continue;
            }
            match (old_entries.get(&path), new_entries.get(&path)) {
                (None, Some(new_entry)) => {
                    let new_object = self.read_blob(new_entry.object_id)?;
                    files.push(DiffFileStat {
                        status: 'A',
                        old_path: None,
                        path,
                        similarity_score: None,
                        insertions: count_lines(&new_object.data),
                        deletions: 0,
                        binary: is_binary_data(&new_object.data),
                        old_size: 0,
                        new_size: new_object.data.len(),
                    });
                }
                (Some(old_entry), None) => {
                    let old_object = self.read_blob(old_entry.object_id)?;
                    files.push(DiffFileStat {
                        status: 'D',
                        old_path: None,
                        path,
                        similarity_score: None,
                        insertions: 0,
                        deletions: count_lines(&old_object.data),
                        binary: is_binary_data(&old_object.data),
                        old_size: old_object.data.len(),
                        new_size: 0,
                    });
                }
                (Some(old_entry), Some(new_entry)) if old_entry != new_entry => {
                    let old_object = self.read_blob(old_entry.object_id)?;
                    let new_object = self.read_blob(new_entry.object_id)?;
                    let (insertions, deletions, binary) =
                        file_delta(&old_object.data, &new_object.data)?;
                    files.push(DiffFileStat {
                        status: 'M',
                        old_path: None,
                        path,
                        similarity_score: None,
                        insertions,
                        deletions,
                        binary,
                        old_size: old_object.data.len(),
                        new_size: new_object.data.len(),
                    });
                }
                _ => {}
            }
        }

        let mut warnings = Vec::new();
        if options.find_renames {
            warnings.extend(self.detect_summary_renames(
                &mut files,
                &old_entries,
                &new_entries,
                &options,
            )?);
        }
        if options.find_copies {
            warnings.extend(self.detect_summary_copies(
                &mut files,
                &old_entries,
                &new_entries,
                &options,
            )?);
        }

        Ok(DiffSummary { files, warnings })
    }

    /// Computes patch output between two commit trees.
    pub fn diff_commits_patch_with_pathspecs(
        &self,
        old_commit_id: ObjectId,
        new_commit_id: ObjectId,
        pathspecs: &PathspecSet,
    ) -> Result<DiffPatch> {
        self.diff_commits_patch_with_options(
            old_commit_id,
            new_commit_id,
            pathspecs,
            &DiffOptions::default(),
        )
    }

    /// Computes patch output between two commit trees with explicit options.
    pub fn diff_commits_patch_with_options(
        &self,
        old_commit_id: ObjectId,
        new_commit_id: ObjectId,
        pathspecs: &PathspecSet,
        options: &DiffOptions,
    ) -> Result<DiffPatch> {
        let old_entries = self.commit_diff_entries(old_commit_id)?;
        let new_entries = self.commit_diff_entries(new_commit_id)?;
        let attributes = self.root_attributes()?;
        let options = self.diff_options_with_config(options)?;
        let paths = old_entries
            .keys()
            .chain(new_entries.keys())
            .cloned()
            .collect::<BTreeSet<_>>();
        let mut files = Vec::new();

        for path in paths {
            if !pathspecs.matches_with_attributes(&path, Some(&attributes)) {
                continue;
            }
            match (old_entries.get(&path), new_entries.get(&path)) {
                (None, Some(new_entry)) => {
                    let new_object = self.read_blob(new_entry.object_id)?;
                    files.push(DiffPatchFile {
                        status: 'A',
                        old_path: None,
                        path,
                        similarity_score: None,
                        old_object_id: None,
                        new_object_id: Some(new_entry.object_id),
                        mode: new_entry.mode,
                        old_data: Vec::new(),
                        new_data: new_object.data,
                    });
                }
                (Some(old_entry), None) => {
                    let old_object = self.read_blob(old_entry.object_id)?;
                    files.push(DiffPatchFile {
                        status: 'D',
                        old_path: None,
                        path,
                        similarity_score: None,
                        old_object_id: Some(old_entry.object_id),
                        new_object_id: None,
                        mode: old_entry.mode,
                        old_data: old_object.data,
                        new_data: Vec::new(),
                    });
                }
                (Some(old_entry), Some(new_entry)) if old_entry != new_entry => {
                    let old_object = self.read_blob(old_entry.object_id)?;
                    let new_object = self.read_blob(new_entry.object_id)?;
                    files.push(DiffPatchFile {
                        status: 'M',
                        old_path: None,
                        path,
                        similarity_score: None,
                        old_object_id: Some(old_entry.object_id),
                        new_object_id: Some(new_entry.object_id),
                        mode: new_entry.mode,
                        old_data: old_object.data,
                        new_data: new_object.data,
                    });
                }
                _ => {}
            }
        }

        let mut warnings = Vec::new();
        if options.find_renames {
            warnings.extend(detect_patch_renames(&mut files, &options)?);
        }
        if options.find_copies {
            warnings.extend(self.detect_patch_copies(
                &mut files,
                &old_entries,
                &new_entries,
                &options,
            )?);
        }

        Ok(DiffPatch { files, warnings })
    }

    /// Computes `git diff --cached` patch output.
    pub fn diff_index_to_head_patch_with_pathspecs(
        &self,
        pathspecs: &PathspecSet,
    ) -> Result<DiffPatch> {
        self.diff_index_to_head_patch_with_options(pathspecs, &DiffOptions::default())
    }

    /// Computes `git diff --cached` patch output with explicit diff options.
    pub fn diff_index_to_head_patch_with_options(
        &self,
        pathspecs: &PathspecSet,
        options: &DiffOptions,
    ) -> Result<DiffPatch> {
        let index = Index::read(&self.git_dir().join("index"))?;
        let index_entries = index
            .entries
            .iter()
            .map(|entry| {
                (
                    entry.path.clone(),
                    DiffTreeEntry {
                        object_id: entry.object_id,
                        mode: entry.mode,
                    },
                )
            })
            .collect::<BTreeMap<_, _>>();
        let head_entries = self.head_diff_entries()?;
        let attributes = self.root_attributes()?;
        let options = self.diff_options_with_config(options)?;
        let paths = index_entries
            .keys()
            .chain(head_entries.keys())
            .cloned()
            .collect::<BTreeSet<_>>();
        let mut files = Vec::new();

        for path in paths {
            if !pathspecs.matches_with_attributes(&path, Some(&attributes)) {
                continue;
            }
            match (head_entries.get(&path), index_entries.get(&path)) {
                (None, Some(new_entry)) => {
                    let new_object = self.read_blob(new_entry.object_id)?;
                    files.push(DiffPatchFile {
                        status: 'A',
                        old_path: None,
                        path,
                        similarity_score: None,
                        old_object_id: None,
                        new_object_id: Some(new_entry.object_id),
                        mode: new_entry.mode,
                        old_data: Vec::new(),
                        new_data: new_object.data,
                    });
                }
                (Some(old_entry), None) => {
                    let old_object = self.read_blob(old_entry.object_id)?;
                    files.push(DiffPatchFile {
                        status: 'D',
                        old_path: None,
                        path,
                        similarity_score: None,
                        old_object_id: Some(old_entry.object_id),
                        new_object_id: None,
                        mode: old_entry.mode,
                        old_data: old_object.data,
                        new_data: Vec::new(),
                    });
                }
                (Some(old_entry), Some(new_entry)) if old_entry != new_entry => {
                    let old_object = self.read_blob(old_entry.object_id)?;
                    let new_object = self.read_blob(new_entry.object_id)?;
                    files.push(DiffPatchFile {
                        status: 'M',
                        old_path: None,
                        path,
                        similarity_score: None,
                        old_object_id: Some(old_entry.object_id),
                        new_object_id: Some(new_entry.object_id),
                        mode: new_entry.mode,
                        old_data: old_object.data,
                        new_data: new_object.data,
                    });
                }
                _ => {}
            }
        }

        let mut warnings = Vec::new();
        if options.find_renames {
            warnings.extend(detect_patch_renames(&mut files, &options)?);
        }
        if options.find_copies {
            warnings.extend(self.detect_patch_copies(
                &mut files,
                &head_entries,
                &index_entries,
                &options,
            )?);
        }

        Ok(DiffPatch { files, warnings })
    }

    fn diff_options_with_config(&self, options: &DiffOptions) -> Result<DiffOptions> {
        let mut effective_options = options.clone();
        if !effective_options.rename_detection_explicit {
            let (find_renames, find_copies) = self.configured_diff_renames()?;
            effective_options.find_renames = find_renames;
            effective_options.find_copies = find_copies;
            effective_options.find_copies_harder = false;
        }
        if effective_options.rename_limit.is_none() {
            effective_options.rename_limit = self.configured_diff_rename_limit()?;
        }
        Ok(effective_options)
    }

    fn configured_diff_renames(&self) -> Result<(bool, bool)> {
        let config_path = self.common_dir().join("config");
        if !config_path.exists() {
            return Ok((true, false));
        }
        let config = GitConfig::read(&config_path)?;
        let Some(value) = config.get("diff", "renames") else {
            return Ok((true, false));
        };
        let normalized = value.trim().to_ascii_lowercase();
        match normalized.as_str() {
            "false" | "no" | "off" | "0" => Ok((false, false)),
            "copies" | "copy" => Ok((true, true)),
            _ => Ok((parse_git_bool_for_diff_renames(value)?, false)),
        }
    }

    fn configured_diff_rename_limit(&self) -> Result<Option<usize>> {
        let config_path = self.common_dir().join("config");
        if !config_path.exists() {
            return Ok(None);
        }
        let config = GitConfig::read(&config_path)?;
        let Some(value) = config.get("diff", "renamelimit") else {
            return Ok(None);
        };
        let limit = value.parse::<usize>().map_err(|_| {
            RitError::invalid_input(format!(
                "bad numeric config value '{value}' for 'diff.renamelimit' in file .git/config: invalid unit"
            ))
        })?;
        Ok(Some(limit))
    }

    fn read_blob(&self, object_id: ObjectId) -> Result<crate::GitObject> {
        let object = self.read_object(object_id)?;
        if object.kind != ObjectKind::Blob {
            return Err(RitError::invalid_input(format!(
                "object {object_id} is {}, not blob",
                object.kind
            )));
        }
        Ok(object)
    }

    fn head_diff_entries(&self) -> Result<BTreeMap<String, DiffTreeEntry>> {
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
        let tree_id = parse_commit(&commit.data)?.tree;
        let mut entries = BTreeMap::new();
        self.collect_diff_tree_entries("", tree_id, &mut entries)?;
        Ok(entries)
    }

    fn commit_diff_entries(&self, commit_id: ObjectId) -> Result<BTreeMap<String, DiffTreeEntry>> {
        let commit = self.read_object(commit_id)?;
        if commit.kind != ObjectKind::Commit {
            return Err(RitError::invalid_input(format!(
                "object {commit_id} is {}, not commit",
                commit.kind
            )));
        }
        let tree_id = parse_commit(&commit.data)?.tree;
        let mut entries = BTreeMap::new();
        self.collect_diff_tree_entries("", tree_id, &mut entries)?;
        Ok(entries)
    }

    fn collect_diff_tree_entries(
        &self,
        prefix: &str,
        tree_id: ObjectId,
        output: &mut BTreeMap<String, DiffTreeEntry>,
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
                self.collect_diff_tree_entries(&path, entry.object_id, output)?;
            } else {
                output.insert(
                    path,
                    DiffTreeEntry {
                        object_id: entry.object_id,
                        mode: parse_tree_mode(&entry.mode)?,
                    },
                );
            }
        }

        Ok(())
    }

    fn detect_summary_renames(
        &self,
        files: &mut Vec<DiffFileStat>,
        old_entries: &BTreeMap<String, DiffTreeEntry>,
        new_entries: &BTreeMap<String, DiffTreeEntry>,
        options: &DiffOptions,
    ) -> Result<Vec<String>> {
        self.detect_exact_summary_renames(files, old_entries, new_entries)?;
        if let Some(candidate_count) =
            rename_limit_exceeded_count(files.iter().map(|file| file.status), options)
        {
            return Ok(rename_limit_warnings(candidate_count));
        }
        let mut remove_indexes = Vec::new();
        for delete_index in 0..files.len() {
            if files[delete_index].status != 'D' {
                continue;
            }
            let old_path = files[delete_index].path.clone();
            let Some(old_entry) = old_entries.get(&old_path) else {
                continue;
            };
            let old_object = self.read_blob(old_entry.object_id)?;
            let Some(match_candidate) = best_similarity_match(
                files,
                |file| file.status == 'A',
                |file| {
                    new_entries
                        .get(&file.path)
                        .is_some_and(|new_entry| new_entry.mode == old_entry.mode)
                },
                |file| {
                    let new_entry = new_entries
                        .get(&file.path)
                        .ok_or_else(|| RitError::invalid_input("added diff entry disappeared"))?;
                    let new_object = self.read_blob(new_entry.object_id)?;
                    similarity_score(&old_object.data, &new_object.data)
                },
                options.rename_similarity_threshold,
                delete_index,
            )?
            else {
                continue;
            };
            let add_index = match_candidate.index;
            let new_path = files[add_index].path.clone();
            let new_entry = new_entries
                .get(&new_path)
                .ok_or_else(|| RitError::invalid_input("added diff entry disappeared"))?;
            let new_object = self.read_blob(new_entry.object_id)?;
            let (insertions, deletions, binary) = file_delta(&old_object.data, &new_object.data)?;

            files[delete_index] = DiffFileStat {
                status: 'R',
                old_path: Some(old_path),
                path: new_path,
                similarity_score: Some(match_candidate.score),
                insertions,
                deletions,
                binary,
                old_size: old_object.data.len(),
                new_size: new_object.data.len(),
            };
            remove_indexes.push(add_index);
        }

        remove_indexes.sort_unstable();
        remove_indexes.dedup();
        for index in remove_indexes.into_iter().rev() {
            files.remove(index);
        }
        Ok(Vec::new())
    }

    fn detect_exact_summary_renames(
        &self,
        files: &mut Vec<DiffFileStat>,
        old_entries: &BTreeMap<String, DiffTreeEntry>,
        new_entries: &BTreeMap<String, DiffTreeEntry>,
    ) -> Result<()> {
        let mut remove_indexes = Vec::new();
        for delete_index in 0..files.len() {
            if files[delete_index].status != 'D' {
                continue;
            }
            let old_path = files[delete_index].path.clone();
            let Some(old_entry) = old_entries.get(&old_path) else {
                continue;
            };
            let Some(add_index) = files.iter().position(|file| {
                file.status == 'A'
                    && new_entries.get(&file.path).is_some_and(|new_entry| {
                        new_entry.mode == old_entry.mode
                            && new_entry.object_id == old_entry.object_id
                    })
            }) else {
                continue;
            };
            let new_path = files[add_index].path.clone();
            let old_object = self.read_blob(old_entry.object_id)?;
            let binary = is_binary_data(&old_object.data);
            files[delete_index] = DiffFileStat {
                status: 'R',
                old_path: Some(old_path),
                path: new_path,
                similarity_score: Some(100),
                insertions: 0,
                deletions: 0,
                binary,
                old_size: old_object.data.len(),
                new_size: old_object.data.len(),
            };
            remove_indexes.push(add_index);
        }

        remove_indexes.sort_unstable();
        remove_indexes.dedup();
        for index in remove_indexes.into_iter().rev() {
            files.remove(index);
        }
        Ok(())
    }

    fn detect_summary_copies(
        &self,
        files: &mut [DiffFileStat],
        old_entries: &BTreeMap<String, DiffTreeEntry>,
        new_entries: &BTreeMap<String, DiffTreeEntry>,
        options: &DiffOptions,
    ) -> Result<Vec<String>> {
        self.detect_exact_summary_copies(files, old_entries, new_entries, options)?;
        if !files.iter().any(|file| file.status == 'A') {
            return Ok(Vec::new());
        }
        if let Some(candidate_count) =
            rename_limit_exceeded_count(files.iter().map(|file| file.status), options)
        {
            return Ok(rename_limit_warnings(candidate_count));
        }
        for add_index in 0..files.len() {
            if files[add_index].status != 'A' {
                continue;
            }
            let new_path = files[add_index].path.clone();
            let Some(new_entry) = new_entries.get(&new_path) else {
                continue;
            };
            let new_object = self.read_blob(new_entry.object_id)?;
            let Some(match_candidate) = self.best_copy_match(
                files,
                old_entries,
                new_entry,
                &new_object.data,
                options,
                add_index,
            )?
            else {
                continue;
            };
            let old_path = match_candidate.path;
            let old_entry = old_entries
                .get(&old_path)
                .ok_or_else(|| RitError::invalid_input("copy source disappeared"))?;
            let old_object = self.read_blob(old_entry.object_id)?;
            let (insertions, deletions, binary) = file_delta(&old_object.data, &new_object.data)?;

            files[add_index] = DiffFileStat {
                status: 'C',
                old_path: Some(old_path),
                path: new_path,
                similarity_score: Some(match_candidate.score),
                insertions,
                deletions,
                binary,
                old_size: old_object.data.len(),
                new_size: new_object.data.len(),
            };
        }
        Ok(Vec::new())
    }

    fn detect_exact_summary_copies(
        &self,
        files: &mut [DiffFileStat],
        old_entries: &BTreeMap<String, DiffTreeEntry>,
        new_entries: &BTreeMap<String, DiffTreeEntry>,
        options: &DiffOptions,
    ) -> Result<()> {
        for add_index in 0..files.len() {
            if files[add_index].status != 'A' {
                continue;
            }
            let new_path = files[add_index].path.clone();
            let Some(new_entry) = new_entries.get(&new_path) else {
                continue;
            };
            let Some((old_path, old_entry)) = old_entries.iter().find(|(path, old_entry)| {
                old_entry.mode == new_entry.mode
                    && old_entry.object_id == new_entry.object_id
                    && copy_source_is_available(files, path.as_str(), options, add_index)
            }) else {
                continue;
            };
            let old_object = self.read_blob(old_entry.object_id)?;
            let binary = is_binary_data(&old_object.data);

            files[add_index] = DiffFileStat {
                status: 'C',
                old_path: Some(old_path.clone()),
                path: new_path,
                similarity_score: Some(100),
                insertions: 0,
                deletions: 0,
                binary,
                old_size: old_object.data.len(),
                new_size: old_object.data.len(),
            };
        }
        Ok(())
    }

    fn best_copy_match(
        &self,
        files: &[DiffFileStat],
        old_entries: &BTreeMap<String, DiffTreeEntry>,
        new_entry: &DiffTreeEntry,
        new_data: &[u8],
        options: &DiffOptions,
        excluded_index: usize,
    ) -> Result<Option<CopyMatch>> {
        let mut best = None;
        for (path, old_entry) in old_entries {
            if old_entry.mode != new_entry.mode {
                continue;
            }
            if !options.find_copies_harder {
                let Some(index) = files
                    .iter()
                    .position(|file| file.path == *path && file.status == 'M')
                else {
                    continue;
                };
                if index == excluded_index {
                    continue;
                }
            }
            let old_object = self.read_blob(old_entry.object_id)?;
            let candidate_score = similarity_score(&old_object.data, new_data)?;
            if u32::from(candidate_score) < options.copy_similarity_threshold {
                continue;
            }
            let should_replace = best
                .as_ref()
                .map(|best: &CopyMatch| candidate_score > best.score)
                .unwrap_or(true);
            if should_replace {
                best = Some(CopyMatch {
                    path: path.clone(),
                    score: candidate_score,
                });
            }
        }
        Ok(best)
    }

    fn detect_patch_copies(
        &self,
        files: &mut [DiffPatchFile],
        old_entries: &BTreeMap<String, DiffTreeEntry>,
        new_entries: &BTreeMap<String, DiffTreeEntry>,
        options: &DiffOptions,
    ) -> Result<Vec<String>> {
        self.detect_exact_patch_copies(files, old_entries, new_entries, options)?;
        if !files.iter().any(|file| file.status == 'A') {
            return Ok(Vec::new());
        }
        if let Some(candidate_count) =
            rename_limit_exceeded_count(files.iter().map(|file| file.status), options)
        {
            return Ok(rename_limit_warnings(candidate_count));
        }
        for add_index in 0..files.len() {
            if files[add_index].status != 'A' {
                continue;
            }
            let new_path = files[add_index].path.clone();
            let Some(new_entry) = new_entries.get(&new_path) else {
                continue;
            };
            let new_object_id = files[add_index].new_object_id;
            let new_data = files[add_index].new_data.clone();
            let Some(match_candidate) = self.best_copy_match(
                &patch_files_to_stats(files),
                old_entries,
                new_entry,
                &new_data,
                options,
                add_index,
            )?
            else {
                continue;
            };
            let old_entry = old_entries
                .get(&match_candidate.path)
                .ok_or_else(|| RitError::invalid_input("copy source disappeared"))?;
            let old_object = self.read_blob(old_entry.object_id)?;

            files[add_index] = DiffPatchFile {
                status: 'C',
                old_path: Some(match_candidate.path),
                path: new_path,
                similarity_score: Some(match_candidate.score),
                old_object_id: Some(old_entry.object_id),
                new_object_id,
                mode: new_entry.mode,
                old_data: old_object.data,
                new_data,
            };
        }
        Ok(Vec::new())
    }

    fn detect_exact_patch_copies(
        &self,
        files: &mut [DiffPatchFile],
        old_entries: &BTreeMap<String, DiffTreeEntry>,
        new_entries: &BTreeMap<String, DiffTreeEntry>,
        options: &DiffOptions,
    ) -> Result<()> {
        for add_index in 0..files.len() {
            if files[add_index].status != 'A' {
                continue;
            }
            let new_path = files[add_index].path.clone();
            let Some(new_entry) = new_entries.get(&new_path) else {
                continue;
            };
            let stats = patch_files_to_stats(files);
            let Some((old_path, old_entry)) = old_entries.iter().find(|(path, old_entry)| {
                old_entry.mode == new_entry.mode
                    && old_entry.object_id == new_entry.object_id
                    && copy_source_is_available(&stats, path.as_str(), options, add_index)
            }) else {
                continue;
            };
            let old_object = self.read_blob(old_entry.object_id)?;

            files[add_index] = DiffPatchFile {
                status: 'C',
                old_path: Some(old_path.clone()),
                path: new_path,
                similarity_score: Some(100),
                old_object_id: Some(old_entry.object_id),
                new_object_id: Some(new_entry.object_id),
                mode: new_entry.mode,
                old_data: old_object.data,
                new_data: files[add_index].new_data.clone(),
            };
        }
        Ok(())
    }
}

fn detect_patch_renames(
    files: &mut Vec<DiffPatchFile>,
    options: &DiffOptions,
) -> Result<Vec<String>> {
    detect_exact_patch_renames(files);
    if let Some(candidate_count) =
        rename_limit_exceeded_count(files.iter().map(|file| file.status), options)
    {
        return Ok(rename_limit_warnings(candidate_count));
    }
    let mut remove_indexes = Vec::new();
    for delete_index in 0..files.len() {
        if files[delete_index].status != 'D' {
            continue;
        }
        let old_path = files[delete_index].path.clone();
        let old_object_id = files[delete_index].old_object_id;
        let old_mode = files[delete_index].mode;
        let old_data = files[delete_index].old_data.clone();
        let Some(match_candidate) = best_similarity_match(
            files,
            |file| file.status == 'A' && file.mode == old_mode,
            |file| file.new_object_id.is_some(),
            |file| similarity_score(&old_data, &file.new_data),
            options.rename_similarity_threshold,
            delete_index,
        )?
        else {
            continue;
        };
        let add_index = match_candidate.index;
        let new_path = files[add_index].path.clone();
        let new_object_id = files[add_index].new_object_id;
        let new_data = files[add_index].new_data.clone();

        files[delete_index] = DiffPatchFile {
            status: 'R',
            old_path: Some(old_path),
            path: new_path,
            similarity_score: Some(match_candidate.score),
            old_object_id,
            new_object_id,
            mode: old_mode,
            old_data,
            new_data,
        };
        remove_indexes.push(add_index);
    }

    remove_indexes.sort_unstable();
    remove_indexes.dedup();
    for index in remove_indexes.into_iter().rev() {
        files.remove(index);
    }
    Ok(Vec::new())
}

fn detect_exact_patch_renames(files: &mut Vec<DiffPatchFile>) {
    let mut remove_indexes = Vec::new();
    for delete_index in 0..files.len() {
        if files[delete_index].status != 'D' {
            continue;
        }
        let old_path = files[delete_index].path.clone();
        let old_object_id = files[delete_index].old_object_id;
        let old_mode = files[delete_index].mode;
        let old_data = files[delete_index].old_data.clone();
        let Some(add_index) = files.iter().position(|file| {
            file.status == 'A' && file.mode == old_mode && file.new_object_id == old_object_id
        }) else {
            continue;
        };
        let new_path = files[add_index].path.clone();
        let new_object_id = files[add_index].new_object_id;
        let new_data = files[add_index].new_data.clone();

        files[delete_index] = DiffPatchFile {
            status: 'R',
            old_path: Some(old_path),
            path: new_path,
            similarity_score: Some(100),
            old_object_id,
            new_object_id,
            mode: old_mode,
            old_data,
            new_data,
        };
        remove_indexes.push(add_index);
    }

    remove_indexes.sort_unstable();
    remove_indexes.dedup();
    for index in remove_indexes.into_iter().rev() {
        files.remove(index);
    }
}

#[cfg(test)]
fn rename_limit_exceeded(statuses: impl Iterator<Item = char>, options: &DiffOptions) -> bool {
    rename_limit_exceeded_count(statuses, options).is_some()
}

fn rename_limit_exceeded_count(
    statuses: impl Iterator<Item = char>,
    options: &DiffOptions,
) -> Option<usize> {
    let limit = options.rename_limit?;
    if limit == 0 {
        return None;
    }
    let mut destination_count = 0;
    let mut source_count = 0;
    for status in statuses {
        match status {
            'A' => destination_count += 1,
            'D' | 'M' => source_count += 1,
            _ => {}
        }
    }
    let candidate_count = destination_count.max(source_count);
    (candidate_count > limit).then_some(candidate_count)
}

fn rename_limit_warnings(candidate_count: usize) -> Vec<String> {
    vec![
        "warning: exhaustive rename detection was skipped due to too many files.".to_owned(),
        format!(
            "warning: you may want to set your diff.renameLimit variable to at least {candidate_count} and retry the command."
        ),
    ]
}

fn parse_git_bool_for_diff_renames(value: &str) -> Result<bool> {
    match value.trim().to_ascii_lowercase().as_str() {
        "true" | "yes" | "on" | "1" | "" => Ok(true),
        "false" | "no" | "off" | "0" => Ok(false),
        _ => Err(RitError::invalid_input(format!(
            "bad boolean config value '{value}' for 'diff.renames'"
        ))),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SimilarityMatch {
    index: usize,
    score: u8,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CopyMatch {
    path: String,
    score: u8,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CopySource {
    path: String,
    mode: u32,
    object_id: Option<ObjectId>,
    data: Vec<u8>,
    changed: bool,
}

fn detect_patch_copies_from_sources(
    files: &mut [DiffPatchFile],
    copy_sources: &[CopySource],
    options: &DiffOptions,
) -> Result<Vec<String>> {
    detect_exact_patch_copies_from_sources(files, copy_sources, options);
    if !files.iter().any(|file| file.status == 'A') {
        return Ok(Vec::new());
    }
    if let Some(candidate_count) =
        rename_limit_exceeded_count(files.iter().map(|file| file.status), options)
    {
        return Ok(rename_limit_warnings(candidate_count));
    }
    for file in files.iter_mut() {
        if file.status != 'A' {
            continue;
        }
        let new_data = file.new_data.clone();
        let Some(match_candidate) =
            best_copy_match_from_sources(copy_sources, file.mode, &new_data, options)?
        else {
            continue;
        };

        file.status = 'C';
        file.old_path = Some(match_candidate.path.clone());
        file.similarity_score = Some(match_candidate.score);
        file.old_object_id = match_candidate.object_id;
        file.old_data = match_candidate.data;
    }
    Ok(Vec::new())
}

fn detect_exact_patch_copies_from_sources(
    files: &mut [DiffPatchFile],
    copy_sources: &[CopySource],
    options: &DiffOptions,
) {
    for file in files.iter_mut() {
        if file.status != 'A' {
            continue;
        }
        let Some(source) = copy_sources.iter().find(|source| {
            source.mode == file.mode
                && source.data == file.new_data
                && (options.find_copies_harder || source.changed)
        }) else {
            continue;
        };
        file.status = 'C';
        file.old_path = Some(source.path.clone());
        file.similarity_score = Some(100);
        file.old_object_id = source.object_id;
        file.old_data = source.data.clone();
    }
}

fn best_copy_match_from_sources(
    copy_sources: &[CopySource],
    new_mode: u32,
    new_data: &[u8],
    options: &DiffOptions,
) -> Result<Option<CopySourceMatch>> {
    let mut best = None;
    for source in copy_sources {
        if source.mode != new_mode || (!options.find_copies_harder && !source.changed) {
            continue;
        }
        let candidate_score = similarity_score(&source.data, new_data)?;
        if u32::from(candidate_score) < options.copy_similarity_threshold {
            continue;
        }
        let should_replace = best
            .as_ref()
            .map(|best: &CopySourceMatch| candidate_score > best.score)
            .unwrap_or(true);
        if should_replace {
            best = Some(CopySourceMatch {
                path: source.path.clone(),
                score: candidate_score,
                object_id: source.object_id,
                data: source.data.clone(),
            });
        }
    }
    Ok(best)
}

fn copy_source_is_available(
    files: &[DiffFileStat],
    path: &str,
    options: &DiffOptions,
    excluded_index: usize,
) -> bool {
    if options.find_copies_harder {
        return true;
    }
    files
        .iter()
        .enumerate()
        .any(|(index, file)| index != excluded_index && file.path == path && file.status == 'M')
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CopySourceMatch {
    path: String,
    score: u8,
    object_id: Option<ObjectId>,
    data: Vec<u8>,
}

fn patch_files_to_summary(patch: &DiffPatch) -> Result<DiffSummary> {
    let mut summary_files = Vec::with_capacity(patch.files.len());
    for file in &patch.files {
        let (insertions, deletions, binary) = match file.status {
            'A' => (
                count_lines(&file.new_data),
                0,
                is_binary_data(&file.new_data),
            ),
            'D' => (
                0,
                count_lines(&file.old_data),
                is_binary_data(&file.old_data),
            ),
            _ => file_delta(&file.old_data, &file.new_data)?,
        };
        summary_files.push(DiffFileStat {
            status: file.status,
            old_path: file.old_path.clone(),
            path: file.path.clone(),
            similarity_score: file.similarity_score,
            insertions,
            deletions,
            binary,
            old_size: file.old_data.len(),
            new_size: file.new_data.len(),
        });
    }
    Ok(DiffSummary {
        files: summary_files,
        warnings: patch.warnings.clone(),
    })
}

fn patch_files_to_stats(files: &[DiffPatchFile]) -> Vec<DiffFileStat> {
    files
        .iter()
        .map(|file| DiffFileStat {
            status: file.status,
            old_path: file.old_path.clone(),
            path: file.path.clone(),
            similarity_score: file.similarity_score,
            insertions: 0,
            deletions: 0,
            binary: is_binary_data(&file.old_data) || is_binary_data(&file.new_data),
            old_size: file.old_data.len(),
            new_size: file.new_data.len(),
        })
        .collect()
}

fn best_similarity_match<T>(
    files: &[T],
    mut is_candidate: impl FnMut(&T) -> bool,
    mut can_compare: impl FnMut(&T) -> bool,
    mut score: impl FnMut(&T) -> Result<u8>,
    threshold: u32,
    excluded_index: usize,
) -> Result<Option<SimilarityMatch>> {
    let mut best = None;
    for (index, file) in files.iter().enumerate() {
        if index == excluded_index || !is_candidate(file) || !can_compare(file) {
            continue;
        }
        let candidate_score = score(file)?;
        if u32::from(candidate_score) < threshold {
            continue;
        }
        let should_replace = best
            .map(|best: SimilarityMatch| candidate_score > best.score)
            .unwrap_or(true);
        if should_replace {
            best = Some(SimilarityMatch {
                index,
                score: candidate_score,
            });
        }
    }
    Ok(best)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct DiffTreeEntry {
    object_id: ObjectId,
    mode: u32,
}

fn parse_tree_mode(mode: &str) -> Result<u32> {
    u32::from_str_radix(mode, 8)
        .map_err(|_| RitError::invalid_input(format!("invalid tree mode: {mode}")))
}

#[cfg(test)]
fn unified_hunk(old_data: &[u8], new_data: &[u8]) -> Result<String> {
    unified_hunk_with_context(old_data, new_data, &PatchRenderOptions::default())
}

fn unified_hunk_with_context(
    old_data: &[u8],
    new_data: &[u8],
    options: &PatchRenderOptions,
) -> Result<String> {
    let old_text = std::str::from_utf8(old_data)
        .map_err(|_| RitError::invalid_input("binary patch output is not implemented"))?;
    let new_text = std::str::from_utf8(new_data)
        .map_err(|_| RitError::invalid_input("binary patch output is not implemented"))?;
    let old_lines = split_lines_like_git(old_text);
    let new_lines = split_lines_like_git(new_text);
    let operations = line_operations(&old_lines, &new_lines);
    if operations.is_empty() {
        return Ok(String::new());
    }

    let mut output = String::new();
    for hunk in split_hunks(
        &operations,
        options.context_lines,
        options.inter_hunk_context,
    ) {
        let old_before = count_old_lines(&operations[..hunk.start]);
        let new_before = count_new_lines(&operations[..hunk.start]);
        let old_count = count_old_lines(&operations[hunk.start..hunk.end]);
        let new_count = count_new_lines(&operations[hunk.start..hunk.end]);
        output.push_str(&format!(
            "@@ -{} +{} @@{}\n",
            hunk_range(hunk_start(old_before, old_count), old_count),
            hunk_range(hunk_start(new_before, new_count), new_count),
            hunk_header_suffix(&operations, hunk.start)
        ));
        for operation in &operations[hunk.start..hunk.end] {
            match operation {
                LineOperation::Context(line) => {
                    push_patch_line(&mut output, options.context_line_indicator, line)
                }
                LineOperation::Delete(line) => {
                    push_patch_line(&mut output, options.old_line_indicator, line)
                }
                LineOperation::Insert(line) => {
                    push_patch_line(&mut output, options.new_line_indicator, line)
                }
            }
        }
    }
    Ok(output)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct HunkRange {
    start: usize,
    end: usize,
}

fn split_hunks(
    operations: &[LineOperation<'_>],
    context_lines: usize,
    inter_hunk_context: usize,
) -> Vec<HunkRange> {
    let mut hunks = Vec::new();
    let mut current: Option<HunkRange> = None;

    for (index, operation) in operations.iter().enumerate() {
        if matches!(operation, LineOperation::Context(_)) {
            continue;
        }

        let start = index.saturating_sub(context_lines);
        let end = (index + context_lines + 1).min(operations.len());
        match &mut current {
            Some(range) if start <= range.end + inter_hunk_context => {
                range.end = range.end.max(end)
            }
            Some(range) => {
                hunks.push(*range);
                current = Some(HunkRange { start, end });
            }
            None => current = Some(HunkRange { start, end }),
        }
    }

    if let Some(range) = current {
        hunks.push(range);
    }
    hunks
}

fn count_old_lines(operations: &[LineOperation<'_>]) -> usize {
    operations
        .iter()
        .filter(|operation| !matches!(operation, LineOperation::Insert(_)))
        .count()
}

fn count_new_lines(operations: &[LineOperation<'_>]) -> usize {
    operations
        .iter()
        .filter(|operation| !matches!(operation, LineOperation::Delete(_)))
        .count()
}

fn hunk_start(lines_before: usize, line_count: usize) -> usize {
    if line_count == 0 {
        lines_before
    } else {
        lines_before + 1
    }
}

fn hunk_header_suffix(operations: &[LineOperation<'_>], hunk_start: usize) -> String {
    if hunk_start == 0 {
        return String::new();
    }
    match operations.get(hunk_start - 1) {
        Some(LineOperation::Context(line)) => {
            let trimmed = line.trim_end_matches('\n');
            if trimmed.is_empty() {
                String::new()
            } else {
                format!(" {trimmed}")
            }
        }
        _ => String::new(),
    }
}

fn push_patch_line(output: &mut String, prefix: Option<char>, line: &str) {
    if let Some(prefix) = prefix {
        output.push(prefix);
    }
    output.push_str(line);
    if !line.ends_with('\n') {
        output.push('\n');
        output.push_str("\\ No newline at end of file\n");
    }
}

fn line_delta(old_data: &[u8], new_data: &[u8]) -> Result<(usize, usize)> {
    let old_text = std::str::from_utf8(old_data)
        .map_err(|_| RitError::invalid_input("binary diff stat is not implemented"))?;
    let new_text = std::str::from_utf8(new_data)
        .map_err(|_| RitError::invalid_input("binary diff stat is not implemented"))?;
    let old_lines = split_lines_like_git(old_text);
    let new_lines = split_lines_like_git(new_text);
    let common = longest_common_subsequence_len(&old_lines, &new_lines);
    Ok((new_lines.len() - common, old_lines.len() - common))
}

fn file_delta(old_data: &[u8], new_data: &[u8]) -> Result<(usize, usize, bool)> {
    if is_binary_data(old_data) || is_binary_data(new_data) {
        return Ok((0, 0, true));
    }
    let (insertions, deletions) = line_delta(old_data, new_data)?;
    Ok((insertions, deletions, false))
}

fn similarity_score(old_data: &[u8], new_data: &[u8]) -> Result<u8> {
    if old_data == new_data {
        return Ok(100);
    }
    if old_data.is_empty()
        || new_data.is_empty()
        || is_binary_data(old_data)
        || is_binary_data(new_data)
    {
        return Ok(0);
    }

    let old_text = std::str::from_utf8(old_data)
        .map_err(|_| RitError::invalid_input("binary similarity scoring is not implemented"))?;
    let new_text = std::str::from_utf8(new_data)
        .map_err(|_| RitError::invalid_input("binary similarity scoring is not implemented"))?;
    let old_lines = split_lines_like_git(old_text);
    let new_lines = split_lines_like_git(new_text);
    let operations = line_operations(&old_lines, &new_lines);
    let common_bytes = operations
        .iter()
        .filter_map(|operation| match operation {
            LineOperation::Context(line) => Some(line.len()),
            _ => None,
        })
        .sum::<usize>();
    let maximum_size = old_data.len().max(new_data.len());
    Ok(((common_bytes * 100) / maximum_size).min(100) as u8)
}

fn is_binary_data(data: &[u8]) -> bool {
    data.contains(&0) || std::str::from_utf8(data).is_err()
}

fn read_worktree_entry_data(path: &std::path::Path, index_mode: u32) -> Result<Vec<u8>> {
    if index_mode == 0o120000 {
        let metadata = fs::symlink_metadata(path).map_err(|source| RitError::io(path, source))?;
        if metadata.file_type().is_symlink() {
            let target = fs::read_link(path).map_err(|source| RitError::io(path, source))?;
            return Ok(target.to_string_lossy().replace('\\', "/").into_bytes());
        }
    }
    fs::read(path).map_err(|source| RitError::io(path, source))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum LineOperation<'a> {
    Context(&'a str),
    Delete(&'a str),
    Insert(&'a str),
}

fn line_operations<'a>(old_lines: &[&'a str], new_lines: &[&'a str]) -> Vec<LineOperation<'a>> {
    let mut table = vec![vec![0; new_lines.len() + 1]; old_lines.len() + 1];
    for old_index in (0..old_lines.len()).rev() {
        for new_index in (0..new_lines.len()).rev() {
            table[old_index][new_index] = if old_lines[old_index] == new_lines[new_index] {
                table[old_index + 1][new_index + 1] + 1
            } else {
                table[old_index + 1][new_index].max(table[old_index][new_index + 1])
            };
        }
    }

    let mut operations = Vec::new();
    let mut old_index = 0;
    let mut new_index = 0;
    while old_index < old_lines.len() && new_index < new_lines.len() {
        if old_lines[old_index] == new_lines[new_index] {
            operations.push(LineOperation::Context(old_lines[old_index]));
            old_index += 1;
            new_index += 1;
        } else if table[old_index + 1][new_index] >= table[old_index][new_index + 1] {
            operations.push(LineOperation::Delete(old_lines[old_index]));
            old_index += 1;
        } else {
            operations.push(LineOperation::Insert(new_lines[new_index]));
            new_index += 1;
        }
    }
    while old_index < old_lines.len() {
        operations.push(LineOperation::Delete(old_lines[old_index]));
        old_index += 1;
    }
    while new_index < new_lines.len() {
        operations.push(LineOperation::Insert(new_lines[new_index]));
        new_index += 1;
    }

    operations
}

fn hunk_range(start: usize, count: usize) -> String {
    if count == 0 {
        "0,0".to_owned()
    } else if count == 1 {
        start.to_string()
    } else {
        format!("{start},{count}")
    }
}

fn patch_object_id(
    object_id: Option<ObjectId>,
    peer: Option<ObjectId>,
    options: &PatchRenderOptions,
) -> String {
    let abbrev = options.abbrev.max(4);
    if let Some(object_id) = object_id {
        let hex = object_id.to_hex();
        if options.full_index {
            return hex;
        }
        return hex[..abbrev.min(hex.len())].to_owned();
    }

    let zero_length = if options.full_index {
        peer.map(|object_id| object_id.to_hex().len()).unwrap_or(40)
    } else {
        abbrev
    };
    "0".repeat(zero_length)
}

fn raw_object_id(object_id: Option<ObjectId>, options: &PatchRenderOptions) -> String {
    let abbrev = options.abbrev.max(4);
    object_id
        .map(|object_id| {
            let hex = object_id.to_hex();
            hex[..abbrev.min(hex.len())].to_owned()
        })
        .unwrap_or_else(|| "0".repeat(abbrev))
}

fn count_lines(data: &[u8]) -> usize {
    if is_binary_data(data) {
        return 0;
    }
    let Ok(text) = std::str::from_utf8(data) else {
        return 0;
    };
    split_lines_like_git(text).len()
}

fn split_lines_like_git(text: &str) -> Vec<&str> {
    if text.is_empty() {
        return Vec::new();
    }

    text.split_inclusive('\n').collect()
}

fn longest_common_subsequence_len(old_lines: &[&str], new_lines: &[&str]) -> usize {
    let mut previous = vec![0; new_lines.len() + 1];
    let mut current = vec![0; new_lines.len() + 1];

    for old_line in old_lines {
        for (new_index, new_line) in new_lines.iter().enumerate() {
            current[new_index + 1] = if old_line == new_line {
                previous[new_index] + 1
            } else {
                previous[new_index + 1].max(current[new_index])
            };
        }
        std::mem::swap(&mut previous, &mut current);
        current.fill(0);
    }

    previous[new_lines.len()]
}

fn plural(count: usize, singular: &str, plural: &str) -> String {
    if count == 1 {
        format!("{count} {singular}")
    } else {
        format!("{count} {plural}")
    }
}

#[cfg(test)]
mod tests {
    use super::{
        DiffFileStat, DiffOptions, DiffPatch, DiffPatchFile, DiffStatusFilter, DiffSummary,
        PatchRenderOptions, file_delta, line_delta, rename_limit_exceeded, similarity_score,
        unified_hunk,
    };
    use crate::{InitOptions, Repository};
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn line_delta_counts_insertions_and_deletions() {
        let (insertions, deletions) =
            line_delta(b"one\ntwo\n", b"one\nthree\n").expect("text diff should work");

        assert_eq!((insertions, deletions), (1, 1));
    }

    #[test]
    fn stat_text_matches_small_git_shape() {
        let summary = DiffSummary {
            files: vec![DiffFileStat {
                status: 'M',
                old_path: None,
                path: "a.txt".to_owned(),
                similarity_score: None,
                insertions: 1,
                deletions: 0,
                binary: false,
                old_size: 0,
                new_size: 0,
            }],
            warnings: Vec::new(),
        };

        assert_eq!(
            summary.to_stat_text(),
            " a.txt | 1 +\n 1 file changed, 1 insertion(+)\n"
        );
    }

    #[test]
    fn compact_stat_text_marks_added_and_deleted_paths() {
        let summary = DiffSummary {
            files: vec![
                DiffFileStat {
                    status: 'A',
                    old_path: None,
                    path: "added.txt".to_owned(),
                    similarity_score: None,
                    insertions: 1,
                    deletions: 0,
                    binary: false,
                    old_size: 0,
                    new_size: 0,
                },
                DiffFileStat {
                    status: 'D',
                    old_path: None,
                    path: "deleted.txt".to_owned(),
                    similarity_score: None,
                    insertions: 0,
                    deletions: 1,
                    binary: false,
                    old_size: 0,
                    new_size: 0,
                },
            ],
            warnings: Vec::new(),
        };

        assert_eq!(
            summary.to_compact_stat_text(),
            " added.txt (new)    | 1 +\n deleted.txt (gone) | 1 -\n 2 files changed, 1 insertion(+), 1 deletion(-)\n"
        );
    }

    #[test]
    fn diff_status_filter_includes_excludes_and_all_or_none() {
        let summary = DiffSummary {
            files: vec![
                DiffFileStat {
                    status: 'A',
                    old_path: None,
                    path: "added.txt".to_owned(),
                    similarity_score: None,
                    insertions: 1,
                    deletions: 0,
                    binary: false,
                    old_size: 0,
                    new_size: 0,
                },
                DiffFileStat {
                    status: 'D',
                    old_path: None,
                    path: "deleted.txt".to_owned(),
                    similarity_score: None,
                    insertions: 0,
                    deletions: 1,
                    binary: false,
                    old_size: 0,
                    new_size: 0,
                },
                DiffFileStat {
                    status: 'M',
                    old_path: None,
                    path: "modified.txt".to_owned(),
                    similarity_score: None,
                    insertions: 1,
                    deletions: 1,
                    binary: false,
                    old_size: 0,
                    new_size: 0,
                },
            ],
            warnings: Vec::new(),
        };

        let added_or_deleted = DiffStatusFilter::from_git_diff_filter("AD").expect("valid filter");
        assert_eq!(
            summary
                .clone()
                .into_filtered_by_status(&added_or_deleted)
                .name_only(),
            vec!["added.txt", "deleted.txt"]
        );

        let exclude_deleted = DiffStatusFilter::from_git_diff_filter("d").expect("valid filter");
        assert_eq!(
            summary
                .clone()
                .into_filtered_by_status(&exclude_deleted)
                .name_only(),
            vec!["added.txt", "modified.txt"]
        );

        let all_if_added = DiffStatusFilter::from_git_diff_filter("A*").expect("valid filter");
        assert_eq!(
            summary.into_filtered_by_status(&all_if_added).name_only(),
            vec!["added.txt", "deleted.txt", "modified.txt"]
        );
    }

    #[test]
    fn name_status_text_lists_status_and_path() {
        let summary = DiffSummary {
            files: vec![DiffFileStat {
                status: 'A',
                old_path: None,
                path: "a.txt".to_owned(),
                similarity_score: None,
                insertions: 1,
                deletions: 0,
                binary: false,
                old_size: 0,
                new_size: 0,
            }],
            warnings: Vec::new(),
        };

        assert_eq!(summary.to_name_status_text(), "A\ta.txt\n");
    }

    #[test]
    fn numstat_text_lists_insertions_deletions_and_path() {
        let summary = DiffSummary {
            files: vec![DiffFileStat {
                status: 'M',
                old_path: None,
                path: "a.txt".to_owned(),
                similarity_score: None,
                insertions: 2,
                deletions: 1,
                binary: false,
                old_size: 0,
                new_size: 0,
            }],
            warnings: Vec::new(),
        };

        assert_eq!(summary.to_numstat_text(), "2\t1\ta.txt\n");
    }

    #[test]
    fn name_status_text_lists_copy_similarity_and_paths() {
        let summary = DiffSummary {
            files: vec![DiffFileStat {
                status: 'C',
                old_path: Some("old.txt".to_owned()),
                path: "copy.txt".to_owned(),
                similarity_score: Some(79),
                insertions: 1,
                deletions: 1,
                binary: false,
                old_size: 24,
                new_size: 24,
            }],
            warnings: Vec::new(),
        };

        assert_eq!(summary.to_name_status_text(), "C079\told.txt\tcopy.txt\n");
    }

    #[test]
    fn nul_terminated_diff_summaries_match_git_field_shape() {
        let summary = DiffSummary {
            files: vec![
                DiffFileStat {
                    status: 'M',
                    old_path: None,
                    path: "a b.txt".to_owned(),
                    similarity_score: None,
                    insertions: 2,
                    deletions: 1,
                    binary: false,
                    old_size: 0,
                    new_size: 0,
                },
                DiffFileStat {
                    status: 'R',
                    old_path: Some("old.txt".to_owned()),
                    path: "new.txt".to_owned(),
                    similarity_score: Some(100),
                    insertions: 0,
                    deletions: 0,
                    binary: false,
                    old_size: 4,
                    new_size: 4,
                },
            ],
            warnings: Vec::new(),
        };

        assert_eq!(summary.to_name_only_z(), b"a b.txt\0new.txt\0");
        assert_eq!(
            summary.to_name_status_z(),
            b"M\0a b.txt\0R100\0old.txt\0new.txt\0"
        );
        assert_eq!(
            summary.to_numstat_z(),
            [
                b"2\t1\ta b.txt\0".as_slice(),
                b"0\t0\t\0old.txt\0new.txt\0".as_slice()
            ]
            .concat()
        );
    }

    #[test]
    fn rename_limit_counts_candidate_paths_and_zero_is_unlimited() {
        let options = DiffOptions {
            rename_limit: Some(1),
            ..DiffOptions::default()
        };

        assert!(rename_limit_exceeded(['A', 'D', 'M'].into_iter(), &options));
        assert!(!rename_limit_exceeded(['A', 'D'].into_iter(), &options));

        let unlimited = DiffOptions {
            rename_limit: Some(0),
            ..DiffOptions::default()
        };

        assert!(!rename_limit_exceeded(
            ['A', 'D', 'M'].into_iter(),
            &unlimited
        ));
    }

    #[test]
    fn binary_numstat_and_stat_match_small_git_shape() {
        let summary = DiffSummary {
            files: vec![DiffFileStat {
                status: 'M',
                old_path: None,
                path: "bin.dat".to_owned(),
                similarity_score: None,
                insertions: 0,
                deletions: 0,
                binary: true,
                old_size: 5,
                new_size: 7,
            }],
            warnings: Vec::new(),
        };

        assert_eq!(summary.to_numstat_text(), "-\t-\tbin.dat\n");
        assert_eq!(
            summary.to_stat_text(),
            " bin.dat | Bin 5 -> 7 bytes\n 1 file changed, 0 insertions(+), 0 deletions(-)\n"
        );
    }

    #[test]
    fn file_delta_treats_nul_bytes_as_binary() {
        let delta = file_delta(&[0, 1, 2], &[0, 1, 2, 3]).expect("binary delta should work");

        assert_eq!(delta, (0, 0, true));
    }

    #[test]
    fn similarity_score_counts_common_text_bytes() {
        let score = similarity_score(
            b"one\ntwo\nthree\nfour\nfive\n",
            b"one\ntwo\nthree\nfour\nsix\n",
        )
        .expect("text similarity should work");

        assert_eq!(score, 79);
    }

    #[test]
    fn unified_hunk_marks_missing_trailing_newlines() {
        let hunk = unified_hunk(b"one", b"two").expect("text patch should render");

        assert_eq!(
            hunk,
            "@@ -1 +1 @@\n-one\n\\ No newline at end of file\n+two\n\\ No newline at end of file\n"
        );
    }

    #[test]
    fn unified_hunk_splits_distant_changes() {
        let old = b"line1\nline2\nline3\nline4\nline5\nline6\nline7\nline8\nline9\nline10\nline11\nline12\n";
        let new = b"line1\nchanged2\nline3\nline4\nline5\nline6\nline7\nline8\nline9\nchanged10\nline11\nline12\n";

        let hunk = unified_hunk(old, new).expect("text patch should render");

        assert_eq!(
            hunk,
            "@@ -1,5 +1,5 @@\n line1\n-line2\n+changed2\n line3\n line4\n line5\n@@ -7,6 +7,6 @@ line6\n line7\n line8\n line9\n-line10\n+changed10\n line11\n line12\n"
        );
    }

    #[test]
    fn patch_render_options_control_unified_context_lines() {
        let patch = DiffPatch {
            files: vec![DiffPatchFile {
                status: 'M',
                old_path: None,
                path: "file.txt".to_owned(),
                similarity_score: None,
                old_object_id: Some(crate::hash_object(crate::ObjectKind::Blob, b"two\n")),
                new_object_id: Some(crate::hash_object(crate::ObjectKind::Blob, b"changed\n")),
                mode: 0o100644,
                old_data: b"one\ntwo\nthree\n".to_vec(),
                new_data: b"one\nchanged\nthree\n".to_vec(),
            }],
            warnings: Vec::new(),
        };

        let text = patch
            .to_patch_text_with_options(&PatchRenderOptions {
                context_lines: 0,
                ..PatchRenderOptions::default()
            })
            .expect("patch should render");

        assert!(text.contains("@@ -2 +2 @@ one\n-two\n+changed\n"));
        assert!(!text.contains("\n one\n"));
        assert!(!text.contains("\n three\n"));
    }

    #[test]
    fn patch_render_options_merge_nearby_hunks_with_inter_hunk_context() {
        let patch = DiffPatch {
            files: vec![DiffPatchFile {
                status: 'M',
                old_path: None,
                path: "file.txt".to_owned(),
                similarity_score: None,
                old_object_id: Some(crate::hash_object(crate::ObjectKind::Blob, b"old\n")),
                new_object_id: Some(crate::hash_object(crate::ObjectKind::Blob, b"new\n")),
                mode: 0o100644,
                old_data: b"one\ntwo\nthree\n".to_vec(),
                new_data: b"ONE\ntwo\nTHREE\n".to_vec(),
            }],
            warnings: Vec::new(),
        };

        let text = patch
            .to_patch_text_with_options(&PatchRenderOptions {
                context_lines: 0,
                inter_hunk_context: 1,
                ..PatchRenderOptions::default()
            })
            .expect("patch should render");

        assert!(text.contains("@@ -1,3 +1,3 @@\n-one\n+ONE\n two\n-three\n+THREE\n"));
        assert_eq!(text.matches("@@").count(), 2);
    }

    #[test]
    fn patch_render_options_can_omit_default_prefixes() {
        let patch = DiffPatch {
            files: vec![DiffPatchFile {
                status: 'M',
                old_path: None,
                path: "file.txt".to_owned(),
                similarity_score: None,
                old_object_id: Some(crate::hash_object(crate::ObjectKind::Blob, b"old\n")),
                new_object_id: Some(crate::hash_object(crate::ObjectKind::Blob, b"new\n")),
                mode: 0o100644,
                old_data: b"old\n".to_vec(),
                new_data: b"new\n".to_vec(),
            }],
            warnings: Vec::new(),
        };

        let text = patch
            .to_patch_text_with_options(&PatchRenderOptions {
                default_prefixes: false,
                ..PatchRenderOptions::default()
            })
            .expect("patch should render");

        assert!(text.contains("diff --git file.txt file.txt\n"));
        assert!(text.contains("--- file.txt\n+++ file.txt\n"));
        assert!(!text.contains("diff --git a/file.txt b/file.txt\n"));
    }

    #[test]
    fn patch_render_options_control_output_indicators() {
        let patch = DiffPatch {
            files: vec![DiffPatchFile {
                status: 'M',
                old_path: None,
                path: "file.txt".to_owned(),
                similarity_score: None,
                old_object_id: Some(crate::hash_object(crate::ObjectKind::Blob, b"two\n")),
                new_object_id: Some(crate::hash_object(crate::ObjectKind::Blob, b"changed\n")),
                mode: 0o100644,
                old_data: b"one\ntwo\nthree\n".to_vec(),
                new_data: b"one\nchanged\nthree\n".to_vec(),
            }],
            warnings: Vec::new(),
        };

        let text = patch
            .to_patch_text_with_options(&PatchRenderOptions {
                new_line_indicator: Some('>'),
                old_line_indicator: Some('<'),
                context_line_indicator: Some('.'),
                ..PatchRenderOptions::default()
            })
            .expect("patch should render");

        assert!(text.contains(".one\n<two\n>changed\n.three\n"));
    }

    #[test]
    fn binary_patch_output_uses_git_like_placeholder() {
        let patch = super::DiffPatch {
            files: vec![super::DiffPatchFile {
                status: 'M',
                old_path: None,
                path: "bin.dat".to_owned(),
                similarity_score: None,
                old_object_id: Some(crate::hash_object(crate::ObjectKind::Blob, &[0, 1])),
                new_object_id: Some(crate::hash_object(crate::ObjectKind::Blob, &[0, 1, 2])),
                mode: 0o100644,
                old_data: vec![0, 1],
                new_data: vec![0, 1, 2],
            }],
            warnings: Vec::new(),
        };

        let text = patch.to_patch_text().expect("binary patch should render");

        assert!(text.contains("Binary files a/bin.dat and b/bin.dat differ\n"));
    }

    #[test]
    fn patch_full_index_renders_complete_object_ids_and_zero_ids() {
        let old_id = crate::hash_object(crate::ObjectKind::Blob, b"old\n");
        let new_id = crate::hash_object(crate::ObjectKind::Blob, b"new\n");
        let patch = super::DiffPatch {
            files: vec![
                super::DiffPatchFile {
                    status: 'M',
                    old_path: None,
                    path: "tracked.txt".to_owned(),
                    similarity_score: None,
                    old_object_id: Some(old_id),
                    new_object_id: Some(new_id),
                    mode: 0o100644,
                    old_data: b"old\n".to_vec(),
                    new_data: b"new\n".to_vec(),
                },
                super::DiffPatchFile {
                    status: 'A',
                    old_path: None,
                    path: "new.txt".to_owned(),
                    similarity_score: None,
                    old_object_id: None,
                    new_object_id: Some(new_id),
                    mode: 0o100644,
                    old_data: Vec::new(),
                    new_data: b"new\n".to_vec(),
                },
            ],
            warnings: Vec::new(),
        };

        let text = patch
            .to_patch_text_with_options(&super::PatchRenderOptions {
                full_index: true,
                ..super::PatchRenderOptions::default()
            })
            .expect("full-index patch should render");

        assert!(text.contains(&format!("index {}..{} 100644\n", old_id, new_id)));
        assert!(text.contains(&format!(
            "index 0000000000000000000000000000000000000000..{}\n",
            new_id
        )));
    }

    #[test]
    fn patch_abbrev_renders_requested_object_id_length_with_git_minimum() {
        let old_id = crate::hash_object(crate::ObjectKind::Blob, b"old\n");
        let new_id = crate::hash_object(crate::ObjectKind::Blob, b"new\n");
        let patch = super::DiffPatch {
            files: vec![super::DiffPatchFile {
                status: 'M',
                old_path: None,
                path: "tracked.txt".to_owned(),
                similarity_score: None,
                old_object_id: Some(old_id),
                new_object_id: Some(new_id),
                mode: 0o100644,
                old_data: b"old\n".to_vec(),
                new_data: b"new\n".to_vec(),
            }],
            warnings: Vec::new(),
        };

        let text = patch
            .to_patch_text_with_options(&super::PatchRenderOptions {
                abbrev: 12,
                ..super::PatchRenderOptions::default()
            })
            .expect("abbreviated patch should render");
        assert!(text.contains(&format!(
            "index {}..{} 100644\n",
            &old_id.to_hex()[..12],
            &new_id.to_hex()[..12]
        )));

        let text = patch
            .to_patch_text_with_options(&super::PatchRenderOptions {
                abbrev: 1,
                ..super::PatchRenderOptions::default()
            })
            .expect("minimum abbreviated patch should render");
        assert!(text.contains(&format!(
            "index {}..{} 100644\n",
            &old_id.to_hex()[..4],
            &new_id.to_hex()[..4]
        )));
    }

    #[test]
    fn raw_patch_text_renders_git_like_records() {
        let old_id = crate::hash_object(crate::ObjectKind::Blob, b"old\n");
        let new_id = crate::hash_object(crate::ObjectKind::Blob, b"new\n");
        let patch = super::DiffPatch {
            files: vec![
                super::DiffPatchFile {
                    status: 'M',
                    old_path: None,
                    path: "tracked.txt".to_owned(),
                    similarity_score: None,
                    old_object_id: Some(old_id),
                    new_object_id: Some(new_id),
                    mode: 0o100644,
                    old_data: b"old\n".to_vec(),
                    new_data: b"new\n".to_vec(),
                },
                super::DiffPatchFile {
                    status: 'A',
                    old_path: None,
                    path: "new.txt".to_owned(),
                    similarity_score: None,
                    old_object_id: None,
                    new_object_id: Some(new_id),
                    mode: 0o100644,
                    old_data: Vec::new(),
                    new_data: b"new\n".to_vec(),
                },
            ],
            warnings: Vec::new(),
        };

        let text = patch.to_raw_text_with_options(&super::PatchRenderOptions::default());

        assert!(text.contains(&format!(
            ":100644 100644 {} {} M\ttracked.txt\n",
            &old_id.to_hex()[..7],
            &new_id.to_hex()[..7]
        )));
        assert!(text.contains(&format!(
            ":000000 100644 0000000 {} A\tnew.txt\n",
            &new_id.to_hex()[..7]
        )));
    }

    #[test]
    fn summary_patch_text_renders_extended_change_records() {
        let old_id = crate::hash_object(crate::ObjectKind::Blob, b"old\n");
        let new_id = crate::hash_object(crate::ObjectKind::Blob, b"new\n");
        let patch = super::DiffPatch {
            files: vec![
                super::DiffPatchFile {
                    status: 'M',
                    old_path: None,
                    path: "modified.txt".to_owned(),
                    similarity_score: None,
                    old_object_id: Some(old_id),
                    new_object_id: Some(new_id),
                    mode: 0o100644,
                    old_data: b"old\n".to_vec(),
                    new_data: b"new\n".to_vec(),
                },
                super::DiffPatchFile {
                    status: 'A',
                    old_path: None,
                    path: "new.txt".to_owned(),
                    similarity_score: None,
                    old_object_id: None,
                    new_object_id: Some(new_id),
                    mode: 0o100644,
                    old_data: Vec::new(),
                    new_data: b"new\n".to_vec(),
                },
                super::DiffPatchFile {
                    status: 'D',
                    old_path: None,
                    path: "deleted.txt".to_owned(),
                    similarity_score: None,
                    old_object_id: Some(old_id),
                    new_object_id: None,
                    mode: 0o100644,
                    old_data: b"old\n".to_vec(),
                    new_data: Vec::new(),
                },
                super::DiffPatchFile {
                    status: 'R',
                    old_path: Some("old.txt".to_owned()),
                    path: "renamed.txt".to_owned(),
                    similarity_score: Some(100),
                    old_object_id: Some(old_id),
                    new_object_id: Some(old_id),
                    mode: 0o100644,
                    old_data: b"old\n".to_vec(),
                    new_data: b"old\n".to_vec(),
                },
            ],
            warnings: Vec::new(),
        };

        assert_eq!(
            patch.to_summary_text(),
            " create mode 100644 new.txt\n delete mode 100644 deleted.txt\n rename old.txt => renamed.txt (100%)\n"
        );
    }

    #[test]
    fn cached_diff_reports_staged_changes_against_head() {
        let temp = temp_path("cached-diff");
        let repository = Repository::init(&InitOptions::new(&temp)).expect("init should work");
        fs::write(temp.join("a.txt"), "one\n").expect("worktree file should be written");
        repository
            .add_paths(&["a.txt".to_owned()])
            .expect("file should be added");
        fs::write(
            repository.git_dir().join("config"),
            "[core]\n\trepositoryformatversion = 0\n\tbare = false\n[user]\n\tname = Rit Test\n\temail = rit@example.test\n",
        )
        .expect("config should be written");
        repository
            .commit_index("base")
            .expect("base commit should be created");
        fs::write(temp.join("a.txt"), "one\ntwo\n").expect("file should be modified");
        repository
            .add_paths(&["a.txt".to_owned()])
            .expect("modified file should be staged");

        let diff = repository
            .diff_index_to_head()
            .expect("cached diff should be computed");

        assert_eq!(
            diff.files,
            vec![DiffFileStat {
                status: 'M',
                old_path: None,
                path: "a.txt".to_owned(),
                similarity_score: None,
                insertions: 1,
                deletions: 0,
                binary: false,
                old_size: 4,
                new_size: 8,
            }]
        );
        remove_dir_all(&temp);
    }

    fn temp_path(name: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("rit-{name}-{unique}"))
    }

    fn remove_dir_all(path: &Path) {
        if path.exists() {
            fs::remove_dir_all(path).expect("temporary directory should be removed");
        }
    }
}
