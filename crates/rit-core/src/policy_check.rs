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
}
