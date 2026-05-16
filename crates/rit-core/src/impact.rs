use crate::workspace_hints::{codeowners_hints, nearest_package_manifest};
use crate::{
    PathspecSet, Repository, Result, SemanticDiffReport, SemanticFileCategory,
    WorkspaceRecommendationHint, semantic_report_from_paths,
};
use std::collections::BTreeSet;

const LARGE_FILE_THRESHOLD: usize = 10 * 1024 * 1024;

/// Impact analysis for one commit range.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImpactReport {
    /// User-provided range expression.
    pub range: String,
    /// Base revision resolved from the left side of the range.
    pub base: crate::ObjectId,
    /// Target revision resolved from the right side of the range.
    pub target: crate::ObjectId,
    /// Changed repository-relative paths.
    pub changed_paths: Vec<String>,
    /// Package roots affected by the changed paths.
    pub changed_packages: Vec<String>,
    /// Test paths directly changed or likely affected.
    pub affected_tests: Vec<String>,
    /// Public API path hints.
    pub public_api_changes: Vec<String>,
    /// Whether all changed paths are documentation.
    pub docs_only: bool,
    /// Large-file changes over rit's conservative reporting threshold.
    pub large_file_changes: Vec<ImpactLargeFileChange>,
    /// Reviewer hints from CODEOWNERS and semantic categories.
    pub reviewer_hints: Vec<WorkspaceRecommendationHint>,
    /// Semantic path classification reused by the impact report.
    pub semantic: SemanticDiffReport,
    /// Paths touched by commits in the range when optional indexdb data could
    /// answer the range cheaply.
    pub history_touched_paths: Vec<String>,
    /// Whether an indexdb-enabled build can accelerate follow-up history queries.
    pub indexdb_acceleration_available: bool,
    /// Whether this report used indexdb for range-level impact hints.
    pub indexdb_acceleration_used: bool,
}

/// One large file touched by an impact range.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImpactLargeFileChange {
    /// Repository-relative path.
    pub path: String,
    /// Maximum old/new size in bytes.
    pub size: usize,
}

impl Repository {
    /// Computes a read-only impact report for `<old>..<new>` or `<old>...<new>`.
    pub fn impact_report(&self, range: &str) -> Result<ImpactReport> {
        let (base_revision, target_revision) = parse_impact_range(range)?;
        let base = self.resolve_revision(base_revision)?;
        let target = self.resolve_revision(target_revision)?;
        let history_touched_paths = self.indexdb_history_touched_paths(base, target)?;
        let indexdb_acceleration_used = history_touched_paths.is_some();
        let diff = self.diff_commits_with_pathspecs(base, target, &PathspecSet::all())?;
        let changed_paths = diff
            .files
            .iter()
            .map(|file| file.path.clone())
            .collect::<Vec<_>>();
        let semantic_paths = history_touched_paths
            .as_ref()
            .filter(|paths| !paths.is_empty())
            .unwrap_or(&changed_paths);
        let semantic = semantic_report_from_paths(semantic_paths.clone());
        let changed_packages = self.changed_packages_for_paths(&changed_paths)?;
        let affected_tests = affected_tests(&changed_paths, &changed_packages);
        let public_api_changes = public_api_changes(&changed_paths);
        let docs_only = !changed_paths.is_empty()
            && semantic
                .files
                .iter()
                .all(|file| file.category == SemanticFileCategory::Docs);
        let large_file_changes = diff
            .files
            .iter()
            .filter_map(|file| {
                let size = file.old_size.max(file.new_size);
                (size >= LARGE_FILE_THRESHOLD).then(|| ImpactLargeFileChange {
                    path: file.path.clone(),
                    size,
                })
            })
            .collect();
        let reviewer_hints = self.impact_reviewer_hints(&changed_paths, &semantic)?;

        Ok(ImpactReport {
            range: range.to_owned(),
            base,
            target,
            changed_paths,
            changed_packages,
            affected_tests,
            public_api_changes,
            docs_only,
            large_file_changes,
            reviewer_hints,
            semantic,
            history_touched_paths: history_touched_paths.unwrap_or_default(),
            indexdb_acceleration_available: cfg!(feature = "indexdb"),
            indexdb_acceleration_used,
        })
    }

    fn changed_packages_for_paths(&self, paths: &[String]) -> Result<Vec<String>> {
        let Some(worktree) = self.worktree() else {
            return Ok(Vec::new());
        };
        let mut packages = BTreeSet::new();
        for path in paths {
            if let Some(manifest) = nearest_package_manifest(worktree, path)? {
                let package = manifest
                    .rsplit_once('/')
                    .map(|(parent, _)| parent)
                    .unwrap_or(".");
                packages.insert(package.to_owned());
            }
        }
        Ok(packages.into_iter().collect())
    }

    fn impact_reviewer_hints(
        &self,
        paths: &[String],
        semantic: &SemanticDiffReport,
    ) -> Result<Vec<WorkspaceRecommendationHint>> {
        let mut hints = if let Some(worktree) = self.worktree() {
            codeowners_hints(worktree, paths)?
        } else {
            Vec::new()
        };
        if semantic
            .files
            .iter()
            .any(|file| file.category == SemanticFileCategory::Tests)
        {
            hints.push(WorkspaceRecommendationHint {
                kind: "tests".to_owned(),
                path: "tests".to_owned(),
                detail: "test files changed directly".to_owned(),
            });
        }
        if semantic
            .files
            .iter()
            .any(|file| file.category == SemanticFileCategory::Code)
        {
            hints.push(WorkspaceRecommendationHint {
                kind: "code-review".to_owned(),
                path: "src".to_owned(),
                detail: "code paths changed; request implementation review".to_owned(),
            });
        }
        hints.sort_by(|left, right| {
            (&left.kind, &left.path, &left.detail).cmp(&(&right.kind, &right.path, &right.detail))
        });
        hints.dedup();
        Ok(hints)
    }

