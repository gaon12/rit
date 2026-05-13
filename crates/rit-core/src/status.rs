use crate::index::{Index, join_slash_path, relative_slash_path};
use crate::object::{ObjectKind, hash_object, parse_tree_entries};
use crate::parse_commit;
use crate::{GitAttributes, ObjectId, PathspecSet, Repository, Result, RitError};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

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

/// Porcelain branch header shown by `status -b`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StatusBranchHeader {
    /// `HEAD` points at an unborn local branch.
    InitialBranch(String),
    /// `HEAD` points at a local branch with at least one commit.
    Branch(String),
    /// `HEAD` is detached.
    Detached,
}

/// Status result formatted by the CLI.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PorcelainStatus {
    /// Optional branch header requested by `status -b`.
    pub branch: Option<StatusBranchHeader>,
    /// Ordered porcelain entries.
    pub entries: Vec<StatusEntry>,
}

/// Explanation of how status classifies one path.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StatusExplanation {
    /// Repository-relative path using `/` separators.
    pub path: String,
    /// Index status column that porcelain v1 would use.
    pub index_status: char,
    /// Working tree status column that porcelain v1 would use.
    pub worktree_status: char,
    /// Whether the path exists in HEAD.
    pub in_head: bool,
    /// Whether the path exists in the index.
    pub in_index: bool,
    /// Whether the path exists in the working tree.
    pub in_worktree: bool,
    /// Whether ignore rules currently mark the path ignored.
    pub ignored: bool,
    /// Human-readable reasons for the classification.
    pub reasons: Vec<String>,
}

/// Controls how porcelain status reports untracked files.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UntrackedFilesMode {
    /// Do not report untracked files.
    No,
    /// Collapse fully untracked directories into one `dir/` entry.
    Normal,
    /// Report every untracked file individually.
    All,
}

/// Options for porcelain status computation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StatusOptions {
    /// How to report untracked files.
    pub untracked_files: UntrackedFilesMode,
    /// Include the `## ...` branch header.
    pub include_branch_header: bool,
    /// Include ignored paths as `!!` entries.
    pub include_ignored: bool,
}

/// Explanation of why one path is or is not ignored.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IgnoreExplanation {
    /// Repository-relative path using `/` separators.
    pub path: String,
    /// Final ignore decision after applying matching rules in order.
    pub ignored: bool,
    /// Matching ignore rules in the order they were applied.
    pub matching_rules: Vec<IgnoreRuleExplanation>,
}

/// One ignore rule that matched an explained path.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IgnoreRuleExplanation {
    /// Ignore file that provided the rule.
    pub source: String,
    /// One-based line number in the ignore file.
    pub line_number: usize,
    /// Normalized ignore pattern.
    pub pattern: String,
    /// Whether this rule negates a previous ignore match.
    pub negated: bool,
}

impl Default for StatusOptions {
    fn default() -> Self {
        Self {
            untracked_files: UntrackedFilesMode::Normal,
            include_branch_header: false,
            include_ignored: false,
        }
    }
}

impl PorcelainStatus {
    /// Renders porcelain v1 text.
    pub fn to_porcelain_v1(&self) -> String {
        let mut output = String::new();
        if let Some(branch) = &self.branch {
            output.push_str(&branch.to_porcelain_v1());
            output.push('\n');
        }
        for entry in &self.entries {
            output.push(entry.index_status);
            output.push(entry.worktree_status);
            output.push(' ');
            output.push_str(&quote_porcelain_path(&entry.path));
            output.push('\n');
        }
        output
    }

    /// Renders porcelain v1 text with NUL-terminated raw paths.
    pub fn to_porcelain_v1_null_terminated(&self) -> String {
        let mut output = String::new();
        if let Some(branch) = &self.branch {
            output.push_str(&branch.to_porcelain_v1());
            output.push('\0');
        }
        for entry in &self.entries {
            output.push(entry.index_status);
            output.push(entry.worktree_status);
            output.push(' ');
            output.push_str(&entry.path);
            output.push('\0');
        }
        output
    }
}

impl StatusBranchHeader {
    fn to_porcelain_v1(&self) -> String {
        match self {
            StatusBranchHeader::InitialBranch(name) => format!("## No commits yet on {name}"),
            StatusBranchHeader::Branch(name) => format!("## {name}"),
            StatusBranchHeader::Detached => "## HEAD (no branch)".to_owned(),
        }
    }
}

impl Repository {
    /// Computes a conservative porcelain v1 status.
    pub fn status_porcelain_v1(&self) -> Result<PorcelainStatus> {
        self.status_porcelain_v1_with_pathspecs(&PathspecSet::all())
    }

    /// Computes a conservative porcelain v1 status for matching paths only.
    pub fn status_porcelain_v1_with_pathspecs(
        &self,
        pathspecs: &PathspecSet,
    ) -> Result<PorcelainStatus> {
        self.status_porcelain_v1_with_options(pathspecs, StatusOptions::default())
    }

