use crate::indexdb::INDEXDB_SCHEMA_VERSION;
use crate::{InitOptions, ObjectId, ObjectKind, Repository, parse_commit};
use rusqlite::{Connection, params};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

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
    assert_eq!(
        storage.worktree_directory,
        repository.common_dir().join("rit")
    );
    assert_eq!(
        storage.worktree_cache_path,
        repository
            .common_dir()
            .join("rit")
            .join("worktree-cache.sqlite")
    );
    assert_eq!(
        storage.worktree_lock_path,
        repository
            .common_dir()
            .join("rit")
            .join("worktree-cache.lock")
    );
    remove_dir_all(&temp);
}

#[test]
fn indexdb_uses_worktree_cache_location_for_linked_worktrees() {
    let temp = temp_path("linked-worktree-storage");
    let main_worktree = temp.join("main");
    let linked_worktree = temp.join("linked");
    let repository = Repository::init(&InitOptions::new(&main_worktree)).expect("init should work");
    let linked_git_dir = repository.git_dir().join("worktrees").join("linked");
    fs::create_dir_all(&linked_git_dir).expect("linked git dir should be written");
    fs::create_dir_all(&linked_worktree).expect("linked worktree should be written");
    fs::write(
        linked_worktree.join(".git"),
        format!("gitdir: {}\n", linked_git_dir.display()),
    )
    .expect(".git file should be written");
    fs::write(linked_git_dir.join("commondir"), "../..").expect("commondir should be written");
    fs::write(linked_git_dir.join("HEAD"), "ref: refs/heads/master\n")
        .expect("linked HEAD should be written");

    let linked_repository =
        Repository::open(&linked_worktree).expect("linked worktree should open");
    let storage = linked_repository.indexdb().storage();

    assert_eq!(
        storage.database_path,
        linked_repository
            .common_dir()
            .join("rit")
            .join("indexdb.sqlite")
    );
    assert_eq!(
        storage.worktree_directory,
        linked_repository
            .common_dir()
            .join("rit")
            .join("worktrees")
            .join("linked")
    );
    assert_eq!(
        storage.worktree_cache_path,
        linked_repository
            .common_dir()
            .join("rit")
            .join("worktrees")
            .join("linked")
            .join("worktree-cache.sqlite")
    );
    assert_eq!(
        storage.worktree_lock_path,
        linked_repository
            .common_dir()
            .join("rit")
            .join("worktrees")
            .join("linked")
            .join("worktree-cache.lock")
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
    assert!(!status.stale);
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
fn indexdb_commit_query_api_reads_commits_and_parents() {
    let temp = temp_path("commit-query");
    let repository = Repository::init(&InitOptions::new(&temp)).expect("init should work");
    write_user_config(&repository);
    fs::write(temp.join("file.txt"), "one\n").expect("file should be written");
    repository
        .add_paths(&["file.txt".to_owned()])
        .expect("add should work");
    let first = repository
        .commit_index("first")
        .expect("first commit should work")
        .commit_id;
    std::thread::sleep(Duration::from_secs(1));
    fs::write(temp.join("file.txt"), "two\n").expect("file should be modified");
    repository
        .add_paths(&["file.txt".to_owned()])
        .expect("add should work");
    let second = repository
        .commit_index("second")
        .expect("second commit should work")
        .commit_id;
    repository.indexdb().ensure().expect("ensure should work");

    let indexed_first = repository
        .indexdb()
        .commit_by_id(first)
        .expect("query should work")
        .expect("first commit should be indexed");
    let indexed_second = repository
        .indexdb()
        .commit_by_id(second)
        .expect("query should work")
        .expect("second commit should be indexed");
    let missing = repository
        .indexdb()
        .commit_by_id(
            ObjectId::from_hex("0000000000000000000000000000000000000000")
                .expect("zero object id should parse"),
        )
        .expect("missing query should work");

    assert_eq!(indexed_first.hash_kind, "sha1");
    assert_eq!(indexed_first.object_id, first);
    assert_eq!(indexed_first.message, "first\n");
    assert!(indexed_first.parents.is_empty());
    assert_eq!(indexed_second.object_id, second);
    assert_eq!(indexed_second.message, "second\n");
    assert_eq!(indexed_second.parents, vec![first]);
    assert!(missing.is_none());
    remove_dir_all(&temp);
}

#[test]
fn indexdb_recent_commit_query_api_orders_newest_first() {
    let temp = temp_path("recent-commit-query");
    let repository = Repository::init(&InitOptions::new(&temp)).expect("init should work");
    write_user_config(&repository);
    fs::write(temp.join("file.txt"), "one\n").expect("file should be written");
    repository
        .add_paths(&["file.txt".to_owned()])
        .expect("add should work");
    let first = repository
        .commit_index("first")
        .expect("first commit should work")
        .commit_id;
    std::thread::sleep(Duration::from_secs(1));
    fs::write(temp.join("file.txt"), "two\n").expect("file should be modified");
    repository
        .add_paths(&["file.txt".to_owned()])
        .expect("add should work");
    let second = repository
        .commit_index("second")
        .expect("second commit should work")
        .commit_id;
    repository.indexdb().ensure().expect("ensure should work");

    let commits = repository
        .indexdb()
        .recent_commits(10)
        .expect("recent commits should load");
    let none = repository
        .indexdb()
        .recent_commits(0)
        .expect("zero limit should work");

    assert_eq!(
        commits
            .iter()
            .map(|commit| commit.object_id)
            .collect::<Vec<_>>(),
        vec![second, first]
    );
    assert!(none.is_empty());
    remove_dir_all(&temp);
}

#[test]
fn indexdb_file_history_query_api_reads_first_parent_changes() {
    let temp = temp_path("file-history-query");
    let repository = Repository::init(&InitOptions::new(&temp)).expect("init should work");
    write_user_config(&repository);
    fs::write(temp.join("file.txt"), "one\n").expect("file should be written");
    fs::write(temp.join("keep.txt"), "keep\n").expect("keep file should be written");
    repository
        .add_paths(&["file.txt".to_owned(), "keep.txt".to_owned()])
        .expect("add should work");
    let first = repository
        .commit_index("first")
        .expect("first commit should work")
        .commit_id;
    std::thread::sleep(Duration::from_secs(1));
    fs::write(temp.join("file.txt"), "two\n").expect("file should be modified");
    repository
        .add_paths(&["file.txt".to_owned()])
        .expect("add should work");
    let second = repository
        .commit_index("second")
        .expect("second commit should work")
        .commit_id;
    std::thread::sleep(Duration::from_secs(1));
    fs::remove_file(temp.join("file.txt")).expect("file should be removed");
    repository
        .add_paths(&["file.txt".to_owned()])
        .expect("delete should be staged");
    let third = repository
        .commit_index("third")
        .expect("third commit should work")
        .commit_id;
    repository.indexdb().ensure().expect("ensure should work");

    let history = repository
        .indexdb()
        .file_history("file.txt")
        .expect("file history should load");
    let missing = repository
        .indexdb()
        .file_history("missing.txt")
        .expect("missing file history should load");

    assert_eq!(history.len(), 3);
    assert_eq!(history[0].commit_id, third);
    assert_eq!(history[0].change_kind, "D");
    assert_eq!(history[0].object_id, None);
    assert_eq!(history[0].mode, None);
    assert_eq!(history[1].commit_id, second);
    assert_eq!(history[1].change_kind, "M");
    assert_eq!(history[1].mode, Some(0o100644));
    assert_eq!(history[2].commit_id, first);
    assert_eq!(history[2].change_kind, "A");
    assert_eq!(history[2].path, "file.txt");
    assert!(missing.is_empty());
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
fn indexdb_refs_snapshot_query_api_reads_head_branches_and_tags() {
    let temp = temp_path("refs-query");
    let repository = Repository::init(&InitOptions::new(&temp)).expect("init should work");
    write_user_config(&repository);
    fs::write(temp.join("file.txt"), "one\n").expect("file should be written");
    repository
        .add_paths(&["file.txt".to_owned()])
        .expect("add should work");
    let commit = repository
        .commit_index("first")
        .expect("commit should work")
        .commit_id;
    repository
        .create_branch("topic")
        .expect("branch should be created");
    repository.create_tag("v1").expect("tag should be created");
    repository.indexdb().ensure().expect("ensure should work");

    let refs = repository
        .indexdb()
        .refs_snapshot()
        .expect("refs snapshot should load");

    assert_eq!(
        refs.iter().map(|row| row.name.as_str()).collect::<Vec<_>>(),
        vec![
            "HEAD",
            "refs/heads/master",
            "refs/heads/topic",
            "refs/tags/v1"
        ]
    );
    let head = refs
        .iter()
        .find(|row| row.name == "HEAD")
        .expect("HEAD row");
    assert_eq!(head.object_id, Some(commit));
    assert_eq!(head.target.as_deref(), Some("refs/heads/master"));
    assert!(
        refs.iter()
            .filter(|row| row.name != "HEAD")
            .all(|row| row.object_id == Some(commit) && row.target.is_none())
    );
    remove_dir_all(&temp);
}

#[test]
fn indexdb_status_detects_external_ref_changes_and_ensure_reconciles_them() {
    let temp = temp_path("external-ref");
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

    let second = write_external_compatible_commit(&repository, first);
    move_current_branch_ref(&repository, second);

    let stale_status = repository.indexdb().status().expect("status should work");
    assert!(stale_status.healthy);
    assert!(stale_status.stale);
    assert!(
        stale_status
            .stale_reasons
            .iter()
            .any(|reason| reason.contains("refs snapshot is stale"))
    );

    let result = repository.indexdb().ensure().expect("ensure should update");
    assert!(result.updated);
    let fresh_status = repository.indexdb().status().expect("status should work");
    assert!(fresh_status.healthy);
    assert!(!fresh_status.stale);
    {
        let connection =
            Connection::open(repository.indexdb().storage().database_path).expect("db should open");
        assert!(commit_exists(&connection, second));
    }
    remove_dir_all(&temp);
}

#[test]
fn indexdb_status_detects_index_changes_and_ensure_reconciles_them() {
    let temp = temp_path("external-index");
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
    fs::write(temp.join("new.txt"), "new\n").expect("new file should be written");
    repository
        .add_paths(&["new.txt".to_owned()])
        .expect("index change should work");

    let stale_status = repository.indexdb().status().expect("status should work");
    assert!(stale_status.healthy);
    assert!(stale_status.stale);
    assert!(
        stale_status
            .stale_reasons
            .iter()
            .any(|reason| reason.contains("index snapshot is stale"))
    );

    repository.indexdb().ensure().expect("ensure should update");
    let fresh_status = repository.indexdb().status().expect("status should work");
    assert!(fresh_status.healthy);
    assert!(!fresh_status.stale);
    remove_dir_all(&temp);
}

#[test]
fn indexdb_status_detects_pack_snapshot_changes_and_ensure_reconciles_them() {
    let temp = temp_path("external-pack");
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
    let pack_dir = repository.common_dir().join("objects").join("pack");
    fs::create_dir_all(&pack_dir).expect("pack directory should exist");
    fs::write(
        pack_dir.join("pack-1111111111111111111111111111111111111111.pack"),
        b"pack snapshot only",
    )
    .expect("pack marker should be written");

    let stale_status = repository.indexdb().status().expect("status should work");
    assert!(stale_status.healthy);
    assert!(stale_status.stale);
    assert!(
        stale_status
            .stale_reasons
            .iter()
            .any(|reason| reason.contains("pack snapshot is stale"))
    );

    repository.indexdb().ensure().expect("ensure should update");
    let fresh_status = repository.indexdb().status().expect("status should work");
    assert!(fresh_status.healthy);
    assert!(!fresh_status.stale);
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

fn write_external_compatible_commit(
    repository: &Repository,
    parent: crate::ObjectId,
) -> crate::ObjectId {
    let parent_object = repository
        .read_object(parent)
        .expect("parent commit should be readable");
    let parent_commit = parse_commit(&parent_object.data).expect("parent commit should parse");
    let commit = format!(
        "tree {}\nparent {parent}\nauthor External Tool <external@example.test> 1700000000 +0000\ncommitter External Tool <external@example.test> 1700000000 +0000\n\nexternal compatible commit\n",
        parent_commit.tree
    );
    repository
        .loose_objects()
        .write_object(ObjectKind::Commit, commit.as_bytes())
        .expect("external-compatible commit object should be written")
}

fn move_current_branch_ref(repository: &Repository, target: crate::ObjectId) {
    let branch = repository
        .current_branch_name()
        .expect("branch should be readable")
        .expect("repository should be on a branch");
    let mut ref_path = repository.common_dir().join("refs").join("heads");
    for part in branch.split('/') {
        ref_path = ref_path.join(part);
    }
    if let Some(parent) = ref_path.parent() {
        fs::create_dir_all(parent).expect("ref parent should exist");
    }
    fs::write(ref_path, format!("{target}\n")).expect("branch ref should be moved");
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
