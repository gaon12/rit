use crate::ObjectId;
use crate::object::ObjectKind;
use crate::write::{AddPlan, CommitPlan, MergePlan, ResetPlan};

/// A common structured wrapper for write-operation dry-run plans.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WritePlan {
    /// Plan for updating the index from the working tree.
    Add(AddPlan),
    /// Plan for writing a commit from the index.
    Commit(CommitPlan),
    /// Plan for restoring index entries from `HEAD`.
    Reset(ResetPlan),
    /// Plan for merging another commit into `HEAD`.
    Merge(MergePlan),
}

/// Stable kind label for a write plan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WritePlanKind {
    /// `rit add`.
    Add,
    /// `rit commit`.
    Commit,
    /// `rit reset`.
    Reset,
    /// `rit merge`.
    Merge,
}

impl WritePlan {
    /// Returns the high-level write command represented by this plan.
    pub fn kind(&self) -> WritePlanKind {
        match self {
            Self::Add(_) => WritePlanKind::Add,
            Self::Commit(_) => WritePlanKind::Commit,
            Self::Reset(_) => WritePlanKind::Reset,
            Self::Merge(_) => WritePlanKind::Merge,
        }
    }

    /// Returns true when applying the operation would have no observable write.
    pub fn is_empty(&self) -> bool {
        match self {
            Self::Add(plan) => plan.is_empty(),
            Self::Commit(plan) => plan.is_empty(),
            Self::Reset(plan) => plan.is_empty(),
            Self::Merge(MergePlan::AlreadyUpToDate { .. }) => true,
            Self::Merge(_) => false,
        }
    }

    /// Describes the write surfaces touched by the plan.
    pub fn effects(&self) -> WritePlanEffects {
        match self {
            Self::Add(plan) => add_effects(plan),
            Self::Commit(plan) => commit_effects(plan),
            Self::Reset(plan) => reset_effects(plan),
            Self::Merge(plan) => merge_effects(plan),
        }
    }
}

impl From<AddPlan> for WritePlan {
    fn from(plan: AddPlan) -> Self {
        Self::Add(plan)
    }
}

impl From<CommitPlan> for WritePlan {
    fn from(plan: CommitPlan) -> Self {
        Self::Commit(plan)
    }
}

impl From<ResetPlan> for WritePlan {
    fn from(plan: ResetPlan) -> Self {
        Self::Reset(plan)
    }
}

impl From<MergePlan> for WritePlan {
    fn from(plan: MergePlan) -> Self {
        Self::Merge(plan)
    }
}

/// Structured write surfaces for a dry-run plan.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct WritePlanEffects {
    /// Ref updates or explicit ref no-ops.
    pub refs: Vec<PlannedRefChange>,
    /// Repository-relative paths whose index entries would change.
    pub index_paths: Vec<PlannedPathChange>,
    /// Repository-relative paths whose working-tree files would change.
    pub worktree_paths: Vec<PlannedPathChange>,
    /// Object kinds that would be written before refs move.
    pub object_writes: Vec<PlannedObjectWrite>,
    /// Hooks considered by the operation.
    pub hooks: Vec<PlannedHook>,
    /// Policy checks considered by the operation.
    pub policy_checks: Vec<PlannedPolicyCheck>,
}

/// A planned ref change.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlannedRefChange {
    /// User-facing ref label.
    pub name: String,
    /// Old object ID, when known during planning.
    pub old_id: Option<ObjectId>,
    /// New object ID, when known during planning.
    pub new_id: Option<ObjectId>,
    /// Planned ref action.
    pub action: PlannedRefAction,
}

/// Ref action planned by a write operation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PlannedRefAction {
    /// The ref is observed but would not change.
    NoChange,
    /// The ref would be created or updated.
    Update,
}

/// A planned path change in the index or working tree.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlannedPathChange {
    /// Repository-relative path.
    pub path: String,
    /// Planned path action.
    pub action: PlannedPathAction,
}

/// Path action planned by a write operation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PlannedPathAction {
    /// Add a new entry or refresh an existing entry.
    AddOrUpdate,
    /// Remove an entry or file.
    Remove,
    /// Include an indexed path in a commit snapshot.
    Commit,
    /// Restore an index entry from `HEAD`.
    Restore,
    /// Leave conflict stages or conflict markers for the user.
    Conflict,
}

/// Object write planned by a write operation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlannedObjectWrite {
    /// Git object kind.
    pub kind: ObjectKind,
    /// Related repository-relative path, when the object represents one path.
    pub path: Option<String>,
}

/// Hook considered by a write operation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlannedHook {
    /// Hook file name.
    pub name: String,
    /// Whether this hook would run for the planned options.
    pub will_run: bool,
}

/// Policy check considered by a write operation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlannedPolicyCheck {
    /// Stable check name.
    pub name: String,
    /// Whether this check is currently wired into the applying command.
    pub will_run: bool,
}

