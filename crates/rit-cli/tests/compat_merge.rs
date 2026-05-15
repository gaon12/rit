use std::ffi::{OsStr, OsString};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn supported_merge_conflict_outputs_match_git_exactly() {
    for scenario in [
        MergeScenario {
            name: "content",
            setup: setup_content_conflict,
        },
        MergeScenario {
            name: "delete-modify-head-deleted",
            setup: setup_delete_modify_head_deleted,
        },
        MergeScenario {
            name: "delete-modify-target-deleted",
            setup: setup_delete_modify_target_deleted,
        },
        MergeScenario {
            name: "binary",
            setup: setup_binary_conflict,
        },
        MergeScenario {
            name: "add-add",
            setup: setup_add_add_conflict,
        },
        MergeScenario {
            name: "distinct-types",
            setup: setup_distinct_type_conflict,
        },
    ] {
        let root = temp_path(scenario.name);
        let git_repo = root.join("git");
        let rit_repo = root.join("rit");
        (scenario.setup)(&git_repo);
        copy_directory(&git_repo, &rit_repo);

        let git_merge = run_capture("git", ["merge", "topic"], &git_repo);
        let rit_merge = run_capture(rit_binary(), ["merge", "topic"], &rit_repo);

        assert_eq!(
            git_merge.exit_code, rit_merge.exit_code,
            "{} exit code",
            scenario.name
        );
        assert_eq!(
            git_merge.stdout, rit_merge.stdout,
            "{} stdout",
            scenario.name
        );
        assert_eq!(
            git_merge.stderr, rit_merge.stderr,
            "{} stderr",
            scenario.name
        );
        assert_eq!(
            run_capture("git", ["status", "--porcelain=v1"], &git_repo).stdout,
            run_capture(rit_binary(), ["status", "--porcelain=v1"], &rit_repo).stdout,
            "{} status",
            scenario.name
        );
        assert_eq!(
            run_capture("git", ["ls-files", "--stage"], &git_repo).stdout,
            run_capture(rit_binary(), ["ls-files", "--stage"], &rit_repo).stdout,
            "{} index stages",
            scenario.name
        );

        let _ = fs::remove_dir_all(root);
    }
}

struct MergeScenario {
    name: &'static str,
    setup: fn(&Path),
}

struct CapturedCommand {
    exit_code: i32,
    stdout: String,
    stderr: String,
}

fn setup_content_conflict(repo: &Path) {
    init_repo(repo);
    commit_text(repo, "tracked.txt", "base\n", "base");
    run_git(repo, ["checkout", "--quiet", "-b", "topic"]);
    commit_text(repo, "tracked.txt", "topic\n", "topic");
    run_git(repo, ["checkout", "--quiet", "master"]);
    commit_text(repo, "tracked.txt", "master\n", "master");
}

fn setup_delete_modify_head_deleted(repo: &Path) {
    init_repo(repo);
    commit_text(repo, "a.txt", "base\n", "base");
    run_git(repo, ["checkout", "--quiet", "-b", "topic"]);
    commit_text(repo, "a.txt", "topic\n", "topic");
    run_git(repo, ["checkout", "--quiet", "master"]);
    fs::remove_file(repo.join("a.txt")).expect("head side should delete the file");
    run_git(repo, ["add", "a.txt"]);
    run_git(repo, ["commit", "--quiet", "-m", "delete"]);
}

fn setup_delete_modify_target_deleted(repo: &Path) {
    init_repo(repo);
    commit_text(repo, "a.txt", "base\n", "base");
    run_git(repo, ["checkout", "--quiet", "-b", "topic"]);
    fs::remove_file(repo.join("a.txt")).expect("target side should delete the file");
    run_git(repo, ["add", "a.txt"]);
    run_git(repo, ["commit", "--quiet", "-m", "delete"]);
    run_git(repo, ["checkout", "--quiet", "master"]);
    commit_text(repo, "a.txt", "head\n", "head");
}

fn setup_binary_conflict(repo: &Path) {
    init_repo(repo);
    commit_bytes(repo, "blob.bin", &[0, 1, 2, 3, 4], "base");
    run_git(repo, ["checkout", "--quiet", "-b", "topic"]);
    commit_bytes(repo, "blob.bin", &[0, 1, 2, 9, 4], "topic");
    run_git(repo, ["checkout", "--quiet", "master"]);
    commit_bytes(repo, "blob.bin", &[0, 8, 2, 3, 4], "head");
}

fn setup_add_add_conflict(repo: &Path) {
    init_repo(repo);
    commit_text(repo, "base.txt", "base\n", "base");
    run_git(repo, ["checkout", "--quiet", "-b", "topic"]);
    commit_text(repo, "a.txt", "topic\n", "topic");
    run_git(repo, ["checkout", "--quiet", "master"]);
    commit_text(repo, "a.txt", "head\n", "head");
}

fn setup_distinct_type_conflict(repo: &Path) {
    init_repo(repo);
    run_git(repo, ["config", "core.symlinks", "false"]);
    commit_text(repo, "a.txt", "base\n", "base");
    run_git(repo, ["checkout", "--quiet", "-b", "topic"]);
    let target_blob = write_git_blob(repo, b"target");
    let cacheinfo = format!("120000,{target_blob},a.txt");
    run_git(repo, ["update-index", "--add", "--cacheinfo", &cacheinfo]);
    run_git(repo, ["commit", "--quiet", "-m", "symlink"]);
    run_git(repo, ["checkout", "--force", "--quiet", "master"]);
    commit_text(repo, "a.txt", "head\n", "content");
}

fn init_repo(repo: &Path) {
    fs::create_dir_all(repo).expect("fixture repository should be created");
    run_git(repo, ["init", "--quiet"]);
    run_git(repo, ["config", "user.name", "Rit Test"]);
    run_git(repo, ["config", "user.email", "rit@example.test"]);
    run_git(repo, ["config", "core.autocrlf", "false"]);
}

fn commit_text(repo: &Path, path: &str, contents: &str, message: &str) {
    commit_bytes(repo, path, contents.as_bytes(), message);
}

fn commit_bytes(repo: &Path, path: &str, contents: &[u8], message: &str) {
    fs::write(repo.join(path), contents).expect("file contents should be written");
    run_git(repo, ["add", path]);
    run_git(repo, ["commit", "--quiet", "-m", message]);
}

fn write_git_blob(repo: &Path, contents: &[u8]) -> String {
    let mut child = Command::new("git")
        .args(["hash-object", "-w", "--stdin"])
        .current_dir(repo)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("git hash-object should start");
    child
        .stdin
        .as_mut()
        .expect("stdin should be piped")
        .write_all(contents)
        .expect("blob bytes should be written to git");
    let output = child
        .wait_with_output()
        .expect("git hash-object should exit");
    assert!(
        output.status.success(),
        "git hash-object failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout)
        .expect("object id should be utf-8")
        .trim()
        .to_owned()
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
    std::env::temp_dir().join(format!("rit-cli-compat-merge-{name}-{unique}"))
}