    /// Computes porcelain v1 status with explicit untracked-file handling.
    pub fn status_porcelain_v1_with_options(
        &self,
        pathspecs: &PathspecSet,
        options: StatusOptions,
    ) -> Result<PorcelainStatus> {
        let Some(worktree) = self.worktree() else {
            return Err(RitError::invalid_input(
                "status must be run in a repository with a working tree",
            ));
        };

        let branch = if options.include_branch_header {
            Some(self.status_branch_header()?)
        } else {
            None
        };
        let index_path = self.git_dir().join("index");
        let mut index = Index::read(&index_path)?;
        let index_entries = index
            .entries
            .iter()
            .map(|entry| {
                (
                    entry.path.clone(),
                    TreeBlobEntry {
                        object_id: entry.object_id,
                        mode: entry.mode,
                    },
                )
            })
            .collect::<BTreeMap<_, _>>();
        let index_entry_positions = index
            .entries
            .iter()
            .enumerate()
            .map(|(position, entry)| (entry.path.clone(), position))
            .collect::<BTreeMap<_, _>>();
        let head_entries = self.head_tree_entries()?;
        let ignore_rules = IgnoreRules::read(worktree, self.common_dir())?;
        let working_tree = scan_working_tree(worktree, &ignore_rules)?;
        let symlinks_enabled = self.core_symlinks_enabled()?;
        let attributes = self.root_attributes()?;

        let mut entries = Vec::new();
        let mut index_refreshed = false;
        let tracked_paths = index_entries
            .keys()
            .chain(head_entries.keys())
            .cloned()
            .collect::<BTreeSet<_>>();

        for path in &tracked_paths {
            let head_object = head_entries.get(path);
            let index_object = index_entries.get(path);
            let index_status = status_index_column(head_object, index_object);

            let worktree_status = match index_object {
                None => ' ',
                Some(index_object) => {
                    let full_path = join_slash_path(worktree, path);
                    let metadata = fs::symlink_metadata(&full_path).ok();
                    let status = status_worktree_column(
                        &full_path,
                        metadata.as_ref(),
                        index_object,
                        symlinks_enabled,
                    )?;
                    if status == ' ' {
                        let metadata = fs::symlink_metadata(&full_path)
                            .map_err(|source| RitError::io(&full_path, source))?;
                        if !worktree_mode_matches_index(
                            &metadata,
                            index_object.mode,
                            symlinks_enabled,
                        ) {
                            'M'
                        } else {
                            if let Some(position) = index_entry_positions.get(path) {
                                let entry = &mut index.entries[*position];
                                let refreshed_stat = entry.stat.with_mtime_from_metadata(&metadata);
                                let refreshed_size = metadata.len().min(u32::MAX as u64) as u32;
                                if entry.stat != refreshed_stat || entry.file_size != refreshed_size
                                {
                                    entry.stat = refreshed_stat;
                                    entry.file_size = refreshed_size;
                                    index_refreshed = true;
                                }
                            }
                            ' '
                        }
                    } else {
                        status
                    }
                }
            };

            if pathspecs.matches_with_attributes(path, Some(&attributes))
                && (index_status != ' ' || worktree_status != ' ')
            {
                entries.push(StatusEntry {
                    index_status,
                    worktree_status,
                    path: path.clone(),
                });
            }
        }

        if options.untracked_files != UntrackedFilesMode::No {
            for path in untracked_status_paths(
                &working_tree.files,
                &tracked_paths,
                pathspecs,
                options.untracked_files,
                &attributes,
            ) {
                if !index_entries.contains_key(&path) {
                    entries.push(StatusEntry {
                        index_status: '?',
                        worktree_status: '?',
                        path,
                    });
                }
            }

            if options.include_ignored {
                for path in ignored_status_paths(
                    &working_tree.ignored,
                    &tracked_paths,
                    pathspecs,
                    &attributes,
                ) {
                    entries.push(StatusEntry {
                        index_status: '!',
                        worktree_status: '!',
                        path,
                    });
                }
            }
        }

        if index_refreshed {
            index.write(&index_path)?;
        }

        Ok(PorcelainStatus { branch, entries })
    }

    /// Explains the ignore decision for one repository-relative path.
    pub fn explain_ignore_path(&self, path: &str) -> Result<IgnoreExplanation> {
        let Some(worktree) = self.worktree() else {
            return Err(RitError::invalid_input(
                "ignore explain must be run in a repository with a working tree",
            ));
        };
        let normalized_path = normalize_explain_path(path);
        let mut matching_rules = Vec::new();
        let mut ignored = false;
        collect_matching_ignore_rules(
            &worktree.join(".gitignore"),
            &normalized_path,
            &mut ignored,
            &mut matching_rules,
        )?;
        collect_matching_ignore_rules(
            &self.common_dir().join("info").join("exclude"),
            &normalized_path,
            &mut ignored,
            &mut matching_rules,
        )?;
        Ok(IgnoreExplanation {
            path: normalized_path,
            ignored,
            matching_rules,
        })
    }

