use rit_testkit::{LocalWriteFixture, LocalWriteFixtureKind};
use std::ffi::{OsStr, OsString};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn add_directory_pathspec_matches_git_status() {
    let fixture = LocalWriteFixture::new("add-directory", LocalWriteFixtureKind::NestedTracked)
        .expect("fixture should build");
    fs::write(
        fixture.path().join("nested").join("tracked.txt"),
        "changed\n",
    )
    .expect("tracked file should be modified");
    fs::write(fixture.path().join("nested").join("new.txt"), "new\n")
        .expect("new file should be written");

    let outcome = compare_after_command(
        fixture.path(),
        command_words("git", ["add", "nested"]),
        command_words(rit_binary(), ["add", "nested"]),
    );

    assert_eq!(outcome.git_command_stdout, outcome.rit_command_stdout);
    assert_eq!(outcome.git_command_stderr, outcome.rit_command_stderr);
    assert_eq!(outcome.git_status, outcome.rit_status);
}

#[test]
fn add_wildcard_pathspec_matches_git_status() {
    let fixture = LocalWriteFixture::new("add-wildcard", LocalWriteFixtureKind::NestedTracked)
        .expect("fixture should build");
    fs::write(
        fixture.path().join("nested").join("tracked.txt"),
        "changed\n",
    )
    .expect("tracked file should be modified");
    fs::write(fixture.path().join("nested").join("new.txt"), "new\n")
        .expect("new file should be written");
    fs::write(fixture.path().join("nested").join("skip.md"), "skip\n")
        .expect("markdown file should be written");

    let outcome = compare_after_command(
        fixture.path(),
        command_words("git", ["add", "nested/[tn]*.txt"]),
        command_words(rit_binary(), ["add", "nested/[tn]*.txt"]),
    );

    assert_eq!(outcome.git_command_stdout, outcome.rit_command_stdout);
    assert_eq!(outcome.git_command_stderr, outcome.rit_command_stderr);
    assert_eq!(outcome.git_status, outcome.rit_status);
}

#[test]
fn restore_directory_pathspec_matches_git_status_and_files() {
    let fixture = LocalWriteFixture::new("restore-directory", LocalWriteFixtureKind::NestedTracked)
        .expect("fixture should build");
    fs::write(
        fixture.path().join("nested").join("tracked.txt"),
        "changed\n",
    )
    .expect("tracked file should be modified");

    let outcome = compare_after_command(
        fixture.path(),
        command_words("git", ["restore", "nested"]),
        command_words(rit_binary(), ["restore", "nested"]),
    );

    assert_eq!(outcome.git_command_stdout, outcome.rit_command_stdout);
    assert_eq!(outcome.git_command_stderr, outcome.rit_command_stderr);
    assert_eq!(outcome.git_status, outcome.rit_status);
    assert_eq!(
        fs::read_to_string(outcome.git_repo.join("nested").join("tracked.txt"))
            .expect("git file should read"),
        fs::read_to_string(outcome.rit_repo.join("nested").join("tracked.txt"))
            .expect("rit file should read")
    );
}

#[test]
fn restore_wildcard_pathspec_matches_git_status_and_files() {
    let fixture = LocalWriteFixture::new("restore-wildcard", LocalWriteFixtureKind::NestedTracked)
        .expect("fixture should build");
    fs::write(
        fixture.path().join("nested").join("tracked.txt"),
        "changed\n",
    )
    .expect("tracked file should be modified");

    let outcome = compare_after_command(
        fixture.path(),
        command_words("git", ["restore", "nested/[tn]*.txt"]),
        command_words(rit_binary(), ["restore", "nested/[tn]*.txt"]),
    );

    assert_eq!(outcome.git_command_stdout, outcome.rit_command_stdout);
    assert_eq!(outcome.git_command_stderr, outcome.rit_command_stderr);
    assert_eq!(outcome.git_status, outcome.rit_status);
    assert_eq!(
        fs::read_to_string(outcome.git_repo.join("nested").join("tracked.txt"))
            .expect("git file should read"),
        fs::read_to_string(outcome.rit_repo.join("nested").join("tracked.txt"))
            .expect("rit file should read")
    );
}

