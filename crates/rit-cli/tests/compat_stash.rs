use std::ffi::{OsStr, OsString};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn stash_list_without_stashes_matches_git() {
    let root = temp_path("empty");
    let git_repo = root.join("git");
    let rit_repo = root.join("rit");
    init_repo(&git_repo);
    copy_directory(&git_repo, &rit_repo);

    let git_list = run_capture("git", ["stash", "list"], &git_repo);
    let rit_list = run_capture(rit_binary(), ["stash", "list"], &rit_repo);

    assert_eq!(git_list.exit_code, 0, "git stderr: {}", git_list.stderr);
    assert_eq!(rit_list.exit_code, 0, "rit stderr: {}", rit_list.stderr);
    assert_eq!(git_list.stdout, rit_list.stdout);

    let _ = fs::remove_dir_all(root);
}

#[test]
fn stash_list_matches_git_reflog_order() {
    let root = temp_path("list");
    let git_repo = root.join("git");
    let rit_repo = root.join("rit");
    setup_stashes(&git_repo);
    copy_directory(&git_repo, &rit_repo);

    let git_list = run_capture("git", ["stash", "list"], &git_repo);
    let rit_list = run_capture(rit_binary(), ["stash", "list"], &rit_repo);

    assert_eq!(git_list.exit_code, 0, "git stderr: {}", git_list.stderr);
    assert_eq!(rit_list.exit_code, 0, "rit stderr: {}", rit_list.stderr);
    assert_eq!(git_list.stdout, rit_list.stdout);

    let _ = fs::remove_dir_all(root);
}

struct CapturedCommand {
    exit_code: i32,
    stdout: String,
    stderr: String,
}

fn setup_stashes(repo: &Path) {
    init_repo(repo);
    fs::write(repo.join("tracked.txt"), "first\n").expect("first change should write");
    run_git(repo, ["stash", "push", "-m", "first stash"]);
    fs::write(repo.join("tracked.txt"), "second\n").expect("second change should write");
    run_git(repo, ["stash", "push", "-m", "second stash"]);
}

fn init_repo(repo: &Path) {
    fs::create_dir_all(repo).expect("fixture repository should be created");
    run_git(repo, ["init", "--quiet"]);
    run_git(repo, ["config", "user.name", "Rit Test"]);
    run_git(repo, ["config", "user.email", "rit@example.test"]);
    run_git(repo, ["config", "core.autocrlf", "false"]);
    fs::write(repo.join("tracked.txt"), "base\n").expect("tracked file should write");
    run_git(repo, ["add", "tracked.txt"]);
    run_git(repo, ["commit", "--quiet", "-m", "base"]);
}

fn run_git<I, S>(repo: &Path, args: I)
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let output = Command::new("git")
        .args(args)
        .current_dir(repo)
        .output()
        .expect("git command should start");
    assert!(
        output.status.success(),
        "git command failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn run_capture<I, S>(program: impl AsRef<OsStr>, args: I, cwd: &Path) -> CapturedCommand
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let output = Command::new(program)
        .args(args)
        .current_dir(cwd)
        .output()
        .expect("command should start");
    CapturedCommand {
        exit_code: output.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    }
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
    std::env::temp_dir().join(format!("rit-cli-compat-stash-{name}-{unique}"))
}
