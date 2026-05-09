use std::ffi::{OsStr, OsString};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn add_directory_pathspec_matches_git_status() {
    let fixture = WriteFixture::new("add-directory");
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
fn restore_directory_pathspec_matches_git_status_and_files() {
    let fixture = WriteFixture::new("restore-directory");
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
fn reset_directory_pathspec_matches_git_status() {
    let fixture = WriteFixture::new("reset-directory");
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
fn checkout_detached_commit_matches_git_state() {
    let fixture = DetachedCheckoutFixture::new("detached-checkout");
    let base = fixture.base_commit.clone();

    let outcome = compare_after_command(
        fixture.path(),
        CommandSpec {
            program: OsString::from("git"),
            args: vec![OsString::from("checkout"), OsString::from(base.clone())],
        },
        CommandSpec {
            program: rit_binary(),
            args: vec![OsString::from("checkout"), OsString::from(base)],
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
    let fixture = BranchDeleteFixture::unmerged("branch-delete-unmerged");
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
    let fixture = BranchDeleteFixture::merged("branch-delete-merged");
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
    }
}

fn run_command(spec: &CommandSpec, cwd: &Path) -> (String, String) {
    let output = Command::new(&spec.program)
        .args(&spec.args)
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

struct CommandRun {
    success: bool,
}

fn run_command_allow_failure(spec: &CommandSpec, cwd: &Path) -> CommandRun {
    let output = Command::new(&spec.program)
        .args(&spec.args)
        .current_dir(cwd)
        .output()
        .expect("command should start");
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

struct WriteFixture {
    path: PathBuf,
}

impl WriteFixture {
    fn new(name: &str) -> Self {
        let path = temp_path(name);
        fs::create_dir_all(path.join("nested")).expect("fixture directory should be created");
        run_git(&path, ["init", "--quiet"]);
        run_git(&path, ["config", "user.name", "Rit Test"]);
        run_git(&path, ["config", "user.email", "rit@example.test"]);
        run_git(&path, ["config", "core.autocrlf", "false"]);

        fs::write(path.join("nested").join("tracked.txt"), "base\n")
            .expect("tracked file should be written");
        run_git(&path, ["add", "nested"]);
        run_git(&path, ["commit", "--quiet", "-m", "base"]);

        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for WriteFixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

struct DetachedCheckoutFixture {
    path: PathBuf,
    base_commit: String,
}

impl DetachedCheckoutFixture {
    fn new(name: &str) -> Self {
        let path = temp_path(name);
        fs::create_dir_all(&path).expect("fixture directory should be created");
        run_git(&path, ["init", "--quiet"]);
        run_git(&path, ["config", "user.name", "Rit Test"]);
        run_git(&path, ["config", "user.email", "rit@example.test"]);
        run_git(&path, ["config", "core.autocrlf", "false"]);

        fs::write(path.join("tracked.txt"), "base\n").expect("tracked file should be written");
        run_git(&path, ["add", "tracked.txt"]);
        run_git(&path, ["commit", "--quiet", "-m", "base"]);
        let base_commit = run_capture("git", ["rev-parse", "HEAD"], &path)
            .0
            .trim()
            .to_owned();

        fs::write(path.join("tracked.txt"), "second\n").expect("tracked file should be modified");
        run_git(&path, ["add", "tracked.txt"]);
        run_git(&path, ["commit", "--quiet", "-m", "second"]);

        Self { path, base_commit }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for DetachedCheckoutFixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

struct BranchDeleteFixture {
    path: PathBuf,
}

impl BranchDeleteFixture {
    fn merged(name: &str) -> Self {
        let path = base_branch_delete_fixture(name);
        run_git(&path, ["branch", "topic"]);
        Self { path }
    }

    fn unmerged(name: &str) -> Self {
        let path = base_branch_delete_fixture(name);
        run_git(&path, ["checkout", "--quiet", "-b", "topic"]);
        fs::write(path.join("tracked.txt"), "topic\n").expect("topic file should be written");
        run_git(&path, ["add", "tracked.txt"]);
        run_git(&path, ["commit", "--quiet", "-m", "topic"]);
        run_git(&path, ["checkout", "--quiet", "master"]);
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for BranchDeleteFixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn base_branch_delete_fixture(name: &str) -> PathBuf {
    let path = temp_path(name);
    fs::create_dir_all(&path).expect("fixture directory should be created");
    run_git(&path, ["init", "--quiet"]);
    run_git(&path, ["config", "user.name", "Rit Test"]);
    run_git(&path, ["config", "user.email", "rit@example.test"]);
    run_git(&path, ["config", "core.autocrlf", "false"]);
    fs::write(path.join("tracked.txt"), "base\n").expect("tracked file should be written");
    run_git(&path, ["add", "tracked.txt"]);
    run_git(&path, ["commit", "--quiet", "-m", "base"]);
    path
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
