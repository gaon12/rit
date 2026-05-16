use crate::workspace_hints::{
    codeowners_hints, has_package_manifest, import_hints_for_path, nearest_package_manifest,
    relative_path_string,
};
use crate::{
    PathspecSet, Repository, Result, RitConfig, StatusOptions, UntrackedFilesMode, WorkspaceProfile,
};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// Source mode used to build a workspace recommendation report.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WorkspaceRecommendationMode {
    /// Recommend workspaces from the current index and working tree changes.
    CurrentChanges,
    /// Recommend workspaces from a package path provided by the user.
    PackagePath(String),
}

/// Read-only workspace recommendation report.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkspaceRecommendationReport {
    /// How the report was produced.
    pub mode: WorkspaceRecommendationMode,
    /// Repository-relative changed paths considered by the report.
    pub changed_paths: Vec<String>,
    /// Package root inferred from `from-package`, when available.
    pub package_root: Option<String>,
    /// Ranked workspace recommendations.
    pub recommendations: Vec<WorkspaceRecommendation>,
    /// Extra evidence used while ranking recommendations.
    pub hints: Vec<WorkspaceRecommendationHint>,
}

/// One ranked workspace recommendation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkspaceRecommendation {
    /// Workspace profile name or synthesized package path.
    pub workspace: String,
    /// Included paths for the workspace.
    pub include: Vec<String>,
    /// Simple deterministic score. Higher is a stronger recommendation.
    pub score: usize,
    /// Changed paths or package paths matched by this recommendation.
    pub matched_paths: Vec<String>,
    /// Human-readable reasons for the score.
    pub reasons: Vec<String>,
}

/// One recommendation hint gathered from repository metadata or source files.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkspaceRecommendationHint {
    /// Stable hint kind, such as `package-manifest` or `codeowners`.
    pub kind: String,
    /// Repository-relative path that produced the hint.
    pub path: String,
    /// Human-readable detail.
    pub detail: String,
}

impl Repository {
    /// Recommends workspace profiles from current changed files.
    pub fn workspace_suggestions(&self) -> Result<WorkspaceRecommendationReport> {
        let changed_paths = self.workspace_changed_paths()?;
        self.workspace_recommendation_from_paths(
            WorkspaceRecommendationMode::CurrentChanges,
            changed_paths,
            None,
        )
    }

    /// Recommends workspace profiles from current changed files.
    pub fn workspace_from_change(&self) -> Result<WorkspaceRecommendationReport> {
        self.workspace_suggestions()
    }

    /// Recommends workspace profiles from a package path.
    pub fn workspace_from_package(
        &self,
        package_path: &str,
    ) -> Result<WorkspaceRecommendationReport> {
        let package_root = self.package_root_for_path(package_path)?;
        let changed_paths = package_root.iter().cloned().collect::<Vec<_>>();
        self.workspace_recommendation_from_paths(
            WorkspaceRecommendationMode::PackagePath(package_path.to_owned()),
            changed_paths,
            package_root,
        )
    }

    fn workspace_recommendation_from_paths(
        &self,
        mode: WorkspaceRecommendationMode,
        changed_paths: Vec<String>,
        package_root: Option<String>,
    ) -> Result<WorkspaceRecommendationReport> {
        let config = self.rit_config()?;
        let hints = self.workspace_recommendation_hints(&changed_paths, package_root.as_deref())?;
        let mut recommendations =
            rank_workspace_profiles(&config, &changed_paths, package_root.as_deref(), &hints);
        if recommendations.is_empty()
            && let Some(package_root) = &package_root
        {
            recommendations.push(WorkspaceRecommendation {
                workspace: package_root.clone(),
                include: vec![package_root.clone()],
                score: 25,
                matched_paths: vec![package_root.clone()],
                reasons: vec![
                    "no configured workspace profile matched; package path can be used directly"
                        .to_owned(),
                ],
            });
        }
        Ok(WorkspaceRecommendationReport {
            mode,
            changed_paths,
            package_root,
            recommendations,
            hints,
        })
    }

    fn workspace_changed_paths(&self) -> Result<Vec<String>> {
        let status = self.status_porcelain_v1_with_options(
            &PathspecSet::all(),
            StatusOptions {
                untracked_files: UntrackedFilesMode::All,
                include_branch_header: false,
                include_ignored: false,
            },
        )?;
        let paths = status
            .entries
            .into_iter()
            .map(|entry| entry.path)
            .collect::<BTreeSet<_>>();
        Ok(paths.into_iter().collect())
    }

    fn package_root_for_path(&self, path: &str) -> Result<Option<String>> {
        let Some(worktree) = self.worktree() else {
            return Ok(None);
        };
        let relative = repository_relative_path(worktree, path);
        let mut current = worktree.join(relative.replace('/', std::path::MAIN_SEPARATOR_STR));
        if current.is_file() {
            current.pop();
        }
        loop {
            if has_package_manifest(&current) {
                return relative_path_string(worktree, &current).map(Some);
            }
            if !current.pop() || current == worktree {
                break;
            }
        }
        let fallback = repository_relative_path(worktree, path);
        if fallback.is_empty() {
            Ok(None)
        } else {
            Ok(Some(fallback))
        }
    }

