use crate::indexdb::INDEXDB_SCHEMA_VERSION;
use crate::{InitOptions, Repository};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn indexdb_uses_git_rit_storage_location() {
    let temp = temp_path("storage");
    let repository = Repository::init(&InitOptions::new(&temp)).expect("init should work");
    let storage = repository.indexdb().storage();

    assert_eq!(storage.directory, repository.common_dir().join("rit"));
    assert_eq!(
        storage.database_path,
        repository.common_dir().join("rit").join("indexdb.sqlite")
    );
    assert_eq!(
        storage.lock_path,
        repository.common_dir().join("rit").join("indexdb.lock")
    );
    remove_dir_all(&temp);
}

#[test]
fn indexdb_ensure_creates_schema_and_indexes_head_commit() {
    let temp = temp_path("ensure");
    let repository = Repository::init(&InitOptions::new(&temp)).expect("init should work");
    write_user_config(&repository);
    fs::write(temp.join("file.txt"), "hello\n").expect("file should be written");
    repository
        .add_paths(&["file.txt".to_owned()])
        .expect("add should work");
    repository
        .commit_index("initial")
        .expect("commit should work");

    let result = repository.indexdb().ensure().expect("ensure should work");
    let status = repository.indexdb().status().expect("status should work");

    assert!(result.created);
    assert_eq!(result.commits_indexed, 1);
    assert!(status.exists);
    assert!(status.healthy);
    assert_eq!(status.schema_version, Some(INDEXDB_SCHEMA_VERSION));
    assert!(status.storage.database_path.exists());
    remove_dir_all(&temp);
}

#[test]
fn indexdb_drop_does_not_break_status() {
    let temp = temp_path("drop");
    let repository = Repository::init(&InitOptions::new(&temp)).expect("init should work");
    write_user_config(&repository);
    fs::write(temp.join("file.txt"), "hello\n").expect("file should be written");
    repository
        .add_paths(&["file.txt".to_owned()])
        .expect("add should work");
    repository
        .commit_index("initial")
        .expect("commit should work");
    repository.indexdb().ensure().expect("ensure should work");
    repository.indexdb().drop().expect("drop should work");

    let status = repository
        .status_porcelain_v1()
        .expect("ordinary status should still work");

    assert_eq!(status.entries, Vec::new());
    assert!(!repository.indexdb().storage().database_path.exists());
    remove_dir_all(&temp);
}

fn write_user_config(repository: &Repository) {
    fs::write(
        repository.git_dir().join("config"),
        "[core]\n\trepositoryformatversion = 0\n\tbare = false\n[user]\n\tname = Rit Test\n\temail = rit@example.test\n",
    )
    .expect("config should be written");
}

fn temp_path(name: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("rit-indexdb-{name}-{unique}"))
}

fn remove_dir_all(path: &Path) {
    if path.exists() {
        fs::remove_dir_all(path).expect("temporary directory should be removed");
    }
}