#[test]
fn reset_directory_pathspec_matches_git_status() {
    let fixture = LocalWriteFixture::new("reset-directory", LocalWriteFixtureKind::NestedTracked)
        .expect("fixture should build");
    fs::write(
        fixture.path().join("nested").join("tracked.txt"),
        "changed\n",
    )
    .expect("tracked file should be modified");
    run_git(fixture.path(), ["add", "nested"]);

    let outcome = compare_after_command(
        fixture.path(),
        command_words("git", ["reset", "nested"]),
        command_words(rit_binary(), ["reset", "nested"]),
    );

    assert_eq!(outcome.git_command_stdout, outcome.rit_command_stdout);
    assert_eq!(outcome.git_command_stderr, outcome.rit_command_stderr);
    assert_eq!(outcome.git_status, outcome.rit_status);
}

#[test]
fn reset_wildcard_pathspec_matches_git_status() {
    let fixture = LocalWriteFixture::new("reset-wildcard", LocalWriteFixtureKind::NestedTracked)
        .expect("fixture should build");
    fs::write(
        fixture.path().join("nested").join("tracked.txt"),
        "changed\n",
    )
    .expect("tracked file should be modified");
    fs::write(fixture.path().join("nested").join("new.txt"), "new\n")
        .expect("new file should be written");
    run_git(fixture.path(), ["add", "nested"]);

    let outcome = compare_after_command(
        fixture.path(),
        command_words("git", ["reset", "nested/[tn]*.txt"]),
        command_words(rit_binary(), ["reset", "nested/[tn]*.txt"]),
    );

    assert_eq!(outcome.git_command_stdout, outcome.rit_command_stdout);
    assert_eq!(outcome.git_command_stderr, outcome.rit_command_stderr);
    assert_eq!(outcome.git_status, outcome.rit_status);
}

#[test]
fn commit_author_and_date_overrides_match_git_object() {
    let fixture =
        LocalWriteFixture::new("commit-author-date", LocalWriteFixtureKind::NestedTracked)
            .expect("fixture should build");
    let workspace = temp_path("commit-author-date-compare");
    let git_repo = workspace.join("git");
    let rit_repo = workspace.join("rit");
    copy_directory(fixture.path(), &git_repo);
    copy_directory(fixture.path(), &rit_repo);

    fs::write(git_repo.join("nested").join("tracked.txt"), "changed\n")
        .expect("git file should be changed");
    fs::write(rit_repo.join("nested").join("tracked.txt"), "changed\n")
        .expect("rit file should be changed");
    run_git(&git_repo, ["add", "nested/tracked.txt"]);
    run_git(&rit_repo, ["add", "nested/tracked.txt"]);

    let env = [
        ("GIT_COMMITTER_NAME", "C O Mitter"),
        ("GIT_COMMITTER_EMAIL", "c@example.test"),
        ("GIT_COMMITTER_DATE", "1700000001 +0900"),
    ];
    run_command(
        &command_words_with_env(
            "git",
            [
                "commit",
                "-m",
                "authored",
                "--author=A U Thor <a@example.test>",
                "--date=1700000000 +0900",
            ],
            &env,
        ),
        &git_repo,
    );
    run_command(
        &command_words_with_env(
            rit_binary(),
            [
                "commit",
                "-m",
                "authored",
                "--author=A U Thor <a@example.test>",
                "--date=1700000000 +0900",
            ],
            &env,
        ),
        &rit_repo,
    );

    assert_eq!(
        run_capture("git", ["rev-parse", "HEAD"], &git_repo).0,
        run_capture("git", ["rev-parse", "HEAD"], &rit_repo).0
    );
    assert_eq!(
        run_capture("git", ["cat-file", "-p", "HEAD"], &git_repo).0,
        run_capture("git", ["cat-file", "-p", "HEAD"], &rit_repo).0
    );
    let _ = fs::remove_dir_all(workspace);
}