fn add_effects(plan: &AddPlan) -> WritePlanEffects {
    let mut effects = WritePlanEffects {
        policy_checks: path_content_policy_checks(),
        ..WritePlanEffects::default()
    };
    for path in &plan.paths_to_add {
        effects
            .index_paths
            .push(path_change(path, PlannedPathAction::AddOrUpdate));
        effects.object_writes.push(PlannedObjectWrite {
            kind: ObjectKind::Blob,
            path: Some(path.clone()),
        });
    }
    for path in &plan.paths_to_remove {
        effects
            .index_paths
            .push(path_change(path, PlannedPathAction::Remove));
    }
    effects
}

fn commit_effects(plan: &CommitPlan) -> WritePlanEffects {
    let mut effects = WritePlanEffects {
        policy_checks: vec![policy_check("protected-branch", false)],
        ..WritePlanEffects::default()
    };
    if !plan.is_empty() {
        effects.refs.push(PlannedRefChange {
            name: "HEAD".to_owned(),
            old_id: plan.parent_id,
            new_id: None,
            action: PlannedRefAction::Update,
        });
        effects.object_writes.push(PlannedObjectWrite {
            kind: ObjectKind::Tree,
            path: None,
        });
        effects.object_writes.push(PlannedObjectWrite {
            kind: ObjectKind::Commit,
            path: None,
        });
    }
    for path in &plan.paths_to_commit {
        effects
            .index_paths
            .push(path_change(path, PlannedPathAction::Commit));
    }
    effects.hooks = commit_hooks(plan.verify);
    effects
}

fn reset_effects(plan: &ResetPlan) -> WritePlanEffects {
    let mut effects = WritePlanEffects::default();
    for path in &plan.paths_to_restore {
        effects
            .index_paths
            .push(path_change(path, PlannedPathAction::Restore));
    }
    for path in &plan.paths_to_remove {
        effects
            .index_paths
            .push(path_change(path, PlannedPathAction::Remove));
    }
    effects
}

fn merge_effects(plan: &MergePlan) -> WritePlanEffects {
    match plan {
        MergePlan::AlreadyUpToDate { commit_id } => WritePlanEffects {
            refs: vec![PlannedRefChange {
                name: "HEAD".to_owned(),
                old_id: Some(*commit_id),
                new_id: Some(*commit_id),
                action: PlannedRefAction::NoChange,
            }],
            ..WritePlanEffects::default()
        },
        MergePlan::FastForward {
            old_id,
            new_id,
            paths_to_update,
            paths_to_remove,
        } => {
            let mut effects = WritePlanEffects {
                refs: vec![PlannedRefChange {
                    name: "HEAD".to_owned(),
                    old_id: Some(*old_id),
                    new_id: Some(*new_id),
                    action: PlannedRefAction::Update,
                }],
                policy_checks: vec![policy_check("protected-branch", false)],
                ..WritePlanEffects::default()
            };
            for path in paths_to_update {
                effects
                    .index_paths
                    .push(path_change(path, PlannedPathAction::AddOrUpdate));
                effects
                    .worktree_paths
                    .push(path_change(path, PlannedPathAction::AddOrUpdate));
            }
            for path in paths_to_remove {
                effects
                    .index_paths
                    .push(path_change(path, PlannedPathAction::Remove));
                effects
                    .worktree_paths
                    .push(path_change(path, PlannedPathAction::Remove));
            }
            effects
        }
        MergePlan::NonFastForward {
            head_id,
            target_id: _,
            conflict_paths,
            conflict_stages,
            ..
        } => {
            let mut effects = WritePlanEffects {
                refs: vec![PlannedRefChange {
                    name: "HEAD".to_owned(),
                    old_id: Some(*head_id),
                    new_id: None,
                    action: PlannedRefAction::Update,
                }],
                policy_checks: vec![policy_check("protected-branch", false)],
                ..WritePlanEffects::default()
            };
            if conflict_paths.is_empty() {
                effects.object_writes.push(PlannedObjectWrite {
                    kind: ObjectKind::Tree,
                    path: None,
                });
                effects.object_writes.push(PlannedObjectWrite {
                    kind: ObjectKind::Commit,
                    path: None,
                });
            }
            for conflict in conflict_stages {
                effects
                    .index_paths
                    .push(path_change(&conflict.path, PlannedPathAction::Conflict));
            }
            for path in conflict_paths {
                effects
                    .worktree_paths
                    .push(path_change(path, PlannedPathAction::Conflict));
            }
            effects
        }
    }
}

fn path_content_policy_checks() -> Vec<PlannedPolicyCheck> {
    vec![
        policy_check("blob-size", true),
        policy_check("secret-pattern", true),
    ]
}

fn policy_check(name: &str, will_run: bool) -> PlannedPolicyCheck {
    PlannedPolicyCheck {
        name: name.to_owned(),
        will_run,
    }
}

fn path_change(path: &str, action: PlannedPathAction) -> PlannedPathChange {
    PlannedPathChange {
        path: path.to_owned(),
        action,
    }
}

fn commit_hooks(verify: bool) -> Vec<PlannedHook> {
    vec![
        PlannedHook {
            name: "pre-commit".to_owned(),
            will_run: verify,
        },
        PlannedHook {
            name: "prepare-commit-msg".to_owned(),
            will_run: true,
        },
        PlannedHook {
            name: "commit-msg".to_owned(),
            will_run: verify,
        },
        PlannedHook {
            name: "post-commit".to_owned(),
            will_run: true,
        },
    ]
}
