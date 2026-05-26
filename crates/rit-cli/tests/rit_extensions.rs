use rit_core::{InitOptions, Repository};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn undo_refuses_dirty_tracked_changes_by_default() {
    let fixture = RecordedCommitFixture::new("undo-dirty-tracked");

    fs::write(fixture.path.join("tracked.txt"), "dirty local change\n")
        .expect("dirty tracked file should be written");

    let output = run_rit(&fixture.path, ["undo"]);

    assert_eq!(output.status.code(), Some(1));
    assert!(
        output
            .stderr
            .contains("local staged, modified, or untracked changes"),
        "{}",
        output.stderr
    );
    assert_eq!(
        fixture
            .repository
            .resolve_head()
            .expect("head should read after refused undo"),
        Some(fixture.next_head)
    );
    assert_eq!(
        fs::read_to_string(fixture.path.join("tracked.txt")).expect("tracked file should read"),
        "dirty local change\n"
    );
}

#[test]
fn undo_refuses_dirty_untracked_changes_by_default() {
    let fixture = RecordedCommitFixture::new("undo-dirty-untracked");

    fs::write(fixture.path.join("local.txt"), "untracked\n")
        .expect("untracked file should be written");

    let output = run_rit(&fixture.path, ["undo"]);

    assert_eq!(output.status.code(), Some(1));
    assert!(
        output
            .stderr
            .contains("local staged, modified, or untracked changes"),
        "{}",
        output.stderr
    );
    assert_eq!(
        fixture
            .repository
            .resolve_head()
            .expect("head should read after refused undo"),
        Some(fixture.next_head)
    );
    assert!(fixture.path.join("local.txt").exists());
}

#[test]
fn undo_force_overrides_dirty_guard() {
    let fixture = RecordedCommitFixture::new("undo-dirty-force");

    fs::write(fixture.path.join("tracked.txt"), "dirty local change\n")
        .expect("dirty tracked file should be written");

    let output = run_rit(&fixture.path, ["undo", "--force"]);

    assert_eq!(output.status.code(), Some(0), "{}", output.stderr);
    assert!(
        output.stdout.contains("Undid operation"),
        "{}",
        output.stdout
    );
    assert_eq!(
        fixture
            .repository
            .resolve_head()
            .expect("head should read after forced undo"),
        Some(fixture.base_head)
    );
    assert_eq!(
        fs::read_to_string(fixture.path.join("tracked.txt")).expect("tracked file should read"),
        "base\n"
    );
}

#[test]
fn undo_preserve_changes_bypasses_dirty_guard_for_commit_records() {
    let fixture = RecordedCommitFixture::new("undo-preserve-dirty");

    fs::write(fixture.path.join("tracked.txt"), "dirty local change\n")
        .expect("dirty tracked file should be written");

    let output = run_rit(&fixture.path, ["undo", "--preserve-changes"]);

    assert_eq!(output.status.code(), Some(0), "{}", output.stderr);
    assert!(
        output
            .stdout
            .contains("keeping the staged and working tree changes"),
        "{}",
        output.stdout
    );
    assert_eq!(
        fixture
            .repository
            .resolve_head()
            .expect("head should read after preserve undo"),
        Some(fixture.base_head)
    );
    assert_eq!(
        fs::read_to_string(fixture.path.join("tracked.txt")).expect("tracked file should read"),
        "dirty local change\n"
    );
}

#[test]
fn op_restore_force_overrides_dirty_guard() {
    let fixture = RecordedCommitFixture::new("op-restore-dirty-force");

    fs::write(fixture.path.join("tracked.txt"), "dirty local change\n")
        .expect("dirty tracked file should be written");

    let refused = run_rit(&fixture.path, ["op", "restore", &fixture.record_id]);
    assert_eq!(refused.status.code(), Some(1));
    assert!(
        refused
            .stderr
            .contains("local staged, modified, or untracked changes"),
        "{}",
        refused.stderr
    );

    let forced = run_rit(
        &fixture.path,
        ["op", "restore", &fixture.record_id, "--force"],
    );
    assert_eq!(forced.status.code(), Some(0), "{}", forced.stderr);
    assert_eq!(
        fixture
            .repository
            .resolve_head()
            .expect("head should read after forced restore"),
        Some(fixture.base_head)
    );
    assert_eq!(
        fs::read_to_string(fixture.path.join("tracked.txt")).expect("tracked file should read"),
        "base\n"
    );
}

#[test]
fn diff_semantic_output_reports_changed_path_categories() {
    let fixture = SemanticDiffFixture::new("semantic-diff-output");

    let output = run_rit(&fixture.path, ["diff", "--semantic"]);

    assert_eq!(output.status.code(), Some(0), "{}", output.stderr);
    assert!(output.stdout.contains("semantic summary:"));
    assert!(
        output.stdout.contains("- docs: README.md"),
        "{}",
        output.stdout
    );
    assert!(
        output.stdout.contains("- code: src/lib.rs"),
        "{}",
        output.stdout
    );
    assert!(
        output.stdout.contains("- tests: tests/lib_test.rs"),
        "{}",
        output.stdout
    );
}

