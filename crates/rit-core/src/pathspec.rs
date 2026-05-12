use crate::{AttributeState, GitAttributes, Result, RitError};

/// A small, conservative subset of Git pathspec matching.
///
/// This currently supports ordinary literal file and directory pathspecs plus
/// simple `*`, `?`, and bracket-class wildcard pathspecs. The first supported
/// magic prefixes are `:(literal)`, `:(glob)`, `:(top)`, and `:/`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PathspecSet {
    patterns: Vec<PathspecPattern>,
}

/// One normalized pathspec pattern.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PathspecPattern {
    pattern: String,
    mode: PathspecMatchMode,
    ignore_case: bool,
    exclude: bool,
    attributes: Vec<AttributeRequirement>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct AttributeRequirement {
    name: String,
    state: AttributeRequirementState,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum AttributeRequirementState {
    Set,
    Unset,
    Value(String),
    Unspecified,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PathspecMatchMode {
    Default,
    Literal,
    Glob,
}

impl PathspecSet {
    /// Builds a pathspec set from user-provided CLI pathspecs.
    pub fn from_args(pathspecs: &[String]) -> Result<Self> {
        let mut patterns = Vec::new();
        for pathspec in pathspecs {
            let pattern = parse_pathspec(pathspec)?;
            if pattern.pattern != "." {
                patterns.push(pattern);
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

    /// Returns the normalized pathspec patterns.
    pub fn patterns(&self) -> &[PathspecPattern] {
        &self.patterns
    }

    /// Returns true when a repository-relative slash path matches this set.
    pub fn matches(&self, path: &str) -> bool {
        self.matches_with_attributes(path, None)
    }

    /// Returns true when a path and optional attributes match this set.
    pub fn matches_with_attributes(&self, path: &str, attributes: Option<&GitAttributes>) -> bool {
        if self.is_all() {
            return true;
        }

        let has_positive = self.patterns.iter().any(|pattern| !pattern.exclude);
        let mut matched = !has_positive;
        for pattern in &self.patterns {
            if pattern.matches_with_attributes(path, attributes) {
                if pattern.exclude {
                    return false;
                }
                matched = true;
            }
        }
        matched
    }
}

impl PathspecPattern {
    /// Returns the normalized pattern text without pathspec magic prefixes.
    pub fn pattern(&self) -> &str {
        &self.pattern
    }

    /// Returns true when this pattern contains wildcard matching syntax.
    pub fn has_wildcard(&self) -> bool {
        match self.mode {
            PathspecMatchMode::Literal => false,
            PathspecMatchMode::Default | PathspecMatchMode::Glob => {
                pattern_has_wildcard(&self.pattern)
            }
        }
    }

    /// Returns true when this pattern uses `:(icase)` matching.
    pub fn ignore_case(&self) -> bool {
        self.ignore_case
    }

    /// Returns true when this pattern removes paths from the matched set.
    pub fn is_exclude(&self) -> bool {
        self.exclude
    }

    /// Returns true when this pattern requires `.gitattributes` state.
    pub fn has_attribute_requirements(&self) -> bool {
        !self.attributes.is_empty()
    }

    /// Returns true when this pattern is a non-wildcard exact path filter.
    pub(crate) fn is_exact_path(&self, path: &str) -> bool {
        !self.has_wildcard() && self.pattern == path
    }

    /// Returns true when this pattern could refer to files below `directory`.
    pub(crate) fn starts_with_directory(&self, directory: &str) -> bool {
        !self.has_wildcard() && self.pattern.starts_with(&format!("{directory}/"))
    }

    pub(crate) fn matches_with_attributes(
        &self,
        path: &str,
        attributes: Option<&GitAttributes>,
    ) -> bool {
        if !self.attributes_match(path, attributes) {
            return false;
        }
        if self.ignore_case {
            let pattern = self.pattern.to_ascii_lowercase();
            let path = path.to_ascii_lowercase();
            return match self.mode {
                PathspecMatchMode::Default => pattern_matches(&pattern, &path),
                PathspecMatchMode::Literal => literal_pattern_matches(&pattern, &path),
                PathspecMatchMode::Glob => glob_pattern_matches(&pattern, &path),
            };
        }
        match self.mode {
            PathspecMatchMode::Default => pattern_matches(&self.pattern, path),
            PathspecMatchMode::Literal => literal_pattern_matches(&self.pattern, path),
            PathspecMatchMode::Glob => glob_pattern_matches(&self.pattern, path),
        }
    }

    fn attributes_match(&self, path: &str, attributes: Option<&GitAttributes>) -> bool {
        self.attributes.iter().all(|requirement| {
            let state = attributes
                .and_then(|attributes| attributes.state_for_path(path, &requirement.name));
            requirement.matches(state)
        })
    }
}

impl AttributeRequirement {
    fn matches(&self, state: Option<AttributeState>) -> bool {
        match (&self.state, state) {
            (AttributeRequirementState::Set, Some(AttributeState::Set)) => true,
            (AttributeRequirementState::Unset, Some(AttributeState::Unset)) => true,
            (AttributeRequirementState::Value(expected), Some(AttributeState::Value(actual))) => {
                expected == &actual
            }
            (AttributeRequirementState::Unspecified, None | Some(AttributeState::Unspecified)) => {
                true
            }
            _ => false,
        }
    }
}

pub(crate) fn pattern_has_wildcard(pattern: &str) -> bool {
    pattern.contains('*') || pattern.contains('?') || pattern.contains('[')
}

pub(crate) fn pattern_matches(pattern: &str, path: &str) -> bool {
    if pattern_has_wildcard(pattern) {
        wildcard_matches(pattern, path)
    } else {
        path == pattern || path.starts_with(&format!("{pattern}/"))
    }
}

fn literal_pattern_matches(pattern: &str, path: &str) -> bool {
    path == pattern || path.starts_with(&format!("{pattern}/"))
}

fn glob_pattern_matches(pattern: &str, path: &str) -> bool {
    if pattern_has_wildcard(pattern) {
        pathspec_glob_matches(pattern, path)
    } else {
        literal_pattern_matches(pattern, path)
    }
}

fn pathspec_glob_matches(pattern: &str, path: &str) -> bool {
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
            b'[' => match_bracket_class(pattern, pattern_index, path_byte).is_some_and(
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

fn wildcard_matches(pattern: &str, path: &str) -> bool {
    let pattern = pattern.as_bytes();
    let path = path.as_bytes();
    let mut pattern_index = 0;
    let mut path_index = 0;
    let mut last_star = None;
    let mut path_after_star = 0;

    while path_index < path.len() {
        if let Some(next_pattern_index) =
            match_single_pattern_item(pattern, pattern_index, path[path_index])
        {
            pattern_index = next_pattern_index;
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

fn match_single_pattern_item(pattern: &[u8], index: usize, path_byte: u8) -> Option<usize> {
    let pattern_byte = *pattern.get(index)?;
    match pattern_byte {
        b'?' => Some(index + 1),
        b'[' => match_bracket_class(pattern, index, path_byte),
        literal if literal == path_byte => Some(index + 1),
        _ => None,
    }
}

fn match_bracket_class(pattern: &[u8], index: usize, path_byte: u8) -> Option<usize> {
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

        if let Some((class_matches, next_cursor)) =
            match_posix_bracket_class(pattern, cursor, path_byte)
        {
            matched |= class_matches;
            cursor = next_cursor;
            continue;
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

fn match_posix_bracket_class(
    pattern: &[u8],
    cursor: usize,
    path_byte: u8,
) -> Option<(bool, usize)> {
    if pattern.get(cursor) != Some(&b'[') || pattern.get(cursor + 1) != Some(&b':') {
        return None;
    }

    let mut end = cursor + 2;
    while end + 1 < pattern.len() {
        if pattern[end] == b':' && pattern[end + 1] == b']' {
            let class_name = &pattern[cursor + 2..end];
            return Some((posix_class_matches(class_name, path_byte), end + 2));
        }
        end += 1;
    }
    None
}

fn posix_class_matches(class_name: &[u8], byte: u8) -> bool {
    match class_name {
        b"alnum" => byte.is_ascii_alphanumeric(),
        b"alpha" => byte.is_ascii_alphabetic(),
        b"blank" => matches!(byte, b' ' | b'\t'),
        b"cntrl" => byte.is_ascii_control(),
        b"digit" => byte.is_ascii_digit(),
        b"graph" => byte.is_ascii_graphic(),
        b"lower" => byte.is_ascii_lowercase(),
        b"print" => byte.is_ascii_graphic() || byte == b' ',
        b"punct" => byte.is_ascii_punctuation(),
        b"space" => byte.is_ascii_whitespace(),
        b"upper" => byte.is_ascii_uppercase(),
        b"xdigit" => byte.is_ascii_hexdigit(),
        _ => false,
    }
}

fn parse_pathspec(pathspec: &str) -> Result<PathspecPattern> {
    let normalized = pathspec.replace('\\', "/");
    if let Some(rest) = normalized.strip_prefix(":/") {
        return Ok(PathspecPattern {
            pattern: normalize_pathspec_pattern(rest, pathspec, true)?,
            mode: PathspecMatchMode::Default,
            ignore_case: false,
            exclude: false,
            attributes: Vec::new(),
        });
    }
    if let Some(pattern) = normalized
        .strip_prefix(":!")
        .or_else(|| normalized.strip_prefix(":^"))
    {
        return Ok(PathspecPattern {
            pattern: normalize_pathspec_pattern(pattern, pathspec, false)?,
            mode: PathspecMatchMode::Default,
            ignore_case: false,
            exclude: true,
            attributes: Vec::new(),
        });
    }
    if let Some(rest) = normalized.strip_prefix(":(") {
        let Some((magic, pattern)) = rest.split_once(')') else {
            return Err(RitError::invalid_input(format!(
                "unterminated pathspec magic: {pathspec}"
            )));
        };
        let mut mode = PathspecMatchMode::Default;
        let mut top = false;
        let mut ignore_case = false;
        let mut exclude = false;
        let mut attributes = Vec::new();
        for word in magic.split(',').filter(|word| !word.is_empty()) {
            match word {
                "top" => top = true,
                "literal" => mode = PathspecMatchMode::Literal,
                "glob" => mode = PathspecMatchMode::Glob,
                "icase" => ignore_case = true,
                "exclude" => exclude = true,
                word if word.starts_with("attr:") => {
                    attributes.extend(parse_attribute_requirements(
                        word.trim_start_matches("attr:"),
                        pathspec,
                    )?);
                }
                _ => {
                    return Err(RitError::invalid_input(format!(
                        "unsupported pathspec magic '{word}' in {pathspec}"
                    )));
                }
            }
        }

        return Ok(PathspecPattern {
            pattern: normalize_pathspec_pattern(pattern, pathspec, top)?,
            mode,
            ignore_case,
            exclude,
            attributes,
        });
    }

    if normalized.starts_with(':') {
        return Err(RitError::invalid_input(format!(
            "unsupported pathspec magic: {pathspec}"
        )));
    }

    Ok(PathspecPattern {
        pattern: normalize_pathspec_pattern(&normalized, pathspec, false)?,
        mode: PathspecMatchMode::Default,
        ignore_case: false,
        exclude: false,
        attributes: Vec::new(),
    })
}

fn parse_attribute_requirements(
    requirements: &str,
    pathspec: &str,
) -> Result<Vec<AttributeRequirement>> {
    let mut output = Vec::new();
    for token in requirements.split_whitespace() {
        if token.is_empty() {
            continue;
        }
        let (name, state) = if let Some(name) = token.strip_prefix('-') {
            (name, AttributeRequirementState::Unset)
        } else if let Some(name) = token.strip_prefix('!') {
            (name, AttributeRequirementState::Unspecified)
        } else if let Some((name, value)) = token.split_once('=') {
            (name, AttributeRequirementState::Value(value.to_owned()))
        } else {
            (token, AttributeRequirementState::Set)
        };
        if name.is_empty() {
            return Err(RitError::invalid_input(format!(
                "invalid attr pathspec magic in {pathspec}"
            )));
        }
        output.push(AttributeRequirement {
            name: name.to_owned(),
            state,
        });
    }
    if output.is_empty() {
        return Err(RitError::invalid_input(format!(
            "empty attr pathspec magic in {pathspec}"
        )));
    }
    Ok(output)
}

fn normalize_pathspec_pattern(pattern: &str, original: &str, top_magic: bool) -> Result<String> {
    let mut normalized = pattern.to_owned();
    while let Some(stripped) = normalized.strip_prefix("./") {
        normalized = stripped.to_owned();
    }
    if top_magic {
        while let Some(stripped) = normalized.strip_prefix('/') {
            normalized = stripped.to_owned();
        }
    }
    while normalized.len() > 1 && normalized.ends_with('/') {
        normalized.pop();
    }

    if normalized.is_empty() {
        return Ok(".".to_owned());
    }
    if normalized.starts_with('/') {
        return Err(RitError::invalid_input(format!(
            "absolute pathspecs are not supported yet: {original}"
        )));
    }
    Ok(normalized)
}

#[cfg(test)]
mod tests {
    use super::PathspecSet;
    use crate::GitAttributes;

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

    #[test]
    fn bracket_pathspec_matches_like_git_simple_globs() {
        let pathspec = PathspecSet::from_args(&["[ab].txt".to_owned()]).expect("valid pathspec");

        assert!(pathspec.matches("a.txt"));
        assert!(pathspec.matches("b.txt"));
        assert!(!pathspec.matches("c.txt"));

        let pathspec = PathspecSet::from_args(&["[a-c].txt".to_owned()]).expect("valid pathspec");

        assert!(pathspec.matches("b.txt"));
        assert!(!pathspec.matches("d.txt"));

        let pathspec = PathspecSet::from_args(&["[!a].txt".to_owned()]).expect("valid pathspec");

        assert!(pathspec.matches("b.txt"));
        assert!(!pathspec.matches("a.txt"));
    }

    #[test]
    fn posix_bracket_pathspec_matches_like_git_simple_globs() {
        let pathspec =
            PathspecSet::from_args(&["[[:digit:]].txt".to_owned()]).expect("valid pathspec");

        assert!(pathspec.matches("1.txt"));
        assert!(!pathspec.matches("a.txt"));

        let pathspec =
            PathspecSet::from_args(&["[![:digit:]].txt".to_owned()]).expect("valid pathspec");

        assert!(pathspec.matches("a.txt"));
        assert!(!pathspec.matches("1.txt"));

        let pathspec =
            PathspecSet::from_args(&["[[:upper:]].txt".to_owned()]).expect("valid pathspec");

        assert!(pathspec.matches("A.txt"));
        assert!(!pathspec.matches("a.txt"));
    }

    #[test]
    fn literal_magic_treats_wildcards_as_plain_text() {
        let pathspec =
            PathspecSet::from_args(&[":(literal)*.txt".to_owned()]).expect("valid pathspec");

        assert!(pathspec.matches("*.txt"));
        assert!(!pathspec.matches("a.txt"));
        assert!(!pathspec.matches("nested/*.txt"));
    }

    #[test]
    fn glob_magic_keeps_stars_from_matching_slashes() {
        let pathspec =
            PathspecSet::from_args(&[":(glob)*.txt".to_owned()]).expect("valid pathspec");

        assert!(pathspec.matches("a.txt"));
        assert!(!pathspec.matches("nested/a.txt"));

        let pathspec =
            PathspecSet::from_args(&[":(glob)**/*.txt".to_owned()]).expect("valid pathspec");

        assert!(pathspec.matches("a.txt"));
        assert!(pathspec.matches("nested/a.txt"));
    }

    #[test]
    fn top_magic_normalizes_to_repository_relative_paths() {
        let pathspec =
            PathspecSet::from_args(&[":(top)/nested/a.txt".to_owned()]).expect("valid pathspec");

        assert!(pathspec.matches("nested/a.txt"));

        let pathspec = PathspecSet::from_args(&[":/nested/a.txt".to_owned()])
            .expect("valid short top pathspec");

        assert!(pathspec.matches("nested/a.txt"));
    }

    #[test]
    fn icase_magic_matches_ascii_case_insensitively() {
        let pathspec =
            PathspecSet::from_args(&[":(icase)camel.txt".to_owned()]).expect("valid pathspec");

        assert!(pathspec.matches("Camel.txt"));
        assert!(pathspec.matches("CAMEL.TXT"));
        assert!(!pathspec.matches("nested/Camel.txt"));

        let pathspec =
            PathspecSet::from_args(&[":(icase,glob)*.txt".to_owned()]).expect("valid pathspec");

        assert!(pathspec.matches("Camel.txt"));
        assert!(!pathspec.matches("nested/Camel.txt"));
    }

    #[test]
    fn exclude_magic_removes_matching_paths() {
        let pathspec = PathspecSet::from_args(&["*.txt".to_owned(), ":!b.txt".to_owned()])
            .expect("valid exclude pathspec");

        assert!(pathspec.matches("a.txt"));
        assert!(!pathspec.matches("b.txt"));

        let pathspec =
            PathspecSet::from_args(&[":^b.txt".to_owned()]).expect("valid short exclude pathspec");

        assert!(pathspec.matches("a.txt"));
        assert!(!pathspec.matches("b.txt"));

        let pathspec =
            PathspecSet::from_args(&["*.txt".to_owned(), ":(exclude,icase)camel.txt".to_owned()])
                .expect("valid long exclude pathspec");

        assert!(pathspec.matches("a.txt"));
        assert!(!pathspec.matches("Camel.txt"));
    }

    #[test]
    fn attr_magic_matches_attribute_states() {
        let attributes = GitAttributes::parse(
            "*.rs text\n*.bin -text\ndocs/*.md diff=markdown\nplain.txt !diff\n",
        )
        .expect("attributes should parse");

        let pathspec =
            PathspecSet::from_args(&[":(attr:text)*".to_owned()]).expect("valid attr pathspec");
        assert!(pathspec.matches_with_attributes("main.rs", Some(&attributes)));
        assert!(!pathspec.matches_with_attributes("image.bin", Some(&attributes)));

        let pathspec =
            PathspecSet::from_args(&[":(attr:-text)*".to_owned()]).expect("valid attr pathspec");
        assert!(pathspec.matches_with_attributes("image.bin", Some(&attributes)));
        assert!(!pathspec.matches_with_attributes("main.rs", Some(&attributes)));

        let pathspec = PathspecSet::from_args(&[":(attr:diff=markdown)*".to_owned()])
            .expect("valid attr value pathspec");
        assert!(pathspec.matches_with_attributes("docs/readme.md", Some(&attributes)));
        assert!(!pathspec.matches_with_attributes("main.rs", Some(&attributes)));

        let pathspec =
            PathspecSet::from_args(&[":(attr:!diff)*".to_owned()]).expect("valid attr pathspec");
        assert!(pathspec.matches_with_attributes("main.rs", Some(&attributes)));
        assert!(pathspec.matches_with_attributes("plain.txt", Some(&attributes)));
        assert!(!pathspec.matches_with_attributes("docs/readme.md", Some(&attributes)));
    }

    #[test]
    fn unsupported_magic_returns_clear_error() {
        let error = PathspecSet::from_args(&[":(max-depth:1)README.md".to_owned()])
            .expect_err("unsupported");

        assert!(error.to_string().contains("unsupported pathspec magic"));
    }
}
