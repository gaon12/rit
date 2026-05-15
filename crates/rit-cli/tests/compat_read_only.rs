use rit_testkit::{CommandSpec, CompareOptions, compare};
use std::ffi::{OsStr, OsString};
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
fn diff_cached_exact_rename_outputs_match_git() {
    let fixture = ExactRenameFixture::new("cached-exact-rename");

    for args in [
        vec!["diff", "--cached", "-M", "--name-only"],
        vec!["diff", "--cached", "-M", "--name-status"],
        vec!["diff", "--cached", "-M", "--numstat"],
        vec!["diff", "--cached", "-M", "--stat"],
        vec!["diff", "--cached", "-M"],
        vec!["diff", "--cached", "-M", "-l1", "--name-status"],
    ] {
        let outcome = compare(&CompareOptions::new(
            fixture.path(),
            git_command_slice(&args),
            rit_command_slice(&args),
        ))
        .expect("comparison should run");

        assert!(
            outcome.is_match(),
            "cached exact rename {:?}\n{}",
            args,
            outcome.report()
        );
    }
}

#[test]
fn diff_worktree_intent_to_add_rename_outputs_match_git() {
    let fixture = WorktreeIntentRenameFixture::new("worktree-intent-rename");

    for args in [
        vec!["diff", "-M", "--name-only"],
        vec!["diff", "-M", "--name-status"],
        vec!["diff", "-M", "--numstat"],
        vec!["diff", "-M", "--stat"],
        vec!["diff", "-M"],
        vec!["diff", "-M", "-l1", "--name-status"],
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
            "worktree intent rename {:?}\n{}",
            args,
            outcome.report()
        );
    }
}

#[test]
fn diff_worktree_intent_to_add_similarity_rename_outputs_match_git() {
    let fixture = WorktreeIntentSimilarityRenameFixture::new("worktree-intent-similarity-rename");

    for args in [
        vec!["diff", "-M", "--name-status"],
        vec!["diff", "-M79%", "--name-status"],
        vec!["diff", "--find-renames=79", "--stat"],
        vec!["diff", "-M"],
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
            "worktree intent similarity rename {:?}\n{}",
            args,
            outcome.report()
        );
    }
}

#[test]
fn diff_worktree_intent_to_add_copy_outputs_match_git() {
    let fixture = WorktreeIntentCopyFixture::new("worktree-intent-copy");

    for args in [
        vec!["diff", "-C", "--name-status"],
        vec!["diff", "-C79%", "--name-status"],
        vec!["diff", "--find-copies=79", "--stat"],
        vec!["diff", "-C"],
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
            "worktree intent copy {:?}\n{}",
            args,
            outcome.report()
        );
    }
}

#[test]
fn diff_worktree_find_copies_harder_outputs_match_git() {
    let fixture = WorktreeIntentHardCopyFixture::new("worktree-intent-hard-copy");

    for args in [
        vec!["diff", "--find-copies-harder", "--name-status"],
        vec!["diff", "-C", "--find-copies-harder", "--name-status"],
        vec!["diff", "-C", "--find-copies-harder"],
    ] {
        let os_args = args.iter().map(OsString::from).collect::<Vec<_>>();
        let (git_stdout, git_stderr) = run_capture_args("git", &os_args, fixture.path());
        let (rit_stdout, rit_stderr) = run_capture_args(rit_binary(), &os_args, fixture.path());

        assert_eq!(git_stdout, rit_stdout, "worktree hard copy {args:?}");
        assert_eq!(git_stderr, rit_stderr, "worktree hard copy {args:?}");
    }
}

#[test]
fn diff_cached_similarity_rename_outputs_match_git() {
    let fixture = SimilarityRenameFixture::new("cached-similarity-rename");

    for args in [
        vec!["diff", "--cached", "-M", "--name-status"],
        vec!["diff", "--cached", "-M", "-l0", "--name-status"],
        vec!["diff", "--cached", "-M", "-l1", "--name-status"],
        vec!["diff", "--cached", "-M79%", "--name-status"],
        vec!["diff", "--cached", "--find-renames=79", "--stat"],
        vec!["diff", "--cached", "-M"],
    ] {
        let outcome = compare(&CompareOptions::new(
            fixture.path(),
            git_command_slice(&args),
            rit_command_slice(&args),
        ))
        .expect("comparison should run");

        assert!(
            outcome.is_match(),
            "cached similarity rename {:?}\n{}",
            args,
            outcome.report()
        );
    }
}