#[test]
fn checkout_detached_commit_matches_git_state() {
    let fixture =
        LocalWriteFixture::new("detached-checkout", LocalWriteFixtureKind::DetachedCheckout)
            .expect("fixture should build");
    let base = fixture
        .base_commit()
        .expect("detached checkout fixture should expose base commit")
        .to_owned();

    let outcome = compare_after_command(
        fixture.path(),
        CommandSpec {
            program: OsString::from("git"),
            args: vec![OsString::from("checkout"), OsString::from(base.clone())],
            env: Vec::new(),
        },
        CommandSpec {
            program: rit_binary(),
            args: vec![OsString::from("checkout"), OsString::from(base)],
            env: Vec::new(),
        },
    );

    assert_eq!(outcome.git_status, outcome.rit_status);
    assert_eq!(
        fs::read_to_string(outcome.git_repo.join(".git").join("HEAD"))
            .expect("git HEAD should read"),
        fs::read_to_string(outcome.rit_repo.join(".git").join("HEAD"))
            .expect("rit HEAD should read")
    );
    assert_eq!(
        run_capture("git", ["branch", "--show-current"], &outcome.git_repo).0,
        run_capture(
            rit_binary(),
            ["branch", "--show-current"],
            &outcome.rit_repo
        )
        .0
    );
    assert_eq!(
        fs::read_to_string(outcome.git_repo.join("tracked.txt")).expect("git file should read"),
        fs::read_to_string(outcome.rit_repo.join("tracked.txt")).expect("rit file should read")
    );
}

#[test]
fn branch_delete_refuses_unmerged_branch_like_git() {
    let fixture = LocalWriteFixture::new(
        "branch-delete-unmerged",
        LocalWriteFixtureKind::UnmergedBranch,
    )
    .expect("fixture should build");
    let workspace = temp_path("branch-delete-unmerged-compare");
    let git_repo = workspace.join("git");
    let rit_repo = workspace.join("rit");
    copy_directory(fixture.path(), &git_repo);
    copy_directory(fixture.path(), &rit_repo);

    let git =
        run_command_allow_failure(&command_words("git", ["branch", "-d", "topic"]), &git_repo);
    let rit = run_command_allow_failure(
        &command_words(rit_binary(), ["branch", "-d", "topic"]),
        &rit_repo,
    );

    assert!(!git.success);
    assert!(!rit.success);
    assert!(
        git_repo
            .join(".git")
            .join("refs")
            .join("heads")
            .join("topic")
            .exists()
    );
    assert!(
        rit_repo
            .join(".git")
            .join("refs")
            .join("heads")
            .join("topic")
            .exists()
    );
    let _ = fs::remove_dir_all(workspace);
}

#[test]
fn branch_delete_allows_merged_branch_like_git() {
    let fixture =
        LocalWriteFixture::new("branch-delete-merged", LocalWriteFixtureKind::MergedBranch)
            .expect("fixture should build");
    let outcome = compare_after_command(
        fixture.path(),
        command_words("git", ["branch", "-d", "topic"]),
        command_words(rit_binary(), ["branch", "-d", "topic"]),
    );

    assert!(
        !outcome
            .git_repo
            .join(".git")
            .join("refs")
            .join("heads")
            .join("topic")
            .exists()
    );
    assert!(
        !outcome
            .rit_repo
            .join(".git")
            .join("refs")
            .join("heads")
            .join("topic")
            .exists()
    );
}

struct CommandSpec {
    program: OsString,
    args: Vec<OsString>,
    env: Vec<(OsString, OsString)>,
}

struct CommandOutcome {
    workspace: PathBuf,
    git_repo: PathBuf,
    rit_repo: PathBuf,
    git_command_stdout: String,
    git_command_stderr: String,
    rit_command_stdout: String,
    rit_command_stderr: String,
    git_status: String,
    rit_status: String,
}

