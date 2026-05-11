use crate::{Result, RitError};
use std::fs;
use std::path::Path;

/// Parsed Git config entries in file order.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct GitConfig {
    entries: Vec<GitConfigEntry>,
}

/// One normalized Git config assignment.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GitConfigEntry {
    /// Lowercase section name, such as `core` or `remote`.
    pub section: String,
    /// Optional subsection, such as `origin` in `[remote "origin"]`.
    pub subsection: Option<String>,
    /// Lowercase key name.
    pub key: String,
    /// Parsed value. Key-only booleans are represented as `true`.
    pub value: String,
}

impl GitConfig {
    /// Reads and parses one config file.
    pub fn read(path: &Path) -> Result<Self> {
        let contents = fs::read_to_string(path).map_err(|source| RitError::io(path, source))?;
        Self::parse(&contents)
    }

    /// Parses a conservative subset of Git config syntax.
    pub fn parse(contents: &str) -> Result<Self> {
        let mut current_section: Option<(String, Option<String>)> = None;
        let mut entries = Vec::new();

        for (line_index, raw_line) in contents.lines().enumerate() {
            let line = strip_comment(raw_line).trim().to_owned();
            if line.is_empty() {
                continue;
            }

            if line.starts_with('[') {
                current_section = Some(parse_section_header(&line, line_index + 1)?);
                continue;
            }

            let Some((section, subsection)) = current_section.as_ref() else {
                return Err(RitError::invalid_input(format!(
                    "config key outside section at line {}",
                    line_index + 1
                )));
            };
            let (key, value) = parse_assignment(&line, line_index + 1)?;
            entries.push(GitConfigEntry {
                section: section.clone(),
                subsection: subsection.clone(),
                key,
                value,
            });
        }

        Ok(Self { entries })
    }

    /// Returns the last value for `section.key`, matching Git's common
    /// last-one-wins behavior for scalar config reads.
    pub fn get(&self, section: &str, key: &str) -> Option<&str> {
        self.get_in_subsection(section, None, key)
    }

    /// Returns the last value for `section.subsection.key`.
    pub fn get_in_subsection(
        &self,
        section: &str,
        subsection: Option<&str>,
        key: &str,
    ) -> Option<&str> {
        let section = section.to_ascii_lowercase();
        let key = key.to_ascii_lowercase();
        self.entries
            .iter()
            .rev()
            .find(|entry| {
                entry.section == section
                    && entry.subsection.as_deref() == subsection
                    && entry.key == key
            })
            .map(|entry| entry.value.as_str())
    }

    /// Returns a boolean config value using Git's common true/false spellings.
    pub fn get_bool(&self, section: &str, key: &str, default: bool) -> Result<bool> {
        match self.get(section, key) {
            Some(value) => parse_git_bool(value, &format!("{section}.{key}")),
            None => Ok(default),
        }
    }

    /// Returns a boolean value from a named subsection.
    pub fn get_bool_in_subsection(
        &self,
        section: &str,
        subsection: &str,
        key: &str,
        default: bool,
    ) -> Result<bool> {
        match self.get_in_subsection(section, Some(subsection), key) {
            Some(value) => parse_git_bool(value, &format!("{section}.{subsection}.{key}")),
            None => Ok(default),
        }
    }

    /// Returns subsections used under one section in first-seen order.
    pub fn subsections_in_section(&self, section: &str) -> Vec<&str> {
        let section = section.to_ascii_lowercase();
        let mut subsections = Vec::new();
        for entry in &self.entries {
            if entry.section == section
                && let Some(subsection) = entry.subsection.as_deref()
                && !subsections.contains(&subsection)
            {
                subsections.push(subsection);
            }
        }
        subsections
    }

    /// Returns all keys present in one section, preserving file order.
    pub fn keys_in_section(&self, section: &str) -> Vec<&str> {
        let section = section.to_ascii_lowercase();
        self.entries
            .iter()
            .filter(|entry| entry.section == section && entry.subsection.is_none())
            .map(|entry| entry.key.as_str())
            .collect()
    }
}

fn strip_comment(line: &str) -> &str {
    let mut in_quotes = false;
    let mut escaped = false;
    for (index, character) in line.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        match character {
            '\\' if in_quotes => escaped = true,
            '"' => in_quotes = !in_quotes,
            '#' | ';' if !in_quotes => return &line[..index],
            _ => {}
        }
    }
    line
}