#[test]
fn diff_cached_rename_limit_warning_outputs_match_git() {
    let fixture = RenameLimitFixture::new("cached-rename-limit");

    for args in [
        vec!["diff", "--cached", "-M", "-l1", "--name-status"],
        vec!["diff", "--cached", "-M", "-l1"],
    ] {
        let outcome = compare(&CompareOptions::new(
            fixture.path(),
            git_command_slice(&args),
            rit_command_slice(&args),
        ))
        .expect("comparison should run");

        assert!(
            outcome.is_match(),
            "cached rename limit {:?}\n{}",
            args,
            outcome.report()
        );
    }
}

#[test]
fn diff_worktree_rename_limit_warning_outputs_match_git() {
    let fixture = WorktreeIntentRenameLimitFixture::new("worktree-rename-limit");

    for args in [
        vec!["diff", "-M", "-l1", "--name-status"],
        vec!["diff", "-M", "-l1"],
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
            "worktree rename limit {:?}\n{}",
            args,
            outcome.report()
        );
    }
}

#[test]
fn diff_worktree_copy_limit_warning_outputs_match_git() {
    let fixture = WorktreeIntentCopyLimitFixture::new("worktree-copy-limit");

    for args in [
        vec!["diff", "-C", "-l1", "--name-status"],
        vec!["diff", "-C", "-l1"],
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
            "worktree copy limit {:?}\n{}",
            args,
            outcome.report()
        );
    }
}

#[test]
fn diff_cached_copy_outputs_match_git() {
    let fixture = CopyFixture::new("cached-copy");

    for args in [
        vec!["diff", "--cached", "-C", "--name-status"],
        vec!["diff", "--cached", "-C", "-l1", "--name-status"],
        vec!["diff", "--cached", "-C79%", "--name-status"],
        vec!["diff", "--cached", "--find-copies=79", "--stat"],
        vec!["diff", "--cached", "-C"],
    ] {
        let outcome = compare(&CompareOptions::new(
            fixture.path(),
            git_command_slice(&args),
            rit_command_slice(&args),
        ))
        .expect("comparison should run");

        assert!(
            outcome.is_match(),
            "cached copy {:?}\n{}",
            args,
            outcome.report()
        );
    }
}