    fn workspace_recommendation_hints(
        &self,
        changed_paths: &[String],
        package_root: Option<&str>,
    ) -> Result<Vec<WorkspaceRecommendationHint>> {
        let Some(worktree) = self.worktree() else {
            return Ok(Vec::new());
        };
        let mut hints = Vec::new();
        let mut candidate_paths = changed_paths.to_vec();
        if let Some(package_root) = package_root {
            candidate_paths.push(package_root.to_owned());
        }
        candidate_paths.sort();
        candidate_paths.dedup();

        for path in &candidate_paths {
            if let Some(manifest) = nearest_package_manifest(worktree, path)? {
                hints.push(WorkspaceRecommendationHint {
                    kind: "package-manifest".to_owned(),
                    path: manifest,
                    detail: "package/build manifest near the selected path".to_owned(),
                });
            }
            hints.extend(import_hints_for_path(worktree, path)?);
        }
        hints.extend(codeowners_hints(worktree, &candidate_paths)?);
        hints.sort_by(|left, right| {
            (&left.kind, &left.path, &left.detail).cmp(&(&right.kind, &right.path, &right.detail))
        });
        hints.dedup();
        Ok(hints)
    }
}

fn rank_workspace_profiles(
    config: &RitConfig,
    changed_paths: &[String],
    package_root: Option<&str>,
    hints: &[WorkspaceRecommendationHint],
) -> Vec<WorkspaceRecommendation> {
    let mut recommendations = config
        .workspace_profiles
        .iter()
        .filter_map(|profile| rank_workspace_profile(profile, changed_paths, package_root, hints))
        .collect::<Vec<_>>();
    recommendations.sort_by(|left, right| {
        right
            .score
            .cmp(&left.score)
            .then_with(|| left.workspace.cmp(&right.workspace))
    });
    recommendations
}

fn rank_workspace_profile(
    profile: &WorkspaceProfile,
    changed_paths: &[String],
    package_root: Option<&str>,
    hints: &[WorkspaceRecommendationHint],
) -> Option<WorkspaceRecommendation> {
    let mut score = 0;
    let mut matched_paths = BTreeSet::new();
    let mut reasons = Vec::new();
    for path in changed_paths {
        if profile_matches_path(profile, path) {
            score += 100;
            matched_paths.insert(path.clone());
        }
    }
    if !matched_paths.is_empty() {
        reasons.push(format!(
            "{} changed path(s) match configured includes",
            matched_paths.len()
        ));
    }
    if let Some(package_root) = package_root
        && (profile_matches_path(profile, package_root)
            || path_matches_profile_include(package_root, profile))
    {
        score += 60;
        matched_paths.insert(package_root.to_owned());
        reasons.push("package path is inside this workspace profile".to_owned());
    }
    let hint_count = hints
        .iter()
        .filter(|hint| profile_matches_path(profile, &hint.path))
        .count();
    if hint_count > 0 {
        score += hint_count * 10;
        reasons.push(format!(
            "{hint_count} metadata/import hint(s) match this workspace"
        ));
    }
    if score == 0 {
        return None;
    }
    Some(WorkspaceRecommendation {
        workspace: profile.name.clone(),
        include: profile.include.clone(),
        score,
        matched_paths: matched_paths.into_iter().collect(),
        reasons,
    })
}

fn profile_matches_path(profile: &WorkspaceProfile, path: &str) -> bool {
    profile
        .include
        .iter()
        .any(|include| slash_path_matches_prefix(path, include))
}

fn path_matches_profile_include(path: &str, profile: &WorkspaceProfile) -> bool {
    profile
        .include
        .iter()
        .any(|include| slash_path_matches_prefix(include, path))
}

fn slash_path_matches_prefix(path: &str, prefix: &str) -> bool {
    let path = path.trim_matches('/');
    let prefix = prefix.trim_matches('/');
    path == prefix || path.starts_with(&format!("{prefix}/"))
}

fn repository_relative_path(worktree: &Path, path: &str) -> String {
    let path = PathBuf::from(path);
    let relative = if path.is_absolute() {
        path.strip_prefix(worktree)
            .map(Path::to_path_buf)
            .unwrap_or(path)
    } else {
        path
    };
    relative.to_string_lossy().replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ranks_profiles_from_changed_paths_and_hints() {
        let config = RitConfig {
            workspace_profiles: vec![
                WorkspaceProfile {
                    name: "mobile".to_owned(),
                    include: vec!["apps/mobile".to_owned(), "packages/ui".to_owned()],
                    partial_clone: false,
                    lazy_files: false,
                },
                WorkspaceProfile {
                    name: "backend".to_owned(),
                    include: vec!["services/api".to_owned()],
                    partial_clone: false,
                    lazy_files: false,
                },
            ],
            ..RitConfig::default()
        };
        let hints = vec![WorkspaceRecommendationHint {
            kind: "package-manifest".to_owned(),
            path: "apps/mobile/Cargo.toml".to_owned(),
            detail: "manifest".to_owned(),
        }];

        let recommendations = rank_workspace_profiles(
            &config,
            &["apps/mobile/src/lib.rs".to_owned()],
            None,
            &hints,
        );

        assert_eq!(recommendations[0].workspace, "mobile");
        assert!(recommendations[0].score > 100);
        assert!(
            recommendations[0]
                .matched_paths
                .contains(&"apps/mobile/src/lib.rs".to_owned())
        );
    }
}
