use crate::object::ObjectKind;
use crate::write::{
    AddPlan, CommitPlan, FileModeOverride, MergeConflictStagePlan, MergePlan, ResetPlan,
};
use crate::{
    ObjectId, PlannedHook, PlannedObjectWrite, PlannedPathAction, PlannedPathChange,
    PlannedPolicyCheck, PlannedRefAction, PlannedRefChange, WritePlan, WritePlanKind,
};

fn object_id(hex: &str) -> ObjectId {
    ObjectId::from_hex(hex).expect("test object id should parse")
}

fn path_change(path: &str, action: PlannedPathAction) -> PlannedPathChange {
    PlannedPathChange {
        path: path.to_owned(),
        action,
    }
}

fn policy_check(name: &str, will_run: bool) -> PlannedPolicyCheck {
    PlannedPolicyCheck {
        name: name.to_owned(),
        will_run,
    }
}

#[test]
fn write_plan_wraps_existing_plan_kinds() {
    let add = AddPlan {
        paths_to_add: vec!["new.txt".to_owned()],
        paths_to_remove: Vec::new(),
        mode_override: Some(FileModeOverride::Executable),
    };
    let commit = CommitPlan {
        message_summary: "subject".to_owned(),
        parent_id: None,
        file_count: 1,
        paths_to_commit: vec!["new.txt".to_owned()],
        verify: true,
        author: None,
        author_date: None,
    };
    let reset = ResetPlan {
        paths_to_restore: vec!["tracked.txt".to_owned()],
        paths_to_remove: Vec::new(),
    };
    let merge = MergePlan::AlreadyUpToDate {
        commit_id: object_id("1111111111111111111111111111111111111111"),
    };

    assert_eq!(WritePlan::from(add).kind(), WritePlanKind::Add);
    assert_eq!(WritePlan::from(commit).kind(), WritePlanKind::Commit);
    assert_eq!(WritePlan::from(reset).kind(), WritePlanKind::Reset);
    assert_eq!(WritePlan::from(merge).kind(), WritePlanKind::Merge);
}

#[test]
fn add_write_plan_effects_describe_index_and_blob_writes() {
    let plan = WritePlan::from(AddPlan {
        paths_to_add: vec!["new.txt".to_owned()],
        paths_to_remove: vec!["gone.txt".to_owned()],
        mode_override: None,
    });

    assert_eq!(
        plan.effects().index_paths,
        vec![
            path_change("new.txt", PlannedPathAction::AddOrUpdate),
            path_change("gone.txt", PlannedPathAction::Remove),
        ]
    );
    assert_eq!(
        plan.effects().object_writes,
        vec![PlannedObjectWrite {
            kind: ObjectKind::Blob,
            path: Some("new.txt".to_owned()),
        }]
    );
    assert_eq!(
        plan.effects().policy_checks,
        vec![
            policy_check("blob-size", true),
            policy_check("secret-pattern", true),
        ]
    );
}

#[test]
fn commit_write_plan_effects_describe_refs_objects_and_hooks() {
    let parent = object_id("2222222222222222222222222222222222222222");
    let plan = WritePlan::from(CommitPlan {
        message_summary: "subject".to_owned(),
        parent_id: Some(parent),
        file_count: 1,
        paths_to_commit: vec!["tracked.txt".to_owned()],
        verify: false,
        author: None,
        author_date: None,
    });
    let effects = plan.effects();

    assert_eq!(
        effects.refs,
        vec![PlannedRefChange {
            name: "HEAD".to_owned(),
            old_id: Some(parent),
            new_id: None,
            action: PlannedRefAction::Update,
        }]
    );
    assert_eq!(
        effects.index_paths,
        vec![path_change("tracked.txt", PlannedPathAction::Commit)]
    );
    assert_eq!(
        effects.object_writes,
        vec![
            PlannedObjectWrite {
                kind: ObjectKind::Tree,
                path: None,
            },
            PlannedObjectWrite {
                kind: ObjectKind::Commit,
                path: None,
            },
        ]
    );
    assert_eq!(
        effects.hooks,
        vec![
            PlannedHook {
                name: "pre-commit".to_owned(),
                will_run: false,
            },
            PlannedHook {
                name: "prepare-commit-msg".to_owned(),
                will_run: true,
            },
            PlannedHook {
                name: "commit-msg".to_owned(),
                will_run: false,
            },
            PlannedHook {
                name: "post-commit".to_owned(),
                will_run: true,
            },
        ]
    );
    assert_eq!(
        effects.policy_checks,
        vec![policy_check("protected-branch", false)]
    );
}

#[test]
fn merge_write_plan_effects_describe_conflict_surfaces() {
    let head = object_id("3333333333333333333333333333333333333333");
    let target = object_id("4444444444444444444444444444444444444444");
    let plan = WritePlan::from(MergePlan::NonFastForward {
        head_id: head,
        target_id: target,
        merge_base: None,
        head_changed_paths: Vec::new(),
        target_changed_paths: Vec::new(),
        conflict_paths: vec!["both.txt".to_owned()],
        conflict_stages: vec![MergeConflictStagePlan {
            path: "both.txt".to_owned(),
            base: None,
            head: None,
            target: None,
        }],
    });
    let effects = plan.effects();

    assert_eq!(
        effects.index_paths,
        vec![path_change("both.txt", PlannedPathAction::Conflict)]
    );
    assert_eq!(
        effects.worktree_paths,
        vec![path_change("both.txt", PlannedPathAction::Conflict)]
    );
    assert!(effects.object_writes.is_empty());
    assert_eq!(
        effects.policy_checks,
        vec![policy_check("protected-branch", false)]
    );
}
