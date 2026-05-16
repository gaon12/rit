use std::ffi::{OsStr, OsString};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn rebase_quit_without_state_matches_git() {
    let root = temp_path("quit-empty");
    let git_repo = root.join("git");
    let rit_repo = root.join("rit");
    init_repo(&git_repo);
    copy_directory(&git_repo, &rit_repo);

    let git_quit = run_capture("git", ["rebase", "--quit"], &git_repo);
    let rit_quit = run_capture(rit_binary(), ["rebase", "--quit"], &rit_repo);

    assert_eq!(git_quit.exit_code, rit_quit.exit_code);
    assert_eq!(git_quit.stdout, rit_quit.stdout);
    assert_eq!(git_quit.stderr, rit_quit.stderr);

    let _ = fs::remove_dir_all(root);
}

#[test]
fn rebase_abort_without_state_matches_git() {
    let root = temp_path("abort-empty");
    let git_repo = root.join("git");
    let rit_repo = root.join("rit");
    init_repo(&git_repo);
    copy_directory(&git_repo, &rit_repo);

    let git_abort = run_capture("git", ["rebase", "--abort"], &git_repo);
    let rit_abort = run_capture(rit_binary(), ["rebase", "--abort"], &rit_repo);

    assert_eq!(git_abort.exit_code, rit_abort.exit_code);
    assert_eq!(git_abort.stdout, rit_abort.stdout);
    assert_eq!(git_abort.stderr, rit_abort.stderr);

    let _ = fs::remove_dir_all(root);
}

