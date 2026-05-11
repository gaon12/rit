use crate::{Result, RitError};
use std::fs;
use std::path::Path;

/// Parsed `.gitattributes` contents in file order.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct GitAttributes {
    rules: Vec<AttributeRule>,
    macros: Vec<AttributeMacro>,
}

impl GitAttributes {
    /// Reads and parses one attributes file.
    pub fn read(path: &Path) -> Result<Self> {
        let contents = fs::read_to_string(path).map_err(|source| RitError::io(path, source))?;
        Self::parse(&contents)
    }

    /// Parses the line-oriented `.gitattributes` format.
    pub fn parse(contents: &str) -> Result<Self> {
        let mut rules = Vec::new();
        let mut macros = Vec::new();

        for (line_index, raw_line) in contents.lines().enumerate() {
            let Some(line) = normalize_attribute_line(raw_line) else {
                continue;
            };
            let line_number = line_index + 1;
            let mut parts = line.split_whitespace();
            let pattern = parts.next().ok_or_else(|| {
                RitError::invalid_input(format!("empty attributes rule at line {line_number}"))
            })?;
            let assignments = parts
                .map(|token| parse_attribute_assignment(token, line_number))
                .collect::<Result<Vec<_>>>()?;
            if assignments.is_empty() {
                return Err(RitError::invalid_input(format!(
                    "attributes rule at line {line_number} has no attributes"
                )));
            }

            if let Some(name) = pattern.strip_prefix("[attr]") {
                validate_attribute_name(name, line_number)?;
                macros.push(AttributeMacro {
                    name: name.to_owned(),
                    assignments,
                });
            } else {
                rules.push(AttributeRule {
                    pattern: pattern.replace('\\', "/"),
                    assignments,
                });
            }
        }

        Ok(Self { rules, macros })
    }

    /// Returns ordinary path rules in file order.
    pub fn rules(&self) -> &[AttributeRule] {
        &self.rules
    }

    /// Returns macro definitions in file order.
    pub fn macros(&self) -> &[AttributeMacro] {
        &self.macros
    }

    /// Returns the final state for one attribute on a repository-relative path.
    pub fn state_for_path(&self, path: &str, name: &str) -> Option<AttributeState> {
        let mut state = None;
        for rule in &self.rules {
            if !attribute_pattern_matches(&rule.pattern, path) {
                continue;
            }
            for assignment in &rule.assignments {
                if assignment.name == name {
                    state = Some(assignment.state.clone());
                }
            }
        }
        state
    }
}

/// One path pattern and the attributes assigned by that line.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttributeRule {
    /// Repository-relative attributes pattern, using `/` separators.
    pub pattern: String,
    /// Attribute assignments applied by this pattern.
    pub assignments: Vec<AttributeAssignment>,
}

/// One `[attr]name` macro definition.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttributeMacro {
    /// Macro name.
    pub name: String,
    /// Assignments expanded by this macro.
    pub assignments: Vec<AttributeAssignment>,
}

/// One attribute assignment token.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttributeAssignment {
    /// Attribute name.
    pub name: String,
    /// Attribute state requested by the token.
    pub state: AttributeState,
}

/// Git attribute state as expressed by one token.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AttributeState {
    /// `name`
    Set,
    /// `-name`
    Unset,
    /// `name=value`
    Value(String),
    /// `!name`
    Unspecified,
}

fn normalize_attribute_line(line: &str) -> Option<String> {
    let trimmed = line.trim();
    if trimmed.is_empty() || trimmed.starts_with('#') {
        return None;
    }
    Some(trimmed.to_owned())
}

fn parse_attribute_assignment(token: &str, line_number: usize) -> Result<AttributeAssignment> {
    let (state, name) = if let Some(name) = token.strip_prefix('-') {
        (AttributeState::Unset, name)
    } else if let Some(name) = token.strip_prefix('!') {
        (AttributeState::Unspecified, name)
    } else if let Some((name, value)) = token.split_once('=') {
        (AttributeState::Value(value.to_owned()), name)
    } else {
        (AttributeState::Set, token)
    };

    validate_attribute_name(name, line_number)?;
    Ok(AttributeAssignment {
        name: name.to_owned(),
        state,
    })
}

