use crate::index::{Index, join_slash_path, relative_slash_path};
use crate::object::{ObjectKind, hash_object, parse_tree_entries};
use crate::parse_commit;
use crate::{ObjectId, PathspecSet, Repository, Result, RitError};
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
            .map(|entry| (entry.path.clone(), entry.object_id))
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
            let index_status = match (head_object, index_object) {
                (None, Some(_)) => 'A',
                (Some(_), None) => 'D',
                (Some(head), Some(index)) if head != index => 'M',
                _ => ' ',
            };

            let worktree_status = match index_object {
                None => ' ',
                Some(index_object) => {
                    let full_path = join_slash_path(worktree, path);
                    if !full_path.exists() {
                        'D'
                    } else if hash_worktree_file(&full_path)? != *index_object {
                        'M'
                    } else {
                        if let Some(position) = index_entry_positions.get(path) {
                            let metadata = fs::metadata(&full_path)
                                .map_err(|source| RitError::io(&full_path, source))?;
                            let entry = &mut index.entries[*position];
                            let refreshed_stat = entry.stat.with_mtime_from_metadata(&metadata);
                            let refreshed_size = metadata.len().min(u32::MAX as u64) as u32;
                            if entry.stat != refreshed_stat || entry.file_size != refreshed_size {
                                entry.stat = refreshed_stat;
                                entry.file_size = refreshed_size;
                                index_refreshed = true;
                            }
                        }
                        ' '
                    }
                }
            };

            if pathspecs.matches(path) && (index_status != ' ' || worktree_status != ' ') {
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
                for path in ignored_status_paths(&working_tree.ignored, &tracked_paths, pathspecs) {
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

    fn status_branch_header(&self) -> Result<StatusBranchHeader> {
        match (self.current_branch_name()?, self.resolve_head()?) {
            (Some(name), Some(_)) => Ok(StatusBranchHeader::Branch(name)),
            (Some(name), None) => Ok(StatusBranchHeader::InitialBranch(name)),
            (None, _) => Ok(StatusBranchHeader::Detached),
        }
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
        let tree_id = parse_commit(&commit.data)?.tree;
        let mut entries = BTreeMap::new();
        self.collect_tree_entries("", tree_id, &mut entries)?;
        Ok(entries)
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

fn hash_worktree_file(path: &Path) -> Result<ObjectId> {
    let bytes = fs::read(path).map_err(|source| RitError::io(path, source))?;
    Ok(hash_object(ObjectKind::Blob, &bytes))
}

fn collapse_untracked_paths(
    working_files: &BTreeSet<String>,
    tracked_paths: &BTreeSet<String>,
    pathspecs: &PathspecSet,
) -> BTreeSet<String> {
    let mut output = BTreeSet::new();

    for path in working_files {
        if tracked_paths.contains(path) || !pathspecs.matches(path) {
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
) -> BTreeSet<String> {
    match mode {
        UntrackedFilesMode::No => BTreeSet::new(),
        UntrackedFilesMode::Normal => {
            collapse_untracked_paths(working_files, tracked_paths, pathspecs)
        }
        UntrackedFilesMode::All => working_files
            .iter()
            .filter(|path| !tracked_paths.contains(*path) && pathspecs.matches(path))
            .cloned()
            .collect(),
    }
}

fn ignored_status_paths(
    ignored_paths: &BTreeSet<String>,
    tracked_paths: &BTreeSet<String>,
    pathspecs: &PathspecSet,
) -> BTreeSet<String> {
    ignored_paths
        .iter()
        .filter(|path| {
            let normalized = path.trim_end_matches('/');
            ignored_path_matches_pathspecs(path, pathspecs)
                && !has_tracked_path_below(tracked_paths, normalized)
        })
        .cloned()
        .collect()
}

fn ignored_path_matches_pathspecs(path: &str, pathspecs: &PathspecSet) -> bool {
    if pathspecs.matches(path) {
        return true;
    }

    let Some(directory) = path.strip_suffix('/') else {
        return false;
    };
    pathspecs
        .patterns()
        .iter()
        .any(|pattern| pattern.starts_with(&format!("{directory}/")))
}

fn display_untracked_path(
    path: &str,
    tracked_paths: &BTreeSet<String>,
    pathspecs: &PathspecSet,
) -> String {
    if pathspecs.patterns().iter().any(|pattern| pattern == path) {
        return path.to_owned();
    }

    if !pathspecs.is_all() {
        for pattern in pathspecs.patterns() {
            if path.starts_with(&format!("{pattern}/"))
                && !has_tracked_path_below(tracked_paths, pattern)
            {
                return format!("{pattern}/");
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
        } else if file_type.is_file() {
            output.files.insert(relative);
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
    use super::{
        IgnoreRules, PorcelainStatus, StatusBranchHeader, StatusEntry, UntrackedFilesMode,
        collapse_untracked_paths, ignored_status_paths, quote_porcelain_path,
        untracked_status_paths,
    };
    use crate::PathspecSet;
    use std::collections::BTreeSet;

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
            patterns: vec!["/target/".to_owned()],
        };

        assert!(rules.matches("target/debug/app"));
        assert!(!rules.matches("src/target.rs"));
    }

    #[test]
    fn untracked_status_collapses_fully_untracked_directories() {
        let working_files = set(["dir/a.txt", "dir/sub/b.txt", "root.txt"]);
        let tracked_paths = BTreeSet::new();
        let pathspecs = PathspecSet::all();

        let collapsed = collapse_untracked_paths(&working_files, &tracked_paths, &pathspecs);

        assert_eq!(collapsed, set(["dir/", "root.txt"]));
    }

    #[test]
    fn untracked_status_keeps_files_below_tracked_directories() {
        let working_files = set(["dir/new/a.txt", "dir/tracked.txt"]);
        let tracked_paths = set(["dir/tracked.txt"]);
        let pathspecs = PathspecSet::all();

        let collapsed = collapse_untracked_paths(&working_files, &tracked_paths, &pathspecs);

        assert_eq!(collapsed, set(["dir/new/"]));
    }

    #[test]
    fn untracked_status_respects_exact_file_pathspecs() {
        let working_files = set(["dir/sub/a.txt"]);
        let tracked_paths = BTreeSet::new();
        let pathspecs =
            PathspecSet::from_args(&["dir/sub/a.txt".to_owned()]).expect("pathspec should parse");

        let collapsed = collapse_untracked_paths(&working_files, &tracked_paths, &pathspecs);

        assert_eq!(collapsed, set(["dir/sub/a.txt"]));
    }

    #[test]
    fn untracked_status_mode_all_keeps_every_file() {
        let working_files = set(["dir/a.txt", "dir/sub/b.txt", "root.txt"]);
        let tracked_paths = BTreeSet::new();
        let pathspecs = PathspecSet::all();

        let paths = untracked_status_paths(
            &working_files,
            &tracked_paths,
            &pathspecs,
            UntrackedFilesMode::All,
        );

        assert_eq!(paths, set(["dir/a.txt", "dir/sub/b.txt", "root.txt"]));
    }

    #[test]
    fn untracked_status_mode_no_hides_untracked_files() {
        let working_files = set(["dir/a.txt", "root.txt"]);
        let tracked_paths = BTreeSet::new();
        let pathspecs = PathspecSet::all();

        let paths = untracked_status_paths(
            &working_files,
            &tracked_paths,
            &pathspecs,
            UntrackedFilesMode::No,
        );

        assert!(paths.is_empty());
    }

    #[test]
    fn ignored_status_keeps_untracked_ignored_paths_only() {
        let ignored_paths = set(["ignored/", "secret.txt", "tracked.log"]);
        let tracked_paths = set(["tracked.log"]);
        let pathspecs = PathspecSet::all();

        let paths = ignored_status_paths(&ignored_paths, &tracked_paths, &pathspecs);

        assert_eq!(paths, set(["ignored/", "secret.txt"]));
    }

    #[test]
    fn ignored_status_matches_pathspecs_below_collapsed_directories() {
        let ignored_paths = set(["ignored/"]);
        let tracked_paths = BTreeSet::new();
        let pathspecs =
            PathspecSet::from_args(&["ignored/deep/a.txt".to_owned()]).expect("valid pathspec");

        let paths = ignored_status_paths(&ignored_paths, &tracked_paths, &pathspecs);

        assert_eq!(paths, set(["ignored/"]));
    }

    fn set<const N: usize>(paths: [&str; N]) -> BTreeSet<String> {
        paths.into_iter().map(ToOwned::to_owned).collect()
    }
}
