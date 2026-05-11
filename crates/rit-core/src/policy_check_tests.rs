use crate::{PolicyConfig, PolicyEnforcement, PolicyFindingKind, PolicySeverity};

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

#[test]
fn protected_branch_policy_reports_matching_branch() {
    let policy = PolicyConfig {
        protect_branches: vec!["main".to_owned()],
        ..PolicyConfig::default()
    };

    let finding = policy
        .check_protected_branch_update("main")
        .expect("main branch should be protected");

    assert_eq!(finding.severity, PolicySeverity::Warning);
    assert_eq!(finding.path, "refs/heads/main");
    assert_eq!(
        finding.kind,
        PolicyFindingKind::ProtectedBranch {
            branch: "main".to_owned(),
        }
    );
    assert_eq!(policy.check_protected_branch_update("feature"), None);
}

#[test]
fn protected_branch_policy_accepts_full_ref_names() {
    let policy = PolicyConfig {
        protect_branches: vec!["refs/heads/release".to_owned()],
        enforcement: PolicyEnforcement::Block,
        ..PolicyConfig::default()
    };

    let finding = policy
        .check_protected_branch_update("release")
        .expect("release branch should be protected");

    assert_eq!(finding.severity, PolicySeverity::Blocking);
    assert_eq!(finding.path, "refs/heads/release");
}
