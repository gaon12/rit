use crate::{
    FallbackMaterializedAction, FallbackMaterializedBackend, InitOptions,
    LazyMaterializationPolicy, ObjectKind, Repository, VfsAvailability, VfsBackendPreference,
    VfsMaterializeRequest, VfsMaterializeStatus, VfsPlan, VfsPlatformBackend,
    VfsPlatformBackendPlan,
};
use std::fs;

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

#[test]
fn platform_backend_names_are_stable() {
    assert_eq!(
        VfsPlatformBackend::WindowsProjectedFileSystem.name(),
        "windows-projected-file-system"
    );
    assert_eq!(VfsPlatformBackend::MacFuse.name(), "macos-fuse");
    assert_eq!(VfsPlatformBackend::LinuxFuse.name(), "linux-fuse");
}

#[test]
fn platform_backend_plan_matches_build_availability() {
    let plan = VfsPlatformBackendPlan::current();

    assert!(plan.message.contains("VFS") || plan.message.contains("backend"));
    if cfg!(feature = "vfs") {
        assert_eq!(plan.availability, VfsAvailability::Available);
        assert!(plan.backend.is_some());
    } else {
        assert!(matches!(
            plan.availability,
            VfsAvailability::BuildDisabled { feature: "vfs" }
        ));
        assert_eq!(plan.backend, None);
    }
}

#[test]
fn materialize_vfs_blob_writes_missing_file() {
    let root = temp_path("vfs-materialize");
    let repository = Repository::init(&InitOptions::new(&root)).expect("repo should init");
    let object_id = repository
        .loose_objects()
        .write_object(ObjectKind::Blob, b"hello\n")
        .expect("blob should write");

    let result = repository
        .materialize_vfs_blob(&VfsMaterializeRequest {
            path: "src/hello.txt".to_owned(),
            object_id,
            executable: false,
        })
        .expect("blob should materialize");

    assert_eq!(result.status, VfsMaterializeStatus::Materialized);
    assert_eq!(result.bytes_written, 6);
    assert_eq!(
        fs::read_to_string(root.join("src").join("hello.txt")).expect("file should read"),
        "hello\n"
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn materialize_vfs_blob_keeps_existing_file() {
    let root = temp_path("vfs-existing");
    let repository = Repository::init(&InitOptions::new(&root)).expect("repo should init");
    fs::write(root.join("existing.txt"), "worktree").expect("file should write");
    let object_id = repository
        .loose_objects()
        .write_object(ObjectKind::Blob, b"object")
        .expect("blob should write");

    let result = repository
        .materialize_vfs_blob(&VfsMaterializeRequest {
            path: "existing.txt".to_owned(),
            object_id,
            executable: false,
        })
        .expect("existing file should be kept");

    assert_eq!(result.status, VfsMaterializeStatus::AlreadyMaterialized);
    assert_eq!(
        fs::read_to_string(root.join("existing.txt")).expect("file should read"),
        "worktree"
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn materialize_vfs_blob_rejects_path_escape() {
    let root = temp_path("vfs-escape");
    let repository = Repository::init(&InitOptions::new(&root)).expect("repo should init");
    let object_id = repository
        .loose_objects()
        .write_object(ObjectKind::Blob, b"hello")
        .expect("blob should write");

    let error = repository
        .materialize_vfs_blob(&VfsMaterializeRequest {
            path: "../outside.txt".to_owned(),
            object_id,
            executable: false,
        })
        .expect_err("path escape should fail");

    assert!(error.to_string().contains("cannot escape"));
    let _ = fs::remove_dir_all(root);
}

fn lazy_policy() -> LazyMaterializationPolicy {
    LazyMaterializationPolicy {
        workspace: "mobile".to_owned(),
        enabled: true,
        include: vec!["apps/mobile".to_owned(), "packages/ui".to_owned()],
        requires_partial_clone: true,
    }
}

fn temp_path(name: &str) -> std::path::PathBuf {
    let suffix = std::process::id();
    let path = std::env::temp_dir().join(format!("rit-{name}-{suffix}"));
    let _ = fs::remove_dir_all(&path);
    path
}
