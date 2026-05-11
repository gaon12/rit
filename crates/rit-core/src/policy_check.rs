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
}

/// One policy finding.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PolicyFinding {
    /// Repository-relative path involved in the finding.
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::PolicyEnforcement;

    #[test]
    fn blob_size_policy_warns_by_default() {
        let policy = PolicyConfig {
            max_regular_blob_size: Some(10),
            ..PolicyConfig::default()
        };

        let finding = policy
            .check_blob_size("large.bin", 11)
            .expect("large blob should be reported");

        assert_eq!(finding.severity, PolicySeverity::Warning);
        assert_eq!(
            finding.kind,
            PolicyFindingKind::BlobTooLarge {
                limit: 10,
                actual: 11,
            }
        );
    }

    #[test]
    fn blob_size_policy_blocks_only_when_explicit() {
        let policy = PolicyConfig {
            max_regular_blob_size: Some(10),
            enforcement: PolicyEnforcement::Block,
            ..PolicyConfig::default()
        };

        assert_eq!(
            policy
                .check_blob_size("large.bin", 11)
                .expect("large blob should be reported")
                .severity,
            PolicySeverity::Blocking
        );
        assert_eq!(policy.check_blob_size("small.bin", 10), None);
    }

    #[test]
    fn secret_policy_is_disabled_by_default() {
        let policy = PolicyConfig::default();

        assert_eq!(
            policy.check_text_for_secrets(
                "config.txt",
                "token = ghp_123456789012345678901234567890123456"
            ),
            Vec::new()
        );
    }

    #[test]
    fn secret_policy_reports_without_revealing_secret() {
        let policy = PolicyConfig {
            deny_secrets: true,
            ..PolicyConfig::default()
        };

        let findings = policy.check_text_for_secrets(
            "config.txt",
            "token = ghp_123456789012345678901234567890123456",
        );

        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].severity, PolicySeverity::Warning);
        assert_eq!(
            findings[0].kind,
            PolicyFindingKind::SecretPattern {
                pattern: "GitHub token".to_owned(),
            }
        );
        assert!(!findings[0].message.contains("ghp_"));
    }

    #[test]
    fn secret_policy_blocks_only_when_explicit() {
        let policy = PolicyConfig {
            deny_secrets: true,
            enforcement: PolicyEnforcement::Block,
            ..PolicyConfig::default()
        };

        let findings = policy.check_text_for_secrets(
            "key.pem",
            "-----BEGIN OPENSSH PRIVATE KEY-----\nsecret\n-----END OPENSSH PRIVATE KEY-----",
        );

        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].severity, PolicySeverity::Blocking);
    }
}
