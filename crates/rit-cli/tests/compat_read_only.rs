use rit_testkit::{CommandSpec, CompareOptions, compare};
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn diff_worktree_outputs_match_git() {
    let fixture = DiffFixture::new("worktree-diff");

    for option in ["--name-only", "--name-status", "--numstat", "--stat"] {
        let mut options = CompareOptions::new(
            fixture.path(),
            git_command(["diff", option]),
            rit_command(["diff", option]),
        );
        options.compare_repository_state = false;
        let outcome = compare(&options).expect("comparison should run");

        assert!(outcome.is_match(), "diff {option}\n{}", outcome.report());
    }
}

#[test]
fn diff_cached_outputs_match_git() {
    let fixture = DiffFixture::new("cached-diff");

    for option in ["--name-only", "--name-status", "--numstat", "--stat"] {
        let outcome = compare(&CompareOptions::new(
            fixture.path(),
            git_command(["diff", "--cached", option]),
            rit_command(["diff", "--cached", option]),
        ))
        .expect("comparison should run");

        assert!(
            outcome.is_match(),
            "diff --cached {option}\n{}",
            outcome.report()
        );
    }
}

#[test]
fn binary_diff_summary_outputs_match_git() {
    let fixture = BinaryDiffFixture::new("binary-diff");

    for option in ["--name-only", "--name-status", "--numstat", "--stat"] {
        let mut options = CompareOptions::new(
            fixture.path(),
            git_command(["diff", option]),
            rit_command(["diff", option]),
        );
        options.compare_repository_state = false;
        let outcome = compare(&options).expect("comparison should run");

        assert!(
            outcome.is_match(),
            "binary diff {option}\n{}",
            outcome.report()
        );
    }
}

#[test]
fn diff_patch_outputs_match_git_for_small_text_files() {
    let fixture = DiffFixture::new("patch-diff");

    for args in [vec!["diff"], vec!["diff", "--cached"], vec!["diff", "-p"]] {
        let mut options = CompareOptions::new(
            fixture.path(),
            git_command_slice(&args),
            rit_command_slice(&args),
        );
        options.compare_repository_state = false;
        let outcome = compare(&options).expect("comparison should run");

        assert!(
            outcome.is_match(),
            "patch diff {:?}\n{}",
            args,
            outcome.report()
        );
    }
}

#[test]
fn diff_patch_marks_missing_trailing_newlines_like_git() {
    let fixture = NoNewlineDiffFixture::new("no-newline-diff");

    for args in [vec!["diff"], vec!["diff", "--cached"]] {
        let mut options = CompareOptions::new(
            fixture.path(),
            git_command_slice(&args),
            rit_command_slice(&args),
        );
        options.compare_repository_state = false;
        let outcome = compare(&options).expect("comparison should run");

        assert!(
            outcome.is_match(),
            "no-newline patch diff {:?}\n{}",
            args,
            outcome.report()
        );
    }
}

#[test]
fn diff_pathspec_outputs_match_git() {
    let fixture = DiffFixture::new("pathspec-diff");

    for (git_args, rit_args) in [
        (
            vec!["diff", "--name-only", "--", "nested"],
            vec!["diff", "--name-only", "--", "nested"],
        ),
        (
            vec!["diff", "--cached", "--name-status", "--", "nested"],
            vec!["diff", "--cached", "--name-status", "--", "nested"],
        ),
    ] {
        let mut options = CompareOptions::new(
            fixture.path(),
            git_command_slice(&git_args),
            rit_command_slice(&rit_args),
        );
        options.compare_repository_state = false;
        let outcome = compare(&options).expect("comparison should run");

        assert!(
            outcome.is_match(),
            "pathspec diff {:?}\n{}",
            git_args,
            outcome.report()
        );
    }
}

#[test]
fn status_pathspec_outputs_match_git() {
    let fixture = DiffFixture::new("pathspec-status");

    for args in [
        ["status", "--porcelain=v1", "--", "a.txt"],
        ["status", "--porcelain=v1", "--", "nested"],
    ] {
        let mut options = CompareOptions::new(fixture.path(), git_command(args), rit_command(args));
        options.compare_repository_state = false;
        let outcome = compare(&options).expect("comparison should run");

        assert!(
            outcome.is_match(),
            "pathspec status {:?}\n{}",
            args,
            outcome.report()
        );
    }
}

#[test]
fn status_untracked_directory_outputs_match_git() {
    let fixture = StatusUntrackedFixture::new("untracked-directory-status");

    for args in [
        vec!["status", "--porcelain=v1"],
        vec!["status", "--porcelain=v1", "--", "untracked"],
        vec!["status", "--porcelain=v1", "--", "untracked/deep/new.txt"],
    ] {
        let mut options = CompareOptions::new(
            fixture.path(),
            git_command_slice(&args),
            rit_command_slice(&args),
        );
        options.compare_repository_state = false;
        let outcome = compare(&options).expect("comparison should run");

        assert!(
            outcome.is_match(),
            "untracked directory status {:?}\n{}",
            args,
            outcome.report()
        );
    }
}

