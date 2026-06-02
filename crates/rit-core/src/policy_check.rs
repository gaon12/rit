use crate::{PolicyConfig, PolicyEnforcement};

/// Policy finding severity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PolicySeverity {
    /// Finding should be reported but should not block the write.
    Warning,
    /// Finding should block the write.
    Blocking,
}

/// Policy finding kind.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PolicyFindingKind {
    /// Regular Git blob exceeds configured size limit.
    BlobTooLarge {
        /// Configured size limit in bytes.
        limit: u64,
        /// Actual blob size in bytes.
        actual: u64,
    },
    /// Text content contains a known secret-looking pattern.
    SecretPattern {
        /// Human-readable pattern name, never the detected secret value.
        pattern: String,
    },
    /// A protected branch would be updated by a write operation.
    ProtectedBranch {
        /// Protected branch name.
        branch: String,
    },
}

/// One policy finding.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PolicyFinding {
    /// Repository-relative path or ref involved in the finding.
    pub path: String,
    /// Finding kind and details.
    pub kind: PolicyFindingKind,
    /// Whether this finding warns or blocks.
    pub severity: PolicySeverity,
    /// Human-readable finding message.
    pub message: String,
}

impl PolicyConfig {
    /// Checks one regular Git blob size against policy.
    pub fn check_blob_size(&self, path: impl Into<String>, size: u64) -> Option<PolicyFinding> {
        let limit = self.max_regular_blob_size?;
        if size <= limit {
            return None;
        }

        let path = path.into();
        Some(PolicyFinding {
            message: format!(
                "{path} is {size} bytes, exceeding max regular blob size {limit} bytes"
            ),
            path,
            kind: PolicyFindingKind::BlobTooLarge {
                limit,
                actual: size,
            },
            severity: severity_from_enforcement(self.enforcement),
        })
    }

    /// Checks text content for conservative secret-looking patterns.
    pub fn check_text_for_secrets(
        &self,
        path: impl Into<String>,
        contents: &str,
    ) -> Vec<PolicyFinding> {
        if !self.deny_secrets {
            return Vec::new();
        }

        let path = path.into();
        secret_patterns(contents)
            .into_iter()
            .map(|pattern| secret_finding(path.clone(), pattern, self.enforcement))
            .collect()
    }

    /// Checks whether a branch update targets a protected branch.
    pub fn check_protected_branch_update(
        &self,
        branch_name: impl Into<String>,
    ) -> Option<PolicyFinding> {
        let branch_name = branch_name.into();
        if !is_protected_branch(&self.protect_branches, &branch_name) {
            return None;
        }

        Some(PolicyFinding {
            message: format!("{branch_name} is protected by rit policy"),
            path: branch_ref_name(&branch_name),
            kind: PolicyFindingKind::ProtectedBranch {
                branch: branch_name,
            },
            severity: severity_from_enforcement(self.enforcement),
        })
    }
}

fn secret_finding(
    path: String,
    pattern: &'static str,
    enforcement: PolicyEnforcement,
) -> PolicyFinding {
    PolicyFinding {
        message: format!("{path} appears to contain a {pattern} secret pattern"),
        path,
        kind: PolicyFindingKind::SecretPattern {
            pattern: pattern.to_owned(),
        },
        severity: severity_from_enforcement(enforcement),
    }
}

fn secret_patterns(contents: &str) -> Vec<&'static str> {
    let mut patterns = Vec::new();

    if contents.contains("-----BEGIN ") && contents.contains(" PRIVATE KEY-----") {
        patterns.push("private key");
    }
    if contains_token_with_prefix(contents, "ghp_", 36)
        || contains_token_with_prefix(contents, "github_pat_", 50)
    {
        patterns.push("GitHub token");
    }
    if contains_token_with_prefix(contents, "glpat-", 20) {
        patterns.push("GitLab token");
    }
    if contains_token_with_prefix(contents, "hf_", 20) {
        patterns.push("Hugging Face token");
    }
    if contains_token_with_prefix(contents, "AKIA", 20)
        || contains_token_with_prefix(contents, "ASIA", 20)
    {
        patterns.push("AWS access key id");
    }
    if contains_high_entropy_secret(contents) {
        patterns.push("high-entropy secret");
    }

    patterns
}

fn is_protected_branch(protected_branches: &[String], branch_name: &str) -> bool {
    let branch_ref = branch_ref_name(branch_name);
    protected_branches
        .iter()
        .any(|protected| branch_ref_name(protected) == branch_ref)
}

fn branch_ref_name(branch_name: &str) -> String {
    branch_name.strip_prefix("refs/heads/").map_or_else(
        || format!("refs/heads/{branch_name}"),
        |name| format!("refs/heads/{name}"),
    )
}

