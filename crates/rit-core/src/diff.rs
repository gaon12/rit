use crate::index::{Index, join_slash_path};
use crate::object::parse_tree_entries;
use crate::{
    ObjectId, ObjectKind, PathspecSet, Repository, Result, RitError, hash_object, parse_commit,
};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;

/// Summary of working tree changes compared with the index.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiffSummary {
    /// Changed file stats in stable path order.
    pub files: Vec<DiffFileStat>,
}

impl DiffSummary {
    /// Returns changed path names only.
    pub fn name_only(&self) -> Vec<&str> {
        self.files.iter().map(|file| file.path.as_str()).collect()
    }

    /// Renders a Git-like `--name-status` summary.
    pub fn to_name_status_text(&self) -> String {
        let mut output = String::new();
        for file in &self.files {
            output.push(file.status);
            output.push('\t');
            output.push_str(&file.path);
            output.push('\n');
        }
        output
    }

    /// Renders a Git-like `--numstat` summary for text files.
    pub fn to_numstat_text(&self) -> String {
        let mut output = String::new();
        for file in &self.files {
            output.push_str(&file.insertions.to_string());
            output.push('\t');
            output.push_str(&file.deletions.to_string());
            output.push('\t');
            output.push_str(&file.path);
            output.push('\n');
        }
        output
    }

    /// Renders a Git-like `--stat` summary.
    pub fn to_stat_text(&self) -> String {
        if self.files.is_empty() {
            return String::new();
        }

        let max_path_width = self
            .files
            .iter()
            .map(|file| file.path.len())
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

        for file in &self.files {
            total_insertions += file.insertions;
            total_deletions += file.deletions;
            let mut graph = String::new();
            graph.extend(std::iter::repeat_n('+', file.insertions));
            graph.extend(std::iter::repeat_n('-', file.deletions));
            output.push_str(&format!(
                " {:path_width$} | {:change_width$} {}\n",
                file.path,
                file.changed_lines(),
                graph,
                path_width = max_path_width,
                change_width = max_change_width
            ));
        }

        output.push_str(&format!(
            " {} changed",
            plural(self.files.len(), "file", "files")
        ));
        if total_insertions > 0 {
            output.push_str(&format!(
                ", {}",
                plural(total_insertions, "insertion(+)", "insertions(+)")
            ));
        }
        if total_deletions > 0 {
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
}

impl DiffPatch {
    /// Renders a small Git-like unified patch.
    pub fn to_patch_text(&self) -> Result<String> {
        let mut output = String::new();
        for file in &self.files {
            output.push_str(&file.to_patch_text()?);
        }
        Ok(output)
    }
}

/// One file in a patch-form diff.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiffPatchFile {
    /// Git name-status code for this path.
    pub status: char,
    /// Repository-relative path using `/` separators.
    pub path: String,
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
    fn to_patch_text(&self) -> Result<String> {
        let mut output = String::new();
        output.push_str(&format!("diff --git a/{0} b/{0}\n", self.path));
        match self.status {
            'A' => {
                output.push_str(&format!("new file mode {:06o}\n", self.mode));
                output.push_str(&format!(
                    "index 0000000..{}\n",
                    short_object_id(self.new_object_id)
                ));
                output.push_str("--- /dev/null\n");
                output.push_str(&format!("+++ b/{}\n", self.path));
            }
            'D' => {
                output.push_str(&format!("deleted file mode {:06o}\n", self.mode));
                output.push_str(&format!(
                    "index {}..0000000\n",
                    short_object_id(self.old_object_id)
                ));
                output.push_str(&format!("--- a/{}\n", self.path));
                output.push_str("+++ /dev/null\n");
            }
            _ => {
                output.push_str(&format!(
                    "index {}..{} {:06o}\n",
                    short_object_id(self.old_object_id),
                    short_object_id(self.new_object_id),
                    self.mode
                ));
                output.push_str(&format!("--- a/{}\n", self.path));
                output.push_str(&format!("+++ b/{}\n", self.path));
            }
        }
        output.push_str(&unified_hunk(&self.old_data, &self.new_data)?);
        Ok(output)
    }
}

/// Per-file line statistics.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiffFileStat {
    /// Git name-status code for this path.
    pub status: char,
    /// Repository-relative path using `/` separators.
    pub path: String,
    /// Added line count.
    pub insertions: usize,
    /// Deleted line count.
    pub deletions: usize,
}

