use std::ffi::{OsStr, OsString};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn clean_single_parent_cherry_pick_matches_git_state() {
    let root = temp_path("clean");
    let git_repo = root.join("git");
    let rit_repo = root.join("rit");
    setup_clean_cherry_pick(&git_repo);
    copy_directory(&git_repo, &rit_repo);

    let git_pick = run_capture("git", ["cherry-pick", "topic"], &git_repo);
    let rit_pick = run_capture(rit_binary(), ["cherry-pick", "topic"], &rit_repo);

    assert_eq!(git_pick.exit_code, 0, "git stderr: {}", git_pick.stderr);
    assert_eq!(rit_pick.exit_code, 0, "rit stderr: {}", rit_pick.stderr);
    assert_eq!(
        run_capture("git", ["status", "--porcelain=v1"], &git_repo).stdout,
        run_capture(rit_binary(), ["status", "--porcelain=v1"], &rit_repo).stdout
    );
    assert_eq!(
        run_capture(
            "git",
            ["show", "--pretty=format:", "--name-only", "HEAD"],
            &git_repo
        )
        .stdout,
        run_capture(
            "git",
            ["show", "--pretty=format:", "--name-only", "HEAD"],
            &rit_repo
        )
        .stdout
    );
    assert_eq!(
        run_capture(
            "git",
            [
                "show",
                "--pretty=format:%an <%ae>%n%s",
                "--no-patch",
                "HEAD"
            ],
            &git_repo,
        )
        .stdout,
        run_capture(
            "git",
            [
                "show",
                "--pretty=format:%an <%ae>%n%s",
                "--no-patch",
                "HEAD"
            ],
            &rit_repo,
        )
        .stdout
    );
    assert_eq!(
        run_capture(
            "git",
            ["rev-list", "--parents", "-n", "1", "HEAD"],
            &git_repo
        )
        .stdout
        .split_whitespace()
        .count(),
        2
    );
    assert_eq!(
        run_capture(
            "git",
            ["rev-list", "--parents", "-n", "1", "HEAD"],
            &rit_repo
        )
        .stdout
        .split_whitespace()
        .count(),
        2
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn clean_cherry_pick_x_appends_origin_like_git() {
    let root = temp_path("x-origin");
    let git_repo = root.join("git");
    let rit_repo = root.join("rit");
    setup_clean_cherry_pick(&git_repo);
    copy_directory(&git_repo, &rit_repo);

    let git_pick = run_capture("git", ["cherry-pick", "-x", "topic"], &git_repo);
    let rit_pick = run_capture(rit_binary(), ["cherry-pick", "-x", "topic"], &rit_repo);

    assert_eq!(git_pick.exit_code, 0, "git stderr: {}", git_pick.stderr);
    assert_eq!(rit_pick.exit_code, 0, "rit stderr: {}", rit_pick.stderr);
    assert_eq!(
        run_capture(
            "git",
            ["show", "--pretty=format:%B", "--no-patch", "HEAD"],
            &git_repo
        )
        .stdout,
        run_capture(
            "git",
            ["show", "--pretty=format:%B", "--no-patch", "HEAD"],
            &rit_repo
        )
        .stdout
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn clean_cherry_pick_signoff_appends_trailer_like_git() {
    let root = temp_path("signoff");
    let git_repo = root.join("git");
    let rit_repo = root.join("rit");
    setup_clean_cherry_pick(&git_repo);
    copy_directory(&git_repo, &rit_repo);

    let git_pick = run_capture("git", ["cherry-pick", "-s", "topic"], &git_repo);
    let rit_pick = run_capture(rit_binary(), ["cherry-pick", "-s", "topic"], &rit_repo);

    assert_eq!(git_pick.exit_code, 0, "git stderr: {}", git_pick.stderr);
    assert_eq!(rit_pick.exit_code, 0, "rit stderr: {}", rit_pick.stderr);
    assert_eq!(
        run_capture(
            "git",
            ["show", "--pretty=format:%B", "--no-patch", "HEAD"],
            &git_repo
        )
        .stdout,
        run_capture(
            "git",
            ["show", "--pretty=format:%B", "--no-patch", "HEAD"],
            &rit_repo
        )
        .stdout
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn clean_cherry_pick_ff_fast_forwards_like_git() {
    let root = temp_path("ff");
    let git_repo = root.join("git");
    let rit_repo = root.join("rit");
    setup_fast_forward_cherry_pick(&git_repo);
    copy_directory(&git_repo, &rit_repo);

    let topic_id = run_capture("git", ["rev-parse", "topic"], &git_repo).stdout;
    let git_pick = run_capture("git", ["cherry-pick", "--ff", "topic"], &git_repo);
    let rit_pick = run_capture(rit_binary(), ["cherry-pick", "--ff", "topic"], &rit_repo);

    assert_eq!(git_pick.exit_code, 0, "git stderr: {}", git_pick.stderr);
    assert_eq!(rit_pick.exit_code, 0, "rit stderr: {}", rit_pick.stderr);
    assert_eq!(git_pick.stdout, rit_pick.stdout);
    assert_eq!(
        run_capture("git", ["rev-parse", "HEAD"], &git_repo).stdout,
        topic_id
    );
    assert_eq!(
        run_capture("git", ["rev-parse", "HEAD"], &rit_repo).stdout,
        topic_id
    );
    assert_eq!(
        run_capture("git", ["status", "--porcelain=v1"], &git_repo).stdout,
        run_capture(rit_binary(), ["status", "--porcelain=v1"], &rit_repo).stdout
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn clean_no_commit_cherry_pick_matches_git_state() {
    let root = temp_path("no-commit");
    let git_repo = root.join("git");
    let rit_repo = root.join("rit");
    setup_clean_cherry_pick(&git_repo);
    copy_directory(&git_repo, &rit_repo);

    let original_head = run_capture("git", ["rev-parse", "HEAD"], &git_repo).stdout;
    let git_pick = run_capture("git", ["cherry-pick", "--no-commit", "topic"], &git_repo);
    let rit_pick = run_capture(
        rit_binary(),
        ["cherry-pick", "--no-commit", "topic"],
        &rit_repo,
    );

    assert_eq!(git_pick.exit_code, 0, "git stderr: {}", git_pick.stderr);
    assert_eq!(rit_pick.exit_code, 0, "rit stderr: {}", rit_pick.stderr);
    assert_eq!(git_pick.stdout, rit_pick.stdout);
    assert_eq!(
        run_capture("git", ["rev-parse", "HEAD"], &git_repo).stdout,
        original_head
    );
    assert_eq!(
        run_capture("git", ["rev-parse", "HEAD"], &rit_repo).stdout,
        original_head
    );
    assert_eq!(
        run_capture("git", ["status", "--porcelain=v1"], &git_repo).stdout,
        run_capture(rit_binary(), ["status", "--porcelain=v1"], &rit_repo).stdout
    );
    assert!(!git_repo.join(".git").join("CHERRY_PICK_HEAD").exists());
    assert!(!rit_repo.join(".git").join("CHERRY_PICK_HEAD").exists());

    let _ = fs::remove_dir_all(root);
}

#[test]
fn clean_multi_no_commit_cherry_pick_matches_git_state() {
    let root = temp_path("multi-no-commit");
    let git_repo = root.join("git");
    let rit_repo = root.join("rit");
    setup_multi_commit_cherry_pick(&git_repo);
    copy_directory(&git_repo, &rit_repo);

    let original_head = run_capture("git", ["rev-parse", "HEAD"], &git_repo).stdout;
    let git_pick = run_capture(
        "git",
        ["cherry-pick", "--no-commit", "pick-one", "pick-two"],
        &git_repo,
    );
    let rit_pick = run_capture(
        rit_binary(),
        ["cherry-pick", "--no-commit", "pick-one", "pick-two"],
        &rit_repo,
    );

    assert_eq!(git_pick.exit_code, 0, "git stderr: {}", git_pick.stderr);
    assert_eq!(rit_pick.exit_code, 0, "rit stderr: {}", rit_pick.stderr);
    assert_eq!(git_pick.stdout, rit_pick.stdout);
    assert_eq!(
        run_capture("git", ["rev-parse", "HEAD"], &git_repo).stdout,
        original_head
    );
    assert_eq!(
        run_capture("git", ["rev-parse", "HEAD"], &rit_repo).stdout,
        original_head
    );
    assert_eq!(
        run_capture("git", ["status", "--porcelain=v1"], &git_repo).stdout,
        run_capture(rit_binary(), ["status", "--porcelain=v1"], &rit_repo).stdout
    );
    assert_eq!(
        fs::read_to_string(git_repo.join("one.txt")).expect("git one should read"),
        fs::read_to_string(rit_repo.join("one.txt")).expect("rit one should read")
    );
    assert_eq!(
        fs::read_to_string(git_repo.join("two.txt")).expect("git two should read"),
        fs::read_to_string(rit_repo.join("two.txt")).expect("rit two should read")
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn clean_merge_commit_cherry_pick_with_mainline_matches_git_state() {
    let root = temp_path("mainline");
    let git_repo = root.join("git");
    let rit_repo = root.join("rit");
    setup_merge_commit_cherry_pick(&git_repo);
    copy_directory(&git_repo, &rit_repo);

    let git_pick = run_capture("git", ["cherry-pick", "-m", "1", "merge-topic"], &git_repo);
    let rit_pick = run_capture(
        rit_binary(),
        ["cherry-pick", "-m", "1", "merge-topic"],
        &rit_repo,
    );

    assert_eq!(git_pick.exit_code, 0, "git stderr: {}", git_pick.stderr);
    assert_eq!(rit_pick.exit_code, 0, "rit stderr: {}", rit_pick.stderr);
    assert_eq!(
        run_capture("git", ["status", "--porcelain=v1"], &git_repo).stdout,
        run_capture(rit_binary(), ["status", "--porcelain=v1"], &rit_repo).stdout
    );
    assert_eq!(
        run_capture(
            "git",
            ["show", "--pretty=format:", "--name-only", "HEAD"],
            &git_repo
        )
        .stdout,
        run_capture(
            "git",
            ["show", "--pretty=format:", "--name-only", "HEAD"],
            &rit_repo
        )
        .stdout
    );
    assert_eq!(
        run_capture(
            "git",
            [
                "show",
                "--pretty=format:%an <%ae>%n%s",
                "--no-patch",
                "HEAD"
            ],
            &git_repo,
        )
        .stdout,
        run_capture(
            "git",
            [
                "show",
                "--pretty=format:%an <%ae>%n%s",
                "--no-patch",
                "HEAD"
            ],
            &rit_repo,
        )
        .stdout
    );
    assert_eq!(
        run_capture(
            "git",
            ["rev-list", "--parents", "-n", "1", "HEAD"],
            &rit_repo
        )
        .stdout
        .split_whitespace()
        .count(),
        2
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn clean_multi_commit_cherry_pick_matches_git_state() {
    let root = temp_path("multi-clean");
    let git_repo = root.join("git");
    let rit_repo = root.join("rit");
    setup_multi_commit_cherry_pick(&git_repo);
    copy_directory(&git_repo, &rit_repo);

    let git_pick = run_capture("git", ["cherry-pick", "pick-one", "pick-two"], &git_repo);
    let rit_pick = run_capture(
        rit_binary(),
        ["cherry-pick", "pick-one", "pick-two"],
        &rit_repo,
    );

    assert_eq!(git_pick.exit_code, 0, "git stderr: {}", git_pick.stderr);
    assert_eq!(rit_pick.exit_code, 0, "rit stderr: {}", rit_pick.stderr);
    assert_eq!(
        run_capture("git", ["status", "--porcelain=v1"], &git_repo).stdout,
        run_capture(rit_binary(), ["status", "--porcelain=v1"], &rit_repo).stdout
    );
    assert_eq!(
        run_capture(
            "git",
            ["show", "--pretty=format:", "--name-only", "HEAD"],
            &git_repo
        )
        .stdout,
        run_capture(
            "git",
            ["show", "--pretty=format:", "--name-only", "HEAD"],
            &rit_repo
        )
        .stdout
    );
    assert_eq!(
        run_capture("git", ["log", "--pretty=format:%s", "-2"], &git_repo).stdout,
        run_capture("git", ["log", "--pretty=format:%s", "-2"], &rit_repo).stdout
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn multi_commit_cherry_pick_conflict_writes_sequencer_like_git() {
    let root = temp_path("multi-conflict-sequencer");
    let git_repo = root.join("git");
    let rit_repo = root.join("rit");
    setup_multi_commit_cherry_pick_with_second_conflict(&git_repo);
    copy_directory(&git_repo, &rit_repo);

    let original_head = run_capture("git", ["rev-parse", "HEAD"], &git_repo).stdout;
    let git_pick = run_capture("git", ["cherry-pick", "pick-one", "pick-two"], &git_repo);
    let rit_pick = run_capture(
        rit_binary(),
        ["cherry-pick", "pick-one", "pick-two"],
        &rit_repo,
    );

    assert_ne!(git_pick.exit_code, 0);
    assert_ne!(rit_pick.exit_code, 0);
    let git_current_head = run_capture("git", ["rev-parse", "HEAD"], &git_repo).stdout;
    let rit_current_head = run_capture("git", ["rev-parse", "HEAD"], &rit_repo).stdout;
    assert_eq!(
        read_optional_file(&git_repo.join(".git").join("sequencer").join("head")),
        Some(original_head.clone())
    );
    assert_eq!(
        read_optional_file(&rit_repo.join(".git").join("sequencer").join("head")),
        Some(original_head.clone())
    );
    assert_eq!(
        read_optional_file(&git_repo.join(".git").join("sequencer").join("abort-safety")),
        Some(git_current_head)
    );
    assert_eq!(
        read_optional_file(&rit_repo.join(".git").join("sequencer").join("abort-safety")),
        Some(rit_current_head)
    );
    assert_eq!(
        read_optional_file(&git_repo.join(".git").join("sequencer").join("todo")),
        read_optional_file(&rit_repo.join(".git").join("sequencer").join("todo"))
    );
    assert_eq!(
        read_optional_file(&git_repo.join(".git").join("CHERRY_PICK_HEAD")),
        read_optional_file(&rit_repo.join(".git").join("CHERRY_PICK_HEAD"))
    );
    assert_eq!(
        read_optional_file(&git_repo.join(".git").join("MERGE_MSG")),
        read_optional_file(&rit_repo.join(".git").join("MERGE_MSG"))
    );
    assert_eq!(
        run_capture("git", ["status", "--porcelain=v1"], &git_repo).stdout,
        run_capture(rit_binary(), ["status", "--porcelain=v1"], &rit_repo).stdout
    );
    assert_eq!(
        run_capture("git", ["ls-files", "--stage"], &git_repo).stdout,
        run_capture(rit_binary(), ["ls-files", "--stage"], &rit_repo).stdout
    );
    assert_eq!(
        fs::read_to_string(git_repo.join("conflict.txt")).expect("git conflict file should read"),
        fs::read_to_string(rit_repo.join("conflict.txt")).expect("rit conflict file should read")
    );

    let git_abort = run_capture("git", ["cherry-pick", "--abort"], &git_repo);
    let rit_abort = run_capture(rit_binary(), ["cherry-pick", "--abort"], &rit_repo);

    assert_eq!(git_abort.exit_code, 0, "git stderr: {}", git_abort.stderr);
    assert_eq!(rit_abort.exit_code, 0, "rit stderr: {}", rit_abort.stderr);
    assert_eq!(
        run_capture("git", ["rev-parse", "HEAD"], &git_repo).stdout,
        original_head
    );
    assert_eq!(
        run_capture("git", ["rev-parse", "HEAD"], &rit_repo).stdout,
        original_head
    );
    assert!(!git_repo.join(".git").join("sequencer").exists());
    assert!(!rit_repo.join(".git").join("sequencer").exists());
    assert_eq!(
        run_capture("git", ["status", "--porcelain=v1"], &git_repo).stdout,
        run_capture(rit_binary(), ["status", "--porcelain=v1"], &rit_repo).stdout
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn multi_commit_cherry_pick_continue_clears_final_sequencer_like_git() {
    let root = temp_path("multi-conflict-continue");
    let git_repo = root.join("git");
    let rit_repo = root.join("rit");
    setup_multi_commit_cherry_pick_with_second_conflict(&git_repo);
    copy_directory(&git_repo, &rit_repo);

    run_capture("git", ["cherry-pick", "pick-one", "pick-two"], &git_repo);
    run_capture(
        rit_binary(),
        ["cherry-pick", "pick-one", "pick-two"],
        &rit_repo,
    );
    fs::write(git_repo.join("conflict.txt"), "resolved\n").expect("git resolution should write");
    fs::write(rit_repo.join("conflict.txt"), "resolved\n").expect("rit resolution should write");
    run_git(&git_repo, ["add", "conflict.txt"]);
    run_git(&rit_repo, ["add", "conflict.txt"]);

    let git_continue = run_capture("git", ["cherry-pick", "--continue"], &git_repo);
    let rit_continue = run_capture(rit_binary(), ["cherry-pick", "--continue"], &rit_repo);

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
    assert!(!git_repo.join(".git").join("sequencer").exists());
    assert!(!rit_repo.join(".git").join("sequencer").exists());
    assert!(!git_repo.join(".git").join("CHERRY_PICK_HEAD").exists());
    assert!(!rit_repo.join(".git").join("CHERRY_PICK_HEAD").exists());
    assert_eq!(
        run_capture("git", ["status", "--porcelain=v1"], &git_repo).stdout,
        run_capture(rit_binary(), ["status", "--porcelain=v1"], &rit_repo).stdout
    );
    assert_eq!(
        run_capture("git", ["log", "--pretty=format:%s", "-3"], &git_repo).stdout,
        run_capture("git", ["log", "--pretty=format:%s", "-3"], &rit_repo).stdout
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn multi_commit_cherry_pick_continue_replays_remaining_clean_todo_like_git() {
    let root = temp_path("multi-conflict-continue-remaining");
    let git_repo = root.join("git");
    let rit_repo = root.join("rit");
    setup_multi_commit_cherry_pick_with_remaining_clean_todo(&git_repo);
    copy_directory(&git_repo, &rit_repo);

    run_capture(
        "git",
        ["cherry-pick", "pick-one", "pick-two", "pick-three"],
        &git_repo,
    );
    run_capture(
        rit_binary(),
        ["cherry-pick", "pick-one", "pick-two", "pick-three"],
        &rit_repo,
    );
    fs::write(git_repo.join("conflict.txt"), "resolved\n").expect("git resolution should write");
    fs::write(rit_repo.join("conflict.txt"), "resolved\n").expect("rit resolution should write");
    run_git(&git_repo, ["add", "conflict.txt"]);
    run_git(&rit_repo, ["add", "conflict.txt"]);

    let git_continue = run_capture("git", ["cherry-pick", "--continue"], &git_repo);
    let rit_continue = run_capture(rit_binary(), ["cherry-pick", "--continue"], &rit_repo);

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
    assert!(!git_repo.join(".git").join("sequencer").exists());
    assert!(!rit_repo.join(".git").join("sequencer").exists());
    assert_eq!(
        run_capture("git", ["status", "--porcelain=v1"], &git_repo).stdout,
        run_capture(rit_binary(), ["status", "--porcelain=v1"], &rit_repo).stdout
    );
    assert_eq!(
        run_capture("git", ["log", "--pretty=format:%s", "-4"], &git_repo).stdout,
        run_capture("git", ["log", "--pretty=format:%s", "-4"], &rit_repo).stdout
    );
    assert_eq!(
        fs::read_to_string(git_repo.join("tail.txt")).expect("git tail file should read"),
        fs::read_to_string(rit_repo.join("tail.txt")).expect("rit tail file should read")
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn multi_commit_cherry_pick_continue_stops_on_later_conflict_like_git() {
    let root = temp_path("multi-conflict-continue-later-conflict");
    let git_repo = root.join("git");
    let rit_repo = root.join("rit");
    setup_multi_commit_cherry_pick_with_later_conflict(&git_repo);
    copy_directory(&git_repo, &rit_repo);

    run_capture(
        "git",
        ["cherry-pick", "pick-one", "pick-two", "pick-three"],
        &git_repo,
    );
    run_capture(
        rit_binary(),
        ["cherry-pick", "pick-one", "pick-two", "pick-three"],
        &rit_repo,
    );
    fs::write(git_repo.join("two.txt"), "resolved two\n").expect("git resolution should write");
    fs::write(rit_repo.join("two.txt"), "resolved two\n").expect("rit resolution should write");
    run_git(&git_repo, ["add", "two.txt"]);
    run_git(&rit_repo, ["add", "two.txt"]);

    let git_continue = run_capture("git", ["cherry-pick", "--continue"], &git_repo);
    let rit_continue = run_capture(rit_binary(), ["cherry-pick", "--continue"], &rit_repo);

    assert_ne!(git_continue.exit_code, 0);
    assert_ne!(rit_continue.exit_code, 0);
    assert!(git_continue.stdout.contains("Auto-merging three.txt\n"));
    assert!(rit_continue.stdout.contains("Auto-merging three.txt\n"));
    assert!(
        git_continue
            .stdout
            .contains("CONFLICT (content): Merge conflict in three.txt\n")
    );
    assert!(
        rit_continue
            .stdout
            .contains("CONFLICT (content): Merge conflict in three.txt\n")
    );
    assert!(rit_continue.stderr.contains("error: could not apply "));
    assert!(rit_continue.stderr.contains("pick three"));
    assert!(
        !rit_continue
            .stderr
            .contains("cherry-pick continue stopped on a later conflict")
    );
    assert_eq!(
        read_optional_file(&git_repo.join(".git").join("sequencer").join("todo")),
        read_optional_file(&rit_repo.join(".git").join("sequencer").join("todo"))
    );
    assert_eq!(
        read_optional_file(&git_repo.join(".git").join("CHERRY_PICK_HEAD")),
        read_optional_file(&rit_repo.join(".git").join("CHERRY_PICK_HEAD"))
    );
    assert_eq!(
        read_optional_file(&git_repo.join(".git").join("MERGE_MSG")),
        read_optional_file(&rit_repo.join(".git").join("MERGE_MSG"))
    );
    assert_eq!(
        run_capture("git", ["status", "--porcelain=v1"], &git_repo).stdout,
        run_capture(rit_binary(), ["status", "--porcelain=v1"], &rit_repo).stdout
    );
    assert_eq!(
        run_capture("git", ["ls-files", "--stage"], &git_repo).stdout,
        run_capture(rit_binary(), ["ls-files", "--stage"], &rit_repo).stdout
    );
    assert_eq!(
        fs::read_to_string(git_repo.join("three.txt")).expect("git conflict file should read"),
        fs::read_to_string(rit_repo.join("three.txt")).expect("rit conflict file should read")
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn multi_commit_cherry_pick_skip_replays_remaining_clean_todo_like_git() {
    let root = temp_path("multi-conflict-skip-remaining");
    let git_repo = root.join("git");
    let rit_repo = root.join("rit");
    setup_multi_commit_cherry_pick_with_remaining_clean_todo(&git_repo);
    copy_directory(&git_repo, &rit_repo);

    run_capture(
        "git",
        ["cherry-pick", "pick-one", "pick-two", "pick-three"],
        &git_repo,
    );
    run_capture(
        rit_binary(),
        ["cherry-pick", "pick-one", "pick-two", "pick-three"],
        &rit_repo,
    );

    let git_skip = run_capture("git", ["cherry-pick", "--skip"], &git_repo);
    let rit_skip = run_capture(rit_binary(), ["cherry-pick", "--skip"], &rit_repo);

    assert_eq!(git_skip.exit_code, 0, "git stderr: {}", git_skip.stderr);
    assert_eq!(rit_skip.exit_code, 0, "rit stderr: {}", rit_skip.stderr);
    assert!(!git_repo.join(".git").join("sequencer").exists());
    assert!(!rit_repo.join(".git").join("sequencer").exists());
    assert_eq!(
        run_capture("git", ["status", "--porcelain=v1"], &git_repo).stdout,
        run_capture(rit_binary(), ["status", "--porcelain=v1"], &rit_repo).stdout
    );
    assert_eq!(
        run_capture("git", ["log", "--pretty=format:%s", "-4"], &git_repo).stdout,
        run_capture("git", ["log", "--pretty=format:%s", "-4"], &rit_repo).stdout
    );
    assert_eq!(
        fs::read_to_string(git_repo.join("tail.txt")).expect("git tail file should read"),
        fs::read_to_string(rit_repo.join("tail.txt")).expect("rit tail file should read")
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn conflicting_cherry_pick_writes_git_shaped_state_and_abort_restores_head() {
    let root = temp_path("conflict");
    let git_repo = root.join("git");
    let rit_repo = root.join("rit");
    setup_conflicting_cherry_pick(&git_repo);
    copy_directory(&git_repo, &rit_repo);

    let original_head = run_capture("git", ["rev-parse", "HEAD"], &git_repo).stdout;
    let git_pick = run_capture("git", ["cherry-pick", "topic"], &git_repo);
    let rit_pick = run_capture(rit_binary(), ["cherry-pick", "topic"], &rit_repo);

    assert_ne!(git_pick.exit_code, 0);
    assert_ne!(rit_pick.exit_code, 0);
    assert_eq!(
        run_capture("git", ["rev-parse", "HEAD"], &git_repo).stdout,
        original_head
    );
    assert_eq!(
        run_capture("git", ["rev-parse", "HEAD"], &rit_repo).stdout,
        original_head
    );
    assert!(git_repo.join(".git").join("CHERRY_PICK_HEAD").exists());
    assert!(rit_repo.join(".git").join("CHERRY_PICK_HEAD").exists());
    assert!(git_repo.join(".git").join("MERGE_MSG").exists());
    assert!(rit_repo.join(".git").join("MERGE_MSG").exists());
    assert_eq!(
        run_capture("git", ["status", "--porcelain=v1"], &git_repo).stdout,
        run_capture(rit_binary(), ["status", "--porcelain=v1"], &rit_repo).stdout
    );
    assert_eq!(
        run_capture("git", ["ls-files", "--stage"], &git_repo).stdout,
        run_capture(rit_binary(), ["ls-files", "--stage"], &rit_repo).stdout
    );
    assert_eq!(
        fs::read_to_string(git_repo.join("a.txt")).expect("git conflict file should read"),
        fs::read_to_string(rit_repo.join("a.txt")).expect("rit conflict file should read")
    );

    let git_abort = run_capture("git", ["cherry-pick", "--abort"], &git_repo);
    let rit_abort = run_capture(rit_binary(), ["cherry-pick", "--abort"], &rit_repo);

    assert_eq!(git_abort.exit_code, 0, "git stderr: {}", git_abort.stderr);
    assert_eq!(rit_abort.exit_code, 0, "rit stderr: {}", rit_abort.stderr);
    assert_eq!(
        run_capture("git", ["rev-parse", "HEAD"], &git_repo).stdout,
        original_head
    );
    assert_eq!(
        run_capture("git", ["rev-parse", "HEAD"], &rit_repo).stdout,
        original_head
    );
    assert_eq!(
        run_capture("git", ["status", "--porcelain=v1"], &git_repo).stdout,
        run_capture(rit_binary(), ["status", "--porcelain=v1"], &rit_repo).stdout
    );
    assert!(!git_repo.join(".git").join("CHERRY_PICK_HEAD").exists());
    assert!(!rit_repo.join(".git").join("CHERRY_PICK_HEAD").exists());

    let _ = fs::remove_dir_all(root);
}

#[test]
fn conflicting_cherry_pick_continue_commits_resolved_index() {
    let root = temp_path("continue");
    let git_repo = root.join("git");
    let rit_repo = root.join("rit");
    setup_conflicting_cherry_pick(&git_repo);
    copy_directory(&git_repo, &rit_repo);

    run_capture("git", ["cherry-pick", "topic"], &git_repo);
    run_capture(rit_binary(), ["cherry-pick", "topic"], &rit_repo);
    fs::write(git_repo.join("a.txt"), "resolved\n").expect("git resolution should write");
    fs::write(rit_repo.join("a.txt"), "resolved\n").expect("rit resolution should write");
    run_git(&git_repo, ["add", "a.txt"]);
    run_git(&rit_repo, ["add", "a.txt"]);

    let git_continue = run_capture("git", ["cherry-pick", "--continue"], &git_repo);
    let rit_continue = run_capture(rit_binary(), ["cherry-pick", "--continue"], &rit_repo);

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
    assert_eq!(
        run_capture("git", ["status", "--porcelain=v1"], &git_repo).stdout,
        run_capture(rit_binary(), ["status", "--porcelain=v1"], &rit_repo).stdout
    );
    assert_eq!(
        run_capture(
            "git",
            [
                "show",
                "--pretty=format:%an <%ae>%n%s",
                "--no-patch",
                "HEAD"
            ],
            &git_repo,
        )
        .stdout,
        run_capture(
            "git",
            [
                "show",
                "--pretty=format:%an <%ae>%n%s",
                "--no-patch",
                "HEAD"
            ],
            &rit_repo,
        )
        .stdout
    );
    assert!(!git_repo.join(".git").join("CHERRY_PICK_HEAD").exists());
    assert!(!rit_repo.join(".git").join("CHERRY_PICK_HEAD").exists());

    let _ = fs::remove_dir_all(root);
}

#[test]
fn conflicting_cherry_pick_quit_leaves_index_and_worktree() {
    let root = temp_path("quit");
    let git_repo = root.join("git");
    let rit_repo = root.join("rit");
    setup_conflicting_cherry_pick(&git_repo);
    copy_directory(&git_repo, &rit_repo);

    run_capture("git", ["cherry-pick", "topic"], &git_repo);
    run_capture(rit_binary(), ["cherry-pick", "topic"], &rit_repo);
    let git_index_before = run_capture("git", ["ls-files", "--stage"], &git_repo).stdout;
    let rit_index_before = run_capture(rit_binary(), ["ls-files", "--stage"], &rit_repo).stdout;
    let git_file_before = fs::read_to_string(git_repo.join("a.txt")).expect("git file should read");
    let rit_file_before = fs::read_to_string(rit_repo.join("a.txt")).expect("rit file should read");

    let git_quit = run_capture("git", ["cherry-pick", "--quit"], &git_repo);
    let rit_quit = run_capture(rit_binary(), ["cherry-pick", "--quit"], &rit_repo);

    assert_eq!(git_quit.exit_code, 0, "git stderr: {}", git_quit.stderr);
    assert_eq!(rit_quit.exit_code, 0, "rit stderr: {}", rit_quit.stderr);
    assert_eq!(
        run_capture("git", ["ls-files", "--stage"], &git_repo).stdout,
        git_index_before
    );
    assert_eq!(
        run_capture(rit_binary(), ["ls-files", "--stage"], &rit_repo).stdout,
        rit_index_before
    );
    assert_eq!(
        fs::read_to_string(git_repo.join("a.txt")).expect("git file should read"),
        git_file_before
    );
    assert_eq!(
        fs::read_to_string(rit_repo.join("a.txt")).expect("rit file should read"),
        rit_file_before
    );
    assert!(!git_repo.join(".git").join("CHERRY_PICK_HEAD").exists());
    assert!(!rit_repo.join(".git").join("CHERRY_PICK_HEAD").exists());

    let _ = fs::remove_dir_all(root);
}

#[test]
fn conflicting_cherry_pick_skip_restores_head_index_and_worktree() {
    let root = temp_path("skip");
    let git_repo = root.join("git");
    let rit_repo = root.join("rit");
    setup_conflicting_cherry_pick(&git_repo);
    copy_directory(&git_repo, &rit_repo);

    let original_head = run_capture("git", ["rev-parse", "HEAD"], &git_repo).stdout;
    run_capture("git", ["cherry-pick", "topic"], &git_repo);
    run_capture(rit_binary(), ["cherry-pick", "topic"], &rit_repo);

    let git_skip = run_capture("git", ["cherry-pick", "--skip"], &git_repo);
    let rit_skip = run_capture(rit_binary(), ["cherry-pick", "--skip"], &rit_repo);

    assert_eq!(git_skip.exit_code, 0, "git stderr: {}", git_skip.stderr);
    assert_eq!(rit_skip.exit_code, 0, "rit stderr: {}", rit_skip.stderr);
    assert_eq!(git_skip.stdout, rit_skip.stdout);
    assert_eq!(
        run_capture("git", ["rev-parse", "HEAD"], &git_repo).stdout,
        original_head
    );
    assert_eq!(
        run_capture("git", ["rev-parse", "HEAD"], &rit_repo).stdout,
        original_head
    );
    assert_eq!(
        run_capture("git", ["status", "--porcelain=v1"], &git_repo).stdout,
        run_capture(rit_binary(), ["status", "--porcelain=v1"], &rit_repo).stdout
    );
    assert!(!git_repo.join(".git").join("CHERRY_PICK_HEAD").exists());
    assert!(!rit_repo.join(".git").join("CHERRY_PICK_HEAD").exists());

    let _ = fs::remove_dir_all(root);
}

struct CapturedCommand {
    exit_code: i32,
    stdout: String,
    stderr: String,
}

fn setup_clean_cherry_pick(repo: &Path) {
    init_repo(repo);
    commit_text(repo, "base.txt", "base\n", "base");
    run_git(repo, ["checkout", "--quiet", "-b", "topic"]);
    commit_text(repo, "picked.txt", "picked\n", "pick me");
    run_git(repo, ["checkout", "--quiet", "master"]);
    commit_text(repo, "head.txt", "head\n", "head");
}

fn setup_fast_forward_cherry_pick(repo: &Path) {
    init_repo(repo);
    commit_text(repo, "base.txt", "base\n", "base");
    run_git(repo, ["checkout", "--quiet", "-b", "topic"]);
    commit_text(repo, "picked.txt", "picked\n", "picked");
    run_git(repo, ["checkout", "--quiet", "master"]);
}

fn setup_conflicting_cherry_pick(repo: &Path) {
    init_repo(repo);
    commit_text(repo, "a.txt", "base\n", "base");
    run_git(repo, ["checkout", "--quiet", "-b", "topic"]);
    commit_text(repo, "a.txt", "topic\n", "pick me");
    run_git(repo, ["checkout", "--quiet", "master"]);
    commit_text(repo, "a.txt", "head\n", "head");
}

fn setup_merge_commit_cherry_pick(repo: &Path) {
    init_repo(repo);
    commit_text(repo, "base.txt", "base\n", "base");
    run_git(repo, ["checkout", "--quiet", "-b", "side"]);
    commit_text(repo, "side.txt", "side\n", "side");
    run_git(repo, ["checkout", "--quiet", "master"]);
    commit_text(repo, "main.txt", "main\n", "main");
    run_git(
        repo,
        ["merge", "--no-ff", "--quiet", "side", "-m", "merge side"],
    );
    run_git(repo, ["branch", "merge-topic"]);
    run_git(repo, ["checkout", "--quiet", "-b", "replay", "HEAD~1"]);
}

fn setup_multi_commit_cherry_pick(repo: &Path) {
    init_repo(repo);
    commit_text(repo, "base.txt", "base\n", "base");
    run_git(repo, ["checkout", "--quiet", "-b", "topic"]);
    commit_text(repo, "one.txt", "one\n", "pick one");
    run_git(repo, ["branch", "pick-one"]);
    commit_text(repo, "two.txt", "two\n", "pick two");
    run_git(repo, ["branch", "pick-two"]);
    run_git(repo, ["checkout", "--quiet", "master"]);
    commit_text(repo, "head.txt", "head\n", "head");
}

fn setup_multi_commit_cherry_pick_with_second_conflict(repo: &Path) {
    init_repo(repo);
    commit_text(repo, "clean.txt", "base\n", "base clean");
    commit_text(repo, "conflict.txt", "base\n", "base conflict");
    run_git(repo, ["checkout", "--quiet", "-b", "topic"]);
    commit_text(repo, "clean.txt", "picked clean\n", "pick one");
    run_git(repo, ["branch", "pick-one"]);
    commit_text(repo, "conflict.txt", "picked conflict\n", "pick two");
    run_git(repo, ["branch", "pick-two"]);
    run_git(repo, ["checkout", "--quiet", "master"]);
    commit_text(repo, "conflict.txt", "head conflict\n", "head");
}

fn setup_multi_commit_cherry_pick_with_remaining_clean_todo(repo: &Path) {
    init_repo(repo);
    commit_text(repo, "clean.txt", "base\n", "base clean");
    commit_text(repo, "conflict.txt", "base\n", "base conflict");
    commit_text(repo, "tail.txt", "base\n", "base tail");
    run_git(repo, ["checkout", "--quiet", "-b", "topic"]);
    commit_text(repo, "clean.txt", "picked clean\n", "pick one");
    run_git(repo, ["branch", "pick-one"]);
    commit_text(repo, "conflict.txt", "picked conflict\n", "pick two");
    run_git(repo, ["branch", "pick-two"]);
    commit_text(repo, "tail.txt", "picked tail\n", "pick three");
    run_git(repo, ["branch", "pick-three"]);
    run_git(repo, ["checkout", "--quiet", "master"]);
    commit_text(repo, "conflict.txt", "head conflict\n", "head");
}

fn setup_multi_commit_cherry_pick_with_later_conflict(repo: &Path) {
    init_repo(repo);
    commit_text(repo, "one.txt", "base\n", "base one");
    commit_text(repo, "two.txt", "base\n", "base two");
    commit_text(repo, "three.txt", "base\n", "base three");
    run_git(repo, ["checkout", "--quiet", "-b", "topic"]);
    commit_text(repo, "one.txt", "topic one\n", "pick one");
    run_git(repo, ["branch", "pick-one"]);
    commit_text(repo, "two.txt", "topic two\n", "pick two");
    run_git(repo, ["branch", "pick-two"]);
    commit_text(repo, "three.txt", "topic three\n", "pick three");
    run_git(repo, ["branch", "pick-three"]);
    run_git(repo, ["checkout", "--quiet", "master"]);
    commit_text(repo, "two.txt", "head two\n", "head two");
    commit_text(repo, "three.txt", "head three\n", "head three");
}

fn init_repo(repo: &Path) {
    fs::create_dir_all(repo).expect("fixture repository should be created");
    run_git(repo, ["init", "--quiet"]);
    run_git(repo, ["config", "user.name", "Rit Test"]);
    run_git(repo, ["config", "user.email", "rit@example.test"]);
    run_git(repo, ["config", "core.autocrlf", "false"]);
}

fn read_optional_file(path: &Path) -> Option<String> {
    fs::read_to_string(path).ok()
}

fn commit_text(repo: &Path, path: &str, contents: &str, message: &str) {
    fs::write(repo.join(path), contents).expect("file contents should be written");
    run_git(repo, ["add", path]);
    run_git(repo, ["commit", "--quiet", "-m", message]);
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
    std::env::temp_dir().join(format!("rit-cli-compat-cherry-pick-{name}-{unique}"))
}
