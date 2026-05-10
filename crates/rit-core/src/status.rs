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

/// Status result formatted by the CLI.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PorcelainStatus {
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

impl PorcelainStatus {
    /// Renders porcelain v1 text.
    pub fn to_porcelain_v1(&self) -> String {
        let mut output = String::new();
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
        self.status_porcelain_v1_with_options(pathspecs, UntrackedFilesMode::Normal)
    }

    /// Computes porcelain v1 status with explicit untracked-file handling.
    pub fn status_porcelain_v1_with_options(
        &self,
        pathspecs: &PathspecSet,
        untracked_files: UntrackedFilesMode,
    ) -> Result<PorcelainStatus> {
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
        let ignore_rules = IgnoreRules::read(worktree, self.common_dir())?;
        let working_files = scan_working_files(worktree, &ignore_rules)?;

        let mut entries = Vec::new();
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

        if untracked_files != UntrackedFilesMode::No {
            for path in
                untracked_status_paths(&working_files, &tracked_paths, pathspecs, untracked_files)
            {
                if !index_entries.contains_key(&path) {
                    entries.push(StatusEntry {
                        index_status: '?',
                        worktree_status: '?',
                        path,
                    });
                }
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
    use super::{
        IgnoreRules, PorcelainStatus, StatusEntry, UntrackedFilesMode, collapse_untracked_paths,
        quote_porcelain_path, untracked_status_paths,
    };
    use crate::PathspecSet;
    use std::collections::BTreeSet;

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
    fn porcelain_v1_quotes_paths_with_whitespace() {
        let status = PorcelainStatus {
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

    fn set<const N: usize>(paths: [&str; N]) -> BTreeSet<String> {
        paths.into_iter().map(ToOwned::to_owned).collect()
    }
}