fn contains_token_with_prefix(contents: &str, prefix: &str, minimum_len: usize) -> bool {
    contents
        .match_indices(prefix)
        .any(|(start, _)| token_length_at(contents, start) >= minimum_len)
}

fn token_length_at(contents: &str, start: usize) -> usize {
    contents[start..]
        .chars()
        .take_while(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '_' | '-' | '.' | '/')
        })
        .map(char::len_utf8)
        .sum()
}

fn contains_high_entropy_secret(contents: &str) -> bool {
    contents
        .lines()
        .any(line_contains_high_entropy_secret_assignment)
}

fn line_contains_high_entropy_secret_assignment(line: &str) -> bool {
    let lower_line = line.to_ascii_lowercase();
    if !SECRET_NAME_HINTS
        .iter()
        .any(|secret_name_hint| lower_line.contains(secret_name_hint))
    {
        return false;
    }

    let Some(value_text) = assignment_value(line) else {
        return false;
    };

    value_text
        .split(token_separator)
        .any(is_high_entropy_secret_candidate)
}

fn assignment_value(line: &str) -> Option<&str> {
    line.split_once('=')
        .or_else(|| line.split_once(':'))
        .map(|(_, value_text)| value_text)
}

fn token_separator(character: char) -> bool {
    character.is_whitespace()
        || matches!(
            character,
            '"' | '\'' | '`' | ';' | ',' | '[' | ']' | '(' | ')' | '{' | '}'
        )
}

fn is_high_entropy_secret_candidate(raw_candidate: &str) -> bool {
    let candidate = raw_candidate.trim_matches(|character: char| {
        matches!(
            character,
            '"' | '\'' | '`' | ';' | ',' | '.' | ':' | '[' | ']' | '(' | ')' | '{' | '}'
        )
    });

    if candidate.len() < 24 {
        return false;
    }
    if is_known_fixed_prefix_secret(candidate) {
        return false;
    }
    if looks_like_placeholder(candidate) {
        return false;
    }
    if candidate
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .count()
        < 20
    {
        return false;
    }
    if character_class_count(candidate) < 3 {
        return false;
    }

    shannon_entropy(candidate) >= 4.0
}

fn is_known_fixed_prefix_secret(candidate: &str) -> bool {
    candidate.starts_with("ghp_")
        || candidate.starts_with("github_pat_")
        || candidate.starts_with("glpat-")
        || candidate.starts_with("hf_")
        || candidate.starts_with("AKIA")
        || candidate.starts_with("ASIA")
}

fn looks_like_placeholder(candidate: &str) -> bool {
    let lower_candidate = candidate.to_ascii_lowercase();
    if SECRET_PLACEHOLDER_WORDS
        .iter()
        .any(|placeholder_word| lower_candidate.contains(placeholder_word))
    {
        return true;
    }

    let mut characters = candidate.chars();
    let Some(first_character) = characters.next() else {
        return true;
    };
    characters.all(|character| character == first_character)
}

fn character_class_count(candidate: &str) -> usize {
    let has_lowercase = candidate
        .chars()
        .any(|character| character.is_ascii_lowercase());
    let has_uppercase = candidate
        .chars()
        .any(|character| character.is_ascii_uppercase());
    let has_digit = candidate
        .chars()
        .any(|character| character.is_ascii_digit());
    let has_symbol = candidate
        .chars()
        .any(|character| character.is_ascii() && !character.is_ascii_alphanumeric());

    [has_lowercase, has_uppercase, has_digit, has_symbol]
        .into_iter()
        .filter(|has_class| *has_class)
        .count()
}

fn shannon_entropy(candidate: &str) -> f64 {
    let byte_count = candidate.len() as f64;
    let mut frequencies = [0usize; 256];
    for byte in candidate.bytes() {
        frequencies[byte as usize] += 1;
    }

    frequencies
        .into_iter()
        .filter(|frequency| *frequency > 0)
        .map(|frequency| {
            let probability = frequency as f64 / byte_count;
            -probability * probability.log2()
        })
        .sum()
}

const SECRET_NAME_HINTS: [&str; 10] = [
    "api_key",
    "apikey",
    "auth",
    "credential",
    "passwd",
    "password",
    "private_key",
    "secret",
    "token",
    "x-api-key",
];

const SECRET_PLACEHOLDER_WORDS: [&str; 8] = [
    "change_me",
    "changeme",
    "dummy",
    "example",
    "fake",
    "notasecret",
    "placeholder",
    "sample",
];

fn severity_from_enforcement(enforcement: PolicyEnforcement) -> PolicySeverity {
    match enforcement {
        PolicyEnforcement::Warn => PolicySeverity::Warning,
        PolicyEnforcement::Block => PolicySeverity::Blocking,
    }
}