impl DiffFileStat {
    fn changed_lines(&self) -> usize {
        self.insertions + self.deletions
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
        let Some(worktree) = self.worktree() else {
            return Err(RitError::invalid_input(
                "diff must be run in a repository with a working tree",
            ));
        };
        let index = Index::read(&self.git_dir().join("index"))?;
        let mut files = Vec::new();

        for entry in index.entries {
            if !pathspecs.matches(&entry.path) {
                continue;
            }
            let worktree_path = join_slash_path(worktree, &entry.path);
            let old_object = self.read_object(entry.object_id)?;
            if old_object.kind != ObjectKind::Blob {
                continue;
            }

            if !worktree_path.exists() {
                files.push(DiffFileStat {
                    status: 'D',
                    path: entry.path,
                    insertions: 0,
                    deletions: count_lines(&old_object.data),
                });
                continue;
            }

            let new_data =
                fs::read(&worktree_path).map_err(|source| RitError::io(&worktree_path, source))?;
            let new_object_id = hash_object(ObjectKind::Blob, &new_data);
            if new_object_id == entry.object_id {
                continue;
            }

            let (insertions, deletions) = line_delta(&old_object.data, &new_data)?;
            files.push(DiffFileStat {
                status: 'M',
                path: entry.path,
                insertions,
                deletions,
            });
        }

        Ok(DiffSummary { files })
    }

    /// Computes default `git diff` patch output.
    pub fn diff_worktree_to_index_patch_with_pathspecs(
        &self,
        pathspecs: &PathspecSet,
    ) -> Result<DiffPatch> {
        let Some(worktree) = self.worktree() else {
            return Err(RitError::invalid_input(
                "diff must be run in a repository with a working tree",
            ));
        };
        let index = Index::read(&self.git_dir().join("index"))?;
        let mut files = Vec::new();

        for entry in index.entries {
            if !pathspecs.matches(&entry.path) {
                continue;
            }
            let worktree_path = join_slash_path(worktree, &entry.path);
            let old_object = self.read_object(entry.object_id)?;
            if old_object.kind != ObjectKind::Blob {
                continue;
            }

            if !worktree_path.exists() {
                files.push(DiffPatchFile {
                    status: 'D',
                    path: entry.path,
                    old_object_id: Some(entry.object_id),
                    new_object_id: None,
                    mode: entry.mode,
                    old_data: old_object.data,
                    new_data: Vec::new(),
                });
                continue;
            }

            let new_data =
                fs::read(&worktree_path).map_err(|source| RitError::io(&worktree_path, source))?;
            let new_object_id = hash_object(ObjectKind::Blob, &new_data);
            if new_object_id == entry.object_id {
                continue;
            }

            files.push(DiffPatchFile {
                status: 'M',
                path: entry.path,
                old_object_id: Some(entry.object_id),
                new_object_id: Some(new_object_id),
                mode: entry.mode,
                old_data: old_object.data,
                new_data,
            });
        }

        Ok(DiffPatch { files })
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
        let index = Index::read(&self.git_dir().join("index"))?;
        let index_entries = index
            .entries
            .iter()
            .map(|entry| (entry.path.clone(), entry.object_id))
            .collect::<BTreeMap<_, _>>();
        let head_entries = self.head_diff_entries()?;
        let paths = index_entries
            .keys()
            .chain(head_entries.keys())
            .cloned()
            .collect::<BTreeSet<_>>();
        let mut files = Vec::new();

        for path in paths {
            if !pathspecs.matches(&path) {
                continue;
            }
            match (head_entries.get(&path), index_entries.get(&path)) {
                (None, Some(new_id)) => {
                    let new_object = self.read_blob(*new_id)?;
                    files.push(DiffFileStat {
                        status: 'A',
                        path,
                        insertions: count_lines(&new_object.data),
                        deletions: 0,
                    });
                }
                (Some(old_id), None) => {
                    let old_object = self.read_blob(*old_id)?;
                    files.push(DiffFileStat {
                        status: 'D',
                        path,
                        insertions: 0,
                        deletions: count_lines(&old_object.data),
                    });
                }
                (Some(old_id), Some(new_id)) if old_id != new_id => {
                    let old_object = self.read_blob(*old_id)?;
                    let new_object = self.read_blob(*new_id)?;
                    let (insertions, deletions) = line_delta(&old_object.data, &new_object.data)?;
                    files.push(DiffFileStat {
                        status: 'M',
                        path,
                        insertions,
                        deletions,
                    });
                }
                _ => {}
            }
        }

        Ok(DiffSummary { files })
    }

