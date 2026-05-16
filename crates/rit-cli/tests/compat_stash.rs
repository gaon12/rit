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

#[test]
fn stash_clear_removes_stash_list_like_git() {
    let root = temp_path("clear");
    let git_repo = root.join("git");
    let rit_repo = root.join("rit");
    setup_stashes(&git_repo);
    copy_directory(&git_repo, &rit_repo);

    let git_clear = run_capture("git", ["stash", "clear"], &git_repo);
    let rit_clear = run_capture(rit_binary(), ["stash", "clear"], &rit_repo);

    assert_eq!(git_clear.exit_code, 0, "git stderr: {}", git_clear.stderr);
    assert_eq!(rit_clear.exit_code, 0, "rit stderr: {}", rit_clear.stderr);
    assert_eq!(git_clear.stdout, rit_clear.stdout);
    assert_eq!(
        run_capture("git", ["stash", "list"], &git_repo).stdout,
        run_capture(rit_binary(), ["stash", "list"], &rit_repo).stdout
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn stash_push_tracked_change_matches_git_messages_and_cleans_tree() {
    let root = temp_path("push-tracked");
    let git_repo = root.join("git");
    let rit_repo = root.join("rit");
    init_repo(&git_repo);
    copy_directory(&git_repo, &rit_repo);
    fs::write(repo_file(&git_repo, "tracked.txt"), "pushed\n")
        .expect("git tracked change should write");
    fs::write(repo_file(&rit_repo, "tracked.txt"), "pushed\n")
        .expect("rit tracked change should write");

    let git_push = run_capture("git", ["stash", "push", "-m", "pushed msg"], &git_repo);
    let rit_push = run_capture(
        rit_binary(),
        ["stash", "push", "-m", "pushed msg"],
        &rit_repo,
    );

    assert_eq!(git_push.exit_code, 0, "git stderr: {}", git_push.stderr);
    assert_eq!(rit_push.exit_code, 0, "rit stderr: {}", rit_push.stderr);
    assert_eq!(git_push.stdout, rit_push.stdout);
    assert_eq!(git_push.stderr, rit_push.stderr);
    assert_eq!(
        run_capture("git", ["stash", "list"], &git_repo).stdout,
        run_capture(rit_binary(), ["stash", "list"], &rit_repo).stdout
    );
    assert_eq!(
        run_capture("git", ["status", "--porcelain=v1"], &git_repo).stdout,
        run_capture(rit_binary(), ["status", "--porcelain=v1"], &rit_repo).stdout
    );
    assert_eq!(
        run_capture("git", ["stash", "show", "--stat"], &git_repo).stdout,
        run_capture(rit_binary(), ["stash", "show", "--stat"], &rit_repo).stdout
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn stash_default_no_local_changes_matches_git() {
    let root = temp_path("push-clean");
    let git_repo = root.join("git");
    let rit_repo = root.join("rit");
    init_repo(&git_repo);
    copy_directory(&git_repo, &rit_repo);

    let git_push = run_capture("git", ["stash"], &git_repo);
    let rit_push = run_capture(rit_binary(), ["stash"], &rit_repo);

    assert_eq!(git_push.exit_code, 0, "git stderr: {}", git_push.stderr);
    assert_eq!(rit_push.exit_code, 0, "rit stderr: {}", rit_push.stderr);
    assert_eq!(git_push.stdout, rit_push.stdout);
    assert_eq!(git_push.stderr, rit_push.stderr);
    assert_eq!(
        run_capture("git", ["stash", "list"], &git_repo).stdout,
        run_capture(rit_binary(), ["stash", "list"], &rit_repo).stdout
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn stash_push_options_without_push_word_match_git() {
    let root = temp_path("push-implicit");
    let git_repo = root.join("git");
    let rit_repo = root.join("rit");
    init_repo(&git_repo);
    copy_directory(&git_repo, &rit_repo);
    fs::write(repo_file(&git_repo, "tracked.txt"), "implicit\n")
        .expect("git tracked change should write");
    fs::write(repo_file(&rit_repo, "tracked.txt"), "implicit\n")
        .expect("rit tracked change should write");

    let git_push = run_capture("git", ["stash", "-m", "implicit msg"], &git_repo);
    let rit_push = run_capture(rit_binary(), ["stash", "-m", "implicit msg"], &rit_repo);

    assert_eq!(git_push.exit_code, 0, "git stderr: {}", git_push.stderr);
    assert_eq!(rit_push.exit_code, 0, "rit stderr: {}", rit_push.stderr);
    assert_eq!(git_push.stdout, rit_push.stdout);
    assert_eq!(git_push.stderr, rit_push.stderr);
    assert_eq!(
        run_capture("git", ["stash", "list"], &git_repo).stdout,
        run_capture(rit_binary(), ["stash", "list"], &rit_repo).stdout
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn stash_push_pathspec_stashes_only_selected_tracked_change() {
    let root = temp_path("push-pathspec");
    let git_repo = root.join("git");
    let rit_repo = root.join("rit");
    init_repo(&git_repo);
    fs::write(repo_file(&git_repo, "other.txt"), "other base\n")
        .expect("second tracked file should write");
    run_git(&git_repo, ["add", "other.txt"]);
    run_git(&git_repo, ["commit", "--quiet", "-m", "other"]);
    copy_directory(&git_repo, &rit_repo);
    fs::write(repo_file(&git_repo, "tracked.txt"), "selected\n")
        .expect("git selected change should write");
    fs::write(repo_file(&rit_repo, "tracked.txt"), "selected\n")
        .expect("rit selected change should write");
    fs::write(repo_file(&git_repo, "other.txt"), "other change\n")
        .expect("git unselected change should write");
    fs::write(repo_file(&rit_repo, "other.txt"), "other change\n")
        .expect("rit unselected change should write");

    let git_push = run_capture(
        "git",
        ["stash", "push", "-m", "selected only", "--", "tracked.txt"],
        &git_repo,
    );
    let rit_push = run_capture(
        rit_binary(),
        ["stash", "push", "-m", "selected only", "--", "tracked.txt"],
        &rit_repo,
    );

    assert_eq!(git_push.exit_code, 0, "git stderr: {}", git_push.stderr);
    assert_eq!(rit_push.exit_code, 0, "rit stderr: {}", rit_push.stderr);
    assert_eq!(git_push.stdout, rit_push.stdout);
    assert_eq!(git_push.stderr, rit_push.stderr);
    assert_eq!(
        run_capture("git", ["status", "--porcelain=v1"], &git_repo).stdout,
        run_capture(rit_binary(), ["status", "--porcelain=v1"], &rit_repo).stdout
    );
    assert_eq!(
        fs::read_to_string(repo_file(&git_repo, "tracked.txt")).ok(),
        fs::read_to_string(repo_file(&rit_repo, "tracked.txt")).ok()
    );
    assert_eq!(
        fs::read_to_string(repo_file(&git_repo, "other.txt")).ok(),
        fs::read_to_string(repo_file(&rit_repo, "other.txt")).ok()
    );
    assert_eq!(
        run_capture("git", ["stash", "show", "--name-only"], &git_repo).stdout,
        run_capture(rit_binary(), ["stash", "show", "--name-only"], &rit_repo).stdout
    );
    assert_eq!(
        run_capture("git", ["stash", "list"], &git_repo).stdout,
        run_capture(rit_binary(), ["stash", "list"], &rit_repo).stdout
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn stash_push_keep_index_keeps_staged_state_like_git() {
    let root = temp_path("push-keep-index");
    let git_repo = root.join("git");
    let rit_repo = root.join("rit");
    init_repo(&git_repo);
    fs::write(repo_file(&git_repo, "other.txt"), "other base\n")
        .expect("second tracked file should write");
    run_git(&git_repo, ["add", "other.txt"]);
    run_git(&git_repo, ["commit", "--quiet", "-m", "other"]);
    copy_directory(&git_repo, &rit_repo);
    fs::write(repo_file(&git_repo, "tracked.txt"), "staged\n")
        .expect("git staged change should write");
    fs::write(repo_file(&rit_repo, "tracked.txt"), "staged\n")
        .expect("rit staged change should write");
    run_git(&git_repo, ["add", "tracked.txt"]);
    run_git(&rit_repo, ["add", "tracked.txt"]);
    fs::write(repo_file(&git_repo, "other.txt"), "unstaged\n")
        .expect("git unstaged change should write");
    fs::write(repo_file(&rit_repo, "other.txt"), "unstaged\n")
        .expect("rit unstaged change should write");

    let git_push = run_capture(
        "git",
        ["stash", "push", "--keep-index", "-m", "keep staged"],
        &git_repo,
    );
    let rit_push = run_capture(
        rit_binary(),
        ["stash", "push", "--keep-index", "-m", "keep staged"],
        &rit_repo,
    );

    assert_eq!(git_push.exit_code, 0, "git stderr: {}", git_push.stderr);
    assert_eq!(rit_push.exit_code, 0, "rit stderr: {}", rit_push.stderr);
    assert_eq!(git_push.stdout, rit_push.stdout);
    assert_eq!(git_push.stderr, rit_push.stderr);
    assert_eq!(
        run_capture("git", ["status", "--porcelain=v1"], &git_repo).stdout,
        run_capture(rit_binary(), ["status", "--porcelain=v1"], &rit_repo).stdout
    );
    assert_eq!(
        fs::read_to_string(repo_file(&git_repo, "tracked.txt")).ok(),
        fs::read_to_string(repo_file(&rit_repo, "tracked.txt")).ok()
    );
    assert_eq!(
        fs::read_to_string(repo_file(&git_repo, "other.txt")).ok(),
        fs::read_to_string(repo_file(&rit_repo, "other.txt")).ok()
    );
    assert_eq!(
        run_capture("git", ["stash", "show", "--name-status"], &git_repo).stdout,
        run_capture(rit_binary(), ["stash", "show", "--name-status"], &rit_repo).stdout
    );
    assert_eq!(
        run_capture("git", ["stash", "list"], &git_repo).stdout,
        run_capture(rit_binary(), ["stash", "list"], &rit_repo).stdout
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn stash_save_legacy_message_matches_git() {
    let root = temp_path("save-message");
    let git_repo = root.join("git");
    let rit_repo = root.join("rit");
    init_repo(&git_repo);
    copy_directory(&git_repo, &rit_repo);
    fs::write(repo_file(&git_repo, "tracked.txt"), "saved\n")
        .expect("git tracked change should write");
    fs::write(repo_file(&rit_repo, "tracked.txt"), "saved\n")
        .expect("rit tracked change should write");

    let git_save = run_capture("git", ["stash", "save", "legacy", "msg"], &git_repo);
    let rit_save = run_capture(rit_binary(), ["stash", "save", "legacy", "msg"], &rit_repo);

    assert_eq!(git_save.exit_code, 0, "git stderr: {}", git_save.stderr);
    assert_eq!(rit_save.exit_code, 0, "rit stderr: {}", rit_save.stderr);
    assert_eq!(git_save.stdout, rit_save.stdout);
    assert_eq!(git_save.stderr, rit_save.stderr);
    assert_eq!(
        run_capture("git", ["stash", "list"], &git_repo).stdout,
        run_capture(rit_binary(), ["stash", "list"], &rit_repo).stdout
    );
    assert_eq!(
        run_capture("git", ["status", "--porcelain=v1"], &git_repo).stdout,
        run_capture(rit_binary(), ["status", "--porcelain=v1"], &rit_repo).stdout
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn stash_save_quiet_and_clean_match_git() {
    let quiet_root = temp_path("save-quiet");
    let quiet_git_repo = quiet_root.join("git");
    let quiet_rit_repo = quiet_root.join("rit");
    init_repo(&quiet_git_repo);
    copy_directory(&quiet_git_repo, &quiet_rit_repo);
    fs::write(repo_file(&quiet_git_repo, "tracked.txt"), "quiet\n")
        .expect("git tracked change should write");
    fs::write(repo_file(&quiet_rit_repo, "tracked.txt"), "quiet\n")
        .expect("rit tracked change should write");

    let git_quiet = run_capture("git", ["stash", "save", "-q", "quiet msg"], &quiet_git_repo);
    let rit_quiet = run_capture(
        rit_binary(),
        ["stash", "save", "-q", "quiet msg"],
        &quiet_rit_repo,
    );

    assert_eq!(git_quiet.exit_code, 0, "git stderr: {}", git_quiet.stderr);
    assert_eq!(rit_quiet.exit_code, 0, "rit stderr: {}", rit_quiet.stderr);
    assert_eq!(git_quiet.stdout, rit_quiet.stdout);
    assert_eq!(git_quiet.stderr, rit_quiet.stderr);
    assert_eq!(
        run_capture("git", ["stash", "list"], &quiet_git_repo).stdout,
        run_capture(rit_binary(), ["stash", "list"], &quiet_rit_repo).stdout
    );

    let clean_root = temp_path("save-clean");
    let clean_git_repo = clean_root.join("git");
    let clean_rit_repo = clean_root.join("rit");
    init_repo(&clean_git_repo);
    copy_directory(&clean_git_repo, &clean_rit_repo);

    let git_clean = run_capture("git", ["stash", "save"], &clean_git_repo);
    let rit_clean = run_capture(rit_binary(), ["stash", "save"], &clean_rit_repo);

    assert_eq!(git_clean.exit_code, 0, "git stderr: {}", git_clean.stderr);
    assert_eq!(rit_clean.exit_code, 0, "rit stderr: {}", rit_clean.stderr);
    assert_eq!(git_clean.stdout, rit_clean.stdout);
    assert_eq!(git_clean.stderr, rit_clean.stderr);
    assert_eq!(
        run_capture("git", ["stash", "list"], &clean_git_repo).stdout,
        run_capture(rit_binary(), ["stash", "list"], &clean_rit_repo).stdout
    );

    let _ = fs::remove_dir_all(quiet_root);
    let _ = fs::remove_dir_all(clean_root);
}

#[test]
fn stash_create_tracked_change_matches_git_without_storing_or_cleaning() {
    let root = temp_path("create-tracked");
    let git_repo = root.join("git");
    let rit_repo = root.join("rit");
    init_repo(&git_repo);
    copy_directory(&git_repo, &rit_repo);
    fs::write(repo_file(&git_repo, "tracked.txt"), "created\n")
        .expect("git tracked change should write");
    fs::write(repo_file(&rit_repo, "tracked.txt"), "created\n")
        .expect("rit tracked change should write");

    let git_create = run_capture("git", ["stash", "create", "created msg"], &git_repo);
    let rit_create = run_capture(rit_binary(), ["stash", "create", "created msg"], &rit_repo);
    let git_id = git_create.stdout.trim();
    let rit_id = rit_create.stdout.trim();

    assert_eq!(git_create.exit_code, 0, "git stderr: {}", git_create.stderr);
    assert_eq!(rit_create.exit_code, 0, "rit stderr: {}", rit_create.stderr);
    assert!(is_hex_object_id(git_id), "git id: {git_id:?}");
    assert!(is_hex_object_id(rit_id), "rit id: {rit_id:?}");
    assert_eq!(git_create.stderr, rit_create.stderr);
    assert_eq!(
        run_capture("git", ["status", "--porcelain=v1"], &git_repo).stdout,
        run_capture(rit_binary(), ["status", "--porcelain=v1"], &rit_repo).stdout
    );
    assert_eq!(
        run_capture("git", ["stash", "list"], &git_repo).stdout,
        run_capture(rit_binary(), ["stash", "list"], &rit_repo).stdout
    );
    assert_eq!(
        read_optional_file(&git_repo.join(".git").join("refs").join("stash")),
        read_optional_file(&rit_repo.join(".git").join("refs").join("stash"))
    );

    let git_commit = run_capture("git", ["cat-file", "-p", git_id], &git_repo);
    let rit_commit = run_capture("git", ["cat-file", "-p", rit_id], &rit_repo);
    assert_eq!(git_commit.exit_code, 0, "git stderr: {}", git_commit.stderr);
    assert_eq!(rit_commit.exit_code, 0, "rit stderr: {}", rit_commit.stderr);
    assert_eq!(
        commit_tree_line(&git_commit.stdout),
        commit_tree_line(&rit_commit.stdout)
    );
    assert_eq!(
        first_parent_line(&git_commit.stdout),
        first_parent_line(&rit_commit.stdout)
    );
    assert_eq!(parent_count(&git_commit.stdout), 2);
    assert_eq!(parent_count(&rit_commit.stdout), 2);
    assert_eq!(
        commit_message_line(&git_commit.stdout),
        Some("On master: created msg")
    );
    assert_eq!(
        commit_message_line(&rit_commit.stdout),
        Some("On master: created msg")
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn stash_create_clean_tree_matches_git() {
    let root = temp_path("create-clean");
    let git_repo = root.join("git");
    let rit_repo = root.join("rit");
    init_repo(&git_repo);
    copy_directory(&git_repo, &rit_repo);

    let git_create = run_capture("git", ["stash", "create"], &git_repo);
    let rit_create = run_capture(rit_binary(), ["stash", "create"], &rit_repo);

    assert_eq!(git_create.exit_code, 0, "git stderr: {}", git_create.stderr);
    assert_eq!(rit_create.exit_code, 0, "rit stderr: {}", rit_create.stderr);
    assert_eq!(git_create.stdout, rit_create.stdout);
    assert_eq!(git_create.stderr, rit_create.stderr);
    assert_eq!(
        run_capture("git", ["stash", "list"], &git_repo).stdout,
        run_capture(rit_binary(), ["stash", "list"], &rit_repo).stdout
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn stash_apply_quiet_restores_tracked_change_without_dropping() {
    let root = temp_path("apply-default");
    let git_repo = root.join("git");
    let rit_repo = root.join("rit");
    setup_stashes(&git_repo);
    copy_directory(&git_repo, &rit_repo);

    let git_apply = run_capture("git", ["stash", "apply", "-q"], &git_repo);
    let rit_apply = run_capture(rit_binary(), ["stash", "apply", "-q"], &rit_repo);

    assert_eq!(git_apply.exit_code, 0, "git stderr: {}", git_apply.stderr);
    assert_eq!(rit_apply.exit_code, 0, "rit stderr: {}", rit_apply.stderr);
    assert_eq!(git_apply.stdout, rit_apply.stdout);
    assert_eq!(git_apply.stderr, rit_apply.stderr);
    assert_eq!(
        run_capture("git", ["status", "--porcelain=v1"], &git_repo).stdout,
        run_capture(rit_binary(), ["status", "--porcelain=v1"], &rit_repo).stdout
    );
    assert_eq!(
        fs::read_to_string(repo_file(&git_repo, "tracked.txt")).ok(),
        fs::read_to_string(repo_file(&rit_repo, "tracked.txt")).ok()
    );
    assert_eq!(
        run_capture("git", ["stash", "list"], &git_repo).stdout,
        run_capture(rit_binary(), ["stash", "list"], &rit_repo).stdout
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn stash_apply_quiet_older_entry_matches_git_state() {
    let root = temp_path("apply-older");
    let git_repo = root.join("git");
    let rit_repo = root.join("rit");
    setup_stashes(&git_repo);
    copy_directory(&git_repo, &rit_repo);

    let git_apply = run_capture("git", ["stash", "apply", "-q", "stash@{1}"], &git_repo);
    let rit_apply = run_capture(
        rit_binary(),
        ["stash", "apply", "-q", "stash@{1}"],
        &rit_repo,
    );

    assert_eq!(git_apply.exit_code, 0, "git stderr: {}", git_apply.stderr);
    assert_eq!(rit_apply.exit_code, 0, "rit stderr: {}", rit_apply.stderr);
    assert_eq!(git_apply.stdout, rit_apply.stdout);
    assert_eq!(git_apply.stderr, rit_apply.stderr);
    assert_eq!(
        run_capture("git", ["status", "--porcelain=v1"], &git_repo).stdout,
        run_capture(rit_binary(), ["status", "--porcelain=v1"], &rit_repo).stdout
    );
    assert_eq!(
        fs::read_to_string(repo_file(&git_repo, "tracked.txt")).ok(),
        fs::read_to_string(repo_file(&rit_repo, "tracked.txt")).ok()
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn stash_apply_default_prints_human_status_like_git() {
    let root = temp_path("apply-human");
    let git_repo = root.join("git");
    let rit_repo = root.join("rit");
    setup_stashes(&git_repo);
    copy_directory(&git_repo, &rit_repo);

    let git_apply = run_capture("git", ["stash", "apply"], &git_repo);
    let rit_apply = run_capture(rit_binary(), ["stash", "apply"], &rit_repo);

    assert_eq!(git_apply.exit_code, 0, "git stderr: {}", git_apply.stderr);
    assert_eq!(rit_apply.exit_code, 0, "rit stderr: {}", rit_apply.stderr);
    assert_eq!(git_apply.stdout, rit_apply.stdout);
    assert_eq!(git_apply.stderr, rit_apply.stderr);
    assert_eq!(
        run_capture("git", ["status", "--porcelain=v1"], &git_repo).stdout,
        run_capture(rit_binary(), ["status", "--porcelain=v1"], &rit_repo).stdout
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn stash_pop_quiet_restores_tracked_change_and_drops_entry() {
    let root = temp_path("pop-default");
    let git_repo = root.join("git");
    let rit_repo = root.join("rit");
    setup_stashes(&git_repo);
    copy_directory(&git_repo, &rit_repo);

    let git_pop = run_capture("git", ["stash", "pop", "-q"], &git_repo);
    let rit_pop = run_capture(rit_binary(), ["stash", "pop", "-q"], &rit_repo);

    assert_eq!(git_pop.exit_code, 0, "git stderr: {}", git_pop.stderr);
    assert_eq!(rit_pop.exit_code, 0, "rit stderr: {}", rit_pop.stderr);
    assert_eq!(git_pop.stdout, rit_pop.stdout);
    assert_eq!(git_pop.stderr, rit_pop.stderr);
    assert_eq!(
        run_capture("git", ["status", "--porcelain=v1"], &git_repo).stdout,
        run_capture(rit_binary(), ["status", "--porcelain=v1"], &rit_repo).stdout
    );
    assert_eq!(
        fs::read_to_string(repo_file(&git_repo, "tracked.txt")).ok(),
        fs::read_to_string(repo_file(&rit_repo, "tracked.txt")).ok()
    );
    assert_eq!(
        run_capture("git", ["stash", "list"], &git_repo).stdout,
        run_capture(rit_binary(), ["stash", "list"], &rit_repo).stdout
    );
    assert_eq!(
        read_optional_file(&git_repo.join(".git").join("refs").join("stash")),
        read_optional_file(&rit_repo.join(".git").join("refs").join("stash"))
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn stash_pop_default_prints_human_status_and_drop_like_git() {
    let root = temp_path("pop-human");
    let git_repo = root.join("git");
    let rit_repo = root.join("rit");
    setup_stashes(&git_repo);
    copy_directory(&git_repo, &rit_repo);

    let git_pop = run_capture("git", ["stash", "pop"], &git_repo);
    let rit_pop = run_capture(rit_binary(), ["stash", "pop"], &rit_repo);

    assert_eq!(git_pop.exit_code, 0, "git stderr: {}", git_pop.stderr);
    assert_eq!(rit_pop.exit_code, 0, "rit stderr: {}", rit_pop.stderr);
    assert_eq!(git_pop.stdout, rit_pop.stdout);
    assert_eq!(git_pop.stderr, rit_pop.stderr);
    assert_eq!(
        run_capture("git", ["status", "--porcelain=v1"], &git_repo).stdout,
        run_capture(rit_binary(), ["status", "--porcelain=v1"], &rit_repo).stdout
    );
    assert_eq!(
        run_capture("git", ["stash", "list"], &git_repo).stdout,
        run_capture(rit_binary(), ["stash", "list"], &rit_repo).stdout
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn stash_branch_default_matches_git_state_and_output() {
    let root = temp_path("branch-default");
    let git_repo = root.join("git");
    let rit_repo = root.join("rit");
    setup_stashes(&git_repo);
    copy_directory(&git_repo, &rit_repo);

    let git_branch = run_capture("git", ["stash", "branch", "topic"], &git_repo);
    let rit_branch = run_capture(rit_binary(), ["stash", "branch", "topic"], &rit_repo);

    assert_eq!(git_branch.exit_code, 0, "git stderr: {}", git_branch.stderr);
    assert_eq!(rit_branch.exit_code, 0, "rit stderr: {}", rit_branch.stderr);
    assert_eq!(git_branch.stdout, rit_branch.stdout);
    assert_eq!(git_branch.stderr, rit_branch.stderr);
    assert_eq!(
        run_capture("git", ["branch", "--show-current"], &git_repo).stdout,
        run_capture(rit_binary(), ["branch", "--show-current"], &rit_repo).stdout
    );
    assert_eq!(
        run_capture("git", ["status", "--porcelain=v1"], &git_repo).stdout,
        run_capture(rit_binary(), ["status", "--porcelain=v1"], &rit_repo).stdout
    );
    assert_eq!(
        run_capture("git", ["stash", "list"], &git_repo).stdout,
        run_capture(rit_binary(), ["stash", "list"], &rit_repo).stdout
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn stash_pop_quiet_older_entry_drops_selected_entry() {
    let root = temp_path("pop-older");
    let git_repo = root.join("git");
    let rit_repo = root.join("rit");
    setup_stashes(&git_repo);
    copy_directory(&git_repo, &rit_repo);

    let git_pop = run_capture("git", ["stash", "pop", "-q", "stash@{1}"], &git_repo);
    let rit_pop = run_capture(rit_binary(), ["stash", "pop", "-q", "stash@{1}"], &rit_repo);

    assert_eq!(git_pop.exit_code, 0, "git stderr: {}", git_pop.stderr);
    assert_eq!(rit_pop.exit_code, 0, "rit stderr: {}", rit_pop.stderr);
    assert_eq!(git_pop.stdout, rit_pop.stdout);
    assert_eq!(git_pop.stderr, rit_pop.stderr);
    assert_eq!(
        run_capture("git", ["status", "--porcelain=v1"], &git_repo).stdout,
        run_capture(rit_binary(), ["status", "--porcelain=v1"], &rit_repo).stdout
    );
    assert_eq!(
        fs::read_to_string(repo_file(&git_repo, "tracked.txt")).ok(),
        fs::read_to_string(repo_file(&rit_repo, "tracked.txt")).ok()
    );
    assert_eq!(
        run_capture("git", ["stash", "list"], &git_repo).stdout,
        run_capture(rit_binary(), ["stash", "list"], &rit_repo).stdout
    );
    assert_eq!(
        read_optional_file(&git_repo.join(".git").join("refs").join("stash")),
        read_optional_file(&rit_repo.join(".git").join("refs").join("stash"))
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn stash_show_summary_formats_match_git() {
    let root = temp_path("show");
    let git_repo = root.join("git");
    let rit_repo = root.join("rit");
    setup_stashes(&git_repo);
    copy_directory(&git_repo, &rit_repo);

    for args in [
        vec!["stash", "show"],
        vec!["stash", "show", "-p"],
        vec!["stash", "show", "--patch", "stash@{1}"],
        vec!["stash", "show", "--stat", "stash@{1}"],
        vec!["stash", "show", "--name-only"],
        vec!["stash", "show", "--name-status"],
        vec!["stash", "show", "--numstat", "1"],
    ] {
        let git_show = run_capture("git", args.iter().copied(), &git_repo);
        let rit_show = run_capture(rit_binary(), args.iter().copied(), &rit_repo);

        assert_eq!(git_show.exit_code, 0, "git stderr: {}", git_show.stderr);
        assert_eq!(rit_show.exit_code, 0, "rit stderr: {}", rit_show.stderr);
        assert_eq!(git_show.stdout, rit_show.stdout, "args: {args:?}");
        assert_eq!(git_show.stderr, rit_show.stderr, "args: {args:?}");
    }

    let _ = fs::remove_dir_all(root);
}

#[test]
fn stash_drop_default_matches_git() {
    let root = temp_path("drop-default");
    let git_repo = root.join("git");
    let rit_repo = root.join("rit");
    setup_stashes(&git_repo);
    copy_directory(&git_repo, &rit_repo);

    let git_drop = run_capture("git", ["stash", "drop"], &git_repo);
    let rit_drop = run_capture(rit_binary(), ["stash", "drop"], &rit_repo);

    assert_eq!(git_drop.exit_code, 0, "git stderr: {}", git_drop.stderr);
    assert_eq!(rit_drop.exit_code, 0, "rit stderr: {}", rit_drop.stderr);
    assert_eq!(git_drop.stdout, rit_drop.stdout);
    assert_eq!(
        run_capture("git", ["stash", "list"], &git_repo).stdout,
        run_capture(rit_binary(), ["stash", "list"], &rit_repo).stdout
    );
    assert_eq!(
        read_optional_file(&git_repo.join(".git").join("refs").join("stash")),
        read_optional_file(&rit_repo.join(".git").join("refs").join("stash"))
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn stash_drop_older_entry_relinks_reflog_like_git() {
    let root = temp_path("drop-older");
    let git_repo = root.join("git");
    let rit_repo = root.join("rit");
    setup_stashes(&git_repo);
    copy_directory(&git_repo, &rit_repo);

    let git_drop = run_capture("git", ["stash", "drop", "stash@{1}"], &git_repo);
    let rit_drop = run_capture(rit_binary(), ["stash", "drop", "stash@{1}"], &rit_repo);

    assert_eq!(git_drop.exit_code, 0, "git stderr: {}", git_drop.stderr);
    assert_eq!(rit_drop.exit_code, 0, "rit stderr: {}", rit_drop.stderr);
    assert_eq!(git_drop.stdout, rit_drop.stdout);
    assert_eq!(
        read_optional_file(&git_repo.join(".git").join("refs").join("stash")),
        read_optional_file(&rit_repo.join(".git").join("refs").join("stash"))
    );
    assert_eq!(
        read_optional_file(
            &git_repo
                .join(".git")
                .join("logs")
                .join("refs")
                .join("stash")
        ),
        read_optional_file(
            &rit_repo
                .join(".git")
                .join("logs")
                .join("refs")
                .join("stash")
        )
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn stash_drop_quiet_matches_git() {
    let root = temp_path("drop-quiet");
    let git_repo = root.join("git");
    let rit_repo = root.join("rit");
    setup_stashes(&git_repo);
    copy_directory(&git_repo, &rit_repo);

    let git_drop = run_capture("git", ["stash", "drop", "-q", "1"], &git_repo);
    let rit_drop = run_capture(rit_binary(), ["stash", "drop", "-q", "1"], &rit_repo);

    assert_eq!(git_drop.exit_code, 0, "git stderr: {}", git_drop.stderr);
    assert_eq!(rit_drop.exit_code, 0, "rit stderr: {}", rit_drop.stderr);
    assert_eq!(git_drop.stdout, rit_drop.stdout);
    assert_eq!(git_drop.stderr, rit_drop.stderr);
    assert_eq!(
        run_capture("git", ["stash", "list"], &git_repo).stdout,
        run_capture(rit_binary(), ["stash", "list"], &rit_repo).stdout
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn stash_drop_errors_match_git() {
    let root = temp_path("drop-errors");
    let empty_git_repo = root.join("empty-git");
    let empty_rit_repo = root.join("empty-rit");
    init_repo(&empty_git_repo);
    copy_directory(&empty_git_repo, &empty_rit_repo);

    let git_empty = run_capture("git", ["stash", "drop"], &empty_git_repo);
    let rit_empty = run_capture(rit_binary(), ["stash", "drop"], &empty_rit_repo);

    assert_eq!(git_empty.exit_code, rit_empty.exit_code);
    assert_eq!(git_empty.stdout, rit_empty.stdout);
    assert_eq!(git_empty.stderr, rit_empty.stderr);

    let git_repo = root.join("range-git");
    let rit_repo = root.join("range-rit");
    setup_stashes(&git_repo);
    copy_directory(&git_repo, &rit_repo);

    let git_range = run_capture("git", ["stash", "drop", "stash@{9}"], &git_repo);
    let rit_range = run_capture(rit_binary(), ["stash", "drop", "stash@{9}"], &rit_repo);

    assert_eq!(git_range.exit_code, rit_range.exit_code);
    assert_eq!(git_range.stdout, rit_range.stdout);
    assert_eq!(git_range.stderr, rit_range.stderr);

    let _ = fs::remove_dir_all(root);
}

#[test]
fn stash_store_existing_commit_matches_git_list_and_ref() {
    let root = temp_path("store");
    let git_repo = root.join("git");
    let rit_repo = root.join("rit");
    init_repo(&git_repo);
    fs::write(repo_file(&git_repo, "tracked.txt"), "stored\n")
        .expect("tracked change should write");
    let created = run_capture("git", ["stash", "create"], &git_repo);
    assert_eq!(created.exit_code, 0, "git stderr: {}", created.stderr);
    let stash_id = created.stdout.trim().to_owned();
    copy_directory(&git_repo, &rit_repo);

    let git_store = run_capture(
        "git",
        ["stash", "store", "-m", "stored msg", &stash_id],
        &git_repo,
    );
    let rit_store = run_capture(
        rit_binary(),
        ["stash", "store", "-m", "stored msg", &stash_id],
        &rit_repo,
    );

    assert_eq!(git_store.exit_code, 0, "git stderr: {}", git_store.stderr);
    assert_eq!(rit_store.exit_code, 0, "rit stderr: {}", rit_store.stderr);
    assert_eq!(git_store.stdout, rit_store.stdout);
    assert_eq!(git_store.stderr, rit_store.stderr);
    assert_eq!(
        run_capture("git", ["stash", "list"], &git_repo).stdout,
        run_capture(rit_binary(), ["stash", "list"], &rit_repo).stdout
    );
    assert_eq!(
        read_optional_file(&git_repo.join(".git").join("refs").join("stash")),
        read_optional_file(&rit_repo.join(".git").join("refs").join("stash"))
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn stash_store_default_message_and_quiet_match_git() {
    let root = temp_path("store-default");
    let git_repo = root.join("git");
    let rit_repo = root.join("rit");
    init_repo(&git_repo);
    fs::write(repo_file(&git_repo, "tracked.txt"), "stored\n")
        .expect("tracked change should write");
    let created = run_capture("git", ["stash", "create"], &git_repo);
    assert_eq!(created.exit_code, 0, "git stderr: {}", created.stderr);
    let stash_id = created.stdout.trim().to_owned();
    copy_directory(&git_repo, &rit_repo);

    let git_store = run_capture("git", ["stash", "store", "-q", &stash_id], &git_repo);
    let rit_store = run_capture(rit_binary(), ["stash", "store", "-q", &stash_id], &rit_repo);

    assert_eq!(git_store.exit_code, 0, "git stderr: {}", git_store.stderr);
    assert_eq!(rit_store.exit_code, 0, "rit stderr: {}", rit_store.stderr);
    assert_eq!(git_store.stdout, rit_store.stdout);
    assert_eq!(git_store.stderr, rit_store.stderr);
    assert_eq!(
        run_capture("git", ["stash", "list"], &git_repo).stdout,
        run_capture(rit_binary(), ["stash", "list"], &rit_repo).stdout
    );

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

fn repo_file(repo: &Path, name: &str) -> PathBuf {
    repo.join(name)
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

fn read_optional_file(path: &Path) -> Option<String> {
    fs::read_to_string(path).ok()
}

fn is_hex_object_id(value: &str) -> bool {
    value.len() == 40 && value.chars().all(|character| character.is_ascii_hexdigit())
}

fn commit_tree_line(commit: &str) -> Option<&str> {
    commit.lines().find(|line| line.starts_with("tree "))
}

fn first_parent_line(commit: &str) -> Option<&str> {
    commit.lines().find(|line| line.starts_with("parent "))
}

fn parent_count(commit: &str) -> usize {
    commit
        .lines()
        .filter(|line| line.starts_with("parent "))
        .count()
}

fn commit_message_line(commit: &str) -> Option<&str> {
    commit.lines().last()
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
