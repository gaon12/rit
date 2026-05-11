use crate::{Repository, Result, RitError};
use std::fs;
use std::path::{Path, PathBuf};

/// A safe repository repair action.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RepairAction {
    /// Create a missing directory.
    CreateDirectory {
        /// Directory to create.
        path: PathBuf,
    },
}

impl RepairAction {
    /// Returns a short human-readable action description.
    pub fn description(&self) -> String {
        match self {
            Self::CreateDirectory { path } => {
                format!("create directory {}", path.display())
            }
        }
    }
}

/// Dry-run repair plan.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RepairPlan {
    /// Actions that would be applied.
    pub actions: Vec<RepairAction>,
}

impl RepairPlan {
    /// Returns true when no repair actions are needed.
    pub fn is_empty(&self) -> bool {
        self.actions.is_empty()
    }
}

/// Result of applying a repair plan.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RepairResult {
    /// Actions that were applied.
    pub applied: Vec<RepairAction>,
}

impl Repository {
    /// Builds a conservative repair plan without changing repository state.
    pub fn repair_plan(&self) -> RepairPlan {
        let mut actions = Vec::new();
        for directory in standard_directories(self) {
            if !directory.is_dir() {
                actions.push(RepairAction::CreateDirectory { path: directory });
            }
        }
        RepairPlan { actions }
    }

    /// Applies safe repair actions.
    pub fn apply_repair_plan(&self, plan: &RepairPlan) -> Result<RepairResult> {
        let mut applied = Vec::new();
        for action in &plan.actions {
            match action {
                RepairAction::CreateDirectory { path } => {
                    create_repair_directory(self, path)?;
                    applied.push(action.clone());
                }
            }
        }
        Ok(RepairResult { applied })
    }
}

fn standard_directories(repository: &Repository) -> Vec<PathBuf> {
    let common_dir = repository.common_dir();
    let mut directories = vec![
        common_dir.join("objects"),
        common_dir.join("objects").join("info"),
        common_dir.join("objects").join("pack"),
        common_dir.join("refs"),
        common_dir.join("refs").join("heads"),
        common_dir.join("refs").join("tags"),
        common_dir.join("hooks"),
        common_dir.join("info"),
    ];
    if repository.git_dir() != common_dir {
        directories.push(repository.git_dir().join("info"));
    }
    directories
}

fn create_repair_directory(repository: &Repository, path: &Path) -> Result<()> {
    ensure_path_in_repository(repository, path)?;
    fs::create_dir_all(path).map_err(|source| RitError::io(path, source))
}

fn ensure_path_in_repository(repository: &Repository, path: &Path) -> Result<()> {
    if path.starts_with(repository.git_dir()) || path.starts_with(repository.common_dir()) {
        return Ok(());
    }
    Err(RitError::invalid_input(format!(
        "refusing to repair path outside repository: {}",
        path.display()
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::InitOptions;

    #[test]
    fn repair_plan_is_empty_for_new_repository() {
        let root = temp_path("repair-empty");
        let repository = Repository::init(&InitOptions::new(&root)).expect("repo should init");

        assert!(repository.repair_plan().is_empty());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn repair_plan_recreates_missing_pack_directory() {
        let root = temp_path("repair-pack");
        let repository = Repository::init(&InitOptions::new(&root)).expect("repo should init");
        let pack_dir = repository.common_dir().join("objects").join("pack");
        fs::remove_dir_all(&pack_dir).expect("pack dir should be removable");

        let plan = repository.repair_plan();
        assert_eq!(
            plan.actions,
            vec![RepairAction::CreateDirectory {
                path: pack_dir.clone()
            }]
        );

        let result = repository
            .apply_repair_plan(&plan)
            .expect("repair should apply");
        assert_eq!(result.applied, plan.actions);
        assert!(pack_dir.is_dir());
        let _ = fs::remove_dir_all(root);
    }

    fn temp_path(name: &str) -> PathBuf {
        let suffix = std::process::id();
        let path = std::env::temp_dir().join(format!("rit-{name}-{suffix}"));
        let _ = fs::remove_dir_all(&path);
        path
    }
}
