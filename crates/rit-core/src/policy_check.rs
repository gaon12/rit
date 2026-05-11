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

fn severity_from_enforcement(enforcement: PolicyEnforcement) -> PolicySeverity {
    match enforcement {
        PolicyEnforcement::Warn => PolicySeverity::Warning,
        PolicyEnforcement::Block => PolicySeverity::Blocking,
    }
}