    /// Explains how status classifies one repository-relative path.
    pub fn explain_status_path(&self, path: &str) -> Result<StatusExplanation> {
        let Some(worktree) = self.worktree() else {
            return Err(RitError::invalid_input(
                "status explain must be run in a repository with a working tree",
            ));
        };
        let normalized_path = normalize_explain_path(path);
        let index = Index::read(&self.git_dir().join("index"))?;
        let index_entries = index
            .entries
            .iter()
            .map(|entry| {
                (
                    entry.path.clone(),
                    TreeBlobEntry {
                        object_id: entry.object_id,
                        mode: entry.mode,
                    },
                )
            })
            .collect::<BTreeMap<_, _>>();
        let head_entries = self.head_tree_entries()?;
        let ignore_rules = IgnoreRules::read(worktree, self.common_dir())?;
        let symlinks_enabled = self.core_symlinks_enabled()?;
        let head_object = head_entries.get(&normalized_path);
        let index_object = index_entries.get(&normalized_path);
        let full_path = join_slash_path(worktree, &normalized_path);
        let worktree_metadata = fs::symlink_metadata(&full_path).ok();
        let in_head = head_object.is_some();
        let in_index = index_object.is_some();
        let in_worktree = worktree_metadata.is_some();
        let ignored = ignore_rules.matches(&normalized_path);
        let index_status = status_index_column(head_object, index_object);
        let worktree_status = match index_object {
            Some(index_object) => status_worktree_column(
                &full_path,
                worktree_metadata.as_ref(),
                index_object,
                symlinks_enabled,
            )?,
            None if in_worktree && ignored => '!',
            None if in_worktree => '?',
            None => ' ',
        };
        let reasons = status_explanation_reasons(StatusReasonInput {
            in_head,
            in_index,
            in_worktree,
            ignored,
            head_object,
            index_object,
            index_status,
            worktree_status,
        });

        Ok(StatusExplanation {
            path: normalized_path,
            index_status,
            worktree_status,
            in_head,
            in_index,
            in_worktree,
            ignored,
            reasons,
        })
    }

    fn status_branch_header(&self) -> Result<StatusBranchHeader> {
        match (self.current_branch_name()?, self.resolve_head()?) {
            (Some(name), Some(_)) => Ok(StatusBranchHeader::Branch(name)),
            (Some(name), None) => Ok(StatusBranchHeader::InitialBranch(name)),
            (None, _) => Ok(StatusBranchHeader::Detached),
        }
    }

    fn head_tree_entries(&self) -> Result<BTreeMap<String, TreeBlobEntry>> {
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
        self.collect_tree_entries("", tree_id, &mut entries)?;
        Ok(entries)
    }

    fn collect_tree_entries(
        &self,
        prefix: &str,
        tree_id: ObjectId,
        output: &mut BTreeMap<String, TreeBlobEntry>,
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
                output.insert(
                    path,
                    TreeBlobEntry {
                        object_id: entry.object_id,
                        mode: parse_tree_mode(&entry.mode)?,
                    },
                );
            }
        }

        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct TreeBlobEntry {
    object_id: ObjectId,
    mode: u32,
}

fn status_index_column(
    head_object: Option<&TreeBlobEntry>,
    index_object: Option<&TreeBlobEntry>,
) -> char {
    match (head_object, index_object) {
        (None, Some(_)) => 'A',
        (Some(_), None) => 'D',
        (Some(head), Some(index)) if head != index => 'M',
        _ => ' ',
    }
}

fn status_worktree_column(
    full_path: &Path,
    metadata: Option<&fs::Metadata>,
    index_object: &TreeBlobEntry,
    symlinks_enabled: bool,
) -> Result<char> {
    let Some(metadata) = metadata else {
        return Ok('D');
    };
    if hash_worktree_entry(full_path, index_object.mode, symlinks_enabled)?
        != index_object.object_id
    {
        return Ok('M');
    }
    if !worktree_mode_matches_index(metadata, index_object.mode, symlinks_enabled) {
        return Ok('M');
    }
    Ok(' ')
}

struct StatusReasonInput<'a> {
    in_head: bool,
    in_index: bool,
    in_worktree: bool,
    ignored: bool,
    head_object: Option<&'a TreeBlobEntry>,
    index_object: Option<&'a TreeBlobEntry>,
    index_status: char,
    worktree_status: char,
}

fn status_explanation_reasons(input: StatusReasonInput<'_>) -> Vec<String> {
    let StatusReasonInput {
        in_head,
        in_index,
        in_worktree,
        ignored,
        head_object,
        index_object,
        index_status,
        worktree_status,
    } = input;
    let mut reasons = Vec::new();
    reasons.push(presence_reason("HEAD", in_head));
    reasons.push(presence_reason("the index", in_index));
    reasons.push(presence_reason("the working tree", in_worktree));

    match index_status {
        'A' => reasons.push("the path is added in the index because it is not in HEAD".to_owned()),
        'D' => reasons.push("the path is deleted in the index because it is missing from the index but exists in HEAD".to_owned()),
        'M' => reasons.push("the index object or mode differs from HEAD".to_owned()),
        _ if head_object.is_some() && index_object.is_some() => {
            reasons.push("HEAD and the index agree for this path".to_owned());
        }
        _ => {}
    }

    match worktree_status {
        'M' => reasons.push("the working tree content or file mode differs from the index".to_owned()),
        'D' => reasons.push("the path is deleted in the working tree because the index tracks it but the file is missing".to_owned()),
        '?' => reasons.push("the path is untracked because it exists in the working tree but not in HEAD or the index".to_owned()),
        '!' => reasons.push("the path is ignored because ignore rules match it and it is not tracked".to_owned()),
        _ if in_index && in_worktree => {
            reasons.push("the working tree matches the index for this path".to_owned());
        }
        _ => {}
    }

    if ignored && in_index {
        reasons.push("ignore rules match this path, but tracked paths remain tracked".to_owned());
    }
    if index_status == ' ' && worktree_status == ' ' {
        reasons.push("status has no changes to report for this path".to_owned());
    }
    reasons
}