fn parse_section_header(line: &str, line_number: usize) -> Result<(String, Option<String>)> {
    if !line.ends_with(']') {
        return Err(RitError::invalid_input(format!(
            "invalid config section header at line {line_number}"
        )));
    }

    let inner = line[1..line.len() - 1].trim();
    if inner.is_empty() {
        return Err(RitError::invalid_input(format!(
            "empty config section header at line {line_number}"
        )));
    }

    if let Some(quote_start) = inner.find('"') {
        let section = inner[..quote_start].trim();
        let quoted = inner[quote_start..].trim();
        if section.is_empty() {
            return Err(RitError::invalid_input(format!(
                "missing config section name at line {line_number}"
            )));
        }
        let subsection = unquote_value(quoted, line_number)?;
        return Ok((section.to_ascii_lowercase(), Some(subsection)));
    }

    Ok((inner.to_ascii_lowercase(), None))
}

fn parse_assignment(line: &str, line_number: usize) -> Result<(String, String)> {
    let (raw_key, raw_value) = line.split_once('=').unwrap_or((line, "true"));
    let key = raw_key.trim();
    if key.is_empty() {
        return Err(RitError::invalid_input(format!(
            "empty config key at line {line_number}"
        )));
    }
    if key
        .chars()
        .any(|character| character.is_whitespace() || character == '.')
    {
        return Err(RitError::invalid_input(format!(
            "invalid config key at line {line_number}: {key}"
        )));
    }

    let value = raw_value.trim();
    let value = if value.starts_with('"') {
        unquote_value(value, line_number)?
    } else {
        value.to_owned()
    };
    Ok((key.to_ascii_lowercase(), value))
}

fn unquote_value(value: &str, line_number: usize) -> Result<String> {
    if !value.starts_with('"') || !value.ends_with('"') {
        return Err(RitError::invalid_input(format!(
            "unterminated quoted config value at line {line_number}"
        )));
    }

    let mut output = String::new();
    let mut escaped = false;
    for character in value[1..value.len() - 1].chars() {
        if escaped {
            match character {
                '"' => output.push('"'),
                '\\' => output.push('\\'),
                'n' => output.push('\n'),
                't' => output.push('\t'),
                'b' => output.push('\u{0008}'),
                other => output.push(other),
            }
            escaped = false;
        } else if character == '\\' {
            escaped = true;
        } else {
            output.push(character);
        }
    }
    if escaped {
        return Err(RitError::invalid_input(format!(
            "unterminated config escape at line {line_number}"
        )));
    }
    Ok(output)
}

fn parse_git_bool(value: &str, name: &str) -> Result<bool> {
    match value.to_ascii_lowercase().as_str() {
        "true" | "yes" | "on" | "1" => Ok(true),
        "false" | "no" | "off" | "0" => Ok(false),
        _ => Err(RitError::invalid_input(format!(
            "invalid boolean config value for {name}: {value}"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::GitConfig;

    #[test]
    fn parses_sections_comments_and_last_value() {
        let config = GitConfig::parse(
            r#"
            [Core]
                repositoryFormatVersion = 0
                bare = false
                bare = true # last one wins
            "#,
        )
        .expect("config should parse");

        assert_eq!(config.get("core", "repositoryformatversion"), Some("0"));
        assert_eq!(config.get("core", "bare"), Some("true"));
    }

    #[test]
    fn parses_quoted_subsections_and_values() {
        let config = GitConfig::parse(
            r#"
            [remote "origin"]
                url = "https://example.test/a;still-value"
                fetch
            "#,
        )
        .expect("config should parse");

        assert_eq!(
            config.get_in_subsection("remote", Some("origin"), "url"),
            Some("https://example.test/a;still-value")
        );
        assert_eq!(
            config.get_in_subsection("remote", Some("origin"), "fetch"),
            Some("true")
        );
    }

    #[test]
    fn lists_extension_keys_in_order() {
        let config = GitConfig::parse(
            r#"
            [extensions]
                worktreeConfig = true
                objectFormat = sha1
            "#,
        )
        .expect("config should parse");

        assert_eq!(
            config.keys_in_section("extensions"),
            vec!["worktreeconfig", "objectformat"]
        );
    }

    #[test]
    fn parses_git_bool_values() {
        let config = GitConfig::parse(
            r#"
            [core]
                sparseCheckout = true
                ignorecase = off
            "#,
        )
        .expect("config should parse");

        assert!(
            config
                .get_bool("core", "sparsecheckout", false)
                .expect("bool should parse")
        );
        assert!(
            !config
                .get_bool("core", "ignorecase", true)
                .expect("bool should parse")
        );
        assert!(
            config
                .get_bool("core", "missing", true)
                .expect("default should be returned")
        );
    }
}