#[test]
fn diff_cached_find_copies_harder_outputs_match_git() {
    let fixture = HardCopyFixture::new("cached-hard-copy");

    for args in [
        vec!["diff", "--cached", "--find-copies-harder", "--name-status"],
        vec![
            "diff",
            "--cached",
            "-C",
            "--find-copies-harder",
            "--name-status",
        ],
        vec!["diff", "--cached", "-C", "--find-copies-harder"],
    ] {
        let outcome = compare(&CompareOptions::new(
            fixture.path(),
            git_command_slice(&args),
            rit_command_slice(&args),
        ))
        .expect("comparison should run");

        assert!(
            outcome.is_match(),
            "cached hard copy {:?}\n{}",
            args,
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
        (
            vec!["diff", "--name-only", "--", ":(glob)*.txt"],
            vec!["diff", "--name-only", "--", ":(glob)*.txt"],
        ),
        (
            vec!["diff", "--name-only", "--", ":(glob)**/*.txt"],
            vec!["diff", "--name-only", "--", ":(glob)**/*.txt"],
        ),
        (
            vec!["diff", "--name-only", "--", ":(top)nested/base.txt"],
            vec!["diff", "--name-only", "--", ":(top)nested/base.txt"],
        ),
        (
            vec!["diff", "--name-only", "--", ":(icase)camel.txt"],
            vec!["diff", "--name-only", "--", ":(icase)camel.txt"],
        ),
        (
            vec!["diff", "--name-only", "--", "*.txt", ":!b.txt"],
            vec!["diff", "--name-only", "--", "*.txt", ":!b.txt"],
        ),
        (
            vec!["diff", "--name-only", "--", "*.txt", ":(exclude)Camel.txt"],
            vec!["diff", "--name-only", "--", "*.txt", ":(exclude)Camel.txt"],
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
fn diff_attr_pathspec_outputs_match_git() {
    let fixture = AttrPathspecFixture::new("attr-pathspec-diff");

    for args in [
        vec!["diff", "--name-only", "--", ":(attr:text)*"],
        vec!["diff", "--name-only", "--", ":(attr:-text)*"],
        vec!["diff", "--name-only", "--", ":(attr:diff=markdown)*"],
        vec!["diff", "--name-only", "--", ":(attr:!diff)*"],
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
            "attr pathspec diff {:?}\n{}",
            args,
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
        ["status", "--porcelain=v1", "--", ":(literal)a.txt"],
        ["status", "--porcelain=v1", "--", ":(glob)*.txt"],
        ["status", "--porcelain=v1", "--", ":(glob)**/*.txt"],
        ["status", "--porcelain=v1", "--", ":(top)nested/base.txt"],
        ["status", "--porcelain=v1", "--", ":/nested/base.txt"],
        ["status", "--porcelain=v1", "--", ":(icase)camel.txt"],
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

    for args in [
        vec!["status", "--porcelain=v1", "--", "*.txt", ":!b.txt"],
        vec![
            "status",
            "--porcelain=v1",
            "--",
            "*.txt",
            ":(exclude)Camel.txt",
        ],
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
            "exclude pathspec status {:?}\n{}",
            args,
            outcome.report()
        );
    }
}

#[test]
fn status_attr_pathspec_outputs_match_git() {
    let fixture = AttrPathspecFixture::new("attr-pathspec-status");

    for args in [
        vec!["status", "--porcelain=v1", "--", ":(attr:text)*"],
        vec!["status", "--porcelain=v1", "--", ":(attr:-text)*"],
        vec!["status", "--porcelain=v1", "--", ":(attr:diff=markdown)*"],
        vec!["status", "--porcelain=v1", "--", ":(attr:!diff)*"],
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
            "attr pathspec status {:?}\n{}",
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
fn status_ignored_glob_outputs_match_git() {
    let fixture = StatusIgnoredGlobFixture::new("ignored-glob-status");

    for args in [
        vec!["status", "--porcelain=v1", "--ignored", "-uall"],
        vec![
            "status",
            "--porcelain=v1",
            "--ignored",
            "-uall",
            "--",
            "nested/error.log",
        ],
        vec![
            "status",
            "--porcelain=v1",
            "--ignored",
            "-uall",
            "--",
            "docs/deep/generated.txt",
        ],
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
            "ignored glob status {:?}\n{}",
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
fn status_index_refresh_matches_git_state() {
    let fixture = StatusRefreshFixture::new("status-refresh-difference");
    let index_path = fixture.path().join(".git").join("index");
    let stale_index = fs::read(&index_path).expect("stale index should read");

    let (git_stdout, git_stderr) = run_capture("git", ["status", "--porcelain=v1"], fixture.path());
    let git_index = fs::read(&index_path).expect("git-refreshed index should read");

    fs::write(&index_path, &stale_index).expect("stale index should be restored");
    let (rit_stdout, rit_stderr) =
        run_capture(rit_binary(), ["status", "--porcelain=v1"], fixture.path());
    let rit_index = fs::read(&index_path).expect("rit-refreshed index should read");

    assert_eq!(git_stdout, rit_stdout);
    assert_eq!(git_stderr, rit_stderr);
    assert_ne!(stale_index, rit_index);
    assert_eq!(git_index, rit_index);
}

#[test]
fn cat_file_reads_delta_packed_blob_like_git() {
    let fixture = DeltaPackFixture::new("delta-pack-cat-file");
    let blob_id = run_capture("git", ["rev-parse", "HEAD:large.txt"], fixture.path())
        .0
        .trim()
        .to_owned();

    let git = run_capture_args(
        "git",
        &[
            OsString::from("cat-file"),
            OsString::from("-p"),
            OsString::from(&blob_id),
        ],
        fixture.path(),
    )
    .0;
    let rit = run_capture_args(
        rit_binary(),
        &[
            OsString::from("cat-file"),
            OsString::from("-p"),
            OsString::from(&blob_id),
        ],
        fixture.path(),
    )
    .0;

    assert_eq!(git, rit);
}

#[test]
fn ls_files_pathspec_outputs_match_git() {
    let fixture = DiffFixture::new("pathspec-ls-files");

    for args in [
        vec!["ls-files", "--", "nested"],
        vec!["ls-files", "--stage", "--", "nested"],
        vec!["ls-files", "--", "*.txt"],
        vec!["ls-files", "--stage", "--", "nested/*.txt"],
        vec!["ls-files", "--", ":(glob)*.txt"],
        vec!["ls-files", "--stage", "--", ":(top)nested/base.txt"],
        vec!["ls-files", "--", ":(icase)camel.txt"],
        vec!["ls-files", "--", "*.txt", ":!Camel.txt"],
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
fn ls_files_attr_pathspec_outputs_match_git() {
    let fixture = AttrPathspecFixture::new("attr-pathspec-ls-files");

    for args in [
        vec!["ls-files", "--", ":(attr:text)*"],
        vec!["ls-files", "--stage", "--", ":(attr:-text)*"],
        vec!["ls-files", "--", ":(attr:diff=markdown)*"],
        vec!["ls-files", "--", ":(attr:!diff)*"],
    ] {
        let outcome = compare(&CompareOptions::new(
            fixture.path(),
            git_command_slice(&args),
            rit_command_slice(&args),
        ))
        .expect("comparison should run");

        assert!(
            outcome.is_match(),
            "attr pathspec ls-files {:?}\n{}",
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
        vec!["log", "--oneline", "--", ":(glob)*.txt"],
        vec!["log", "--oneline", "--", ":(top)nested/base.txt"],
        vec!["log", "--oneline", "--", ":(icase)camel.txt"],
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
        vec!["show", "--no-patch", "--", ":(glob)*.txt"],
        vec!["show", "--no-patch", "HEAD", "--", ":(top)nested/base.txt"],
        vec!["show", "--no-patch", "--", ":(icase)camel.txt"],
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
        fs::write(path.join("Camel.txt"), "camel\n").expect("case fixture should be written");
        fs::create_dir_all(path.join("nested")).expect("nested directory should be created");
        fs::write(path.join("nested").join("base.txt"), "base\n")
            .expect("nested base file should be written");
        run_git(&path, ["add", "a.txt", "Camel.txt", "nested/base.txt"]);
        run_git(&path, ["commit", "--quiet", "-m", "base"]);

        fs::write(path.join("a.txt"), "one\nthree\nfour\n")
            .expect("modified file should be written");
        fs::write(path.join("nested").join("base.txt"), "base\nstaged\n")
            .expect("nested staged file should be written");
        fs::write(path.join("b.txt"), "new\n").expect("added file should be written");
        run_git(&path, ["add", "a.txt", "b.txt", "nested/base.txt"]);

        fs::write(path.join("a.txt"), "one\nthree\nfour\nfive\n")
            .expect("worktree modification should be written");
        fs::write(path.join("Camel.txt"), "camel changed\n")
            .expect("case fixture should be modified");
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

struct ExactRenameFixture {
    path: PathBuf,
}

impl ExactRenameFixture {
    fn new(name: &str) -> Self {
        let path = temp_path(name);
        fs::create_dir_all(&path).expect("fixture directory should be created");
        run_git(&path, ["init", "--quiet"]);
        run_git(&path, ["config", "user.name", "Rit Test"]);
        run_git(&path, ["config", "user.email", "rit@example.test"]);
        run_git(&path, ["config", "core.autocrlf", "false"]);

        fs::write(path.join("old.txt"), "one\ntwo\n").expect("base file should be written");
        run_git(&path, ["add", "old.txt"]);
        run_git(&path, ["commit", "--quiet", "-m", "base"]);
        run_git(&path, ["mv", "old.txt", "new.txt"]);

        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for ExactRenameFixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

struct WorktreeIntentRenameFixture {
    path: PathBuf,
}

impl WorktreeIntentRenameFixture {
    fn new(name: &str) -> Self {
        let path = temp_path(name);
        fs::create_dir_all(&path).expect("fixture directory should be created");
        run_git(&path, ["init", "--quiet"]);
        run_git(&path, ["config", "user.name", "Rit Test"]);
        run_git(&path, ["config", "user.email", "rit@example.test"]);
        run_git(&path, ["config", "core.autocrlf", "false"]);

        fs::write(path.join("old.txt"), "one\ntwo\nthree\n").expect("base file should be written");
        run_git(&path, ["add", "old.txt"]);
        run_git(&path, ["commit", "--quiet", "-m", "base"]);
        fs::rename(path.join("old.txt"), path.join("new.txt"))
            .expect("worktree file should be renamed");
        run_git(&path, ["add", "-N", "new.txt"]);

        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for WorktreeIntentRenameFixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

struct WorktreeIntentSimilarityRenameFixture {
    path: PathBuf,
}

impl WorktreeIntentSimilarityRenameFixture {
    fn new(name: &str) -> Self {
        let path = temp_path(name);
        fs::create_dir_all(&path).expect("fixture directory should be created");
        run_git(&path, ["init", "--quiet"]);
        run_git(&path, ["config", "user.name", "Rit Test"]);
        run_git(&path, ["config", "user.email", "rit@example.test"]);
        run_git(&path, ["config", "core.autocrlf", "false"]);

        fs::write(path.join("old.txt"), "one\ntwo\nthree\nfour\nfive\n")
            .expect("base file should be written");
        run_git(&path, ["add", "old.txt"]);
        run_git(&path, ["commit", "--quiet", "-m", "base"]);
        fs::remove_file(path.join("old.txt")).expect("old file should be removed");
        fs::write(path.join("new.txt"), "one\ntwo\nthree\nfour\nsix\n")
            .expect("renamed worktree file should be written");
        run_git(&path, ["add", "-N", "new.txt"]);

        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for WorktreeIntentSimilarityRenameFixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

struct WorktreeIntentRenameLimitFixture {
    path: PathBuf,
}

impl WorktreeIntentRenameLimitFixture {
    fn new(name: &str) -> Self {
        let path = temp_path(name);
        fs::create_dir_all(&path).expect("fixture directory should be created");
        run_git(&path, ["init", "--quiet"]);
        run_git(&path, ["config", "user.name", "Rit Test"]);
        run_git(&path, ["config", "user.email", "rit@example.test"]);
        run_git(&path, ["config", "core.autocrlf", "false"]);

        for index in 1..=5 {
            fs::write(
                path.join(format!("old{index}.txt")),
                format!("common\nline{index}\nkeep\n"),
            )
            .expect("old file should be written");
        }
        run_git(&path, ["add", "."]);
        run_git(&path, ["commit", "--quiet", "-m", "base"]);

        for index in 1..=5 {
            fs::remove_file(path.join(format!("old{index}.txt")))
                .expect("old file should be removed");
            fs::write(
                path.join(format!("new{index}.txt")),
                format!("common\nchanged{index}\nkeep\n"),
            )
            .expect("new file should be written");
            run_git(&path, ["add", "-N", &format!("new{index}.txt")]);
        }

        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for WorktreeIntentRenameLimitFixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

struct WorktreeIntentCopyLimitFixture {
    path: PathBuf,
}

impl WorktreeIntentCopyLimitFixture {
    fn new(name: &str) -> Self {
        let path = temp_path(name);
        fs::create_dir_all(&path).expect("fixture directory should be created");
        run_git(&path, ["init", "--quiet"]);
        run_git(&path, ["config", "user.name", "Rit Test"]);
        run_git(&path, ["config", "user.email", "rit@example.test"]);
        run_git(&path, ["config", "core.autocrlf", "false"]);

        for index in 1..=5 {
            fs::write(
                path.join(format!("old{index}.txt")),
                format!("common\nline{index}\nkeep\n"),
            )
            .expect("old file should be written");
        }
        run_git(&path, ["add", "."]);
        run_git(&path, ["commit", "--quiet", "-m", "base"]);

        for index in 1..=5 {
            let contents = format!("common\nchanged{index}\nkeep\n");
            fs::write(path.join(format!("old{index}.txt")), &contents)
                .expect("copy source file should be modified");
            fs::write(path.join(format!("copy{index}.txt")), contents)
                .expect("copy destination file should be written");
            run_git(&path, ["add", "-N", &format!("copy{index}.txt")]);
        }

        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for WorktreeIntentCopyLimitFixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

struct WorktreeIntentCopyFixture {
    path: PathBuf,
}

impl WorktreeIntentCopyFixture {
    fn new(name: &str) -> Self {
        let path = temp_path(name);
        fs::create_dir_all(&path).expect("fixture directory should be created");
        run_git(&path, ["init", "--quiet"]);
        run_git(&path, ["config", "user.name", "Rit Test"]);
        run_git(&path, ["config", "user.email", "rit@example.test"]);
        run_git(&path, ["config", "core.autocrlf", "false"]);

        fs::write(path.join("old.txt"), "one\ntwo\nthree\nfour\nfive\n")
            .expect("base file should be written");
        run_git(&path, ["add", "old.txt"]);
        run_git(&path, ["commit", "--quiet", "-m", "base"]);
        fs::write(path.join("old.txt"), "one\ntwo\nthree\nfour\nsix\n")
            .expect("source file should be modified");
        fs::write(path.join("copy.txt"), "one\ntwo\nthree\nfour\nsix\n")
            .expect("copy file should be written");
        run_git(&path, ["add", "-N", "copy.txt"]);

        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for WorktreeIntentCopyFixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

struct WorktreeIntentHardCopyFixture {
    path: PathBuf,
}

impl WorktreeIntentHardCopyFixture {
    fn new(name: &str) -> Self {
        let path = temp_path(name);
        fs::create_dir_all(&path).expect("fixture directory should be created");
        run_git(&path, ["init", "--quiet"]);
        run_git(&path, ["config", "user.name", "Rit Test"]);
        run_git(&path, ["config", "user.email", "rit@example.test"]);
        run_git(&path, ["config", "core.autocrlf", "false"]);

        fs::write(path.join("old.txt"), "one\ntwo\nthree\nfour\nfive\n")
            .expect("base file should be written");
        run_git(&path, ["add", "old.txt"]);
        run_git(&path, ["commit", "--quiet", "-m", "base"]);
        fs::write(path.join("copy.txt"), "one\ntwo\nthree\nfour\nfive\n")
            .expect("copy file should be written");
        run_git(&path, ["add", "-N", "copy.txt"]);

        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for WorktreeIntentHardCopyFixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

struct SimilarityRenameFixture {
    path: PathBuf,
}

impl SimilarityRenameFixture {
    fn new(name: &str) -> Self {
        let path = temp_path(name);
        fs::create_dir_all(&path).expect("fixture directory should be created");
        run_git(&path, ["init", "--quiet"]);
        run_git(&path, ["config", "user.name", "Rit Test"]);
        run_git(&path, ["config", "user.email", "rit@example.test"]);
        run_git(&path, ["config", "core.autocrlf", "false"]);

        fs::write(path.join("old.txt"), "one\ntwo\nthree\nfour\nfive\n")
            .expect("base file should be written");
        run_git(&path, ["add", "old.txt"]);
        run_git(&path, ["commit", "--quiet", "-m", "base"]);
        fs::remove_file(path.join("old.txt")).expect("old file should be removed");
        fs::write(path.join("new.txt"), "one\ntwo\nthree\nfour\nsix\n")
            .expect("renamed file should be written");
        run_git(&path, ["add", "-A"]);

        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for SimilarityRenameFixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

struct RenameLimitFixture {
    path: PathBuf,
}

impl RenameLimitFixture {
    fn new(name: &str) -> Self {
        let path = temp_path(name);
        fs::create_dir_all(&path).expect("fixture directory should be created");
        run_git(&path, ["init", "--quiet"]);
        run_git(&path, ["config", "user.name", "Rit Test"]);
        run_git(&path, ["config", "user.email", "rit@example.test"]);
        run_git(&path, ["config", "core.autocrlf", "false"]);

        for index in 1..=5 {
            fs::write(
                path.join(format!("old{index}.txt")),
                format!("common\nline{index}\nkeep\n"),
            )
            .expect("old file should be written");
        }
        run_git(&path, ["add", "."]);
        run_git(&path, ["commit", "--quiet", "-m", "base"]);

        for index in 1..=5 {
            fs::remove_file(path.join(format!("old{index}.txt")))
                .expect("old file should be removed");
            fs::write(
                path.join(format!("new{index}.txt")),
                format!("common\nchanged{index}\nkeep\n"),
            )
            .expect("new file should be written");
        }
        run_git(&path, ["add", "-A"]);

        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for RenameLimitFixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

struct CopyFixture {
    path: PathBuf,
}

impl CopyFixture {
    fn new(name: &str) -> Self {
        let path = temp_path(name);
        fs::create_dir_all(&path).expect("fixture directory should be created");
        run_git(&path, ["init", "--quiet"]);
        run_git(&path, ["config", "user.name", "Rit Test"]);
        run_git(&path, ["config", "user.email", "rit@example.test"]);
        run_git(&path, ["config", "core.autocrlf", "false"]);

        fs::write(path.join("old.txt"), "one\ntwo\nthree\nfour\nfive\n")
            .expect("base file should be written");
        run_git(&path, ["add", "old.txt"]);
        run_git(&path, ["commit", "--quiet", "-m", "base"]);
        fs::write(path.join("old.txt"), "one\ntwo\nthree\nfour\nsix\n")
            .expect("source file should be modified");
        fs::write(path.join("copy.txt"), "one\ntwo\nthree\nfour\nsix\n")
            .expect("copy file should be written");
        run_git(&path, ["add", "-A"]);

        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for CopyFixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

struct HardCopyFixture {
    path: PathBuf,
}

impl HardCopyFixture {
    fn new(name: &str) -> Self {
        let path = temp_path(name);
        fs::create_dir_all(&path).expect("fixture directory should be created");
        run_git(&path, ["init", "--quiet"]);
        run_git(&path, ["config", "user.name", "Rit Test"]);
        run_git(&path, ["config", "user.email", "rit@example.test"]);
        run_git(&path, ["config", "core.autocrlf", "false"]);

        fs::write(path.join("old.txt"), "one\ntwo\nthree\nfour\nfive\n")
            .expect("base file should be written");
        run_git(&path, ["add", "old.txt"]);
        run_git(&path, ["commit", "--quiet", "-m", "base"]);
        fs::write(path.join("copy.txt"), "one\ntwo\nthree\nfour\nfive\n")
            .expect("copy file should be written");
        run_git(&path, ["add", "copy.txt"]);

        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for HardCopyFixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

struct AttrPathspecFixture {
    path: PathBuf,
}

impl AttrPathspecFixture {
    fn new(name: &str) -> Self {
        let path = temp_path(name);
        fs::create_dir_all(&path).expect("fixture directory should be created");
        run_git(&path, ["init", "--quiet"]);
        run_git(&path, ["config", "user.name", "Rit Test"]);
        run_git(&path, ["config", "user.email", "rit@example.test"]);
        run_git(&path, ["config", "core.autocrlf", "false"]);
        run_git(&path, ["config", "core.eol", "lf"]);

        fs::write(
            path.join(".gitattributes"),
            "*.rs text\n*.bin -text\ndocs/*.md diff=markdown\nplain.txt !diff\n",
        )
        .expect("attributes file should be written");
        fs::write(path.join("main.rs"), "fn main() {}\n").expect("rust file should be written");
        fs::write(path.join("image.bin"), [0, 1, 2]).expect("binary file should be written");
        fs::create_dir_all(path.join("docs")).expect("docs directory should be created");
        fs::write(path.join("docs").join("readme.md"), "hello\n")
            .expect("markdown file should be written");
        fs::write(path.join("plain.txt"), "plain\n").expect("plain file should be written");
        run_git(
            &path,
            [
                "add",
                ".gitattributes",
                "main.rs",
                "image.bin",
                "docs/readme.md",
                "plain.txt",
            ],
        );
        run_git(&path, ["commit", "--quiet", "-m", "base"]);

        fs::write(path.join("main.rs"), "fn main() { println!(\"hi\"); }\n")
            .expect("rust file should be modified");
        fs::write(path.join("image.bin"), [0, 1, 2, 3]).expect("binary file should be modified");
        fs::write(path.join("docs").join("readme.md"), "hello\nworld\n")
            .expect("markdown file should be modified");
        fs::write(path.join("plain.txt"), "plain\nchanged\n")
            .expect("plain file should be modified");

        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for AttrPathspecFixture {
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

struct StatusIgnoredGlobFixture {
    path: PathBuf,
}

impl StatusIgnoredGlobFixture {
    fn new(name: &str) -> Self {
        let path = temp_path(name);
        fs::create_dir_all(path.join("docs").join("deep"))
            .expect("docs directory should be created");
        fs::create_dir_all(path.join("nested")).expect("nested directory should be created");
        run_git(&path, ["init", "--quiet"]);
        run_git(&path, ["config", "user.name", "Rit Test"]);
        run_git(&path, ["config", "user.email", "rit@example.test"]);
        run_git(&path, ["config", "core.autocrlf", "false"]);

        fs::write(
            path.join(".gitignore"),
            "*.log\nbuild?.tmp\n[ab].cache\n/root-only.txt\ndocs/**/generated.txt\n!keep.log\n",
        )
        .expect("gitignore should be written");
        fs::write(
            path.join(".git").join("info").join("exclude"),
            "local-only.tmp\n",
        )
        .expect("info exclude should be written");
        run_git(&path, ["add", ".gitignore"]);
        run_git(&path, ["commit", "--quiet", "-m", "base"]);

        fs::write(path.join("error.log"), "ignored\n").expect("ignored log should be written");
        fs::write(path.join("nested").join("error.log"), "ignored\n")
            .expect("nested ignored log should be written");
        fs::write(path.join("keep.log"), "visible\n").expect("unignored log should be written");
        fs::write(path.join("build1.tmp"), "ignored\n").expect("ignored tmp should be written");
        fs::write(path.join("build12.tmp"), "visible\n").expect("visible tmp should be written");
        fs::write(path.join("a.cache"), "ignored\n").expect("ignored cache should be written");
        fs::write(path.join("c.cache"), "visible\n").expect("visible cache should be written");
        fs::write(path.join("root-only.txt"), "ignored\n")
            .expect("root ignored file should be written");
        fs::write(path.join("nested").join("root-only.txt"), "visible\n")
            .expect("nested visible file should be written");
        fs::write(path.join("docs").join("generated.txt"), "ignored\n")
            .expect("docs generated file should be written");
        fs::write(
            path.join("docs").join("deep").join("generated.txt"),
            "ignored\n",
        )
        .expect("deep generated file should be written");
        fs::write(path.join("local-only.tmp"), "ignored\n")
            .expect("info excluded file should be written");

        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for StatusIgnoredGlobFixture {
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

struct DeltaPackFixture {
    path: PathBuf,
}

impl DeltaPackFixture {
    fn new(name: &str) -> Self {
        let path = temp_path(name);
        fs::create_dir_all(&path).expect("fixture directory should be created");
        run_git(&path, ["init", "--quiet"]);
        run_git(&path, ["config", "user.name", "Rit Test"]);
        run_git(&path, ["config", "user.email", "rit@example.test"]);
        run_git(&path, ["config", "core.autocrlf", "false"]);

        let base = "common line\n".repeat(2000);
        fs::write(path.join("large.txt"), &base).expect("base file should be written");
        run_git(&path, ["add", "large.txt"]);
        run_git(&path, ["commit", "--quiet", "-m", "base"]);

        let changed = format!("{base}changed line\n");
        fs::write(path.join("large.txt"), changed).expect("changed file should be written");
        run_git(&path, ["add", "large.txt"]);
        run_git(&path, ["commit", "--quiet", "-m", "change"]);
        run_git(&path, ["gc", "--aggressive", "--prune=now"]);
        assert!(
            verify_pack_has_delta(&path),
            "fixture should contain at least one delta object"
        );

        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for DeltaPackFixture {
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

fn run_capture_args(program: impl AsRef<OsStr>, args: &[OsString], cwd: &Path) -> (String, String) {
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

fn verify_pack_has_delta(path: &Path) -> bool {
    let pack_dir = path.join(".git").join("objects").join("pack");
    for entry in fs::read_dir(&pack_dir).expect("pack directory should read") {
        let entry = entry.expect("pack entry should read");
        let index_path = entry.path();
        if index_path
            .extension()
            .and_then(|extension| extension.to_str())
            != Some("idx")
        {
            continue;
        }
        let output = Command::new("git")
            .arg("verify-pack")
            .arg("-v")
            .arg(&index_path)
            .current_dir(path)
            .output()
            .expect("git verify-pack should start");
        assert!(
            output.status.success(),
            "git verify-pack failed\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        let stdout = String::from_utf8_lossy(&output.stdout);
        if stdout
            .lines()
            .any(|line| line.contains(" blob ") && line.split_whitespace().count() >= 7)
        {
            return true;
        }
    }
    false
}

fn temp_path(name: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("rit-cli-compat-{name}-{unique}"))
}
