use crate::{Result, RitError};

/// A small, conservative subset of Git pathspec matching.
///
/// This currently supports ordinary literal file and directory pathspecs plus
/// simple `*` and `?` wildcard pathspecs. More advanced Git pathspec features
/// such as magic prefixes and pathspec files are deliberately left out until
/// they can be tested against Git behavior.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PathspecSet {
    patterns: Vec<String>,
}

impl PathspecSet {
    /// Builds a pathspec set from user-provided CLI pathspecs.
    pub fn from_args(pathspecs: &[String]) -> Result<Self> {
        let mut patterns = Vec::new();
        for pathspec in pathspecs {
            let normalized = normalize_pathspec(pathspec)?;
            if normalized != "." {
                patterns.push(normalized);
            }
        }
        Ok(Self { patterns })
    }

    /// Builds an empty pathspec set that matches every path.
    pub fn all() -> Self {
        Self {
            patterns: Vec::new(),
        }
    }

    /// Returns true when this set has no path filters.
    pub fn is_all(&self) -> bool {
        self.patterns.is_empty()
    }

    /// Returns the normalized literal pathspec patterns.
    pub fn patterns(&self) -> &[String] {
        &self.patterns
    }

    /// Returns true when a repository-relative slash path matches this set.
    pub fn matches(&self, path: &str) -> bool {
        self.is_all()
            || self.patterns.iter().any(|pattern| {
                if has_wildcard(pattern) {
                    wildcard_matches(pattern, path)
                } else {
                    path == pattern || path.starts_with(&format!("{pattern}/"))
                }
            })
    }
}

fn has_wildcard(pattern: &str) -> bool {
    pattern.contains('*') || pattern.contains('?')
}

fn wildcard_matches(pattern: &str, path: &str) -> bool {
    let pattern = pattern.as_bytes();
    let path = path.as_bytes();
    let mut pattern_index = 0;
    let mut path_index = 0;
    let mut last_star = None;
    let mut path_after_star = 0;

    while path_index < path.len() {
        if pattern_index < pattern.len()
            && (pattern[pattern_index] == b'?' || pattern[pattern_index] == path[path_index])
        {
            pattern_index += 1;
            path_index += 1;
        } else if pattern_index < pattern.len() && pattern[pattern_index] == b'*' {
            last_star = Some(pattern_index);
            pattern_index += 1;
            path_after_star = path_index;
        } else if let Some(star_index) = last_star {
            pattern_index = star_index + 1;
            path_after_star += 1;
            path_index = path_after_star;
        } else {
            return false;
        }
    }

    pattern[pattern_index..]
        .iter()
        .all(|character| *character == b'*')
}

fn normalize_pathspec(pathspec: &str) -> Result<String> {
    let mut normalized = pathspec.replace('\\', "/");
    while let Some(stripped) = normalized.strip_prefix("./") {
        normalized = stripped.to_owned();
    }
    while normalized.len() > 1 && normalized.ends_with('/') {
        normalized.pop();
    }

    if normalized.is_empty() {
        return Err(RitError::invalid_input("empty pathspec"));
    }
    if normalized.starts_with('/') {
        return Err(RitError::invalid_input(format!(
            "absolute pathspecs are not supported yet: {pathspec}"
        )));
    }
    if normalized.contains(':') {
        return Err(RitError::invalid_input(format!(
            "pathspec magic is not supported yet: {pathspec}"
        )));
    }

    Ok(normalized)
}

#[cfg(test)]
mod tests {
    use super::PathspecSet;

    #[test]
    fn empty_pathspec_matches_every_path() {
        let pathspec = PathspecSet::all();

        assert!(pathspec.matches("a.txt"));
        assert!(pathspec.matches("nested/a.txt"));
    }

    #[test]
    fn literal_pathspec_matches_file_or_directory_contents() {
        let pathspec = PathspecSet::from_args(&["nested".to_owned()]).expect("valid pathspec");

        assert!(pathspec.matches("nested/a.txt"));
        assert!(pathspec.matches("nested"));
        assert!(!pathspec.matches("other/nested/a.txt"));
    }

    #[test]
    fn dot_pathspec_matches_every_path() {
        let pathspec = PathspecSet::from_args(&[".".to_owned()]).expect("valid pathspec");

        assert!(pathspec.matches("a.txt"));
        assert!(pathspec.matches("nested/a.txt"));
    }

    #[test]
    fn wildcard_pathspec_matches_like_git_simple_globs() {
        let pathspec = PathspecSet::from_args(&["*.txt".to_owned()]).expect("valid pathspec");

        assert!(pathspec.matches("a.txt"));
        assert!(pathspec.matches("nested/a.txt"));
        assert!(!pathspec.matches("nested/a.md"));

        let pathspec =
            PathspecSet::from_args(&["nested/?.txt".to_owned()]).expect("valid pathspec");

        assert!(pathspec.matches("nested/a.txt"));
        assert!(!pathspec.matches("nested/ab.txt"));
    }
}
