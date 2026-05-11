use crate::{
    FallbackMaterializedAction, FallbackMaterializedBackend, LazyMaterializationPolicy,
    VfsAvailability, VfsBackendPreference, VfsPlan,
};

#[test]
fn disabled_plan_uses_fallback_materialized_backend() {
    let plan = VfsPlan::disabled();

    assert_eq!(plan.backend, VfsBackendPreference::FallbackMaterialized);
    assert!(!plan.lazy_materialization.enabled);
    assert!(!plan.needs_unavailable_vfs());
}

#[test]
fn plan_from_lazy_policy_keeps_workspace_paths() {
    let policy = lazy_policy();

    let plan = VfsPlan::from_lazy_policy(&policy, VfsBackendPreference::Auto, true);

    assert_eq!(plan.workspace.as_deref(), Some("mobile"));
    assert_eq!(plan.lazy_materialization.include, policy.include);
    assert!(plan.lazy_materialization.requires_partial_clone);
    assert!(plan.background_prefetch);
}

#[test]
fn availability_message_is_clear() {
    let availability = VfsAvailability::current();

    assert!(availability.message().contains("VFS"));
}

#[test]
fn fallback_backend_keeps_full_worktree_when_no_paths_are_configured() {
    let plan = VfsPlan::disabled();
    let fallback = FallbackMaterializedBackend;

    assert_eq!(
        fallback.plan(&plan).actions,
        vec![FallbackMaterializedAction::KeepFullWorktreeMaterialized]
    );
}

#[test]
fn fallback_backend_keeps_workspace_paths_materialized() {
    let policy = lazy_policy();
    let plan = VfsPlan::from_lazy_policy(&policy, VfsBackendPreference::FallbackMaterialized, true);
    let fallback_plan = FallbackMaterializedBackend.plan(&plan);

    assert_eq!(fallback_plan.workspace.as_deref(), Some("mobile"));
    assert_eq!(
        fallback_plan.actions,
        vec![
            FallbackMaterializedAction::KeepPathMaterialized {
                path: "apps/mobile".to_owned(),
            },
            FallbackMaterializedAction::KeepPathMaterialized {
                path: "packages/ui".to_owned(),
            },
        ]
    );
    assert!(fallback_plan.partial_clone_required);
    assert!(fallback_plan.background_prefetch_requested);
}

fn lazy_policy() -> LazyMaterializationPolicy {
    LazyMaterializationPolicy {
        workspace: "mobile".to_owned(),
        enabled: true,
        include: vec!["apps/mobile".to_owned(), "packages/ui".to_owned()],
        requires_partial_clone: true,
    }
}
