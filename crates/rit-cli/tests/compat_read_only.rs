use rit_testkit::{CommandSpec, CompareOptions, compare};
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

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
fn binary_diff_patch_outputs_match_git() {
    let fixture = BinaryPatchFixture::new("binary-patch");

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
            "binary patch {:?}\n{}",
            args,
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
fn diff_patch_splits_distant_changes_like_git() {
    let fixture = MultiHunkDiffFixture::new("multi-hunk-diff");
    let mut options =
        CompareOptions::new(fixture.path(), git_command(["diff"]), rit_command(["diff"]));
    options.compare_repository_state = false;
    let outcome = compare(&options).expect("comparison should run");

    assert!(
        outcome.is_match(),
        "multi-hunk patch diff\n{}",
        outcome.report()
    );
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
        (
            vec!["diff", "--name-only", "--", "*.txt"],
            vec!["diff", "--name-only", "--", "*.txt"],
        ),
        (
            vec!["diff", "--cached", "--name-status", "--", "nested/*.txt"],
            vec!["diff", "--cached", "--name-status", "--", "nested/*.txt"],
        ),
        (
            vec!["diff", "--name-only", "--", "[ab].txt"],
            vec!["diff", "--name-only", "--", "[ab].txt"],
        ),
        (
            vec![
                "diff",
                "--cached",
                "--name-status",
                "--",
                "nested/[ab]*.txt",
            ],
            vec![
                "diff",
                "--cached",
                "--name-status",
                "--",
                "nested/[ab]*.txt",
            ],
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
        ["status", "--porcelain=v1", "--", "*.txt"],
        ["status", "--porcelain=v1", "--", "nested/*.txt"],
        ["status", "--porcelain=v1", "--", "[!a].txt"],
        ["status", "--porcelain=v1", "--", "nested/[ab]*.txt"],
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
fn status_untracked_file_modes_match_git() {
    let fixture = StatusUntrackedFixture::new("untracked-file-modes-status");

    for args in [
        vec!["status", "--porcelain=v1", "-uno"],
        vec!["status", "--porcelain=v1", "-unormal"],
        vec!["status", "--porcelain=v1", "-uall"],
        vec!["status", "--porcelain=v1", "-u"],
        vec!["status", "--porcelain=v1", "--untracked-files"],
        vec!["status", "--porcelain=v1", "--no-untracked-files"],
        vec!["status", "--porcelain=v1", "--untracked-files=no"],
        vec!["status", "--porcelain=v1", "--untracked-files=normal"],
        vec!["status", "--porcelain=v1", "--untracked-files=all"],
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
            "untracked file mode status {:?}\n{}",
            args,
            outcome.report()
        );
    }
}

#[test]
fn status_ignored_outputs_match_git() {
    let fixture = StatusIgnoredFixture::new("ignored-status");

    for args in [
        vec!["status", "--porcelain=v1", "--ignored"],
        vec!["status", "--porcelain=v1", "--ignored=traditional"],
        vec!["status", "--porcelain=v1", "--ignored=matching"],
        vec!["status", "--porcelain=v1", "--ignored", "-z"],
        vec![
            "status",
            "--porcelain=v1",
            "--ignored",
            "--",
            "ignored/deep/a.txt",
        ],
        vec!["status", "--porcelain=v1", "--ignored", "-uno"],
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
            "ignored status {:?}\n{}",
            args,
            outcome.report()
        );
    }
}

#[test]
fn status_quotes_paths_like_git() {
    let fixture = StatusQuotedPathFixture::new("quoted-status");

    for args in [
        vec!["status", "--porcelain=v1"],
        vec!["status", "--porcelain=v1", "--", "tracked space.txt"],
        vec!["status", "--porcelain=v1", "--", "new dir"],
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
            "quoted status {:?}\n{}",
            args,
            outcome.report()
        );
    }
}

#[test]
fn status_null_terminated_output_matches_git() {
    let fixture = StatusQuotedPathFixture::new("null-terminated-status");

    for args in [
        vec!["status", "--porcelain=v1", "-z"],
        vec!["status", "--porcelain=v1", "-z", "--", "tracked space.txt"],
        vec!["status", "--porcelain=v1", "-z", "-uall"],
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
            "null-terminated status {:?}\n{}",
            args,
            outcome.report()
        );
    }
}

