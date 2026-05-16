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
    /// Rebuild a corrupted auxiliary index database from canonical Git data.
    #[cfg(feature = "indexdb")]
    RebuildIndexDb {
        /// SQLite database path.
        path: PathBuf,
        /// Health check reason that made the database unsafe to reuse.
        reason: String,
    },
    /// Drop a corrupted auxiliary index database without rebuilding it.
    #[cfg(feature = "indexdb")]
    DropIndexDb {
        /// SQLite database path.
        path: PathBuf,
        /// Health check reason that made the database unsafe to reuse.
        reason: String,
    },
}

impl RepairAction {
    /// Returns a short human-readable action description.
    pub fn description(&self) -> String {
        match self {
            Self::CreateDirectory { path } => {
                format!("create directory {}", path.display())
            }
            #[cfg(feature = "indexdb")]
            Self::RebuildIndexDb { path, reason } => {
                format!("rebuild indexdb {} ({reason})", path.display())
            }
            #[cfg(feature = "indexdb")]
            Self::DropIndexDb { path, reason } => {
                format!("drop indexdb {} ({reason})", path.display())
            }
        }
    }
}

/// Repair behavior for a corrupted optional SQLite index database.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[cfg(feature = "indexdb")]
pub enum CorruptIndexDbRepair {
    /// Rebuild the auxiliary database from canonical Git data.
    #[default]
    Rebuild,
    /// Delete the auxiliary database and leave it absent.
    Drop,
}

/// Options for building a conservative repair plan.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RepairOptions {
    /// How to handle a corrupted optional SQLite index database.
    #[cfg(feature = "indexdb")]
    pub corrupt_indexdb: CorruptIndexDbRepair,
}

impl RepairOptions {
    /// Returns default repair options.
    pub fn new() -> Self {
        Self::default()
    }

    /// Leaves a corrupted indexdb absent instead of rebuilding it.
    #[cfg(feature = "indexdb")]
    pub fn drop_corrupt_indexdb(mut self) -> Self {
        self.corrupt_indexdb = CorruptIndexDbRepair::Drop;
        self
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
        self.repair_plan_with_options(RepairOptions::default())
    }

    /// Builds a conservative repair plan with explicit options.
    pub fn repair_plan_with_options(&self, options: RepairOptions) -> RepairPlan {
        let mut actions = Vec::new();
        for directory in standard_directories(self) {
            if !directory.is_dir() {
                actions.push(RepairAction::CreateDirectory { path: directory });
            }
        }
        add_indexdb_repair_actions(self, options, &mut actions);
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
                #[cfg(feature = "indexdb")]
                RepairAction::RebuildIndexDb { path, .. } => {
                    ensure_indexdb_action_path(self, path)?;
                    self.indexdb().repair()?;
                    applied.push(action.clone());
                }
                #[cfg(feature = "indexdb")]
                RepairAction::DropIndexDb { path, .. } => {
                    ensure_indexdb_action_path(self, path)?;
                    self.indexdb().drop()?;
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

#[cfg(feature = "indexdb")]
fn add_indexdb_repair_actions(
    repository: &Repository,
    options: RepairOptions,
    actions: &mut Vec<RepairAction>,
) {
    let Ok(status) = repository.indexdb().status() else {
        return;
    };
    if !status.exists || status.healthy {
        return;
    }

    let path = status.storage.database_path;
    let reason = indexdb_repair_reason(&status.stale_reasons);
    match options.corrupt_indexdb {
        CorruptIndexDbRepair::Rebuild => {
            actions.push(RepairAction::RebuildIndexDb { path, reason })
        }
        CorruptIndexDbRepair::Drop => actions.push(RepairAction::DropIndexDb { path, reason }),
    }
}

#[cfg(not(feature = "indexdb"))]
fn add_indexdb_repair_actions(
    _repository: &Repository,
    _options: RepairOptions,
    _actions: &mut Vec<RepairAction>,
) {
}

#[cfg(feature = "indexdb")]
fn indexdb_repair_reason(reasons: &[String]) -> String {
    if reasons.is_empty() {
        "indexdb did not pass health checks".to_owned()
    } else {
        reasons.join("; ")
    }
}

fn create_repair_directory(repository: &Repository, path: &Path) -> Result<()> {
    ensure_path_in_repository(repository, path)?;
    fs::create_dir_all(path).map_err(|source| RitError::io(path, source))
}

#[cfg(feature = "indexdb")]
fn ensure_indexdb_action_path(repository: &Repository, path: &Path) -> Result<()> {
    ensure_path_in_repository(repository, path)?;
    let expected_path = repository.indexdb().storage().database_path;
    if path == expected_path {
        return Ok(());
    }
    Err(RitError::invalid_input(format!(
        "refusing to repair unexpected indexdb path: {}",
        path.display()
    )))
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

    #[cfg(feature = "indexdb")]
    #[test]
    fn repair_plan_rebuilds_corrupt_indexdb_by_default() {
        let root = temp_path("repair-indexdb-rebuild");
        let repository = Repository::init(&InitOptions::new(&root)).expect("repo should init");
        repository.indexdb().ensure().expect("indexdb should build");
        let database_path = repository.indexdb().storage().database_path;
        fs::write(&database_path, b"not a sqlite database").expect("db should be corruptible");

        let plan = repository.repair_plan();

        assert!(matches!(
            plan.actions.as_slice(),
            [RepairAction::RebuildIndexDb { path, reason }]
                if path == &database_path && !reason.is_empty()
        ));
        let result = repository
            .apply_repair_plan(&plan)
            .expect("repair should rebuild corrupt indexdb");
        assert_eq!(result.applied, plan.actions);
        let status = repository.indexdb().status().expect("status should work");
        assert!(status.exists);
        assert!(status.healthy);
        assert!(repository.common_dir().join("objects").is_dir());
        let _ = fs::remove_dir_all(root);
    }

    #[cfg(feature = "indexdb")]
    #[test]
    fn repair_plan_can_drop_corrupt_indexdb_without_touching_git_objects() {
        let root = temp_path("repair-indexdb-drop");
        let repository = Repository::init(&InitOptions::new(&root)).expect("repo should init");
        repository.indexdb().ensure().expect("indexdb should build");
        let database_path = repository.indexdb().storage().database_path;
        let objects_dir = repository.common_dir().join("objects");
        fs::write(&database_path, b"not a sqlite database").expect("db should be corruptible");

        let plan = repository.repair_plan_with_options(RepairOptions::new().drop_corrupt_indexdb());

        assert!(matches!(
            plan.actions.as_slice(),
            [RepairAction::DropIndexDb { path, reason }]
                if path == &database_path && !reason.is_empty()
        ));
        let result = repository
            .apply_repair_plan(&plan)
            .expect("repair should drop corrupt indexdb");
        assert_eq!(result.applied, plan.actions);
        assert!(!database_path.exists());
        assert!(objects_dir.is_dir());
        let _ = fs::remove_dir_all(root);
    }

    fn temp_path(name: &str) -> PathBuf {
        let suffix = std::process::id();
        let path = std::env::temp_dir().join(format!("rit-{name}-{suffix}"));
        let _ = fs::remove_dir_all(&path);
        path
    }
}