#[test]
fn rebase_quit_clears_rebase_merge_state_like_git() {
    let root = temp_path("quit-state");
    let git_repo = root.join("git");
    let rit_repo = root.join("rit");
    init_repo(&git_repo);
    write_rebase_merge_state(&git_repo);
    copy_directory(&git_repo, &rit_repo);

    let git_quit = run_capture("git", ["rebase", "--quit"], &git_repo);
    let rit_quit = run_capture(rit_binary(), ["rebase", "--quit"], &rit_repo);

    assert_eq!(git_quit.exit_code, 0, "git stderr: {}", git_quit.stderr);
    assert_eq!(rit_quit.exit_code, 0, "rit stderr: {}", rit_quit.stderr);
    assert_eq!(git_quit.stdout, rit_quit.stdout);
    assert_eq!(git_quit.stderr, rit_quit.stderr);
    assert_eq!(
        run_capture("git", ["rev-parse", "HEAD"], &git_repo).stdout,
        run_capture(rit_binary(), ["rev-parse", "HEAD"], &rit_repo).stdout
    );
    assert_eq!(
        git_repo.join(".git").join("rebase-merge").exists(),
        rit_repo.join(".git").join("rebase-merge").exists()
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn rebase_abort_restores_original_branch_like_git() {
    let root = temp_path("abort-conflict");
    let git_repo = root.join("git");
    let rit_repo = root.join("rit");
    init_repo(&git_repo);
    create_conflicting_rebase_state(&git_repo);
    copy_directory(&git_repo, &rit_repo);

    let git_abort = run_capture("git", ["rebase", "--abort"], &git_repo);
    let rit_abort = run_capture(rit_binary(), ["rebase", "--abort"], &rit_repo);

    assert_eq!(git_abort.exit_code, 0, "git stderr: {}", git_abort.stderr);
    assert_eq!(rit_abort.exit_code, 0, "rit stderr: {}", rit_abort.stderr);
    assert_eq!(git_abort.stdout, rit_abort.stdout);
    assert_eq!(git_abort.stderr, rit_abort.stderr);
    assert_eq!(
        run_capture("git", ["rev-parse", "HEAD"], &git_repo).stdout,
        run_capture(rit_binary(), ["rev-parse", "HEAD"], &rit_repo).stdout
    );
    assert_eq!(
        run_capture(
            "git",
            ["symbolic-ref", "--quiet", "--short", "HEAD"],
            &git_repo
        )
        .stdout,
        run_capture(
            "git",
            ["symbolic-ref", "--quiet", "--short", "HEAD"],
            &rit_repo
        )
        .stdout
    );
    assert_eq!(
        run_capture("git", ["status", "--porcelain=v1"], &git_repo).stdout,
        run_capture(rit_binary(), ["status", "--porcelain=v1"], &rit_repo).stdout
    );
    assert_eq!(
        fs::read_to_string(git_repo.join("tracked.txt")).expect("git file should read"),
        fs::read_to_string(rit_repo.join("tracked.txt")).expect("rit file should read")
    );
    for state_path in [
        "rebase-merge",
        "rebase-apply",
        "REBASE_HEAD",
        "MERGE_MSG",
        "AUTO_MERGE",
    ] {
        assert_eq!(
            git_repo.join(".git").join(state_path).exists(),
            rit_repo.join(".git").join(state_path).exists(),
            "{state_path}"
        );
    }

    let _ = fs::remove_dir_all(root);
}

#[test]
fn rebase_show_current_patch_matches_git() {
    let root = temp_path("show-current-patch");
    let git_repo = root.join("git");
    let rit_repo = root.join("rit");
    init_repo(&git_repo);
    create_conflicting_rebase_state(&git_repo);
    copy_directory(&git_repo, &rit_repo);

    let git_show = run_capture("git", ["rebase", "--show-current-patch"], &git_repo);
    let rit_show = run_capture(rit_binary(), ["rebase", "--show-current-patch"], &rit_repo);

    assert_eq!(git_show.exit_code, 0, "git stderr: {}", git_show.stderr);
    assert_eq!(rit_show.exit_code, 0, "rit stderr: {}", rit_show.stderr);
    assert_eq!(git_show.stdout, rit_show.stdout);
    assert_eq!(git_show.stderr, rit_show.stderr);

    let _ = fs::remove_dir_all(root);
}

#[test]
fn rebase_show_current_patch_without_state_matches_git() {
    let root = temp_path("show-current-patch-empty");
    let git_repo = root.join("git");
    let rit_repo = root.join("rit");
    init_repo(&git_repo);
    copy_directory(&git_repo, &rit_repo);

    let git_show = run_capture("git", ["rebase", "--show-current-patch"], &git_repo);
    let rit_show = run_capture(rit_binary(), ["rebase", "--show-current-patch"], &rit_repo);

    assert_eq!(git_show.exit_code, rit_show.exit_code);
    assert_eq!(git_show.stdout, rit_show.stdout);
    assert_eq!(git_show.stderr, rit_show.stderr);

    let _ = fs::remove_dir_all(root);
}

#[test]
fn rebase_skip_final_stopped_commit_matches_git() {
    let root = temp_path("skip-final");
    let git_repo = root.join("git");
    let rit_repo = root.join("rit");
    init_repo(&git_repo);
    create_conflicting_rebase_state(&git_repo);
    copy_directory(&git_repo, &rit_repo);

    let git_skip = run_capture("git", ["rebase", "--skip"], &git_repo);
    let rit_skip = run_capture(rit_binary(), ["rebase", "--skip"], &rit_repo);

    assert_eq!(git_skip.exit_code, 0, "git stderr: {}", git_skip.stderr);
    assert_eq!(rit_skip.exit_code, 0, "rit stderr: {}", rit_skip.stderr);
    assert_eq!(git_skip.stdout, rit_skip.stdout);
    assert_eq!(git_skip.stderr, rit_skip.stderr);
    assert_eq!(
        run_capture("git", ["rev-parse", "HEAD"], &git_repo).stdout,
        run_capture(rit_binary(), ["rev-parse", "HEAD"], &rit_repo).stdout
    );
    assert_eq!(
        run_capture(
            "git",
            ["symbolic-ref", "--quiet", "--short", "HEAD"],
            &git_repo
        )
        .stdout,
        run_capture(
            "git",
            ["symbolic-ref", "--quiet", "--short", "HEAD"],
            &rit_repo
        )
        .stdout
    );
    assert_eq!(
        run_capture("git", ["status", "--porcelain=v1"], &git_repo).stdout,
        run_capture(rit_binary(), ["status", "--porcelain=v1"], &rit_repo).stdout
    );
    assert_eq!(
        fs::read_to_string(git_repo.join("tracked.txt")).expect("git file should read"),
        fs::read_to_string(rit_repo.join("tracked.txt")).expect("rit file should read")
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn rebase_skip_without_state_matches_git() {
    let root = temp_path("skip-empty");
    let git_repo = root.join("git");
    let rit_repo = root.join("rit");
    init_repo(&git_repo);
    copy_directory(&git_repo, &rit_repo);

    let git_skip = run_capture("git", ["rebase", "--skip"], &git_repo);
    let rit_skip = run_capture(rit_binary(), ["rebase", "--skip"], &rit_repo);

    assert_eq!(git_skip.exit_code, rit_skip.exit_code);
    assert_eq!(git_skip.stdout, rit_skip.stdout);
    assert_eq!(git_skip.stderr, rit_skip.stderr);

    let _ = fs::remove_dir_all(root);
}

#[test]
fn rebase_continue_final_resolved_commit_matches_git() {
    let root = temp_path("continue-final");
    let git_repo = root.join("git");
    let rit_repo = root.join("rit");
    init_repo(&git_repo);
    create_conflicting_rebase_state(&git_repo);
    copy_directory(&git_repo, &rit_repo);
    fs::write(git_repo.join("tracked.txt"), "resolved\n").expect("git resolution should write");
    fs::write(rit_repo.join("tracked.txt"), "resolved\n").expect("rit resolution should write");
    run_git(&git_repo, ["add", "tracked.txt"]);
    run_git(&rit_repo, ["add", "tracked.txt"]);
    let envs = [
        ("GIT_EDITOR", "true"),
        ("GIT_COMMITTER_DATE", "1700000000 +0900"),
    ];

    let git_continue = run_capture_with_env("git", ["rebase", "--continue"], &git_repo, &envs);
    let rit_continue =
        run_capture_with_env(rit_binary(), ["rebase", "--continue"], &rit_repo, &envs);

    assert_eq!(
        git_continue.exit_code, 0,
        "git stderr: {}",
        git_continue.stderr
    );
    assert_eq!(
        rit_continue.exit_code, 0,
        "rit stderr: {}",
        rit_continue.stderr
    );
    assert_eq!(git_continue.stdout, rit_continue.stdout);
    assert_eq!(git_continue.stderr, rit_continue.stderr);
    assert_eq!(
        run_capture("git", ["rev-parse", "HEAD"], &git_repo).stdout,
        run_capture(rit_binary(), ["rev-parse", "HEAD"], &rit_repo).stdout
    );
    assert_eq!(
        run_capture(
            "git",
            ["symbolic-ref", "--quiet", "--short", "HEAD"],
            &git_repo
        )
        .stdout,
        run_capture(
            "git",
            ["symbolic-ref", "--quiet", "--short", "HEAD"],
            &rit_repo
        )
        .stdout
    );
    assert_eq!(
        run_capture("git", ["status", "--porcelain=v1"], &git_repo).stdout,
        run_capture(rit_binary(), ["status", "--porcelain=v1"], &rit_repo).stdout
    );
    assert!(!git_repo.join(".git").join("rebase-merge").exists());
    assert!(!rit_repo.join(".git").join("rebase-merge").exists());

    let _ = fs::remove_dir_all(root);
}

#[test]
fn rebase_continue_without_state_matches_git() {
    let root = temp_path("continue-empty");
    let git_repo = root.join("git");
    let rit_repo = root.join("rit");
    init_repo(&git_repo);
    copy_directory(&git_repo, &rit_repo);

    let git_continue = run_capture("git", ["rebase", "--continue"], &git_repo);
    let rit_continue = run_capture(rit_binary(), ["rebase", "--continue"], &rit_repo);

    assert_eq!(git_continue.exit_code, rit_continue.exit_code);
    assert_eq!(git_continue.stdout, rit_continue.stdout);
    assert_eq!(git_continue.stderr, rit_continue.stderr);

    let _ = fs::remove_dir_all(root);
}

#[test]
fn rebase_up_to_date_branch_matches_git() {
    let root = temp_path("start-up-to-date");
    let git_repo = root.join("git");
    let rit_repo = root.join("rit");
    init_repo(&git_repo);
    run_git(&git_repo, ["branch", "topic"]);
    fs::write(git_repo.join("tracked.txt"), "master\n").expect("tracked file should write");
    run_git(&git_repo, ["commit", "--quiet", "-am", "master"]);
    fs::write(git_repo.join("untracked.txt"), "u\n").expect("untracked file should write");
    copy_directory(&git_repo, &rit_repo);

    let git_rebase = run_capture("git", ["rebase", "topic"], &git_repo);
    let rit_rebase = run_capture(rit_binary(), ["rebase", "topic"], &rit_repo);

    assert_eq!(git_rebase.exit_code, 0, "git stderr: {}", git_rebase.stderr);
    assert_eq!(rit_rebase.exit_code, 0, "rit stderr: {}", rit_rebase.stderr);
    assert_eq!(git_rebase.stdout, rit_rebase.stdout);
    assert_eq!(git_rebase.stderr, rit_rebase.stderr);
    assert_eq!(
        run_capture("git", ["rev-parse", "HEAD"], &git_repo).stdout,
        run_capture(rit_binary(), ["rev-parse", "HEAD"], &rit_repo).stdout
    );
    assert_eq!(
        run_capture("git", ["status", "--porcelain=v1"], &git_repo).stdout,
        run_capture(rit_binary(), ["status", "--porcelain=v1"], &rit_repo).stdout
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn rebase_up_to_date_detached_head_matches_git() {
    let root = temp_path("start-detached-up-to-date");
    let git_repo = root.join("git");
    let rit_repo = root.join("rit");
    init_repo(&git_repo);
    run_git(&git_repo, ["branch", "topic"]);
    run_git(&git_repo, ["checkout", "--detach", "HEAD"]);
    copy_directory(&git_repo, &rit_repo);

    let git_rebase = run_capture("git", ["rebase", "topic"], &git_repo);
    let rit_rebase = run_capture(rit_binary(), ["rebase", "topic"], &rit_repo);

    assert_eq!(git_rebase.exit_code, 0, "git stderr: {}", git_rebase.stderr);
    assert_eq!(rit_rebase.exit_code, 0, "rit stderr: {}", rit_rebase.stderr);
    assert_eq!(git_rebase.stdout, rit_rebase.stdout);
    assert_eq!(git_rebase.stderr, rit_rebase.stderr);
    assert_eq!(
        run_capture("git", ["rev-parse", "HEAD"], &git_repo).stdout,
        run_capture(rit_binary(), ["rev-parse", "HEAD"], &rit_repo).stdout
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn rebase_fast_forwards_branch_to_upstream_like_git() {
    let root = temp_path("start-fast-forward");
    let git_repo = root.join("git");
    let rit_repo = root.join("rit");
    init_repo(&git_repo);
    run_git(&git_repo, ["checkout", "-b", "topic"]);
    fs::write(git_repo.join("tracked.txt"), "topic\n").expect("tracked file should write");
    run_git(&git_repo, ["commit", "--quiet", "-am", "topic"]);
    run_git(&git_repo, ["checkout", "master"]);
    copy_directory(&git_repo, &rit_repo);

    let git_rebase = run_capture("git", ["rebase", "topic"], &git_repo);
    let rit_rebase = run_capture(rit_binary(), ["rebase", "topic"], &rit_repo);

    assert_eq!(git_rebase.exit_code, 0, "git stderr: {}", git_rebase.stderr);
    assert_eq!(rit_rebase.exit_code, 0, "rit stderr: {}", rit_rebase.stderr);
    assert_eq!(git_rebase.stdout, rit_rebase.stdout);
    assert_eq!(git_rebase.stderr, rit_rebase.stderr);
    assert_eq!(
        run_capture("git", ["rev-parse", "HEAD"], &git_repo).stdout,
        run_capture(rit_binary(), ["rev-parse", "HEAD"], &rit_repo).stdout
    );
    assert_eq!(
        run_capture(
            "git",
            ["symbolic-ref", "--quiet", "--short", "HEAD"],
            &git_repo
        )
        .stdout,
        run_capture(
            "git",
            ["symbolic-ref", "--quiet", "--short", "HEAD"],
            &rit_repo
        )
        .stdout
    );
    assert_eq!(
        fs::read_to_string(git_repo.join("tracked.txt")).expect("git file should read"),
        fs::read_to_string(rit_repo.join("tracked.txt")).expect("rit file should read")
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn rebase_fast_forwards_detached_head_to_upstream_like_git() {
    let root = temp_path("start-detached-fast-forward");
    let git_repo = root.join("git");
    let rit_repo = root.join("rit");
    init_repo(&git_repo);
    run_git(&git_repo, ["checkout", "-b", "topic"]);
    fs::write(git_repo.join("tracked.txt"), "topic\n").expect("tracked file should write");
    run_git(&git_repo, ["commit", "--quiet", "-am", "topic"]);
    run_git(&git_repo, ["checkout", "--detach", "master"]);
    copy_directory(&git_repo, &rit_repo);

    let git_rebase = run_capture("git", ["rebase", "topic"], &git_repo);
    let rit_rebase = run_capture(rit_binary(), ["rebase", "topic"], &rit_repo);

    assert_eq!(git_rebase.exit_code, 0, "git stderr: {}", git_rebase.stderr);
    assert_eq!(rit_rebase.exit_code, 0, "rit stderr: {}", rit_rebase.stderr);
    assert_eq!(git_rebase.stdout, rit_rebase.stdout);
    assert_eq!(git_rebase.stderr, rit_rebase.stderr);
    assert_eq!(
        run_capture("git", ["rev-parse", "HEAD"], &git_repo).stdout,
        run_capture(rit_binary(), ["rev-parse", "HEAD"], &rit_repo).stdout
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn rebase_replays_one_clean_commit_like_git() {
    let root = temp_path("start-clean-replay");
    let git_repo = root.join("git");
    let rit_repo = root.join("rit");
    init_repo(&git_repo);
    run_git(&git_repo, ["checkout", "-b", "topic"]);
    fs::write(git_repo.join("topic.txt"), "topic\n").expect("topic file should write");
    run_git(&git_repo, ["add", "topic.txt"]);
    run_git(&git_repo, ["commit", "--quiet", "-m", "topic"]);
    run_git(&git_repo, ["checkout", "master"]);
    fs::write(git_repo.join("master.txt"), "master\n").expect("master file should write");
    run_git(&git_repo, ["add", "master.txt"]);
    run_git(&git_repo, ["commit", "--quiet", "-m", "master"]);
    copy_directory(&git_repo, &rit_repo);
    let envs = [("GIT_COMMITTER_DATE", "1700000000 +0900")];

    let git_rebase = run_capture_with_env("git", ["rebase", "topic"], &git_repo, &envs);
    let rit_rebase = run_capture_with_env(rit_binary(), ["rebase", "topic"], &rit_repo, &envs);

    assert_eq!(git_rebase.exit_code, 0, "git stderr: {}", git_rebase.stderr);
    assert_eq!(rit_rebase.exit_code, 0, "rit stderr: {}", rit_rebase.stderr);
    assert_eq!(git_rebase.stdout, rit_rebase.stdout);
    assert_eq!(git_rebase.stderr, rit_rebase.stderr);
    assert_eq!(
        run_capture("git", ["rev-parse", "HEAD"], &git_repo).stdout,
        run_capture(rit_binary(), ["rev-parse", "HEAD"], &rit_repo).stdout
    );
    assert_eq!(
        run_capture("git", ["log", "-1", "--format=%P%n%B"], &git_repo).stdout,
        run_capture("git", ["log", "-1", "--format=%P%n%B"], &rit_repo).stdout
    );
    assert_eq!(
        run_capture("git", ["status", "--porcelain=v1"], &git_repo).stdout,
        run_capture(rit_binary(), ["status", "--porcelain=v1"], &rit_repo).stdout
    );
    assert_eq!(
        run_capture("git", ["ls-files"], &git_repo).stdout,
        run_capture(rit_binary(), ["ls-files"], &rit_repo).stdout
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn rebase_replays_two_clean_commits_like_git() {
    let root = temp_path("start-two-clean-replay");
    let git_repo = root.join("git");
    let rit_repo = root.join("rit");
    init_repo(&git_repo);
    run_git(&git_repo, ["checkout", "-b", "topic"]);
    fs::write(git_repo.join("topic.txt"), "topic\n").expect("topic file should write");
    run_git(&git_repo, ["add", "topic.txt"]);
    run_git(&git_repo, ["commit", "--quiet", "-m", "topic"]);
    run_git(&git_repo, ["checkout", "master"]);
    fs::write(git_repo.join("one.txt"), "one\n").expect("one file should write");
    run_git(&git_repo, ["add", "one.txt"]);
    run_git(&git_repo, ["commit", "--quiet", "-m", "one"]);
    fs::write(git_repo.join("two.txt"), "two\n").expect("two file should write");
    run_git(&git_repo, ["add", "two.txt"]);
    run_git(&git_repo, ["commit", "--quiet", "-m", "two"]);
    copy_directory(&git_repo, &rit_repo);
    let envs = [("GIT_COMMITTER_DATE", "1700000000 +0900")];

    let git_rebase = run_capture_with_env("git", ["rebase", "topic"], &git_repo, &envs);
    let rit_rebase = run_capture_with_env(rit_binary(), ["rebase", "topic"], &rit_repo, &envs);

    assert_eq!(git_rebase.exit_code, 0, "git stderr: {}", git_rebase.stderr);
    assert_eq!(rit_rebase.exit_code, 0, "rit stderr: {}", rit_rebase.stderr);
    assert_eq!(git_rebase.stdout, rit_rebase.stdout);
    assert_eq!(git_rebase.stderr, rit_rebase.stderr);
    assert_eq!(
        run_capture("git", ["rev-parse", "HEAD"], &git_repo).stdout,
        run_capture(rit_binary(), ["rev-parse", "HEAD"], &rit_repo).stdout
    );
    assert_eq!(
        run_capture("git", ["log", "--format=%P%n%B", "-2"], &git_repo).stdout,
        run_capture("git", ["log", "--format=%P%n%B", "-2"], &rit_repo).stdout
    );
    assert_eq!(
        run_capture("git", ["status", "--porcelain=v1"], &git_repo).stdout,
        run_capture(rit_binary(), ["status", "--porcelain=v1"], &rit_repo).stdout
    );
    assert_eq!(
        run_capture("git", ["ls-files"], &git_repo).stdout,
        run_capture(rit_binary(), ["ls-files"], &rit_repo).stdout
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn rebase_stops_on_single_conflict_like_git() {
    let root = temp_path("start-single-conflict");
    let git_repo = root.join("git");
    let rit_repo = root.join("rit");
    init_repo(&git_repo);
    run_git(&git_repo, ["checkout", "-b", "topic"]);
    fs::write(git_repo.join("tracked.txt"), "topic\n").expect("topic file should write");
    run_git(&git_repo, ["commit", "--quiet", "-am", "topic"]);
    run_git(&git_repo, ["checkout", "master"]);
    fs::write(git_repo.join("tracked.txt"), "master\n").expect("master file should write");
    run_git(&git_repo, ["commit", "--quiet", "-am", "master"]);
    copy_directory(&git_repo, &rit_repo);

    let git_rebase = run_capture("git", ["rebase", "topic"], &git_repo);
    let rit_rebase = run_capture(rit_binary(), ["rebase", "topic"], &rit_repo);

    assert_eq!(git_rebase.exit_code, 1, "git stderr: {}", git_rebase.stderr);
    assert_eq!(rit_rebase.exit_code, 1, "rit stderr: {}", rit_rebase.stderr);
    assert_eq!(git_rebase.stdout, rit_rebase.stdout);
    assert_eq!(git_rebase.stderr, rit_rebase.stderr);
    assert_eq!(
        fs::read_to_string(git_repo.join("tracked.txt")).expect("git tracked file should read"),
        fs::read_to_string(rit_repo.join("tracked.txt")).expect("rit tracked file should read")
    );
    for path in [
        "HEAD",
        "REBASE_HEAD",
        "MERGE_MSG",
        "refs/heads/master",
        "rebase-merge/head-name",
        "rebase-merge/onto",
        "rebase-merge/orig-head",
        "rebase-merge/msgnum",
        "rebase-merge/end",
        "rebase-merge/stopped-sha",
        "rebase-merge/message",
        "rebase-merge/git-rebase-todo",
        "rebase-merge/done",
        "rebase-merge/author-script",
    ] {
        assert_eq!(
            fs::read_to_string(git_repo.join(".git").join(path))
                .unwrap_or_else(|error| panic!("git state {path} should read: {error}")),
            fs::read_to_string(rit_repo.join(".git").join(path))
                .unwrap_or_else(|error| panic!("rit state {path} should read: {error}")),
            "state file {path} should match"
        );
    }
    assert_eq!(
        run_capture("git", ["ls-files", "-s"], &git_repo).stdout,
        run_capture(rit_binary(), ["ls-files", "-s"], &rit_repo).stdout
    );
    assert_eq!(
        run_capture("git", ["status", "--porcelain=v1"], &git_repo).stdout,
        run_capture(rit_binary(), ["status", "--porcelain=v1"], &rit_repo).stdout
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn rebase_stops_on_final_conflict_after_clean_replay_like_git() {
    let root = temp_path("final-conflict-after-clean");
    let git_repo = root.join("git");
    let rit_repo = root.join("rit");
    init_repo(&git_repo);
    run_git(&git_repo, ["checkout", "-b", "topic"]);
    fs::write(git_repo.join("tracked.txt"), "topic\n").expect("topic file should write");
    run_git(&git_repo, ["commit", "--quiet", "-am", "topic"]);
    run_git(&git_repo, ["checkout", "master"]);
    fs::write(git_repo.join("one.txt"), "one\n").expect("one file should write");
    run_git(&git_repo, ["add", "one.txt"]);
    run_git(&git_repo, ["commit", "--quiet", "-m", "one"]);
    fs::write(git_repo.join("tracked.txt"), "master\n").expect("master file should write");
    run_git(&git_repo, ["commit", "--quiet", "-am", "master"]);
    copy_directory(&git_repo, &rit_repo);
    let envs = [("GIT_COMMITTER_DATE", "1700000000 +0900")];

    let git_rebase = run_capture_with_env("git", ["rebase", "topic"], &git_repo, &envs);
    let rit_rebase = run_capture_with_env(rit_binary(), ["rebase", "topic"], &rit_repo, &envs);

    assert_eq!(git_rebase.exit_code, 1, "git stderr: {}", git_rebase.stderr);
    assert_eq!(rit_rebase.exit_code, 1, "rit stderr: {}", rit_rebase.stderr);
    assert_eq!(git_rebase.stdout, rit_rebase.stdout);
    assert_eq!(git_rebase.stderr, rit_rebase.stderr);
    for path in [
        "HEAD",
        "REBASE_HEAD",
        "MERGE_MSG",
        "refs/heads/master",
        "rebase-merge/msgnum",
        "rebase-merge/end",
        "rebase-merge/stopped-sha",
        "rebase-merge/git-rebase-todo",
        "rebase-merge/done",
    ] {
        assert_eq!(
            fs::read_to_string(git_repo.join(".git").join(path))
                .unwrap_or_else(|error| panic!("git state {path} should read: {error}")),
            fs::read_to_string(rit_repo.join(".git").join(path))
                .unwrap_or_else(|error| panic!("rit state {path} should read: {error}")),
            "state file {path} should match"
        );
    }
    assert_eq!(
        run_capture("git", ["ls-files", "-s"], &git_repo).stdout,
        run_capture(rit_binary(), ["ls-files", "-s"], &rit_repo).stdout
    );
    assert_eq!(
        run_capture("git", ["status", "--porcelain=v1"], &git_repo).stdout,
        run_capture(rit_binary(), ["status", "--porcelain=v1"], &rit_repo).stdout
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn rebase_stops_on_conflict_with_remaining_todo_like_git() {
    let root = temp_path("conflict-with-remaining-todo");
    let git_repo = root.join("git");
    let rit_repo = root.join("rit");
    init_repo(&git_repo);
    run_git(&git_repo, ["checkout", "-b", "topic"]);
    fs::write(git_repo.join("tracked.txt"), "topic\n").expect("topic file should write");
    run_git(&git_repo, ["commit", "--quiet", "-am", "topic"]);
    run_git(&git_repo, ["checkout", "master"]);
    fs::write(git_repo.join("tracked.txt"), "master\n").expect("master file should write");
    run_git(&git_repo, ["commit", "--quiet", "-am", "conflict"]);
    fs::write(git_repo.join("after.txt"), "after\n").expect("after file should write");
    run_git(&git_repo, ["add", "after.txt"]);
    run_git(&git_repo, ["commit", "--quiet", "-m", "after"]);
    copy_directory(&git_repo, &rit_repo);

    let git_rebase = run_capture("git", ["rebase", "topic"], &git_repo);
    let rit_rebase = run_capture(rit_binary(), ["rebase", "topic"], &rit_repo);

    assert_eq!(git_rebase.exit_code, 1, "git stderr: {}", git_rebase.stderr);
    assert_eq!(rit_rebase.exit_code, 1, "rit stderr: {}", rit_rebase.stderr);
    assert_eq!(git_rebase.stdout, rit_rebase.stdout);
    assert_eq!(git_rebase.stderr, rit_rebase.stderr);
    for path in [
        "HEAD",
        "REBASE_HEAD",
        "MERGE_MSG",
        "refs/heads/master",
        "rebase-merge/msgnum",
        "rebase-merge/end",
        "rebase-merge/stopped-sha",
        "rebase-merge/git-rebase-todo",
        "rebase-merge/done",
    ] {
        assert_eq!(
            fs::read_to_string(git_repo.join(".git").join(path))
                .unwrap_or_else(|error| panic!("git state {path} should read: {error}")),
            fs::read_to_string(rit_repo.join(".git").join(path))
                .unwrap_or_else(|error| panic!("rit state {path} should read: {error}")),
            "state file {path} should match"
        );
    }
    assert_eq!(
        run_capture("git", ["ls-files", "-s"], &git_repo).stdout,
        run_capture(rit_binary(), ["ls-files", "-s"], &rit_repo).stdout
    );
    assert_eq!(
        run_capture("git", ["status", "--porcelain=v1"], &git_repo).stdout,
        run_capture(rit_binary(), ["status", "--porcelain=v1"], &rit_repo).stdout
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn rebase_continue_replays_remaining_clean_todo_like_git() {
    let root = temp_path("continue-remaining-clean-todo");
    let git_repo = root.join("git");
    let rit_repo = root.join("rit");
    init_repo(&git_repo);
    run_git(&git_repo, ["checkout", "-b", "topic"]);
    fs::write(git_repo.join("tracked.txt"), "topic\n").expect("topic file should write");
    run_git(&git_repo, ["commit", "--quiet", "-am", "topic"]);
    run_git(&git_repo, ["checkout", "master"]);
    fs::write(git_repo.join("tracked.txt"), "master\n").expect("master file should write");
    run_git(&git_repo, ["commit", "--quiet", "-am", "conflict"]);
    fs::write(git_repo.join("after.txt"), "after\n").expect("after file should write");
    run_git(&git_repo, ["add", "after.txt"]);
    run_git(&git_repo, ["commit", "--quiet", "-m", "after"]);
    copy_directory(&git_repo, &rit_repo);
    assert_eq!(
        run_capture("git", ["rebase", "topic"], &git_repo).exit_code,
        1
    );
    assert_eq!(
        run_capture(rit_binary(), ["rebase", "topic"], &rit_repo).exit_code,
        1
    );
    fs::write(git_repo.join("tracked.txt"), "resolved\n").expect("git resolved file should write");
    fs::write(rit_repo.join("tracked.txt"), "resolved\n").expect("rit resolved file should write");
    run_git(&git_repo, ["add", "tracked.txt"]);
    run_git(&rit_repo, ["add", "tracked.txt"]);
    let envs = [
        ("GIT_COMMITTER_DATE", "1700000000 +0900"),
        ("GIT_EDITOR", "true"),
    ];

    let git_continue = run_capture_with_env("git", ["rebase", "--continue"], &git_repo, &envs);
    let rit_continue =
        run_capture_with_env(rit_binary(), ["rebase", "--continue"], &rit_repo, &envs);

    assert_eq!(
        git_continue.exit_code, 0,
        "git stderr: {}",
        git_continue.stderr
    );
    assert_eq!(
        rit_continue.exit_code, 0,
        "rit stderr: {}",
        rit_continue.stderr
    );
    assert_eq!(git_continue.stdout, rit_continue.stdout);
    assert_eq!(git_continue.stderr, rit_continue.stderr);
    assert_eq!(
        run_capture("git", ["rev-parse", "HEAD"], &git_repo).stdout,
        run_capture(rit_binary(), ["rev-parse", "HEAD"], &rit_repo).stdout
    );
    assert_eq!(
        run_capture("git", ["rev-parse", "refs/heads/master"], &git_repo).stdout,
        run_capture(rit_binary(), ["rev-parse", "refs/heads/master"], &rit_repo).stdout
    );
    assert_eq!(
        run_capture("git", ["log", "--format=%P%n%B", "-2"], &git_repo).stdout,
        run_capture("git", ["log", "--format=%P%n%B", "-2"], &rit_repo).stdout
    );
    assert_eq!(
        run_capture("git", ["status", "--porcelain=v1"], &git_repo).stdout,
        run_capture(rit_binary(), ["status", "--porcelain=v1"], &rit_repo).stdout
    );
    assert_eq!(
        run_capture("git", ["ls-files"], &git_repo).stdout,
        run_capture(rit_binary(), ["ls-files"], &rit_repo).stdout
    );

    let _ = fs::remove_dir_all(root);
}

struct CapturedCommand {
    exit_code: i32,
    stdout: String,
    stderr: String,
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

fn create_conflicting_rebase_state(repo: &Path) {
    run_git(repo, ["checkout", "-b", "topic"]);
    fs::write(repo.join("tracked.txt"), "topic\n").expect("topic file should write");
    run_git(repo, ["commit", "--quiet", "-am", "topic"]);
    run_git(repo, ["checkout", "master"]);
    fs::write(repo.join("tracked.txt"), "master\n").expect("master file should write");
    run_git(repo, ["commit", "--quiet", "-am", "master"]);

    let rebase = run_capture("git", ["rebase", "topic"], repo);
    assert_eq!(rebase.exit_code, 1, "rebase should stop on conflict");
    assert!(
        repo.join(".git").join("rebase-merge").is_dir(),
        "rebase should record merge backend state"
    );
}

fn write_rebase_merge_state(repo: &Path) {
    let head = run_capture("git", ["rev-parse", "HEAD"], repo)
        .stdout
        .trim()
        .to_owned();
    let rebase_dir = repo.join(".git").join("rebase-merge");
    fs::create_dir_all(&rebase_dir).expect("rebase state directory should be created");
    fs::write(rebase_dir.join("head-name"), "refs/heads/master\n").expect("head-name");
    fs::write(rebase_dir.join("onto"), format!("{head}\n")).expect("onto");
    fs::write(rebase_dir.join("orig-head"), format!("{head}\n")).expect("orig-head");
    fs::write(rebase_dir.join("msgnum"), "1\n").expect("msgnum");
    fs::write(rebase_dir.join("end"), "1\n").expect("end");
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

fn run_capture_with_env<I, S>(
    program: impl AsRef<OsStr>,
    args: I,
    cwd: &Path,
    envs: &[(&str, &str)],
) -> CapturedCommand
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let output = Command::new(program)
        .args(args)
        .envs(envs.iter().copied())
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
    std::env::temp_dir().join(format!("rit-cli-compat-rebase-{name}-{unique}"))
}