fn presence_reason(place: &str, present: bool) -> String {
    if present {
        format!("the path exists in {place}")
    } else {
        format!("the path does not exist in {place}")
    }
}

fn parse_tree_mode(mode: &str) -> Result<u32> {
    u32::from_str_radix(mode, 8)
        .map_err(|_| RitError::invalid_input(format!("invalid tree mode: {mode}")))
}

#[cfg(unix)]
fn worktree_mode_matches_index(
    metadata: &fs::Metadata,
    index_mode: u32,
    symlinks_enabled: bool,
) -> bool {
    if index_mode == 0o120000 && symlinks_enabled {
        return metadata.file_type().is_symlink();
    }
    if index_mode == 0o120000 && !symlinks_enabled {
        return true;
    }
    use std::os::unix::fs::PermissionsExt;
    let executable = metadata.permissions().mode() & 0o111 != 0;
    let worktree_mode = if executable { 0o100755 } else { 0o100644 };
    worktree_mode == index_mode
}

#[cfg(not(unix))]
fn worktree_mode_matches_index(
    _metadata: &fs::Metadata,
    _index_mode: u32,
    _symlinks_enabled: bool,
) -> bool {
    true
}

fn quote_porcelain_path(path: &str) -> String {
    if !path
        .chars()
        .any(|character| character.is_whitespace() || matches!(character, '"' | '\\'))
    {
        return path.to_owned();
    }

    let mut output = String::from("\"");
    for character in path.chars() {
        match character {
            '"' => output.push_str("\\\""),
            '\\' => output.push_str("\\\\"),
            '\n' => output.push_str("\\n"),
            '\t' => output.push_str("\\t"),
            '\r' => output.push_str("\\r"),
            other => output.push(other),
        }
    }
    output.push('"');
    output
}

fn hash_worktree_entry(path: &Path, index_mode: u32, symlinks_enabled: bool) -> Result<ObjectId> {
    let bytes = if index_mode == 0o120000 && symlinks_enabled {
        let metadata = fs::symlink_metadata(path).map_err(|source| RitError::io(path, source))?;
        if metadata.file_type().is_symlink() {
            read_symlink_target_bytes(path)?
        } else {
            fs::read(path).map_err(|source| RitError::io(path, source))?
        }
    } else {
        fs::read(path).map_err(|source| RitError::io(path, source))?
    };
    Ok(hash_object(ObjectKind::Blob, &bytes))
}

fn read_symlink_target_bytes(path: &Path) -> Result<Vec<u8>> {
    let target = fs::read_link(path).map_err(|source| RitError::io(path, source))?;
    Ok(target.to_string_lossy().replace('\\', "/").into_bytes())
}

fn collapse_untracked_paths(
    working_files: &BTreeSet<String>,
    tracked_paths: &BTreeSet<String>,
    pathspecs: &PathspecSet,
    attributes: &GitAttributes,
) -> BTreeSet<String> {
    let mut output = BTreeSet::new();

    for path in working_files {
        if tracked_paths.contains(path)
            || !pathspecs.matches_with_attributes(path, Some(attributes))
        {
            continue;
        }

        output.insert(display_untracked_path(path, tracked_paths, pathspecs));
    }

    output
}

fn untracked_status_paths(
    working_files: &BTreeSet<String>,
    tracked_paths: &BTreeSet<String>,
    pathspecs: &PathspecSet,
    mode: UntrackedFilesMode,
    attributes: &GitAttributes,
) -> BTreeSet<String> {
    match mode {
        UntrackedFilesMode::No => BTreeSet::new(),
        UntrackedFilesMode::Normal => {
            collapse_untracked_paths(working_files, tracked_paths, pathspecs, attributes)
        }
        UntrackedFilesMode::All => working_files
            .iter()
            .filter(|path| {
                !tracked_paths.contains(*path)
                    && pathspecs.matches_with_attributes(path, Some(attributes))
            })
            .cloned()
            .collect(),
    }
}

fn ignored_status_paths(
    ignored_paths: &BTreeSet<String>,
    tracked_paths: &BTreeSet<String>,
    pathspecs: &PathspecSet,
    attributes: &GitAttributes,
) -> BTreeSet<String> {
    ignored_paths
        .iter()
        .filter(|path| {
            let normalized = path.trim_end_matches('/');
            ignored_path_matches_pathspecs(path, pathspecs, attributes)
                && !has_tracked_path_below(tracked_paths, normalized)
        })
        .cloned()
        .collect()
}

fn ignored_path_matches_pathspecs(
    path: &str,
    pathspecs: &PathspecSet,
    attributes: &GitAttributes,
) -> bool {
    if pathspecs.matches_with_attributes(path, Some(attributes)) {
        return true;
    }

    let Some(directory) = path.strip_suffix('/') else {
        return false;
    };
    pathspecs
        .patterns()
        .iter()
        .filter(|pattern| !pattern.is_exclude())
        .any(|pattern| pattern.starts_with_directory(directory))
}