#[test]
fn status_branch_header_matches_git() {
    let fixture = DiffFixture::new("branch-header-status");

    for args in [
        vec!["status", "--porcelain=v1", "-b"],
        vec!["status", "--porcelain=v1", "--branch"],
        vec!["status", "--porcelain=v1", "-b", "-z"],
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
            "branch header status {:?}\n{}",
            args,
            outcome.report()
        );
    }
}

#[test]
fn status_detached_branch_header_matches_git() {
    let fixture = DetachedStatusFixture::new("detached-branch-header-status");

    for args in [
        vec!["status", "--porcelain=v1", "-b"],
        vec!["status", "--porcelain=v1", "-b", "-z"],
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
            "detached branch header status {:?}\n{}",
            args,
            outcome.report()
        );
    }
}

#[test]
fn status_unborn_branch_header_matches_git() {
    let fixture = UnbornStatusFixture::new("unborn-branch-header-status");

    for args in [
        vec!["status", "--porcelain=v1", "-b"],
        vec!["status", "--porcelain=v1", "-b", "-z"],
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
            "unborn branch header status {:?}\n{}",
            args,
            outcome.report()
        );
    }
}

#[test]
fn status_index_refresh_difference_is_documented() {
    let fixture = StatusRefreshFixture::new("status-refresh-difference");
    let outcome = compare(&CompareOptions::new(
        fixture.path(),
        git_command(["status", "--porcelain=v1"]),
        rit_command(["status", "--porcelain=v1"]),
    ))
    .expect("comparison should run");
    let state = outcome
        .repository_state
        .as_ref()
        .expect("repository state should be compared");

    assert_eq!(outcome.git.stdout, outcome.rit.stdout);
    assert_eq!(outcome.git.stderr, outcome.rit.stderr);
    assert_eq!(outcome.git.exit_code, outcome.rit.exit_code);
    assert!(
        state
            .differing_paths
            .iter()
            .any(|path| path == Path::new(".git").join("index").as_path()),
        "git status should refresh .git/index while rit status leaves it unchanged\n{}",
        outcome.report()
    );
}

