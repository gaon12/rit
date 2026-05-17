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
    run_git(&git_repo, ["pack-refs", "--all"]);
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
    assert_eq!(
        packed_refs_contains(&git_repo, "refs/stash"),
        packed_refs_contains(&rit_repo, "refs/stash")
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
fn stash_push_pathspec_from_file_matches_git() {
    let root = temp_path("push-pathspec-file");
    let git_repo = root.join("git");
    let rit_repo = root.join("rit");
    init_repo(&git_repo);
    fs::write(repo_file(&git_repo, "other.txt"), "other base\n")
        .expect("second tracked file should write");
    run_git(&git_repo, ["add", "other.txt"]);
    run_git(&git_repo, ["commit", "--quiet", "-m", "other"]);
    fs::write(repo_file(&git_repo, "paths.txt"), "tracked.txt\n")
        .expect("git pathspec file should write");
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
        [
            "stash",
            "push",
            "-m",
            "file selected",
            "--pathspec-from-file=paths.txt",
        ],
        &git_repo,
    );
    let rit_push = run_capture(
        rit_binary(),
        [
            "stash",
            "push",
            "-m",
            "file selected",
            "--pathspec-from-file=paths.txt",
        ],
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
        run_capture("git", ["stash", "show", "--name-only"], &git_repo).stdout,
        run_capture(rit_binary(), ["stash", "show", "--name-only"], &rit_repo).stdout
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn stash_push_nul_pathspec_from_file_matches_git() {
    let root = temp_path("push-pathspec-nul");
    let git_repo = root.join("git");
    let rit_repo = root.join("rit");
    init_repo(&git_repo);
    fs::write(repo_file(&git_repo, "other.txt"), "other base\n")
        .expect("second tracked file should write");
    run_git(&git_repo, ["add", "other.txt"]);
    run_git(&git_repo, ["commit", "--quiet", "-m", "other"]);
    fs::write(repo_file(&git_repo, "paths.nul"), b"tracked.txt\0")
        .expect("git nul pathspec file should write");
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
        [
            "stash",
            "push",
            "-m",
            "nul selected",
            "--pathspec-from-file=paths.nul",
            "--pathspec-file-nul",
        ],
        &git_repo,
    );
    let rit_push = run_capture(
        rit_binary(),
        [
            "stash",
            "push",
            "-m",
            "nul selected",
            "--pathspec-from-file=paths.nul",
            "--pathspec-file-nul",
        ],
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
        run_capture("git", ["stash", "show", "--name-only"], &git_repo).stdout,
        run_capture(rit_binary(), ["stash", "show", "--name-only"], &rit_repo).stdout
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn stash_push_no_pathspec_file_nul_reverts_to_text_mode_like_git() {
    let root = temp_path("push-no-pathspec-file-nul");
    let git_repo = root.join("git");
    let rit_repo = root.join("rit");
    init_repo(&git_repo);
    fs::write(repo_file(&git_repo, "other.txt"), "other base\n")
        .expect("second tracked file should write");
    run_git(&git_repo, ["add", "other.txt"]);
    run_git(&git_repo, ["commit", "--quiet", "-m", "other"]);
    fs::write(repo_file(&git_repo, "paths.txt"), "tracked.txt\n")
        .expect("git pathspec file should write");
    copy_directory(&git_repo, &rit_repo);
    fs::write(repo_file(&git_repo, "tracked.txt"), "selected\n")
        .expect("git selected change should write");
    fs::write(repo_file(&rit_repo, "tracked.txt"), "selected\n")
        .expect("rit selected change should write");
    fs::write(repo_file(&git_repo, "other.txt"), "other change\n")
        .expect("git unselected change should write");
    fs::write(repo_file(&rit_repo, "other.txt"), "other change\n")
        .expect("rit unselected change should write");

    let args = [
        "stash",
        "push",
        "-m",
        "text selected",
        "--pathspec-from-file=paths.txt",
        "--pathspec-file-nul",
        "--no-pathspec-file-nul",
    ];
    let git_push = run_capture("git", args, &git_repo);
    let rit_push = run_capture(rit_binary(), args, &rit_repo);

    assert_eq!(git_push.exit_code, 0, "git stderr: {}", git_push.stderr);
    assert_eq!(rit_push.exit_code, 0, "rit stderr: {}", rit_push.stderr);
    assert_eq!(git_push.stdout, rit_push.stdout);
    assert_eq!(git_push.stderr, rit_push.stderr);
    assert_eq!(
        run_capture("git", ["status", "--porcelain=v1"], &git_repo).stdout,
        run_capture(rit_binary(), ["status", "--porcelain=v1"], &rit_repo).stdout
    );
    assert_eq!(
        run_capture("git", ["stash", "show", "--name-only"], &git_repo).stdout,
        run_capture(rit_binary(), ["stash", "show", "--name-only"], &rit_repo).stdout
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn stash_push_include_untracked_stores_third_parent_like_git() {
    let root = temp_path("push-include-untracked");
    let git_repo = root.join("git");
    let rit_repo = root.join("rit");
    init_repo(&git_repo);
    copy_directory(&git_repo, &rit_repo);
    fs::write(repo_file(&git_repo, "tracked.txt"), "tracked change\n")
        .expect("git tracked change should write");
    fs::write(repo_file(&rit_repo, "tracked.txt"), "tracked change\n")
        .expect("rit tracked change should write");
    fs::write(repo_file(&git_repo, "new.txt"), "new\n").expect("git untracked should write");
    fs::write(repo_file(&rit_repo, "new.txt"), "new\n").expect("rit untracked should write");
    fs::create_dir_all(repo_file(&git_repo, "dir")).expect("git untracked dir should create");
    fs::create_dir_all(repo_file(&rit_repo, "dir")).expect("rit untracked dir should create");
    fs::write(repo_file(&git_repo, "dir/nested.txt"), "nested\n")
        .expect("git nested untracked should write");
    fs::write(repo_file(&rit_repo, "dir/nested.txt"), "nested\n")
        .expect("rit nested untracked should write");

    let git_push = run_capture(
        "git",
        [
            "stash",
            "push",
            "--include-untracked",
            "-m",
            "with untracked",
        ],
        &git_repo,
    );
    let rit_push = run_capture(
        rit_binary(),
        [
            "stash",
            "push",
            "--include-untracked",
            "-m",
            "with untracked",
        ],
        &rit_repo,
    );

    assert_eq!(git_push.exit_code, 0, "git stderr: {}", git_push.stderr);
    assert_eq!(rit_push.exit_code, 0, "rit stderr: {}", rit_push.stderr);
    assert_eq!(git_push.stdout, rit_push.stdout);
    assert_eq!(git_push.stderr, rit_push.stderr);
    assert_eq!(
        run_capture("git", ["status", "--porcelain=v1", "-uall"], &git_repo).stdout,
        run_capture(
            rit_binary(),
            ["status", "--porcelain=v1", "-uall"],
            &rit_repo
        )
        .stdout
    );
    assert_eq!(
        run_capture("git", ["stash", "show", "--name-status"], &git_repo).stdout,
        run_capture(rit_binary(), ["stash", "show", "--name-status"], &rit_repo).stdout
    );
    assert_eq!(
        run_capture(
            "git",
            ["ls-tree", "-r", "--name-only", "refs/stash^3"],
            &git_repo
        )
        .stdout,
        run_capture(
            "git",
            ["ls-tree", "-r", "--name-only", "refs/stash^3"],
            &rit_repo
        )
        .stdout
    );
    assert_eq!(
        run_capture("git", ["stash", "list"], &git_repo).stdout,
        run_capture(rit_binary(), ["stash", "list"], &rit_repo).stdout
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn stash_push_all_includes_ignored_files_like_git() {
    let root = temp_path("push-all");
    let git_repo = root.join("git");
    let rit_repo = root.join("rit");
    init_repo(&git_repo);
    fs::write(repo_file(&git_repo, ".gitignore"), "ignored.txt\n").expect("gitignore should write");
    run_git(&git_repo, ["add", ".gitignore"]);
    run_git(&git_repo, ["commit", "--quiet", "-m", "ignore"]);
    copy_directory(&git_repo, &rit_repo);
    fs::write(repo_file(&git_repo, "new.txt"), "new\n").expect("git untracked should write");
    fs::write(repo_file(&rit_repo, "new.txt"), "new\n").expect("rit untracked should write");
    fs::write(repo_file(&git_repo, "ignored.txt"), "ignored\n").expect("git ignored should write");
    fs::write(repo_file(&rit_repo, "ignored.txt"), "ignored\n").expect("rit ignored should write");

    let git_push = run_capture(
        "git",
        ["stash", "push", "--all", "-m", "all files"],
        &git_repo,
    );
    let rit_push = run_capture(
        rit_binary(),
        ["stash", "push", "--all", "-m", "all files"],
        &rit_repo,
    );

    assert_eq!(git_push.exit_code, 0, "git stderr: {}", git_push.stderr);
    assert_eq!(rit_push.exit_code, 0, "rit stderr: {}", rit_push.stderr);
    assert_eq!(git_push.stdout, rit_push.stdout);
    assert_eq!(git_push.stderr, rit_push.stderr);
    assert_eq!(
        run_capture(
            "git",
            ["status", "--porcelain=v1", "-uall", "--ignored"],
            &git_repo
        )
        .stdout,
        run_capture(
            rit_binary(),
            ["status", "--porcelain=v1", "-uall", "--ignored"],
            &rit_repo
        )
        .stdout
    );
    assert_eq!(
        run_capture(
            "git",
            ["ls-tree", "-r", "--name-only", "refs/stash^3"],
            &git_repo
        )
        .stdout,
        run_capture(
            "git",
            ["ls-tree", "-r", "--name-only", "refs/stash^3"],
            &rit_repo
        )
        .stdout
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
fn stash_push_staged_stashes_only_staged_changes_like_git() {
    let root = temp_path("push-staged");
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
        ["stash", "push", "--staged", "-m", "staged only"],
        &git_repo,
    );
    let rit_push = run_capture(
        rit_binary(),
        ["stash", "push", "--staged", "-m", "staged only"],
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
fn stash_staged_same_path_cleanup_failure_still_stores_stash_like_git() {
    for (name, args) in [
        (
            "push-staged-same-path",
            vec!["stash", "push", "--staged", "-m", "staged same"],
        ),
        (
            "save-staged-same-path",
            vec!["stash", "save", "--staged", "staged same"],
        ),
    ] {
        let root = temp_path(name);
        let git_repo = root.join("git");
        let rit_repo = root.join("rit");
        init_repo(&git_repo);
        copy_directory(&git_repo, &rit_repo);
        fs::write(repo_file(&git_repo, "tracked.txt"), "staged\n")
            .expect("git staged change should write");
        fs::write(repo_file(&rit_repo, "tracked.txt"), "staged\n")
            .expect("rit staged change should write");
        run_git(&git_repo, ["add", "tracked.txt"]);
        run_git(&rit_repo, ["add", "tracked.txt"]);
        fs::write(repo_file(&git_repo, "tracked.txt"), "unstaged\n")
            .expect("git unstaged change should write");
        fs::write(repo_file(&rit_repo, "tracked.txt"), "unstaged\n")
            .expect("rit unstaged change should write");

        let git_push = run_capture("git", args.iter().copied(), &git_repo);
        let rit_push = run_capture(rit_binary(), args.iter().copied(), &rit_repo);

        assert_eq!(git_push.exit_code, 1, "git stderr: {}", git_push.stderr);
        assert_eq!(rit_push.exit_code, 1, "rit stderr: {}", rit_push.stderr);
        assert_eq!(git_push.stdout, rit_push.stdout, "args: {args:?}");
        assert!(git_push.stderr.contains("Cannot remove worktree changes"));
        assert!(rit_push.stderr.contains("Cannot remove worktree changes"));
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
fn stash_save_untracked_modes_match_git() {
    for (name, args, ignored_file) in [
        (
            "save-include-untracked",
            vec!["stash", "save", "--include-untracked", "legacy u"],
            false,
        ),
        (
            "save-all",
            vec!["stash", "save", "--all", "legacy all"],
            true,
        ),
    ] {
        let root = temp_path(name);
        let git_repo = root.join("git");
        let rit_repo = root.join("rit");
        init_repo(&git_repo);
        if ignored_file {
            fs::write(repo_file(&git_repo, ".gitignore"), "*.log\n")
                .expect("git ignore file should write");
            run_git(&git_repo, ["add", ".gitignore"]);
            run_git(&git_repo, ["commit", "--quiet", "-m", "ignore"]);
        }
        copy_directory(&git_repo, &rit_repo);
        fs::write(repo_file(&git_repo, "tracked.txt"), "tracked change\n")
            .expect("git tracked change should write");
        fs::write(repo_file(&rit_repo, "tracked.txt"), "tracked change\n")
            .expect("rit tracked change should write");
        let untracked_name = if ignored_file {
            "ignored.log"
        } else {
            "new.txt"
        };
        fs::write(repo_file(&git_repo, untracked_name), "new\n")
            .expect("git untracked should write");
        fs::write(repo_file(&rit_repo, untracked_name), "new\n")
            .expect("rit untracked should write");

        let git_save = run_capture("git", args.iter().copied(), &git_repo);
        let rit_save = run_capture(rit_binary(), args.iter().copied(), &rit_repo);

        assert_eq!(git_save.exit_code, 0, "git stderr: {}", git_save.stderr);
        assert_eq!(rit_save.exit_code, 0, "rit stderr: {}", rit_save.stderr);
        assert_eq!(git_save.stdout, rit_save.stdout, "args: {args:?}");
        assert_eq!(git_save.stderr, rit_save.stderr, "args: {args:?}");
        assert_eq!(
            run_capture("git", ["status", "--porcelain=v1", "--ignored"], &git_repo).stdout,
            run_capture(
                rit_binary(),
                ["status", "--porcelain=v1", "--ignored"],
                &rit_repo
            )
            .stdout
        );
        assert_eq!(
            run_capture("git", ["stash", "list"], &git_repo).stdout,
            run_capture(rit_binary(), ["stash", "list"], &rit_repo).stdout
        );
        assert_eq!(
            run_capture(
                "git",
                ["stash", "show", "--include-untracked", "--name-status"],
                &git_repo,
            )
            .stdout,
            run_capture(
                rit_binary(),
                ["stash", "show", "--include-untracked", "--name-status"],
                &rit_repo,
            )
            .stdout
        );

        let _ = fs::remove_dir_all(root);
    }
}

#[test]
fn stash_save_index_selection_modes_match_git() {
    let keep_root = temp_path("save-keep-index");
    let keep_git_repo = keep_root.join("git");
    let keep_rit_repo = keep_root.join("rit");
    init_repo(&keep_git_repo);
    copy_directory(&keep_git_repo, &keep_rit_repo);
    fs::write(repo_file(&keep_git_repo, "tracked.txt"), "staged\n")
        .expect("git staged change should write");
    fs::write(repo_file(&keep_rit_repo, "tracked.txt"), "staged\n")
        .expect("rit staged change should write");
    run_git(&keep_git_repo, ["add", "tracked.txt"]);
    run_git(&keep_rit_repo, ["add", "tracked.txt"]);
    fs::write(repo_file(&keep_git_repo, "tracked.txt"), "unstaged\n")
        .expect("git unstaged change should write");
    fs::write(repo_file(&keep_rit_repo, "tracked.txt"), "unstaged\n")
        .expect("rit unstaged change should write");

    let git_keep = run_capture(
        "git",
        ["stash", "save", "--keep-index", "keep msg"],
        &keep_git_repo,
    );
    let rit_keep = run_capture(
        rit_binary(),
        ["stash", "save", "--keep-index", "keep msg"],
        &keep_rit_repo,
    );
    assert_eq!(git_keep.exit_code, 0, "git stderr: {}", git_keep.stderr);
    assert_eq!(rit_keep.exit_code, 0, "rit stderr: {}", rit_keep.stderr);
    assert_eq!(git_keep.stdout, rit_keep.stdout);
    assert_eq!(git_keep.stderr, rit_keep.stderr);
    assert_eq!(
        run_capture("git", ["status", "--porcelain=v1"], &keep_git_repo).stdout,
        run_capture(rit_binary(), ["status", "--porcelain=v1"], &keep_rit_repo).stdout
    );
    assert_eq!(
        run_capture("git", ["stash", "show", "--name-status"], &keep_git_repo).stdout,
        run_capture(
            rit_binary(),
            ["stash", "show", "--name-status"],
            &keep_rit_repo
        )
        .stdout
    );

    let staged_root = temp_path("save-staged");
    let staged_git_repo = staged_root.join("git");
    let staged_rit_repo = staged_root.join("rit");
    init_repo(&staged_git_repo);
    fs::write(repo_file(&staged_git_repo, "other.txt"), "base\n")
        .expect("git other file should write");
    run_git(&staged_git_repo, ["add", "other.txt"]);
    run_git(&staged_git_repo, ["commit", "--quiet", "-m", "other"]);
    copy_directory(&staged_git_repo, &staged_rit_repo);
    fs::write(repo_file(&staged_git_repo, "tracked.txt"), "staged\n")
        .expect("git staged change should write");
    fs::write(repo_file(&staged_rit_repo, "tracked.txt"), "staged\n")
        .expect("rit staged change should write");
    run_git(&staged_git_repo, ["add", "tracked.txt"]);
    run_git(&staged_rit_repo, ["add", "tracked.txt"]);
    fs::write(repo_file(&staged_git_repo, "other.txt"), "unstaged\n")
        .expect("git unstaged change should write");
    fs::write(repo_file(&staged_rit_repo, "other.txt"), "unstaged\n")
        .expect("rit unstaged change should write");

    let git_staged = run_capture(
        "git",
        ["stash", "save", "--staged", "staged msg"],
        &staged_git_repo,
    );
    let rit_staged = run_capture(
        rit_binary(),
        ["stash", "save", "--staged", "staged msg"],
        &staged_rit_repo,
    );
    assert_eq!(git_staged.exit_code, 0, "git stderr: {}", git_staged.stderr);
    assert_eq!(rit_staged.exit_code, 0, "rit stderr: {}", rit_staged.stderr);
    assert_eq!(git_staged.stdout, rit_staged.stdout);
    assert_eq!(git_staged.stderr, rit_staged.stderr);
    assert_eq!(
        run_capture("git", ["status", "--porcelain=v1"], &staged_git_repo).stdout,
        run_capture(rit_binary(), ["status", "--porcelain=v1"], &staged_rit_repo).stdout
    );
    assert_eq!(
        run_capture("git", ["stash", "show", "--name-status"], &staged_git_repo).stdout,
        run_capture(
            rit_binary(),
            ["stash", "show", "--name-status"],
            &staged_rit_repo
        )
        .stdout
    );

    let _ = fs::remove_dir_all(keep_root);
    let _ = fs::remove_dir_all(staged_root);
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
fn stash_apply_quiet_restores_untracked_files_without_dropping() {
    let root = temp_path("apply-untracked");
    let git_repo = root.join("git");
    let rit_repo = root.join("rit");
    init_repo(&git_repo);
    copy_directory(&git_repo, &rit_repo);
    fs::write(repo_file(&git_repo, "tracked.txt"), "tracked change\n")
        .expect("git tracked change should write");
    fs::write(repo_file(&rit_repo, "tracked.txt"), "tracked change\n")
        .expect("rit tracked change should write");
    fs::write(repo_file(&git_repo, "new.txt"), "new\n").expect("git untracked should write");
    fs::write(repo_file(&rit_repo, "new.txt"), "new\n").expect("rit untracked should write");
    run_git(
        &git_repo,
        [
            "stash",
            "push",
            "--include-untracked",
            "-m",
            "with untracked",
        ],
    );
    let rit_push = run_capture(
        rit_binary(),
        [
            "stash",
            "push",
            "--include-untracked",
            "-m",
            "with untracked",
        ],
        &rit_repo,
    );
    assert_eq!(rit_push.exit_code, 0, "rit stderr: {}", rit_push.stderr);

    let git_apply = run_capture("git", ["stash", "apply", "-q"], &git_repo);
    let rit_apply = run_capture(rit_binary(), ["stash", "apply", "-q"], &rit_repo);

    assert_eq!(git_apply.exit_code, 0, "git stderr: {}", git_apply.stderr);
    assert_eq!(rit_apply.exit_code, 0, "rit stderr: {}", rit_apply.stderr);
    assert_eq!(git_apply.stdout, rit_apply.stdout);
    assert_eq!(git_apply.stderr, rit_apply.stderr);
    assert_eq!(
        run_capture("git", ["status", "--porcelain=v1", "-uall"], &git_repo).stdout,
        run_capture(
            rit_binary(),
            ["status", "--porcelain=v1", "-uall"],
            &rit_repo
        )
        .stdout
    );
    assert_eq!(
        fs::read_to_string(repo_file(&git_repo, "new.txt")).ok(),
        fs::read_to_string(repo_file(&rit_repo, "new.txt")).ok()
    );
    assert_eq!(
        run_capture("git", ["stash", "list"], &git_repo).stdout,
        run_capture(rit_binary(), ["stash", "list"], &rit_repo).stdout
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn stash_apply_and_pop_refuse_to_overwrite_untracked_files_like_git() {
    for subcommand in ["apply", "pop"] {
        let root = temp_path(&format!("{subcommand}-untracked-collision"));
        let git_repo = root.join("git");
        let rit_repo = root.join("rit");
        init_repo(&git_repo);
        copy_directory(&git_repo, &rit_repo);
        fs::write(repo_file(&git_repo, "new.txt"), "stashed\n")
            .expect("git untracked should write");
        fs::write(repo_file(&rit_repo, "new.txt"), "stashed\n")
            .expect("rit untracked should write");
        run_git(
            &git_repo,
            [
                "stash",
                "push",
                "--include-untracked",
                "-m",
                "with untracked",
            ],
        );
        let rit_push = run_capture(
            rit_binary(),
            [
                "stash",
                "push",
                "--include-untracked",
                "-m",
                "with untracked",
            ],
            &rit_repo,
        );
        assert_eq!(rit_push.exit_code, 0, "rit stderr: {}", rit_push.stderr);

        fs::write(repo_file(&git_repo, "new.txt"), "current\n")
            .expect("git collision should write");
        fs::write(repo_file(&rit_repo, "new.txt"), "current\n")
            .expect("rit collision should write");

        let git_apply = run_capture("git", ["stash", subcommand, "-q"], &git_repo);
        let rit_apply = run_capture(rit_binary(), ["stash", subcommand, "-q"], &rit_repo);

        assert_eq!(git_apply.exit_code, 1, "git stderr: {}", git_apply.stderr);
        assert_eq!(rit_apply.exit_code, 1, "rit stderr: {}", rit_apply.stderr);
        assert_eq!(
            git_apply.stdout, rit_apply.stdout,
            "subcommand: {subcommand}"
        );
        assert_eq!(
            git_apply.stderr, rit_apply.stderr,
            "subcommand: {subcommand}"
        );
        assert_eq!(
            fs::read_to_string(repo_file(&git_repo, "new.txt")).ok(),
            fs::read_to_string(repo_file(&rit_repo, "new.txt")).ok()
        );
        assert_eq!(
            run_capture("git", ["status", "--porcelain=v1", "-uall"], &git_repo).stdout,
            run_capture(
                rit_binary(),
                ["status", "--porcelain=v1", "-uall"],
                &rit_repo
            )
            .stdout
        );
        assert_eq!(
            run_capture("git", ["stash", "list"], &git_repo).stdout,
            run_capture(rit_binary(), ["stash", "list"], &rit_repo).stdout
        );

        let _ = fs::remove_dir_all(root);
    }
}

#[test]
fn stash_untracked_only_default_outputs_match_git() {
    for (name, args) in [
        ("apply-untracked-human", vec!["stash", "apply"]),
        ("pop-untracked-human", vec!["stash", "pop"]),
        ("branch-untracked-human", vec!["stash", "branch", "topic"]),
    ] {
        let root = temp_path(name);
        let git_repo = root.join("git");
        let rit_repo = root.join("rit");
        init_repo(&git_repo);
        fs::write(repo_file(&git_repo, "new.txt"), "new\n").expect("git untracked should write");
        run_git(
            &git_repo,
            [
                "stash",
                "push",
                "--include-untracked",
                "-m",
                "with untracked",
            ],
        );
        copy_directory(&git_repo, &rit_repo);

        let git_result = run_capture("git", args.iter().copied(), &git_repo);
        let rit_result = run_capture(rit_binary(), args.iter().copied(), &rit_repo);

        assert_eq!(git_result.exit_code, 0, "git stderr: {}", git_result.stderr);
        assert_eq!(rit_result.exit_code, 0, "rit stderr: {}", rit_result.stderr);
        assert_eq!(git_result.stdout, rit_result.stdout, "args: {args:?}");
        assert_eq!(git_result.stderr, rit_result.stderr, "args: {args:?}");
        assert_eq!(
            run_capture("git", ["status", "--porcelain=v1", "-uall"], &git_repo).stdout,
            run_capture(
                rit_binary(),
                ["status", "--porcelain=v1", "-uall"],
                &rit_repo
            )
            .stdout
        );
        assert_eq!(
            fs::read_to_string(repo_file(&git_repo, "new.txt")).ok(),
            fs::read_to_string(repo_file(&rit_repo, "new.txt")).ok()
        );
        assert_eq!(
            run_capture("git", ["stash", "list"], &git_repo).stdout,
            run_capture(rit_binary(), ["stash", "list"], &rit_repo).stdout
        );

        let _ = fs::remove_dir_all(root);
    }
}

#[test]
fn stash_apply_and_pop_index_restore_staged_state_like_git() {
    for (name, subcommand) in [("apply-index", "apply"), ("pop-index", "pop")] {
        let root = temp_path(name);
        let git_repo = root.join("git");
        let rit_repo = root.join("rit");
        init_repo(&git_repo);
        fs::write(repo_file(&git_repo, "tracked.txt"), "staged\n")
            .expect("git staged change should write");
        run_git(&git_repo, ["add", "tracked.txt"]);
        fs::write(repo_file(&git_repo, "tracked.txt"), "unstaged\n")
            .expect("git unstaged change should write");
        run_git(&git_repo, ["stash", "push", "-m", "indexed"]);
        copy_directory(&git_repo, &rit_repo);

        let git_apply = run_capture("git", ["stash", subcommand, "--index", "-q"], &git_repo);
        let rit_apply = run_capture(
            rit_binary(),
            ["stash", subcommand, "--index", "-q"],
            &rit_repo,
        );

        assert_eq!(git_apply.exit_code, 0, "git stderr: {}", git_apply.stderr);
        assert_eq!(rit_apply.exit_code, 0, "rit stderr: {}", rit_apply.stderr);
        assert_eq!(
            git_apply.stdout, rit_apply.stdout,
            "subcommand: {subcommand}"
        );
        assert_eq!(
            git_apply.stderr, rit_apply.stderr,
            "subcommand: {subcommand}"
        );
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
fn stash_pop_quiet_restores_untracked_files_and_drops_entry() {
    let root = temp_path("pop-untracked");
    let git_repo = root.join("git");
    let rit_repo = root.join("rit");
    init_repo(&git_repo);
    copy_directory(&git_repo, &rit_repo);
    fs::write(repo_file(&git_repo, "tracked.txt"), "tracked change\n")
        .expect("git tracked change should write");
    fs::write(repo_file(&rit_repo, "tracked.txt"), "tracked change\n")
        .expect("rit tracked change should write");
    fs::write(repo_file(&git_repo, "new.txt"), "new\n").expect("git untracked should write");
    fs::write(repo_file(&rit_repo, "new.txt"), "new\n").expect("rit untracked should write");
    fs::create_dir_all(repo_file(&git_repo, "nested")).expect("git nested dir should write");
    fs::create_dir_all(repo_file(&rit_repo, "nested")).expect("rit nested dir should write");
    fs::write(repo_file(&git_repo, "nested/first.txt"), "first\n")
        .expect("git nested untracked should write");
    fs::write(repo_file(&rit_repo, "nested/first.txt"), "first\n")
        .expect("rit nested untracked should write");
    run_git(
        &git_repo,
        [
            "stash",
            "push",
            "--include-untracked",
            "-m",
            "with untracked",
        ],
    );
    let rit_push = run_capture(
        rit_binary(),
        [
            "stash",
            "push",
            "--include-untracked",
            "-m",
            "with untracked",
        ],
        &rit_repo,
    );
    assert_eq!(rit_push.exit_code, 0, "rit stderr: {}", rit_push.stderr);

    let git_pop = run_capture("git", ["stash", "pop", "-q"], &git_repo);
    let rit_pop = run_capture(rit_binary(), ["stash", "pop", "-q"], &rit_repo);

    assert_eq!(git_pop.exit_code, 0, "git stderr: {}", git_pop.stderr);
    assert_eq!(rit_pop.exit_code, 0, "rit stderr: {}", rit_pop.stderr);
    assert_eq!(git_pop.stdout, rit_pop.stdout);
    assert_eq!(git_pop.stderr, rit_pop.stderr);
    assert_eq!(
        run_capture("git", ["status", "--porcelain=v1", "-uall"], &git_repo).stdout,
        run_capture(
            rit_binary(),
            ["status", "--porcelain=v1", "-uall"],
            &rit_repo
        )
        .stdout
    );
    assert_eq!(
        fs::read_to_string(repo_file(&git_repo, "new.txt")).ok(),
        fs::read_to_string(repo_file(&rit_repo, "new.txt")).ok()
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
        vec!["stash", "show", "--abbrev"],
        vec!["stash", "show", "--abbrev=12"],
        vec!["stash", "show", "--abbrev=1"],
        vec!["stash", "show", "--abbrev=foo"],
        vec!["stash", "show", "--patch", "--full-index"],
        vec!["stash", "show", "--patch", "--abbrev=12"],
        vec!["stash", "show", "--patch-with-stat"],
        vec!["stash", "show", "--patch-with-stat", "--abbrev=12"],
        vec!["stash", "show", "--patch-with-stat", "--full-index"],
        vec!["stash", "show", "--compact-summary"],
        vec!["stash", "show", "--compact-summary", "--patch"],
        vec!["stash", "show", "--patch", "--compact-summary"],
        vec!["stash", "show", "--raw"],
        vec!["stash", "show", "--raw", "--abbrev=12"],
        vec!["stash", "show", "--patch-with-raw"],
        vec!["stash", "show", "--patch-with-raw", "--abbrev=12"],
        vec!["stash", "show", "--raw", "--patch"],
        vec!["stash", "show", "--patch", "--raw"],
        vec!["stash", "show", "--summary"],
        vec!["stash", "show", "--abbrev=12", "--full-index"],
        vec!["stash", "show", "--stat", "--patch"],
        vec!["stash", "show", "--patch", "--stat"],
        vec!["stash", "show", "--patch", "stash@{1}"],
        vec!["stash", "show", "--stat", "stash@{1}"],
        vec!["stash", "show", "--shortstat"],
        vec!["stash", "show", "--name-only"],
        vec!["stash", "show", "-z", "--name-only"],
        vec!["stash", "show", "--name-status"],
        vec!["stash", "show", "-z", "--name-status"],
        vec!["stash", "show", "--numstat", "1"],
        vec!["stash", "show", "-z", "--numstat", "1"],
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
fn stash_show_include_untracked_summary_formats_match_git() {
    let root = temp_path("show-untracked");
    let git_repo = root.join("git");
    let rit_repo = root.join("rit");
    init_repo(&git_repo);
    copy_directory(&git_repo, &rit_repo);
    fs::write(repo_file(&git_repo, "tracked.txt"), "tracked change\n")
        .expect("git tracked change should write");
    fs::write(repo_file(&rit_repo, "tracked.txt"), "tracked change\n")
        .expect("rit tracked change should write");
    fs::write(repo_file(&git_repo, "new.txt"), "new\n").expect("git untracked should write");
    fs::write(repo_file(&rit_repo, "new.txt"), "new\n").expect("rit untracked should write");
    run_git(
        &git_repo,
        [
            "stash",
            "push",
            "--include-untracked",
            "-m",
            "with untracked",
        ],
    );
    let rit_push = run_capture(
        rit_binary(),
        [
            "stash",
            "push",
            "--include-untracked",
            "-m",
            "with untracked",
        ],
        &rit_repo,
    );
    assert_eq!(rit_push.exit_code, 0, "rit stderr: {}", rit_push.stderr);

    for args in [
        vec!["stash", "show", "--include-untracked", "--name-only"],
        vec!["stash", "show", "--include-untracked", "--name-status"],
        vec!["stash", "show", "--include-untracked", "--numstat"],
        vec!["stash", "show", "--include-untracked", "--raw"],
        vec!["stash", "show", "--include-untracked", "--summary"],
        vec!["stash", "show", "--include-untracked", "--shortstat"],
        vec!["stash", "show", "--include-untracked", "--patch-with-stat"],
        vec![
            "stash",
            "show",
            "--include-untracked",
            "--patch",
            "--abbrev=12",
        ],
        vec![
            "stash",
            "show",
            "--include-untracked",
            "--patch",
            "--full-index",
        ],
        vec!["stash", "show", "--include-untracked", "--patch"],
        vec!["stash", "show", "--only-untracked"],
        vec!["stash", "show", "--only-untracked", "--name-only"],
        vec!["stash", "show", "--only-untracked", "--name-status"],
        vec!["stash", "show", "--only-untracked", "--numstat"],
        vec!["stash", "show", "--only-untracked", "--raw"],
        vec!["stash", "show", "--only-untracked", "--summary"],
        vec!["stash", "show", "--only-untracked", "--shortstat"],
        vec!["stash", "show", "--only-untracked", "--patch-with-stat"],
        vec![
            "stash",
            "show",
            "--only-untracked",
            "--patch",
            "--abbrev=12",
        ],
        vec![
            "stash",
            "show",
            "--only-untracked",
            "--patch",
            "--full-index",
        ],
        vec!["stash", "show", "--only-untracked", "--patch"],
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
fn stash_show_extended_summary_matches_git() {
    let root = temp_path("show-summary");
    let git_repo = root.join("git");
    let rit_repo = root.join("rit");
    init_repo(&git_repo);
    fs::write(repo_file(&git_repo, "modified.txt"), "one\ntwo\nthree\n")
        .expect("modified base should write");
    fs::write(repo_file(&git_repo, "deleted.txt"), "gone\n").expect("deleted base should write");
    run_git(&git_repo, ["add", "modified.txt", "deleted.txt"]);
    run_git(&git_repo, ["commit", "-m", "base"]);
    fs::write(
        repo_file(&git_repo, "modified.txt"),
        "one\nchanged\nthree\n",
    )
    .expect("modified change should write");
    fs::remove_file(repo_file(&git_repo, "deleted.txt")).expect("deleted file should remove");
    fs::write(repo_file(&git_repo, "added.txt"), "added\n").expect("added file should write");
    run_git(&git_repo, ["add", "added.txt"]);
    fs::write(repo_file(&git_repo, "untracked.txt"), "new\n").expect("untracked file should write");
    run_git(
        &git_repo,
        ["stash", "push", "--include-untracked", "-m", "summary"],
    );
    copy_directory(&git_repo, &rit_repo);

    for args in [
        vec!["stash", "show", "--summary"],
        vec!["stash", "show", "--summary", "--patch"],
        vec!["stash", "show", "--patch", "--summary"],
        vec!["stash", "show", "--summary", "--stat"],
        vec!["stash", "show", "--stat", "--summary"],
        vec!["stash", "show", "--compact-summary"],
        vec!["stash", "show", "--compact-summary", "--patch"],
        vec!["stash", "show", "--patch", "--compact-summary"],
        vec!["stash", "show", "--compact-summary", "--summary"],
        vec!["stash", "show", "--summary", "--compact-summary"],
        vec!["stash", "show", "--diff-filter=A"],
        vec!["stash", "show", "--diff-filter=D"],
        vec!["stash", "show", "--diff-filter=M"],
        vec!["stash", "show", "--diff-filter=AD"],
        vec!["stash", "show", "--diff-filter=d"],
        vec!["stash", "show", "--diff-filter=ad"],
        vec!["stash", "show", "--diff-filter=AM", "--name-status"],
        vec!["stash", "show", "--diff-filter=A", "--patch"],
        vec!["stash", "show", "--diff-filter=D", "--summary"],
        vec!["stash", "show", "--diff-filter=A", "--compact-summary"],
        vec!["stash", "show", "--unified=0"],
        vec!["stash", "show", "-U0"],
        vec!["stash", "show", "--unified=1", "--patch-with-stat"],
        vec!["stash", "show", "--no-prefix"],
        vec!["stash", "show", "--patch-with-stat", "--no-prefix"],
        vec!["stash", "show", "--no-prefix", "--default-prefix"],
        vec![
            "stash",
            "show",
            "--output-indicator-new=>",
            "--output-indicator-old=<",
            "--output-indicator-context=.",
        ],
        vec!["stash", "show", "--output-indicator-new="],
        vec!["stash", "show", "--include-untracked", "--summary"],
        vec!["stash", "show", "--only-untracked", "--summary"],
        vec!["stash", "show", "--only-untracked", "--summary", "--patch"],
    ] {
        let git_show = run_capture("git", args.iter().copied(), &git_repo);
        let rit_show = run_capture(rit_binary(), args.iter().copied(), &rit_repo);

        assert_eq!(git_show.exit_code, 0, "git stderr: {}", git_show.stderr);
        assert_eq!(rit_show.exit_code, 0, "rit stderr: {}", rit_show.stderr);
        assert_eq!(git_show.stdout, rit_show.stdout, "args: {args:?}");
        assert_eq!(git_show.stderr, rit_show.stderr, "args: {args:?}");
    }

    let git_show = run_capture("git", ["stash", "show", "--diff-filter=Z"], &git_repo);
    let rit_show = run_capture(
        rit_binary(),
        ["stash", "show", "--diff-filter=Z"],
        &rit_repo,
    );
    assert_eq!(git_show.exit_code, rit_show.exit_code);
    assert_eq!(git_show.stdout, rit_show.stdout);
    assert_eq!(git_show.stderr, rit_show.stderr);

    for args in [
        ["stash", "show", "--unified=foo"],
        ["stash", "show", "-Ufoo"],
        ["stash", "show", "--output-indicator-new=XY"],
    ] {
        let git_show = run_capture("git", args, &git_repo);
        let rit_show = run_capture(rit_binary(), args, &rit_repo);
        assert_eq!(git_show.exit_code, rit_show.exit_code, "args: {args:?}");
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
fn stash_show_include_untracked_config_matches_git() {
    let root = temp_path("show-untracked-config");
    let git_repo = root.join("git");
    let rit_repo = root.join("rit");
    init_repo(&git_repo);
    copy_directory(&git_repo, &rit_repo);
    fs::write(repo_file(&git_repo, "tracked.txt"), "tracked change\n")
        .expect("git tracked change should write");
    fs::write(repo_file(&rit_repo, "tracked.txt"), "tracked change\n")
        .expect("rit tracked change should write");
    fs::write(repo_file(&git_repo, "new.txt"), "new\n").expect("git untracked should write");
    fs::write(repo_file(&rit_repo, "new.txt"), "new\n").expect("rit untracked should write");
    run_git(
        &git_repo,
        [
            "stash",
            "push",
            "--include-untracked",
            "-m",
            "with untracked",
        ],
    );
    let rit_push = run_capture(
        rit_binary(),
        [
            "stash",
            "push",
            "--include-untracked",
            "-m",
            "with untracked",
        ],
        &rit_repo,
    );
    assert_eq!(rit_push.exit_code, 0, "rit stderr: {}", rit_push.stderr);
    run_git(&git_repo, ["config", "stash.showIncludeUntracked", "true"]);
    run_git(&rit_repo, ["config", "stash.showIncludeUntracked", "true"]);

    for args in [
        vec!["stash", "show", "--name-only"],
        vec!["stash", "show", "--name-status"],
        vec!["stash", "show", "--no-include-untracked", "--name-only"],
        vec!["stash", "show", "--only-untracked", "--name-only"],
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
fn stash_show_quiet_exit_codes_match_git() {
    let root = temp_path("show-quiet");
    let tracked_git_repo = root.join("tracked-git");
    let tracked_rit_repo = root.join("tracked-rit");
    setup_stashes(&tracked_git_repo);
    copy_directory(&tracked_git_repo, &tracked_rit_repo);

    let git_tracked = run_capture("git", ["stash", "show", "--quiet"], &tracked_git_repo);
    let rit_tracked = run_capture(
        rit_binary(),
        ["stash", "show", "--quiet"],
        &tracked_rit_repo,
    );
    assert_eq!(git_tracked.exit_code, rit_tracked.exit_code);
    assert_eq!(git_tracked.stdout, rit_tracked.stdout);
    assert_eq!(git_tracked.stderr, rit_tracked.stderr);

    let untracked_git_repo = root.join("untracked-git");
    let untracked_rit_repo = root.join("untracked-rit");
    init_repo(&untracked_git_repo);
    fs::write(repo_file(&untracked_git_repo, "new.txt"), "new\n")
        .expect("git untracked should write");
    run_git(
        &untracked_git_repo,
        [
            "stash",
            "push",
            "--include-untracked",
            "-m",
            "with untracked",
        ],
    );
    copy_directory(&untracked_git_repo, &untracked_rit_repo);

    for args in [
        vec!["stash", "show", "--quiet"],
        vec!["stash", "show", "--include-untracked", "--quiet"],
        vec!["stash", "show", "--only-untracked", "--quiet"],
    ] {
        let git_show = run_capture("git", args.iter().copied(), &untracked_git_repo);
        let rit_show = run_capture(rit_binary(), args.iter().copied(), &untracked_rit_repo);

        assert_eq!(git_show.exit_code, rit_show.exit_code, "args: {args:?}");
        assert_eq!(git_show.stdout, rit_show.stdout, "args: {args:?}");
        assert_eq!(git_show.stderr, rit_show.stderr, "args: {args:?}");
    }

    let _ = fs::remove_dir_all(root);
}

#[test]
fn stash_show_exit_code_matches_git() {
    let root = temp_path("show-exit-code");
    let tracked_git_repo = root.join("tracked-git");
    let tracked_rit_repo = root.join("tracked-rit");
    setup_stashes(&tracked_git_repo);
    copy_directory(&tracked_git_repo, &tracked_rit_repo);

    for args in [
        vec!["stash", "show", "--exit-code"],
        vec!["stash", "show", "--exit-code", "--stat"],
        vec!["stash", "show", "--exit-code", "--compact-summary"],
        vec!["stash", "show", "--exit-code", "--shortstat"],
        vec!["stash", "show", "--exit-code", "--patch-with-stat"],
        vec!["stash", "show", "--exit-code", "--raw"],
        vec!["stash", "show", "--exit-code", "--patch-with-raw"],
        vec!["stash", "show", "--exit-code", "--summary"],
        vec!["stash", "show", "--exit-code", "--name-only"],
        vec!["stash", "show", "--exit-code", "--name-status"],
        vec!["stash", "show", "--exit-code", "--numstat"],
        vec!["stash", "show", "--exit-code", "--no-patch"],
    ] {
        let git_show = run_capture("git", args.iter().copied(), &tracked_git_repo);
        let rit_show = run_capture(rit_binary(), args.iter().copied(), &tracked_rit_repo);

        assert_eq!(git_show.exit_code, rit_show.exit_code, "args: {args:?}");
        assert_eq!(git_show.stdout, rit_show.stdout, "args: {args:?}");
        assert_eq!(git_show.stderr, rit_show.stderr, "args: {args:?}");
    }

    let untracked_git_repo = root.join("untracked-git");
    let untracked_rit_repo = root.join("untracked-rit");
    init_repo(&untracked_git_repo);
    fs::write(repo_file(&untracked_git_repo, "new.txt"), "new\n")
        .expect("git untracked should write");
    run_git(
        &untracked_git_repo,
        [
            "stash",
            "push",
            "--include-untracked",
            "-m",
            "with untracked",
        ],
    );
    copy_directory(&untracked_git_repo, &untracked_rit_repo);

    for args in [
        vec!["stash", "show", "--exit-code"],
        vec!["stash", "show", "--include-untracked", "--exit-code"],
        vec!["stash", "show", "--only-untracked", "--exit-code"],
    ] {
        let git_show = run_capture("git", args.iter().copied(), &untracked_git_repo);
        let rit_show = run_capture(rit_binary(), args.iter().copied(), &untracked_rit_repo);

        assert_eq!(git_show.exit_code, rit_show.exit_code, "args: {args:?}");
        assert_eq!(git_show.stdout, rit_show.stdout, "args: {args:?}");
        assert_eq!(git_show.stderr, rit_show.stderr, "args: {args:?}");
    }

    let _ = fs::remove_dir_all(root);
}

#[test]
fn stash_show_diff_passthrough_options_match_git() {
    let root = temp_path("show-diff-passthrough-options");
    let git_repo = root.join("git");
    let rit_repo = root.join("rit");
    setup_stashes(&git_repo);
    copy_directory(&git_repo, &rit_repo);

    for args in [
        vec!["stash", "show", "--no-ext-diff"],
        vec!["stash", "show", "--ext-diff"],
        vec!["stash", "show", "--no-color"],
        vec!["stash", "show", "--color=never"],
        vec!["stash", "show", "--color=auto"],
        vec!["stash", "show", "--patch", "--binary"],
        vec!["stash", "show", "--patch", "--no-renames"],
        vec!["stash", "show", "--patch", "--find-renames"],
        vec!["stash", "show", "--patch", "--find-renames=90%"],
        vec!["stash", "show", "--patch", "-M"],
        vec!["stash", "show", "--patch", "-M90%"],
        vec!["stash", "show", "--patch", "--find-copies"],
        vec!["stash", "show", "--patch", "--find-copies=90%"],
        vec!["stash", "show", "--patch", "-C"],
        vec!["stash", "show", "--patch", "-C90%"],
        vec!["stash", "show", "--patch", "--find-copies-harder"],
        vec!["stash", "show", "--patch", "-l0"],
        vec!["stash", "show", "--patch", "-l1"],
        vec!["stash", "show", "--patch", "--minimal"],
        vec!["stash", "show", "--patch", "--patience"],
        vec!["stash", "show", "--patch", "--histogram"],
        vec!["stash", "show", "--textconv"],
        vec!["stash", "show", "--no-textconv"],
        vec!["stash", "show", "--ignore-submodules"],
        vec!["stash", "show", "--ignore-submodules=all"],
        vec!["stash", "show", "--ignore-submodules=none"],
        vec!["stash", "show", "--ignore-submodules=dirty"],
        vec!["stash", "show", "--ignore-submodules=untracked"],
        vec!["stash", "show", "--full-index"],
        vec!["stash", "show", "--abbrev=12"],
        vec!["stash", "show", "--patch-with-raw", "--full-index"],
        vec!["stash", "show", "--text"],
        vec!["stash", "show", "-a", "--patch"],
        vec!["stash", "show", "--patch", "-U0", "--inter-hunk-context=1"],
        vec!["stash", "show", "--patch", "-U0", "--inter-hunk-context=1k"],
        vec!["stash", "show", "--no-color", "--stat"],
        vec!["stash", "show", "--color=never", "--patch"],
        vec!["stash", "show", "--no-ext-diff", "--name-status"],
    ] {
        let git_show = run_capture("git", args.iter().copied(), &git_repo);
        let rit_show = run_capture(rit_binary(), args.iter().copied(), &rit_repo);

        assert_eq!(git_show.exit_code, 0, "git stderr: {}", git_show.stderr);
        assert_eq!(rit_show.exit_code, 0, "rit stderr: {}", rit_show.stderr);
        assert_eq!(git_show.stdout, rit_show.stdout, "args: {args:?}");
        assert_eq!(git_show.stderr, rit_show.stderr, "args: {args:?}");
    }

    let git_show = run_capture(
        "git",
        ["stash", "show", "--ignore-submodules=bad"],
        &git_repo,
    );
    let rit_show = run_capture(
        rit_binary(),
        ["stash", "show", "--ignore-submodules=bad"],
        &rit_repo,
    );
    assert_eq!(git_show.exit_code, rit_show.exit_code);
    assert_eq!(git_show.stdout, rit_show.stdout);
    assert_eq!(git_show.stderr, rit_show.stderr);

    let _ = fs::remove_dir_all(root);
}

#[test]
fn stash_show_stat_and_patch_configs_match_git() {
    let root = temp_path("show-stat-patch-config");
    let source_repo = root.join("source");
    setup_stashes(&source_repo);

    type StashShowConfigCase<'a> = (&'a str, &'a [(&'a str, &'a str)], &'a [&'a [&'a str]]);
    let cases: &[StashShowConfigCase<'_>] = &[
        (
            "stat-false",
            &[("stash.showStat", "false")],
            &[&["stash", "show"]],
        ),
        (
            "patch-true",
            &[("stash.showPatch", "true")],
            &[
                &["stash", "show"],
                &["stash", "show", "--stat"],
                &["stash", "show", "--no-patch"],
                &["stash", "show", "--name-only"],
            ],
        ),
        (
            "stat-and-patch",
            &[("stash.showStat", "true"), ("stash.showPatch", "true")],
            &[&["stash", "show"]],
        ),
        (
            "patch-only",
            &[("stash.showStat", "false"), ("stash.showPatch", "true")],
            &[&["stash", "show"]],
        ),
    ];

    for (case_name, config_values, commands) in cases {
        let git_repo = root.join(format!("{case_name}-git"));
        let rit_repo = root.join(format!("{case_name}-rit"));
        copy_directory(&source_repo, &git_repo);
        copy_directory(&source_repo, &rit_repo);
        for (key, value) in *config_values {
            run_git(&git_repo, ["config", key, value]);
            run_git(&rit_repo, ["config", key, value]);
        }

        for command in *commands {
            let git_show = run_capture("git", command.iter().copied(), &git_repo);
            let rit_show = run_capture(rit_binary(), command.iter().copied(), &rit_repo);

            assert_eq!(git_show.exit_code, 0, "git stderr: {}", git_show.stderr);
            assert_eq!(rit_show.exit_code, 0, "rit stderr: {}", rit_show.stderr);
            assert_eq!(
                git_show.stdout, rit_show.stdout,
                "case: {case_name}, command: {command:?}"
            );
            assert_eq!(
                git_show.stderr, rit_show.stderr,
                "case: {case_name}, command: {command:?}"
            );
        }
    }

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

#[test]
fn stash_export_print_and_import_restores_list_like_git() {
    let root = temp_path("export-print-import");
    let git_repo = root.join("git");
    let rit_repo = root.join("rit");
    setup_stashes(&git_repo);
    copy_directory(&git_repo, &rit_repo);

    let git_export = run_capture("git", ["stash", "export", "--print"], &git_repo);
    let rit_export = run_capture(rit_binary(), ["stash", "export", "--print"], &rit_repo);

    assert_eq!(git_export.exit_code, 0, "git stderr: {}", git_export.stderr);
    assert_eq!(rit_export.exit_code, 0, "rit stderr: {}", rit_export.stderr);
    assert!(is_hex_object_id(git_export.stdout.trim()));
    assert!(is_hex_object_id(rit_export.stdout.trim()));
    assert_eq!(git_export.stderr, rit_export.stderr);

    let git_export_id = git_export.stdout.trim().to_owned();
    let rit_export_id = rit_export.stdout.trim().to_owned();
    run_git(&git_repo, ["stash", "clear"]);
    run_git(&rit_repo, ["stash", "clear"]);

    let git_import = run_capture("git", ["stash", "import", &git_export_id], &git_repo);
    let rit_import = run_capture(rit_binary(), ["stash", "import", &rit_export_id], &rit_repo);

    assert_eq!(git_import.exit_code, 0, "git stderr: {}", git_import.stderr);
    assert_eq!(rit_import.exit_code, 0, "rit stderr: {}", rit_import.stderr);
    assert_eq!(git_import.stdout, rit_import.stdout);
    assert_eq!(git_import.stderr, rit_import.stderr);
    assert_eq!(
        run_capture("git", ["stash", "list"], &git_repo).stdout,
        run_capture(rit_binary(), ["stash", "list"], &rit_repo).stdout
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn stash_export_to_ref_selected_entries_imports_in_argument_order_like_git() {
    let root = temp_path("export-to-ref-import");
    let git_repo = root.join("git");
    let rit_repo = root.join("rit");
    setup_stashes(&git_repo);
    fs::write(repo_file(&git_repo, "tracked.txt"), "third\n").expect("third change should write");
    run_git(&git_repo, ["stash", "push", "-m", "third stash"]);
    copy_directory(&git_repo, &rit_repo);

    let export_args = [
        "stash",
        "export",
        "--to-ref",
        "refs/rit-test/exported-stashes",
        "stash@{2}",
        "stash@{0}",
    ];
    let git_export = run_capture("git", export_args, &git_repo);
    let rit_export = run_capture(rit_binary(), export_args, &rit_repo);

    assert_eq!(git_export.exit_code, 0, "git stderr: {}", git_export.stderr);
    assert_eq!(rit_export.exit_code, 0, "rit stderr: {}", rit_export.stderr);
    assert_eq!(git_export.stdout, rit_export.stdout);
    assert_eq!(git_export.stderr, rit_export.stderr);
    assert!(
        read_optional_file(
            &git_repo
                .join(".git")
                .join("refs")
                .join("rit-test")
                .join("exported-stashes")
        )
        .is_some()
    );
    assert!(
        read_optional_file(
            &rit_repo
                .join(".git")
                .join("refs")
                .join("rit-test")
                .join("exported-stashes")
        )
        .is_some()
    );

    run_git(&git_repo, ["stash", "clear"]);
    run_git(&rit_repo, ["stash", "clear"]);

    let git_import = run_capture(
        "git",
        ["stash", "import", "refs/rit-test/exported-stashes"],
        &git_repo,
    );
    let rit_import = run_capture(
        rit_binary(),
        ["stash", "import", "refs/rit-test/exported-stashes"],
        &rit_repo,
    );

    assert_eq!(git_import.exit_code, 0, "git stderr: {}", git_import.stderr);
    assert_eq!(rit_import.exit_code, 0, "rit stderr: {}", rit_import.stderr);
    assert_eq!(git_import.stdout, rit_import.stdout);
    assert_eq!(git_import.stderr, rit_import.stderr);
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

fn packed_refs_contains(repo: &Path, ref_name: &str) -> bool {
    read_optional_file(&repo.join(".git").join("packed-refs"))
        .map(|contents| {
            contents
                .lines()
                .any(|line| line.split_whitespace().nth(1) == Some(ref_name))
        })
        .unwrap_or(false)
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