fn display_untracked_path(
    path: &str,
    tracked_paths: &BTreeSet<String>,
    pathspecs: &PathspecSet,
) -> String {
    if pathspecs
        .patterns()
        .iter()
        .filter(|pattern| !pattern.is_exclude())
        .any(|pattern| pattern.is_exact_path(path))
    {
        return path.to_owned();
    }

    if !pathspecs.is_all() {
        for pattern in pathspecs.patterns() {
            if pattern.is_exclude() {
                continue;
            }
            let pattern_text = pattern.pattern();
            if path.starts_with(&format!("{pattern_text}/"))
                && !has_tracked_path_below(tracked_paths, pattern_text)
            {
                return format!("{pattern_text}/");
            }
        }
    }

    topmost_untracked_directory(path, tracked_paths)
        .map(|directory| format!("{directory}/"))
        .unwrap_or_else(|| path.to_owned())
}

fn topmost_untracked_directory(path: &str, tracked_paths: &BTreeSet<String>) -> Option<String> {
    let mut best = None;
    let mut prefix = String::new();
    let mut parts = path.split('/').collect::<Vec<_>>();
    parts.pop();

    for part in parts {
        if prefix.is_empty() {
            prefix.push_str(part);
        } else {
            prefix.push('/');
            prefix.push_str(part);
        }
        if !has_tracked_path_below(tracked_paths, &prefix) {
            best = Some(prefix.clone());
            break;
        }
    }

    best
}

fn has_tracked_path_below(tracked_paths: &BTreeSet<String>, directory: &str) -> bool {
    tracked_paths
        .iter()
        .any(|path| path == directory || path.starts_with(&format!("{directory}/")))
}

#[derive(Clone, Debug, Default)]
struct WorkingTreeScan {
    files: BTreeSet<String>,
    ignored: BTreeSet<String>,
}

fn scan_working_tree(root: &Path, ignore_rules: &IgnoreRules) -> Result<WorkingTreeScan> {
    let mut output = WorkingTreeScan::default();
    scan_directory(root, root, ignore_rules, &mut output)?;
    Ok(output)
}

fn scan_directory(
    root: &Path,
    directory: &Path,
    ignore_rules: &IgnoreRules,
    output: &mut WorkingTreeScan,
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
        if ignore_rules.matches(&relative) {
            if file_type.is_dir() {
                output.ignored.insert(format!("{relative}/"));
            } else if file_type.is_file() {
                output.ignored.insert(relative);
            }
            continue;
        }

        if file_type.is_dir() {
            scan_directory(root, &path, ignore_rules, output)?;
        } else if file_type.is_file() || file_type.is_symlink() {
            output.files.insert(relative);
        }
    }

    Ok(())
}

#[derive(Clone, Debug, Default)]
struct IgnoreRules {
    rules: Vec<IgnoreRule>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct IgnoreRule {
    pattern: String,
    negated: bool,
}

impl IgnoreRules {
    fn read(worktree: &Path, git_dir: &Path) -> Result<Self> {
        let mut rules = Vec::new();
        read_ignore_file(&worktree.join(".gitignore"), &mut rules)?;
        read_ignore_file(&git_dir.join("info").join("exclude"), &mut rules)?;
        Ok(Self { rules })
    }

    fn matches(&self, path: &str) -> bool {
        let mut ignored = false;
        for rule in &self.rules {
            if ignore_rule_matches(rule, path) {
                ignored = !rule.negated;
            }
        }
        ignored
    }
}

fn read_ignore_file(path: &Path, rules: &mut Vec<IgnoreRule>) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }

    let contents = fs::read_to_string(path).map_err(|source| RitError::io(path, source))?;
    for line in contents.lines() {
        let Some(rule) = parse_ignore_rule(line) else {
            continue;
        };
        rules.push(rule);
    }

    Ok(())
}

fn collect_matching_ignore_rules(
    source: &Path,
    path: &str,
    ignored: &mut bool,
    output: &mut Vec<IgnoreRuleExplanation>,
) -> Result<()> {
    if !source.exists() {
        return Ok(());
    }
    let contents =
        fs::read_to_string(source).map_err(|source_error| RitError::io(source, source_error))?;
    for (line_index, line) in contents.lines().enumerate() {
        let Some(rule) = parse_ignore_rule(line) else {
            continue;
        };
        if ignore_rule_matches(&rule, path) {
            *ignored = !rule.negated;
            output.push(IgnoreRuleExplanation {
                source: source.display().to_string(),
                line_number: line_index + 1,
                pattern: rule.pattern,
                negated: rule.negated,
            });
        }
    }
    Ok(())
}

fn parse_ignore_rule(line: &str) -> Option<IgnoreRule> {
    let mut pattern = line.trim().to_owned();
    if pattern.is_empty() || pattern.starts_with('#') {
        return None;
    }

    let escaped_literal_prefix = pattern.starts_with("\\!") || pattern.starts_with("\\#");
    let negated = pattern.starts_with('!');
    if negated || escaped_literal_prefix {
        pattern.remove(0);
    }

    if pattern.is_empty() {
        return None;
    }

    Some(IgnoreRule {
        pattern: pattern.replace('\\', "/"),
        negated,
    })
}

fn normalize_explain_path(path: &str) -> String {
    path.replace('\\', "/")
        .trim_start_matches("./")
        .trim_start_matches('/')
        .to_owned()
}