#[test]
fn diff_semantic_output_honors_pathspec_filters() {
    let fixture = SemanticDiffFixture::new("semantic-diff-pathspec");

    let output = run_rit(&fixture.path, ["diff", "--semantic", "--", "src/lib.rs"]);

    assert_eq!(output.status.code(), Some(0), "{}", output.stderr);
    assert!(
        output.stdout.contains("- code: src/lib.rs"),
        "{}",
        output.stdout
    );
    assert!(!output.stdout.contains("README.md"), "{}", output.stdout);
    assert!(
        !output.stdout.contains("tests/lib_test.rs"),
        "{}",
        output.stdout
    );
}

#[test]
fn diff_semantic_output_rejects_standard_output_modes() {
    let fixture = SemanticDiffFixture::new("semantic-diff-output-mode");

    let output = run_rit(&fixture.path, ["diff", "--semantic", "--name-only"]);

    assert_eq!(output.status.code(), Some(129));
    assert!(
        output
            .stderr
            .contains("semantic summary or one standard output mode"),
        "{}",
        output.stderr
    );
}

struct RecordedCommitFixture {
    path: PathBuf,
    repository: Repository,
    record_id: String,
    base_head: rit_core::ObjectId,
    next_head: rit_core::ObjectId,
}

impl RecordedCommitFixture {
    fn new(label: &str) -> Self {
        let path = temp_path(label);
        let repository = Repository::init(&InitOptions::new(&path)).expect("repo should init");
        configure_identity(&path);

        fs::write(path.join("tracked.txt"), "base\n").expect("base file should be written");
        repository
            .add_paths(&["tracked.txt".to_owned()])
            .expect("base file should be added");
        let base_head = repository
            .commit_index("base")
            .expect("base commit should work")
            .commit_id;

        fs::write(path.join("tracked.txt"), "changed\n").expect("changed file should be written");
        repository
            .add_paths(&["tracked.txt".to_owned()])
            .expect("changed file should be staged");
        let before = repository
            .operations()
            .snapshot()
            .expect("before snapshot should work");
        let next_head = repository
            .commit_index("next")
            .expect("next commit should work")
            .commit_id;
        let after = repository
            .operations()
            .snapshot()
            .expect("after snapshot should work");
        let record = repository
            .operations()
            .record_with_details(
                "commit",
                "commit next",
                before,
                after,
                vec!["tracked.txt".to_owned()],
                vec![next_head],
            )
            .expect("record should append");

        Self {
            path,
            repository,
            record_id: record.id,
            base_head,
            next_head,
        }
    }
}

impl Drop for RecordedCommitFixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

struct SemanticDiffFixture {
    path: PathBuf,
}

impl SemanticDiffFixture {
    fn new(label: &str) -> Self {
        let path = temp_path(label);
        let repository = Repository::init(&InitOptions::new(&path)).expect("repo should init");
        configure_identity(&path);

        fs::create_dir_all(path.join("src")).expect("src dir should exist");
        fs::create_dir_all(path.join("tests")).expect("tests dir should exist");
        fs::write(path.join("README.md"), "base docs\n").expect("readme should be written");
        fs::write(path.join("src").join("lib.rs"), "pub fn base() {}\n")
            .expect("lib should be written");
        fs::write(
            path.join("tests").join("lib_test.rs"),
            "#[test]\nfn base() {}\n",
        )
        .expect("test file should be written");
        repository
            .add_paths(&[
                "README.md".to_owned(),
                "src/lib.rs".to_owned(),
                "tests/lib_test.rs".to_owned(),
            ])
            .expect("base files should be added");
        repository
            .commit_index("base")
            .expect("base commit should work");

        fs::write(path.join("README.md"), "changed docs\n").expect("readme should change");
        fs::write(path.join("src").join("lib.rs"), "pub fn changed() {}\n")
            .expect("lib should change");
        fs::write(
            path.join("tests").join("lib_test.rs"),
            "#[test]\nfn changed() {}\n",
        )
        .expect("test file should change");

        Self { path }
    }
}

impl Drop for SemanticDiffFixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

struct CommandOutput {
    status: std::process::ExitStatus,
    stdout: String,
    stderr: String,
}

fn run_rit<const N: usize>(repo: &Path, args: [&str; N]) -> CommandOutput {
    let output = Command::new(rit_binary())
        .args(args)
        .current_dir(repo)
        .output()
        .expect("rit command should run");
    CommandOutput {
        status: output.status,
        stdout: String::from_utf8(output.stdout).expect("stdout should be utf-8"),
        stderr: String::from_utf8(output.stderr).expect("stderr should be utf-8"),
    }
}

fn configure_identity(repo: &Path) {
    run_git(repo, ["config", "user.name", "Rit Test"]);
    run_git(repo, ["config", "user.email", "rit@example.test"]);
}

fn run_git<const N: usize>(repo: &Path, args: [&str; N]) {
    let status = Command::new("git")
        .args(args)
        .current_dir(repo)
        .status()
        .expect("git command should run");
    assert!(status.success(), "git {:?} should succeed", args);
}

fn rit_binary() -> &'static str {
    env!("CARGO_BIN_EXE_rit")
}

fn temp_path(label: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time should move forward")
        .as_nanos();
    std::env::temp_dir().join(format!("rit-cli-{label}-{nanos}"))
}