fn validate_attribute_name(name: &str, line_number: usize) -> Result<()> {
    if name.is_empty()
        || name
            .chars()
            .any(|character| character.is_whitespace() || matches!(character, '!' | '='))
    {
        return Err(RitError::invalid_input(format!(
            "invalid attribute name at line {line_number}: {name}"
        )));
    }
    Ok(())
}

fn attribute_pattern_matches(pattern: &str, path: &str) -> bool {
    let pattern = pattern.trim_start_matches('/');
    if pattern.contains('/') {
        return attribute_wildcard_matches(pattern, path);
    }
    path.rsplit('/')
        .next()
        .is_some_and(|name| attribute_wildcard_matches(pattern, name))
}

fn attribute_wildcard_matches(pattern: &str, path: &str) -> bool {
    let pattern = pattern.as_bytes();
    let path = path.as_bytes();
    let mut pattern_index = 0;
    let mut path_index = 0;
    let mut last_star = None;
    let mut path_after_star = 0;

    while path_index < path.len() {
        if pattern.get(pattern_index) == Some(&b'*') {
            last_star = Some(pattern_index);
            pattern_index += 1;
            path_after_star = path_index;
        } else if pattern.get(pattern_index) == Some(&path[path_index]) {
            pattern_index += 1;
            path_index += 1;
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

#[cfg(test)]
mod tests {
    use super::{AttributeState, GitAttributes};

    #[test]
    fn parses_attribute_rules_and_states() {
        let attributes = GitAttributes::parse(
            r#"
            # text files
            *.rs text eol=lf diff=rust linguist-generated=false
            *.bin -text
            docs/*.md !diff
            "#,
        )
        .expect("attributes should parse");

        assert_eq!(attributes.rules().len(), 3);
        assert_eq!(attributes.rules()[0].pattern, "*.rs");
        assert_eq!(attributes.rules()[0].assignments[0].name, "text");
        assert_eq!(
            attributes.rules()[0].assignments[0].state,
            AttributeState::Set
        );
        assert_eq!(attributes.rules()[0].assignments[1].name, "eol");
        assert_eq!(
            attributes.rules()[0].assignments[1].state,
            AttributeState::Value("lf".to_owned())
        );
        assert_eq!(
            attributes.rules()[1].assignments[0].state,
            AttributeState::Unset
        );
        assert_eq!(
            attributes.rules()[2].assignments[0].state,
            AttributeState::Unspecified
        );
    }

    #[test]
    fn parses_attribute_macros() {
        let attributes = GitAttributes::parse("[attr]binary -diff -merge -text\n")
            .expect("attribute macro should parse");

        assert!(attributes.rules().is_empty());
        assert_eq!(attributes.macros().len(), 1);
        assert_eq!(attributes.macros()[0].name, "binary");
        assert_eq!(attributes.macros()[0].assignments.len(), 3);
    }

    #[test]
    fn rejects_attribute_rules_without_assignments() {
        let error = GitAttributes::parse("*.rs\n").expect_err("rule should fail");

        assert!(error.to_string().contains("has no attributes"));
    }

    #[test]
    fn resolves_attribute_state_for_paths() {
        let attributes = GitAttributes::parse(
            "*.rs text diff=rust\n*.bin -text\ndocs/*.md diff=markdown\nplain.txt !diff\n",
        )
        .expect("attributes should parse");

        assert_eq!(
            attributes.state_for_path("src/main.rs", "text"),
            Some(AttributeState::Set)
        );
        assert_eq!(
            attributes.state_for_path("image.bin", "text"),
            Some(AttributeState::Unset)
        );
        assert_eq!(
            attributes.state_for_path("docs/readme.md", "diff"),
            Some(AttributeState::Value("markdown".to_owned()))
        );
        assert_eq!(
            attributes.state_for_path("plain.txt", "diff"),
            Some(AttributeState::Unspecified)
        );
        assert_eq!(attributes.state_for_path("README.md", "diff"), None);
    }
}