fn ignore_rule_matches(rule: &IgnoreRule, path: &str) -> bool {
    let directory_only = rule.pattern.ends_with('/');
    let anchored = rule.pattern.starts_with('/');
    let pattern = rule.pattern.trim_start_matches('/').trim_end_matches('/');
    if pattern.is_empty() {
        return false;
    }

    let has_slash = pattern.contains('/');
    if directory_only {
        if anchored || has_slash {
            return path
                .split('/')
                .scan(String::new(), |prefix, component| {
                    if !prefix.is_empty() {
                        prefix.push('/');
                    }
                    prefix.push_str(component);
                    Some(prefix.clone())
                })
                .any(|prefix| gitignore_glob_matches(pattern, &prefix));
        }
        return path
            .split('/')
            .any(|component| gitignore_glob_matches(pattern, component));
    }

    if anchored || has_slash {
        return gitignore_glob_matches(pattern, path);
    }

    path.rsplit('/')
        .next()
        .is_some_and(|name| gitignore_glob_matches(pattern, name))
}

fn gitignore_glob_matches(pattern: &str, path: &str) -> bool {
    fn matches_from(pattern: &[u8], path: &[u8], pattern_index: usize, path_index: usize) -> bool {
        if pattern_index == pattern.len() {
            return path_index == path.len();
        }

        if pattern[pattern_index..].starts_with(b"**/") {
            if matches_from(pattern, path, pattern_index + 3, path_index) {
                return true;
            }
            for next_index in path_index..path.len() {
                if path[next_index] == b'/'
                    && matches_from(pattern, path, pattern_index + 3, next_index + 1)
                {
                    return true;
                }
            }
            return false;
        }

        if pattern[pattern_index..].starts_with(b"**") {
            if matches_from(pattern, path, pattern_index + 2, path_index) {
                return true;
            }
            for next_index in path_index..path.len() {
                if matches_from(pattern, path, pattern_index + 2, next_index + 1) {
                    return true;
                }
            }
            return false;
        }

        if pattern[pattern_index] == b'*' {
            if matches_from(pattern, path, pattern_index + 1, path_index) {
                return true;
            }
            let mut next_index = path_index;
            while next_index < path.len() && path[next_index] != b'/' {
                next_index += 1;
                if matches_from(pattern, path, pattern_index + 1, next_index) {
                    return true;
                }
            }
            return false;
        }

        let Some(path_byte) = path.get(path_index).copied() else {
            return false;
        };
        if path_byte == b'/' {
            return pattern[pattern_index] == b'/'
                && matches_from(pattern, path, pattern_index + 1, path_index + 1);
        }

        match pattern[pattern_index] {
            b'?' => matches_from(pattern, path, pattern_index + 1, path_index + 1),
            b'[' => match_gitignore_bracket_class(pattern, pattern_index, path_byte).is_some_and(
                |next_pattern_index| {
                    matches_from(pattern, path, next_pattern_index, path_index + 1)
                },
            ),
            literal if literal == path_byte => {
                matches_from(pattern, path, pattern_index + 1, path_index + 1)
            }
            _ => false,
        }
    }

    matches_from(pattern.as_bytes(), path.as_bytes(), 0, 0)
}

