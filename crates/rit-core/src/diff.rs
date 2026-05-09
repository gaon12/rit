use crate::index::{Index, join_slash_path};
use crate::{ObjectKind, Repository, Result, RitError, hash_object};
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

/// Per-file line statistics.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiffFileStat {
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
        let Some(worktree) = self.worktree() else {
            return Err(RitError::invalid_input(
                "diff must be run in a repository with a working tree",
            ));
        };
        let index = Index::read(&self.git_dir().join("index"))?;
        let mut files = Vec::new();

        for entry in index.entries {
            let worktree_path = join_slash_path(worktree, &entry.path);
            let old_object = self.read_object(entry.object_id)?;
            if old_object.kind != ObjectKind::Blob {
                continue;
            }

            if !worktree_path.exists() {
                files.push(DiffFileStat {
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
                path: entry.path,
                insertions,
                deletions,
            });
        }

        Ok(DiffSummary { files })
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
}