#[test]
fn ls_files_pathspec_outputs_match_git() {
    let fixture = DiffFixture::new("pathspec-ls-files");

    for args in [
        vec!["ls-files", "--", "nested"],
        vec!["ls-files", "--stage", "--", "nested"],
    ] {
        let outcome = compare(&CompareOptions::new(
            fixture.path(),
            git_command_slice(&args),
            rit_command_slice(&args),
        ))
        .expect("comparison should run");

        assert!(
            outcome.is_match(),
            "pathspec ls-files {:?}\n{}",
            args,
            outcome.report()
        );
    }
}

#[test]
fn ls_tree_pathspec_outputs_match_git() {
    let fixture = DiffFixture::new("pathspec-ls-tree");

    for args in [
        vec!["ls-tree", "HEAD", "nested"],
        vec!["ls-tree", "--name-only", "HEAD", "nested/base.txt"],
        vec!["ls-tree", "--object-only", "HEAD", "nested/base.txt"],
    ] {
        let outcome = compare(&CompareOptions::new(
            fixture.path(),
            git_command_slice(&args),
            rit_command_slice(&args),
        ))
        .expect("comparison should run");

        assert!(
            outcome.is_match(),
            "pathspec ls-tree {:?}\n{}",
            args,
            outcome.report()
        );
    }
}

#[test]
fn log_pathspec_outputs_match_git() {
    let fixture = LogPathFixture::new("pathspec-log");

    for args in [
        vec!["log", "--oneline", "--", "a.txt"],
        vec!["log", "--oneline", "--", "nested"],
    ] {
        let outcome = compare(&CompareOptions::new(
            fixture.path(),
            git_command_slice(&args),
            rit_command_slice(&args),
        ))
        .expect("comparison should run");

        assert!(
            outcome.is_match(),
            "pathspec log {:?}\n{}",
            args,
            outcome.report()
        );
    }
}

#[test]
fn show_pathspec_outputs_match_git() {
    let fixture = LogPathFixture::new("pathspec-show");

    for args in [
        vec!["show", "--no-patch", "--", "nested"],
        vec!["show", "--no-patch", "--", "a.txt"],
        vec!["show", "--no-patch", "HEAD", "--", "nested/base.txt"],
    ] {
        let outcome = compare(&CompareOptions::new(
            fixture.path(),
            git_command_slice(&args),
            rit_command_slice(&args),
        ))
        .expect("comparison should run");

        assert!(
            outcome.is_match(),
            "pathspec show {:?}\n{}",
            args,
            outcome.report()
        );
    }
}

struct DiffFixture {
    path: PathBuf,
}

