use crate::{GitConfig, Result, RitError};
use std::fs;
use std::path::Path;

/// Sparse-checkout pattern interpretation mode.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SparseCheckoutMode {
    /// Gitignore-like pattern list.
    Pattern,
    /// Git's cone mode, optimized around directory cones.
    Cone,
}

/// One sparse-checkout pattern line.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SparseCheckoutPattern {
    /// Original pattern text without surrounding whitespace.
    pub raw: String,
    /// Whether the line starts with `!`.
    pub negated: bool,
}

/// Read-only sparse-checkout state for a repository worktree.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SparseCheckout {
    /// Whether `core.sparseCheckout` is enabled.
    pub enabled: bool,
    /// Pattern interpretation mode from `core.sparseCheckoutCone`.
    pub mode: SparseCheckoutMode,
    /// Parsed non-empty sparse-checkout pattern lines.
    pub patterns: Vec<SparseCheckoutPattern>,
}

impl SparseCheckout {
    /// Reads sparse-checkout state from a repository config and git directory.
    pub fn read_from_git_dir(config: &GitConfig, git_dir: &Path) -> Result<Self> {
        let enabled = config.get_bool("core", "sparsecheckout", false)?;
        let mode = if config.get_bool("core", "sparsecheckoutcone", false)? {
            SparseCheckoutMode::Cone
        } else {
            SparseCheckoutMode::Pattern
        };
        let sparse_file = git_dir.join("info").join("sparse-checkout");
        Self::read_from_file(enabled, mode, &sparse_file)
    }

    /// Reads sparse-checkout patterns from one sparse-checkout file.
    pub fn read_from_file(
        enabled: bool,
        mode: SparseCheckoutMode,
        sparse_file: &Path,
    ) -> Result<Self> {
        if !sparse_file.exists() {
            return Ok(Self {
                enabled,
                mode,
                patterns: Vec::new(),
            });
        }

        let contents =
            fs::read_to_string(sparse_file).map_err(|source| RitError::io(sparse_file, source))?;
        Ok(Self {
            enabled,
            mode,
            patterns: parse_sparse_checkout_patterns(&contents),
        })
    }

    /// Returns included directories represented by cone-mode positive patterns.
    pub fn cone_directories(&self) -> Vec<&str> {
        if self.mode != SparseCheckoutMode::Cone {
            return Vec::new();
        }

        self.patterns
            .iter()
            .filter_map(|pattern| {
                if pattern.negated || !pattern.raw.starts_with('/') || !pattern.raw.ends_with('/') {
                    return None;
                }
                let directory = pattern.raw.trim_start_matches('/').trim_end_matches('/');
                if directory.is_empty() || directory.contains('*') {
                    return None;
                }
                Some(directory)
            })
            .collect()
    }
}

fn parse_sparse_checkout_patterns(contents: &str) -> Vec<SparseCheckoutPattern> {
    contents
        .lines()
        .filter_map(|line| {
            let raw = line.trim();
            if raw.is_empty() || raw.starts_with('#') {
                return None;
            }
            Some(SparseCheckoutPattern {
                raw: raw.to_owned(),
                negated: raw.starts_with('!'),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn missing_sparse_checkout_file_keeps_config_state() {
        let config = GitConfig::parse(
            r#"
            [core]
                sparseCheckout = true
            "#,
        )
        .expect("config should parse");

        let sparse = SparseCheckout::read_from_git_dir(&config, Path::new("missing-git-dir"))
            .expect("missing file should be accepted");

        assert!(sparse.enabled);
        assert_eq!(sparse.mode, SparseCheckoutMode::Pattern);
        assert!(sparse.patterns.is_empty());
    }

    #[test]
    fn reads_cone_mode_patterns_and_directories() {
        let git_dir = temp_dir("cone");
        let info_dir = git_dir.join("info");
        fs::create_dir_all(&info_dir).expect("info dir should be created");
        fs::write(
            info_dir.join("sparse-checkout"),
            "/*\n!/*/\n/src/\n!/src/*/\n/src/lib/\n",
        )
        .expect("sparse file should be written");
        let config = GitConfig::parse(
            r#"
            [core]
                sparseCheckout = true
                sparseCheckoutCone = true
            "#,
        )
        .expect("config should parse");

        let sparse =
            SparseCheckout::read_from_git_dir(&config, &git_dir).expect("sparse should parse");

        assert!(sparse.enabled);
        assert_eq!(sparse.mode, SparseCheckoutMode::Cone);
        assert_eq!(sparse.patterns.len(), 5);
        assert_eq!(sparse.cone_directories(), vec!["src", "src/lib"]);

        let _ = fs::remove_dir_all(git_dir);
    }

    fn temp_dir(name: &str) -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time should be available")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("rit-sparse-{name}-{suffix}"));
        let _ = fs::remove_dir_all(&path);
        path
    }
}