fn match_gitignore_bracket_class(pattern: &[u8], index: usize, path_byte: u8) -> Option<usize> {
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
            if start <= path_byte && path_byte <= end {
                matched = true;
            }
            cursor += 3;
        } else {
            if pattern[cursor] == path_byte {
                matched = true;
            }
            cursor += 1;
        }
    }

    if path_byte == b'[' {
        Some(index + 1)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::{
        IgnoreRule, IgnoreRules, PorcelainStatus, StatusBranchHeader, StatusEntry,
        UntrackedFilesMode, collapse_untracked_paths, ignored_status_paths, quote_porcelain_path,
        untracked_status_paths,
    };
    #[cfg(unix)]
    use crate::{AddOptions, FileModeOverride};
    use crate::{GitAttributes, InitOptions, PathspecSet, Repository};
    use std::collections::BTreeSet;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn porcelain_v1_renders_entries() {
        let status = PorcelainStatus {
            branch: None,
            entries: vec![StatusEntry {
                index_status: '?',
                worktree_status: '?',
                path: "new.txt".to_owned(),
            }],
        };

        assert_eq!(status.to_porcelain_v1(), "?? new.txt\n");
    }

    #[test]
    fn porcelain_v1_quotes_paths_with_whitespace() {
        let status = PorcelainStatus {
            branch: None,
            entries: vec![StatusEntry {
                index_status: ' ',
                worktree_status: 'M',
                path: "my dir/a b.txt".to_owned(),
            }],
        };

        assert_eq!(status.to_porcelain_v1(), " M \"my dir/a b.txt\"\n");
    }

    #[test]
    fn porcelain_path_escapes_special_characters() {
        assert_eq!(
            quote_porcelain_path("quote\"tab\t.txt"),
            "\"quote\\\"tab\\t.txt\""
        );
    }

    #[test]
    fn porcelain_v1_null_terminated_uses_raw_paths() {
        let status = PorcelainStatus {
            branch: None,
            entries: vec![StatusEntry {
                index_status: ' ',
                worktree_status: 'M',
                path: "my dir/a b.txt".to_owned(),
            }],
        };

        assert_eq!(
            status.to_porcelain_v1_null_terminated(),
            " M my dir/a b.txt\0"
        );
    }

    #[test]
    fn porcelain_v1_renders_branch_header() {
        let status = PorcelainStatus {
            branch: Some(StatusBranchHeader::Branch("main".to_owned())),
            entries: vec![StatusEntry {
                index_status: ' ',
                worktree_status: 'M',
                path: "a.txt".to_owned(),
            }],
        };

        assert_eq!(status.to_porcelain_v1(), "## main\n M a.txt\n");
        assert_eq!(
            status.to_porcelain_v1_null_terminated(),
            "## main\0 M a.txt\0"
        );
    }

    #[test]
    fn ignore_rules_match_rooted_directory() {
        let rules = IgnoreRules {
            rules: vec![IgnoreRule {
                pattern: "/target/".to_owned(),
                negated: false,
            }],
        };

        assert!(rules.matches("target/debug/app"));
        assert!(!rules.matches("src/target.rs"));
    }

    #[test]
    fn ignore_rules_match_gitignore_globs_and_negation() {
        let rules = IgnoreRules {
            rules: vec![
                IgnoreRule {
                    pattern: "*.log".to_owned(),
                    negated: false,
                },
                IgnoreRule {
                    pattern: "build?.tmp".to_owned(),
                    negated: false,
                },
                IgnoreRule {
                    pattern: "[ab].cache".to_owned(),
                    negated: false,
                },
                IgnoreRule {
                    pattern: "/root-only.txt".to_owned(),
                    negated: false,
                },
                IgnoreRule {
                    pattern: "docs/**/generated.txt".to_owned(),
                    negated: false,
                },
                IgnoreRule {
                    pattern: "keep.log".to_owned(),
                    negated: true,
                },
            ],
        };

        assert!(rules.matches("error.log"));
        assert!(rules.matches("nested/error.log"));
        assert!(rules.matches("build1.tmp"));
        assert!(!rules.matches("build12.tmp"));
        assert!(rules.matches("a.cache"));
        assert!(!rules.matches("c.cache"));
        assert!(rules.matches("root-only.txt"));
        assert!(!rules.matches("nested/root-only.txt"));
        assert!(rules.matches("docs/generated.txt"));
        assert!(rules.matches("docs/deep/generated.txt"));
        assert!(!rules.matches("keep.log"));
    }

    #[test]
    fn untracked_status_collapses_fully_untracked_directories() {
        let working_files = set(["dir/a.txt", "dir/sub/b.txt", "root.txt"]);
        let tracked_paths = BTreeSet::new();
        let pathspecs = PathspecSet::all();
        let attributes = GitAttributes::default();

        let collapsed =
            collapse_untracked_paths(&working_files, &tracked_paths, &pathspecs, &attributes);

        assert_eq!(collapsed, set(["dir/", "root.txt"]));
    }

    #[test]
    fn untracked_status_keeps_files_below_tracked_directories() {
        let working_files = set(["dir/new/a.txt", "dir/tracked.txt"]);
        let tracked_paths = set(["dir/tracked.txt"]);
        let pathspecs = PathspecSet::all();
        let attributes = GitAttributes::default();

        let collapsed =
            collapse_untracked_paths(&working_files, &tracked_paths, &pathspecs, &attributes);

        assert_eq!(collapsed, set(["dir/new/"]));
    }

    #[test]
    fn untracked_status_respects_exact_file_pathspecs() {
        let working_files = set(["dir/sub/a.txt"]);
        let tracked_paths = BTreeSet::new();
        let pathspecs =
            PathspecSet::from_args(&["dir/sub/a.txt".to_owned()]).expect("pathspec should parse");
        let attributes = GitAttributes::default();

        let collapsed =
            collapse_untracked_paths(&working_files, &tracked_paths, &pathspecs, &attributes);

        assert_eq!(collapsed, set(["dir/sub/a.txt"]));
    }

    #[test]
    fn untracked_status_mode_all_keeps_every_file() {
        let working_files = set(["dir/a.txt", "dir/sub/b.txt", "root.txt"]);
        let tracked_paths = BTreeSet::new();
        let pathspecs = PathspecSet::all();
        let attributes = GitAttributes::default();

        let paths = untracked_status_paths(
            &working_files,
            &tracked_paths,
            &pathspecs,
            UntrackedFilesMode::All,
            &attributes,
        );

        assert_eq!(paths, set(["dir/a.txt", "dir/sub/b.txt", "root.txt"]));
    }

    #[test]
    fn untracked_status_mode_no_hides_untracked_files() {
        let working_files = set(["dir/a.txt", "root.txt"]);
        let tracked_paths = BTreeSet::new();
        let pathspecs = PathspecSet::all();
        let attributes = GitAttributes::default();

        let paths = untracked_status_paths(
            &working_files,
            &tracked_paths,
            &pathspecs,
            UntrackedFilesMode::No,
            &attributes,
        );

        assert!(paths.is_empty());
    }

    #[test]
    fn ignored_status_keeps_untracked_ignored_paths_only() {
        let ignored_paths = set(["ignored/", "secret.txt", "tracked.log"]);
        let tracked_paths = set(["tracked.log"]);
        let pathspecs = PathspecSet::all();
        let attributes = GitAttributes::default();

        let paths = ignored_status_paths(&ignored_paths, &tracked_paths, &pathspecs, &attributes);

        assert_eq!(paths, set(["ignored/", "secret.txt"]));
    }

    #[test]
    fn ignored_status_matches_pathspecs_below_collapsed_directories() {
        let ignored_paths = set(["ignored/"]);
        let tracked_paths = BTreeSet::new();
        let pathspecs =
            PathspecSet::from_args(&["ignored/deep/a.txt".to_owned()]).expect("valid pathspec");
        let attributes = GitAttributes::default();

        let paths = ignored_status_paths(&ignored_paths, &tracked_paths, &pathspecs, &attributes);

        assert_eq!(paths, set(["ignored/"]));
    }

    #[test]
    fn explains_ignore_rules_in_order() {
        let temp = temp_path("ignore-explain");
        let repository = Repository::init(&InitOptions::new(&temp)).expect("init should work");
        fs::write(temp.join(".gitignore"), "*.log\n!important.log\n")
            .expect("gitignore should be written");

        let ignored = repository
            .explain_ignore_path("debug.log")
            .expect("ignore explanation should work");
        let negated = repository
            .explain_ignore_path("important.log")
            .expect("ignore explanation should work");

        assert!(ignored.ignored);
        assert_eq!(ignored.matching_rules.len(), 1);
        assert_eq!(ignored.matching_rules[0].pattern, "*.log");
        assert!(!ignored.matching_rules[0].negated);
        assert!(!negated.ignored);
        assert_eq!(negated.matching_rules.len(), 2);
        assert_eq!(negated.matching_rules[1].pattern, "important.log");
        assert!(negated.matching_rules[1].negated);
        remove_dir_all(&temp);
    }

    #[test]
    fn explains_modified_status_for_tracked_path() {
        let temp = temp_path("status-explain-modified");
        let repository = Repository::init(&InitOptions::new(&temp)).expect("init should work");
        fs::write(
            repository.git_dir().join("config"),
            "[core]\n\trepositoryformatversion = 0\n\tbare = false\n[user]\n\tname = Rit Test\n\temail = rit@example.test\n",
        )
        .expect("config should be written");
        fs::write(temp.join("tracked.txt"), "before\n").expect("file should be written");
        repository
            .add_paths(&["tracked.txt".to_owned()])
            .expect("file should be added");
        repository
            .commit_index("add tracked")
            .expect("commit should be created");
        fs::write(temp.join("tracked.txt"), "after\n").expect("file should be modified");

        let explanation = repository
            .explain_status_path("tracked.txt")
            .expect("status explanation should work");

        assert_eq!(explanation.index_status, ' ');
        assert_eq!(explanation.worktree_status, 'M');
        assert!(explanation.in_head);
        assert!(explanation.in_index);
        assert!(explanation.in_worktree);
        assert!(
            explanation
                .reasons
                .iter()
                .any(|reason| reason.contains("working tree content or file mode differs"))
        );
        remove_dir_all(&temp);
    }

    #[test]
    fn explains_ignored_untracked_status() {
        let temp = temp_path("status-explain-ignored");
        let repository = Repository::init(&InitOptions::new(&temp)).expect("init should work");
        fs::write(temp.join(".gitignore"), "*.log\n").expect("gitignore should be written");
        fs::write(temp.join("debug.log"), "ignored\n").expect("file should be written");

        let explanation = repository
            .explain_status_path("debug.log")
            .expect("status explanation should work");

        assert_eq!(explanation.index_status, ' ');
        assert_eq!(explanation.worktree_status, '!');
        assert!(!explanation.in_head);
        assert!(!explanation.in_index);
        assert!(explanation.in_worktree);
        assert!(explanation.ignored);
        remove_dir_all(&temp);
    }

    #[cfg(unix)]
    #[test]
    fn status_reports_worktree_executable_bit_mismatch() {
        let temp = temp_path("status-executable-mode");
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
            .expect("file should be added as executable");
        repository
            .commit_index("add executable")
            .expect("commit should be created");
        set_test_permissions(&script_path, 0o644);

        let status = repository
            .status_porcelain_v1()
            .expect("status should be computed");

        assert_eq!(status.to_porcelain_v1(), " M script.sh\n");
        remove_dir_all(&temp);
    }

    #[cfg(unix)]
    #[test]
    fn status_hashes_symlink_targets() {
        use std::os::unix::fs::symlink;

        let temp = temp_path("status-symlink");
        let repository = Repository::init(&InitOptions::new(&temp)).expect("init should work");
        symlink("target-a.txt", temp.join("link.txt")).expect("symlink should be written");
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
        fs::remove_file(temp.join("link.txt")).expect("link should be removed");
        symlink("target-b.txt", temp.join("link.txt")).expect("changed symlink should be written");

        let status = repository
            .status_porcelain_v1()
            .expect("status should be computed");

        assert_eq!(status.to_porcelain_v1(), " M link.txt\n");
        remove_dir_all(&temp);
    }

    fn set<const N: usize>(paths: [&str; N]) -> BTreeSet<String> {
        paths.into_iter().map(ToOwned::to_owned).collect()
    }

    fn temp_path(name: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("rit-status-{name}-{unique}"))
    }

    #[cfg(unix)]
    fn set_test_permissions(path: &Path, mode: u32) {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(mode))
            .expect("test permissions should be set");
    }

    fn remove_dir_all(path: &Path) {
        if path.exists() {
            fs::remove_dir_all(path).expect("temporary directory should be removed");
        }
    }
}