impl DiffFixture {
    fn new(name: &str) -> Self {
        let path = temp_path(name);
        fs::create_dir_all(&path).expect("fixture directory should be created");
        run_git(&path, ["init", "--quiet"]);
        run_git(&path, ["config", "user.name", "Rit Test"]);
        run_git(&path, ["config", "user.email", "rit@example.test"]);
        run_git(&path, ["config", "core.autocrlf", "false"]);

        fs::write(path.join("a.txt"), "one\ntwo\n").expect("base file should be written");
        fs::create_dir_all(path.join("nested")).expect("nested directory should be created");
        fs::write(path.join("nested").join("base.txt"), "base\n")
            .expect("nested base file should be written");
        run_git(&path, ["add", "a.txt", "nested/base.txt"]);
        run_git(&path, ["commit", "--quiet", "-m", "base"]);

        fs::write(path.join("a.txt"), "one\nthree\nfour\n")
            .expect("modified file should be written");
        fs::write(path.join("nested").join("base.txt"), "base\nstaged\n")
            .expect("nested staged file should be written");
        fs::write(path.join("b.txt"), "new\n").expect("added file should be written");
        run_git(&path, ["add", "a.txt", "b.txt", "nested/base.txt"]);

        fs::write(path.join("a.txt"), "one\nthree\nfour\nfive\n")
            .expect("worktree modification should be written");
        fs::write(
            path.join("nested").join("base.txt"),
            "base\nstaged\nworktree\n",
        )
        .expect("nested worktree modification should be written");

        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for DiffFixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

struct BinaryDiffFixture {
    path: PathBuf,
}

impl BinaryDiffFixture {
    fn new(name: &str) -> Self {
        let path = temp_path(name);
        fs::create_dir_all(&path).expect("fixture directory should be created");
        run_git(&path, ["init", "--quiet"]);
        run_git(&path, ["config", "user.name", "Rit Test"]);
        run_git(&path, ["config", "user.email", "rit@example.test"]);
        run_git(&path, ["config", "core.autocrlf", "false"]);

        fs::write(path.join("bin.dat"), [0, 1, 2, 0, 3]).expect("binary file should be written");
        run_git(&path, ["add", "bin.dat"]);
        run_git(&path, ["commit", "--quiet", "-m", "base"]);

        fs::write(path.join("bin.dat"), [0, 1, 2, 0, 3, 4, 5])
            .expect("binary file should be modified");

        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for BinaryDiffFixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

struct NoNewlineDiffFixture {
    path: PathBuf,
}

impl NoNewlineDiffFixture {
    fn new(name: &str) -> Self {
        let path = temp_path(name);
        fs::create_dir_all(&path).expect("fixture directory should be created");
        run_git(&path, ["init", "--quiet"]);
        run_git(&path, ["config", "user.name", "Rit Test"]);
        run_git(&path, ["config", "user.email", "rit@example.test"]);
        run_git(&path, ["config", "core.autocrlf", "false"]);

        fs::write(path.join("a.txt"), "one").expect("base file should be written");
        fs::write(path.join("cached.txt"), "old").expect("cached file should be written");
        run_git(&path, ["add", "a.txt", "cached.txt"]);
        run_git(&path, ["commit", "--quiet", "-m", "base"]);

        fs::write(path.join("a.txt"), "two").expect("worktree file should be modified");
        fs::write(path.join("cached.txt"), "new").expect("cached file should be modified");
        run_git(&path, ["add", "cached.txt"]);

        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for NoNewlineDiffFixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

struct StatusUntrackedFixture {
    path: PathBuf,
}

impl StatusUntrackedFixture {
    fn new(name: &str) -> Self {
        let path = temp_path(name);
        fs::create_dir_all(path.join("tracked")).expect("fixture directory should be created");
        fs::create_dir_all(path.join("untracked").join("deep"))
            .expect("untracked directory should be created");
        run_git(&path, ["init", "--quiet"]);
        run_git(&path, ["config", "user.name", "Rit Test"]);
        run_git(&path, ["config", "user.email", "rit@example.test"]);
        run_git(&path, ["config", "core.autocrlf", "false"]);

        fs::write(path.join("tracked").join("base.txt"), "base\n")
            .expect("tracked file should be written");
        run_git(&path, ["add", "tracked/base.txt"]);
        run_git(&path, ["commit", "--quiet", "-m", "base"]);

        fs::write(path.join("untracked").join("deep").join("new.txt"), "new\n")
            .expect("untracked file should be written");

        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for StatusUntrackedFixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

struct LogPathFixture {
    path: PathBuf,
}

impl LogPathFixture {
    fn new(name: &str) -> Self {
        let path = temp_path(name);
        fs::create_dir_all(path.join("nested")).expect("fixture directory should be created");
        run_git(&path, ["init", "--quiet"]);
        run_git(&path, ["config", "user.name", "Rit Test"]);
        run_git(&path, ["config", "user.email", "rit@example.test"]);
        run_git(&path, ["config", "core.autocrlf", "false"]);

        fs::write(path.join("a.txt"), "base\n").expect("base file should be written");
        fs::write(path.join("nested").join("base.txt"), "base\n")
            .expect("nested base file should be written");
        run_git(&path, ["add", "a.txt", "nested/base.txt"]);
        run_git(&path, ["commit", "--quiet", "-m", "base"]);

        fs::write(path.join("a.txt"), "base\na change\n").expect("a file should be modified");
        run_git(&path, ["add", "a.txt"]);
        run_git(&path, ["commit", "--quiet", "-m", "change a"]);

        fs::write(
            path.join("nested").join("base.txt"),
            "base\nnested change\n",
        )
        .expect("nested file should be modified");
        run_git(&path, ["add", "nested/base.txt"]);
        run_git(&path, ["commit", "--quiet", "-m", "change nested"]);

        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for LogPathFixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn git_command<const N: usize>(args: [&str; N]) -> CommandSpec {
    CommandSpec::new("git", os_args(args))
}

fn rit_command<const N: usize>(args: [&str; N]) -> CommandSpec {
    CommandSpec::new(rit_binary(), os_args(args))
}

fn git_command_slice(args: &[&str]) -> CommandSpec {
    CommandSpec::new("git", args.iter().map(OsString::from).collect())
}

fn rit_command_slice(args: &[&str]) -> CommandSpec {
    CommandSpec::new(rit_binary(), args.iter().map(OsString::from).collect())
}

fn os_args<const N: usize>(args: [&str; N]) -> Vec<OsString> {
    args.into_iter().map(OsString::from).collect()
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

fn temp_path(name: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("rit-cli-compat-{name}-{unique}"))
}
