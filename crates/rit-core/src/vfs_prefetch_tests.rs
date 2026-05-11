use crate::{InitOptions, ObjectId, ObjectKind, Repository, VfsPrefetchObject, VfsPrefetchRequest};
use std::fs;

#[test]
fn prefetch_vfs_objects_reports_available_and_missing_objects() {
    let root = temp_path("vfs-prefetch");
    let repository = Repository::init(&InitOptions::new(&root)).expect("repo should init");
    let available_id = repository
        .loose_objects()
        .write_object(ObjectKind::Blob, b"prefetch me")
        .expect("blob should write");
    let missing_id =
        ObjectId::from_hex("1111111111111111111111111111111111111111").expect("valid oid");

    let result = repository
        .prefetch_vfs_objects(&VfsPrefetchRequest {
            objects: vec![
                VfsPrefetchObject {
                    path: "available.txt".to_owned(),
                    object_id: available_id,
                },
                VfsPrefetchObject {
                    path: "missing.txt".to_owned(),
                    object_id: missing_id,
                },
            ],
        })
        .expect("prefetch should run");

    assert_eq!(result.available.len(), 1);
    assert_eq!(result.available[0].bytes_read, 11);
    assert_eq!(result.missing.len(), 1);
    assert_eq!(result.missing[0].object_id, missing_id);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn spawn_vfs_prefetch_runs_in_background() {
    let root = temp_path("vfs-prefetch-background");
    let repository = Repository::init(&InitOptions::new(&root)).expect("repo should init");
    let object_id = repository
        .loose_objects()
        .write_object(ObjectKind::Blob, b"background")
        .expect("blob should write");

    let handle = repository.spawn_vfs_prefetch(VfsPrefetchRequest {
        objects: vec![VfsPrefetchObject {
            path: "background.txt".to_owned(),
            object_id,
        }],
    });
    let result = handle
        .join()
        .expect("background worker should not panic")
        .expect("prefetch should succeed");

    assert_eq!(result.available.len(), 1);
    assert!(result.missing.is_empty());
    let _ = fs::remove_dir_all(root);
}

fn temp_path(name: &str) -> std::path::PathBuf {
    let suffix = std::process::id();
    let path = std::env::temp_dir().join(format!("rit-{name}-{suffix}"));
    let _ = fs::remove_dir_all(&path);
    path
}