    /// Computes `git diff --cached` patch output.
    pub fn diff_index_to_head_patch_with_pathspecs(
        &self,
        pathspecs: &PathspecSet,
    ) -> Result<DiffPatch> {
        let index = Index::read(&self.git_dir().join("index"))?;
        let index_entries = index
            .entries
            .iter()
            .map(|entry| (entry.path.clone(), (entry.object_id, entry.mode)))
            .collect::<BTreeMap<_, _>>();
        let head_entries = self.head_diff_entries()?;
        let paths = index_entries
            .keys()
            .chain(head_entries.keys())
            .cloned()
            .collect::<BTreeSet<_>>();
        let mut files = Vec::new();

        for path in paths {
            if !pathspecs.matches(&path) {
                continue;
            }
            match (head_entries.get(&path), index_entries.get(&path)) {
                (None, Some((new_id, mode))) => {
                    let new_object = self.read_blob(*new_id)?;
                    files.push(DiffPatchFile {
                        status: 'A',
                        path,
                        old_object_id: None,
                        new_object_id: Some(*new_id),
                        mode: *mode,
                        old_data: Vec::new(),
                        new_data: new_object.data,
                    });
                }
                (Some(old_id), None) => {
                    let old_object = self.read_blob(*old_id)?;
                    files.push(DiffPatchFile {
                        status: 'D',
                        path,
                        old_object_id: Some(*old_id),
                        new_object_id: None,
                        mode: 0o100644,
                        old_data: old_object.data,
                        new_data: Vec::new(),
                    });
                }
                (Some(old_id), Some((new_id, mode))) if old_id != new_id => {
                    let old_object = self.read_blob(*old_id)?;
                    let new_object = self.read_blob(*new_id)?;
                    files.push(DiffPatchFile {
                        status: 'M',
                        path,
                        old_object_id: Some(*old_id),
                        new_object_id: Some(*new_id),
                        mode: *mode,
                        old_data: old_object.data,
                        new_data: new_object.data,
                    });
                }
                _ => {}
            }
        }

        Ok(DiffPatch { files })
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

    fn head_diff_entries(&self) -> Result<BTreeMap<String, ObjectId>> {
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

    fn collect_diff_tree_entries(
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
                self.collect_diff_tree_entries(&path, entry.object_id, output)?;
            } else {
                output.insert(path, entry.object_id);
            }
        }

        Ok(())
    }
}

fn unified_hunk(old_data: &[u8], new_data: &[u8]) -> Result<String> {
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
    output.push_str(&format!(
        "@@ -{} +{} @@\n",
        hunk_range(1, old_lines.len()),
        hunk_range(1, new_lines.len())
    ));
    for operation in operations {
        match operation {
            LineOperation::Context(line) => {
                output.push(' ');
                output.push_str(line);
            }
            LineOperation::Delete(line) => {
                output.push('-');
                output.push_str(line);
            }
            LineOperation::Insert(line) => {
                output.push('+');
                output.push_str(line);
            }
        }
    }
    Ok(output)
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

fn short_object_id(object_id: Option<ObjectId>) -> String {
    object_id
        .map(|object_id| object_id.to_hex()[..7].to_owned())
        .unwrap_or_else(|| "0000000".to_owned())
}

fn count_lines(data: &[u8]) -> usize {
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
    use super::{DiffFileStat, DiffSummary, line_delta};
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
                path: "a.txt".to_owned(),
                insertions: 1,
                deletions: 0,
            }],
        };

        assert_eq!(
            summary.to_stat_text(),
            " a.txt | 1 +\n 1 file changed, 1 insertion(+)\n"
        );
    }

    #[test]
    fn name_status_text_lists_status_and_path() {
        let summary = DiffSummary {
            files: vec![DiffFileStat {
                status: 'A',
                path: "a.txt".to_owned(),
                insertions: 1,
                deletions: 0,
            }],
        };

        assert_eq!(summary.to_name_status_text(), "A\ta.txt\n");
    }

    #[test]
    fn numstat_text_lists_insertions_deletions_and_path() {
        let summary = DiffSummary {
            files: vec![DiffFileStat {
                status: 'M',
                path: "a.txt".to_owned(),
                insertions: 2,
                deletions: 1,
            }],
        };

        assert_eq!(summary.to_numstat_text(), "2\t1\ta.txt\n");
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
                path: "a.txt".to_owned(),
                insertions: 1,
                deletions: 0,
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
