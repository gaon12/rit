use crate::indexdb::INDEXDB_SCHEMA_VERSION;
use crate::{InitOptions, Repository};
use rusqlite::{Connection, params};
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

#[test]
fn indexdb_write_through_records_new_commit() {
    let temp = temp_path("write-through-commit");
    let repository = Repository::init(&InitOptions::new(&temp)).expect("init should work");
    write_user_config(&repository);
    fs::write(temp.join("file.txt"), "one\n").expect("file should be written");
    repository
        .add_paths(&["file.txt".to_owned()])
        .expect("add should work");
    let first = repository
        .commit_index("first")
        .expect("commit should work")
        .commit_id;
    repository.indexdb().ensure().expect("ensure should work");
    fs::write(temp.join("file.txt"), "two\n").expect("file should be modified");
    repository
        .add_paths(&["file.txt".to_owned()])
        .expect("add should work");

    let second = repository
        .commit_index("second")
        .expect("commit should update indexdb")
        .commit_id;

    {
        let connection =
            Connection::open(repository.indexdb().storage().database_path).expect("db should open");
        assert!(commit_exists(&connection, first));
        assert!(commit_exists(&connection, second));
    }
    remove_dir_all(&temp);
}

#[test]
fn indexdb_write_through_records_refs_and_checkout_state() {
    let temp = temp_path("write-through-refs");
    let repository = Repository::init(&InitOptions::new(&temp)).expect("init should work");
    write_user_config(&repository);
    fs::write(temp.join("file.txt"), "one\n").expect("file should be written");
    repository
        .add_paths(&["file.txt".to_owned()])
        .expect("add should work");
    repository
        .commit_index("first")
        .expect("commit should work");
    repository.indexdb().ensure().expect("ensure should work");

    repository
        .create_branch("topic")
        .expect("branch should update indexdb");
    repository
        .create_tag("v1")
        .expect("tag should update indexdb");
    repository
        .checkout_branch("topic")
        .expect("checkout should update indexdb");

    {
        let connection =
            Connection::open(repository.indexdb().storage().database_path).expect("db should open");
        assert_eq!(
            ref_target(&connection, "HEAD").as_deref(),
            Some("refs/heads/topic")
        );
        assert!(ref_exists(&connection, "refs/heads/topic"));
        assert!(ref_exists(&connection, "refs/tags/v1"));
    }
    remove_dir_all(&temp);
}

#[test]
fn indexdb_write_through_ignores_corrupted_database() {
    let temp = temp_path("write-through-corrupt");
    let repository = Repository::init(&InitOptions::new(&temp)).expect("init should work");
    write_user_config(&repository);
    fs::write(temp.join("file.txt"), "one\n").expect("file should be written");
    repository
        .add_paths(&["file.txt".to_owned()])
        .expect("add should work");
    repository
        .commit_index("first")
        .expect("commit should work");
    repository.indexdb().ensure().expect("ensure should work");
    fs::write(
        repository.indexdb().storage().database_path,
        "not a sqlite database",
    )
    .expect("db should be corruptible");
    fs::write(temp.join("file.txt"), "two\n").expect("file should be modified");
    repository
        .add_paths(&["file.txt".to_owned()])
        .expect("add should work");

    repository
        .commit_index("second")
        .expect("git write should still succeed when indexdb is corrupt");

    assert!(
        repository
            .resolve_head()
            .expect("head should resolve")
            .is_some()
    );
    remove_dir_all(&temp);
}

fn commit_exists(connection: &Connection, object_id: crate::ObjectId) -> bool {
    connection
        .query_row(
            "SELECT COUNT(*) FROM commits WHERE hash_kind = ?1 AND object_id = ?2",
            params!["sha1", object_id.as_bytes().to_vec()],
            |row| row.get::<_, i64>(0),
        )
        .expect("commit query should work")
        > 0
}

fn ref_exists(connection: &Connection, name: &str) -> bool {
    connection
        .query_row(
            "SELECT COUNT(*) FROM refs_snapshot WHERE name = ?1",
            params![name],
            |row| row.get::<_, i64>(0),
        )
        .expect("ref query should work")
        > 0
}

fn ref_target(connection: &Connection, name: &str) -> Option<String> {
    connection
        .query_row(
            "SELECT target FROM refs_snapshot WHERE name = ?1",
            params![name],
            |row| row.get::<_, Option<String>>(0),
        )
        .expect("ref target query should work")
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