#[test]
fn ls_files_pathspec_outputs_match_git() {
    let fixture = DiffFixture::new("pathspec-ls-files");

    for args in [
        vec!["ls-files", "--", "nested"],
        vec!["ls-files", "--stage", "--", "nested"],
        vec!["ls-files", "--", "*.txt"],
        vec!["ls-files", "--stage", "--", "nested/*.txt"],
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
        vec!["log", "--oneline", "--", "*.txt"],
        vec!["log", "--oneline", "--", "nested/*.txt"],
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
        vec!["show", "--no-patch", "--", "*.txt"],
        vec!["show", "--no-patch", "HEAD", "--", "nested/*.txt"],
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

struct BinaryPatchFixture {
    path: PathBuf,
}

impl BinaryPatchFixture {
    fn new(name: &str) -> Self {
        let path = temp_path(name);
        fs::create_dir_all(&path).expect("fixture directory should be created");
        run_git(&path, ["init", "--quiet"]);
        run_git(&path, ["config", "user.name", "Rit Test"]);
        run_git(&path, ["config", "user.email", "rit@example.test"]);
        run_git(&path, ["config", "core.autocrlf", "false"]);

        fs::write(path.join("mod.dat"), [0, 1, 2]).expect("binary file should be written");
        fs::write(path.join("cached.dat"), [0, 8]).expect("cached file should be written");
        run_git(&path, ["add", "mod.dat", "cached.dat"]);
        run_git(&path, ["commit", "--quiet", "-m", "base"]);

        fs::write(path.join("mod.dat"), [0, 1, 2, 3])
            .expect("worktree binary file should be modified");
        fs::write(path.join("cached.dat"), [0, 8, 9])
            .expect("cached binary file should be modified");
        run_git(&path, ["add", "cached.dat"]);

        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for BinaryPatchFixture {
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

struct MultiHunkDiffFixture {
    path: PathBuf,
}

impl MultiHunkDiffFixture {
    fn new(name: &str) -> Self {
        let path = temp_path(name);
        fs::create_dir_all(&path).expect("fixture directory should be created");
        run_git(&path, ["init", "--quiet"]);
        run_git(&path, ["config", "user.name", "Rit Test"]);
        run_git(&path, ["config", "user.email", "rit@example.test"]);
        run_git(&path, ["config", "core.autocrlf", "false"]);

        let base = (1..=12)
            .map(|number| format!("line{number}\n"))
            .collect::<String>();
        fs::write(path.join("a.txt"), base).expect("base file should be written");
        run_git(&path, ["add", "a.txt"]);
        run_git(&path, ["commit", "--quiet", "-m", "base"]);

        let changed = (1..=12)
            .map(|number| match number {
                2 => "changed2\n".to_owned(),
                10 => "changed10\n".to_owned(),
                _ => format!("line{number}\n"),
            })
            .collect::<String>();
        fs::write(path.join("a.txt"), changed).expect("file should be modified");

        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for MultiHunkDiffFixture {
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

struct StatusIgnoredFixture {
    path: PathBuf,
}

impl StatusIgnoredFixture {
    fn new(name: &str) -> Self {
        let path = temp_path(name);
        fs::create_dir_all(path.join("ignored").join("deep"))
            .expect("ignored directory should be created");
        run_git(&path, ["init", "--quiet"]);
        run_git(&path, ["config", "user.name", "Rit Test"]);
        run_git(&path, ["config", "user.email", "rit@example.test"]);
        run_git(&path, ["config", "core.autocrlf", "false"]);

        fs::write(path.join(".gitignore"), "ignored/\nsecret.txt\n")
            .expect("gitignore should be written");
        run_git(&path, ["add", ".gitignore"]);
        run_git(&path, ["commit", "--quiet", "-m", "base"]);

        fs::write(path.join("ignored").join("deep").join("a.txt"), "ignored\n")
            .expect("ignored file should be written");
        fs::write(path.join("secret.txt"), "secret\n").expect("ignored file should be written");
        fs::write(path.join("visible.txt"), "visible\n").expect("visible file should be written");

        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for StatusIgnoredFixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

struct StatusQuotedPathFixture {
    path: PathBuf,
}

impl StatusQuotedPathFixture {
    fn new(name: &str) -> Self {
        let path = temp_path(name);
        fs::create_dir_all(path.join("new dir")).expect("fixture directory should be created");
        run_git(&path, ["init", "--quiet"]);
        run_git(&path, ["config", "user.name", "Rit Test"]);
        run_git(&path, ["config", "user.email", "rit@example.test"]);
        run_git(&path, ["config", "core.autocrlf", "false"]);

        fs::write(path.join("tracked space.txt"), "base\n")
            .expect("tracked file should be written");
        run_git(&path, ["add", "tracked space.txt"]);
        run_git(&path, ["commit", "--quiet", "-m", "base"]);

        fs::write(path.join("tracked space.txt"), "changed\n")
            .expect("tracked file should be modified");
        fs::write(path.join("new dir").join("new file.txt"), "new\n")
            .expect("untracked file should be written");

        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for StatusQuotedPathFixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

struct DetachedStatusFixture {
    path: PathBuf,
}

impl DetachedStatusFixture {
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
        run_git(&path, ["checkout", "--quiet", "--detach", "HEAD"]);
        fs::write(path.join("tracked.txt"), "changed\n").expect("tracked file should be modified");

        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for DetachedStatusFixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

struct UnbornStatusFixture {
    path: PathBuf,
}

impl UnbornStatusFixture {
    fn new(name: &str) -> Self {
        let path = temp_path(name);
        fs::create_dir_all(&path).expect("fixture directory should be created");
        run_git(&path, ["init", "--quiet"]);
        run_git(&path, ["config", "core.autocrlf", "false"]);

        fs::write(path.join("new.txt"), "new\n").expect("untracked file should be written");

        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for UnbornStatusFixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

struct StatusRefreshFixture {
    path: PathBuf,
}

impl StatusRefreshFixture {
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
        std::thread::sleep(Duration::from_millis(1100));
        fs::write(path.join("tracked.txt"), "base\n")
            .expect("tracked file stat data should change");

        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for StatusRefreshFixture {
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