    #[cfg(feature = "indexdb")]
    fn indexdb_history_touched_paths(
        &self,
        base: crate::ObjectId,
        target: crate::ObjectId,
    ) -> Result<Option<Vec<String>>> {
        self.indexdb()
            .changed_paths_between_first_parent(base, target)
    }

    #[cfg(not(feature = "indexdb"))]
    fn indexdb_history_touched_paths(
        &self,
        _base: crate::ObjectId,
        _target: crate::ObjectId,
    ) -> Result<Option<Vec<String>>> {
        Ok(None)
    }
}

fn parse_impact_range(range: &str) -> Result<(&str, &str)> {
    if let Some((left, right)) = range.split_once("...") {
        return validate_range_sides(left, right);
    }
    if let Some((left, right)) = range.split_once("..") {
        return validate_range_sides(left, right);
    }
    Err(crate::RitError::invalid_input(
        "impact range must use <old>..<new>",
    ))
}

fn validate_range_sides<'a>(left: &'a str, right: &'a str) -> Result<(&'a str, &'a str)> {
    if left.is_empty() || right.is_empty() {
        return Err(crate::RitError::invalid_input(
            "impact range must include both revisions",
        ));
    }
    Ok((left, right))
}

fn affected_tests(changed_paths: &[String], changed_packages: &[String]) -> Vec<String> {
    let mut tests = changed_paths
        .iter()
        .filter(|path| {
            let lower = path.to_ascii_lowercase();
            lower.starts_with("tests/")
                || lower.contains("/tests/")
                || lower.ends_with("_test.rs")
                || lower.ends_with(".test.ts")
                || lower.ends_with("_test.py")
        })
        .cloned()
        .collect::<BTreeSet<_>>();
    for package in changed_packages {
        tests.insert(format!("{package}/tests"));
    }
    tests.into_iter().collect()
}

fn public_api_changes(changed_paths: &[String]) -> Vec<String> {
    changed_paths
        .iter()
        .filter(|path| {
            let lower = path.to_ascii_lowercase();
            lower.ends_with("src/lib.rs")
                || lower.ends_with("mod.rs")
                || lower.ends_with("index.ts")
                || lower.contains("/api/")
                || lower.contains("/public/")
        })
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{affected_tests, parse_impact_range, public_api_changes};
    #[cfg(feature = "indexdb")]
    use crate::{InitOptions, Repository};
    #[cfg(feature = "indexdb")]
    use std::fs;
    #[cfg(feature = "indexdb")]
    use std::path::{Path, PathBuf};
    #[cfg(feature = "indexdb")]
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn parses_two_dot_and_three_dot_ranges() {
        assert_eq!(parse_impact_range("main..HEAD").unwrap(), ("main", "HEAD"));
        assert_eq!(
            parse_impact_range("main...feature").unwrap(),
            ("main", "feature")
        );
        assert!(parse_impact_range("HEAD").is_err());
    }

    #[test]
    fn classifies_tests_and_public_api_paths() {
        let paths = vec![
            "crates/rit-core/src/lib.rs".to_owned(),
            "crates/rit-core/tests/impact.rs".to_owned(),
        ];
        assert_eq!(
            public_api_changes(&paths),
            vec!["crates/rit-core/src/lib.rs".to_owned()]
        );
        assert!(
            affected_tests(&paths, &["crates/rit-core".to_owned()])
                .contains(&"crates/rit-core/tests/impact.rs".to_owned())
        );
    }

    #[cfg(feature = "indexdb")]
    #[test]
    fn impact_uses_indexdb_for_history_touched_paths() {
        let temp = temp_path("impact-indexdb");
        let repository = Repository::init(&InitOptions::new(&temp)).expect("init should work");
        write_user_config(&repository);

        fs::write(temp.join("src.rs"), "pub fn one() {}\n").expect("file should be written");
        repository
            .add_paths(&["src.rs".to_owned()])
            .expect("add should work");
        let first = repository
            .commit_index("first")
            .expect("commit should work")
            .commit_id;

        fs::write(temp.join("src.rs"), "pub fn two() {}\n").expect("file should be updated");
        repository
            .add_paths(&["src.rs".to_owned()])
            .expect("add should work");
        let second = repository
            .commit_index("second")
            .expect("commit should work")
            .commit_id;

        repository.indexdb().ensure().expect("indexdb should build");
        let report = repository
            .impact_report(&format!("{first}..{second}"))
            .expect("impact should report");

        assert!(report.indexdb_acceleration_used);
        assert_eq!(report.history_touched_paths, vec!["src.rs".to_owned()]);
        remove_dir_all(&temp);
    }

    #[cfg(feature = "indexdb")]
    fn write_user_config(repository: &Repository) {
        fs::write(
            repository.git_dir().join("config"),
            "[core]\n\trepositoryformatversion = 0\n\tbare = false\n[user]\n\tname = Rit Test\n\temail = rit@example.test\n",
        )
        .expect("config should be written");
    }

    #[cfg(feature = "indexdb")]
    fn temp_path(name: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("rit-impact-{name}-{unique}"))
    }

    #[cfg(feature = "indexdb")]
    fn remove_dir_all(path: &Path) {
        if path.exists() {
            fs::remove_dir_all(path).expect("temporary directory should be removed");
        }
    }
}