fn compare_after_command(fixture: &Path, git: CommandSpec, rit: CommandSpec) -> CommandOutcome {
    let workspace = temp_path("write-compare");
    let git_repo = workspace.join("git");
    let rit_repo = workspace.join("rit");
    copy_directory(fixture, &git_repo);
    copy_directory(fixture, &rit_repo);

    let git_output = run_command(&git, &git_repo);
    let rit_output = run_command(&rit, &rit_repo);
    let git_status = run_capture("git", ["status", "--porcelain=v1"], &git_repo).0;
    let rit_status = run_capture(rit_binary(), ["status", "--porcelain=v1"], &rit_repo).0;

    CommandOutcome {
        workspace,
        git_repo,
        rit_repo,
        git_command_stdout: git_output.0,
        git_command_stderr: git_output.1,
        rit_command_stdout: rit_output.0,
        rit_command_stderr: rit_output.1,
        git_status,
        rit_status,
    }
}

impl Drop for CommandOutcome {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.workspace);
    }
}

fn command_words<const N: usize>(program: impl Into<OsString>, args: [&str; N]) -> CommandSpec {
    CommandSpec {
        program: program.into(),
        args: args.into_iter().map(OsString::from).collect(),
        env: Vec::new(),
    }
}

fn command_words_with_env<const N: usize, const M: usize>(
    program: impl Into<OsString>,
    args: [&str; N],
    env: &[(&str, &str); M],
) -> CommandSpec {
    CommandSpec {
        program: program.into(),
        args: args.into_iter().map(OsString::from).collect(),
        env: env
            .iter()
            .map(|(name, value)| (OsString::from(name), OsString::from(value)))
            .collect(),
    }
}

fn run_command(spec: &CommandSpec, cwd: &Path) -> (String, String) {
    let mut command = Command::new(&spec.program);
    command.args(&spec.args).current_dir(cwd);
    for (name, value) in &spec.env {
        command.env(name, value);
    }
    let output = command.output().expect("command should start");
    assert!(
        output.status.success(),
        "command failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    (
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

struct CommandRun {
    success: bool,
}

fn run_command_allow_failure(spec: &CommandSpec, cwd: &Path) -> CommandRun {
    let mut command = Command::new(&spec.program);
    command.args(&spec.args).current_dir(cwd);
    for (name, value) in &spec.env {
        command.env(name, value);
    }
    let output = command.output().expect("command should start");
    CommandRun {
        success: output.status.success(),
    }
}

fn run_capture<const N: usize>(
    program: impl AsRef<OsStr>,
    args: [&str; N],
    cwd: &Path,
) -> (String, String) {
    let output = Command::new(program)
        .args(args)
        .current_dir(cwd)
        .output()
        .expect("command should start");
    assert!(
        output.status.success(),
        "command failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    (
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

fn run_git<const N: usize>(cwd: &Path, args: [&str; N]) {
    let output = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .output()
        .expect("git should start");
    assert!(
        output.status.success(),
        "git command failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn copy_directory(from: &Path, to: &Path) {
    fs::create_dir_all(to).expect("target directory should be created");
    for entry in fs::read_dir(from).expect("source directory should be readable") {
        let entry = entry.expect("directory entry should be readable");
        let source_path = entry.path();
        let target_path = to.join(entry.file_name());
        let file_type = entry.file_type().expect("file type should be readable");
        if file_type.is_dir() {
            copy_directory(&source_path, &target_path);
        } else if file_type.is_file() {
            fs::copy(&source_path, &target_path).expect("file should be copied");
        }
    }
}

fn rit_binary() -> OsString {
    std::env::var_os("CARGO_BIN_EXE_rit").unwrap_or_else(|| {
        let mut path =
            std::env::current_exe().expect("current test executable path should be available");
        path.pop();
        if path.ends_with("deps") {
            path.pop();
        }
        path.push(format!("rit{}", std::env::consts::EXE_SUFFIX));
        path.into_os_string()
    })
}

fn temp_path(name: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("rit-cli-compat-{name}-{unique}"))
}
