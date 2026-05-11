use rit_testkit::{LocalWriteFixture, LocalWriteFixtureKind};
use std::ffi::{OsStr, OsString};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn add_directory_pathspec_matches_git_status() {
    let fixture = LocalWriteFixture::new("add-directory", LocalWriteFixtureKind::NestedTracked)
        .expect("fixture should build");
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
fn add_wildcard_pathspec_matches_git_status() {
    let fixture = LocalWriteFixture::new("add-wildcard", LocalWriteFixtureKind::NestedTracked)
        .expect("fixture should build");
    fs::write(
        fixture.path().join("nested").join("tracked.txt"),
        "changed\n",
    )
    .expect("tracked file should be modified");
    fs::write(fixture.path().join("nested").join("new.txt"), "new\n")
        .expect("new file should be written");
    fs::write(fixture.path().join("nested").join("skip.md"), "skip\n")
        .expect("markdown file should be written");

    let outcome = compare_after_command(
        fixture.path(),
        command_words("git", ["add", "nested/[tn]*.txt"]),
        command_words(rit_binary(), ["add", "nested/[tn]*.txt"]),
    );

    assert_eq!(outcome.git_command_stdout, outcome.rit_command_stdout);
    assert_eq!(outcome.git_command_stderr, outcome.rit_command_stderr);
    assert_eq!(outcome.git_status, outcome.rit_status);
}

#[test]
fn add_magic_pathspec_matches_git_status() {
    let fixture = LocalWriteFixture::new("add-magic", LocalWriteFixtureKind::NestedTracked)
        .expect("fixture should build");
    fs::write(
        fixture.path().join("nested").join("tracked.txt"),
        "changed\n",
    )
    .expect("tracked file should be modified");
    fs::write(fixture.path().join("new.txt"), "new\n").expect("new file should be written");
    fs::write(
        fixture.path().join("nested").join("new.txt"),
        "nested new\n",
    )
    .expect("nested new file should be written");

    let outcome = compare_after_command(
        fixture.path(),
        command_words("git", ["add", ":(glob)*.txt"]),
        command_words(rit_binary(), ["add", ":(glob)*.txt"]),
    );

    assert_eq!(outcome.git_command_stdout, outcome.rit_command_stdout);
    assert_eq!(outcome.git_command_stderr, outcome.rit_command_stderr);
    assert_eq!(outcome.git_status, outcome.rit_status);
}

#[test]
fn add_exclude_magic_pathspec_matches_git_status() {
    let fixture = LocalWriteFixture::new("add-exclude-magic", LocalWriteFixtureKind::NestedTracked)
        .expect("fixture should build");
    fs::write(fixture.path().join("a.txt"), "root\n").expect("root file should be written");
    fs::write(
        fixture.path().join("nested").join("new.txt"),
        "nested new\n",
    )
    .expect("nested new file should be written");

    let outcome = compare_after_command(
        fixture.path(),
        command_words("git", ["add", "*.txt", ":!nested/new.txt"]),
        command_words(rit_binary(), ["add", "*.txt", ":!nested/new.txt"]),
    );

    assert_eq!(outcome.git_command_stdout, outcome.rit_command_stdout);
    assert_eq!(outcome.git_command_stderr, outcome.rit_command_stderr);
    assert_eq!(outcome.git_status, outcome.rit_status);
}

#[test]
fn add_icase_magic_pathspec_matches_git_status() {
    let fixture = LocalWriteFixture::new("add-icase-magic", LocalWriteFixtureKind::NestedTracked)
        .expect("fixture should build");
    fs::write(fixture.path().join("New.txt"), "new\n").expect("new file should be written");

    let outcome = compare_after_command(
        fixture.path(),
        command_words("git", ["add", ":(icase)new.txt"]),
        command_words(rit_binary(), ["add", ":(icase)new.txt"]),
    );

    assert_eq!(outcome.git_command_stdout, outcome.rit_command_stdout);
    assert_eq!(outcome.git_command_stderr, outcome.rit_command_stderr);
    assert_eq!(outcome.git_status, outcome.rit_status);
}

#[test]
fn add_attr_pathspec_matches_git_status() {
    for (name, pathspec) in [
        ("set", ":(attr:text)*"),
        ("unset", ":(attr:-text)*"),
        ("value", ":(attr:diff=markdown)*"),
        ("unspecified", ":(attr:!diff)*"),
    ] {
        let fixture = AttrPathspecWriteFixture::new(&format!("add-attr-{name}"));

        let outcome = compare_after_command(
            fixture.path(),
            command_words("git", ["add", pathspec]),
            command_words(rit_binary(), ["add", pathspec]),
        );

        assert_eq!(outcome.git_command_stdout, outcome.rit_command_stdout);
        assert_eq!(outcome.git_command_stderr, outcome.rit_command_stderr);
        assert_eq!(outcome.git_status, outcome.rit_status);
    }
}

#[test]
fn add_honors_core_ignorecase_for_mismatched_case_pathspec() {
    let fixture = temp_path("add-core-ignorecase-fixture");
    fs::create_dir_all(&fixture).expect("fixture should be created");
    run_git(&fixture, ["init", "--quiet"]);
    run_git(&fixture, ["config", "user.name", "Rit Test"]);
    run_git(&fixture, ["config", "user.email", "rit@example.test"]);
    run_git(&fixture, ["config", "core.autocrlf", "false"]);
    run_git(&fixture, ["config", "core.ignorecase", "true"]);
    fs::write(fixture.join("Camel.txt"), "base\n").expect("case file should be written");
    run_git(&fixture, ["add", "Camel.txt"]);
    run_git(&fixture, ["commit", "--quiet", "-m", "base"]);
    fs::write(fixture.join("Camel.txt"), "changed\n").expect("case file should be changed");

    let outcome = compare_after_command(
        &fixture,
        command_words("git", ["add", "camel.txt"]),
        command_words(rit_binary(), ["add", "camel.txt"]),
    );

    assert_eq!(outcome.git_command_stdout, outcome.rit_command_stdout);
    assert_eq!(outcome.git_command_stderr, outcome.rit_command_stderr);
    assert_eq!(outcome.git_status, outcome.rit_status);
    let _ = fs::remove_dir_all(fixture);
}

#[test]
fn add_chmod_executable_matches_git_status_and_tree_mode() {
    let fixture = LocalWriteFixture::new("add-chmod", LocalWriteFixtureKind::NestedTracked)
        .expect("fixture should build");
    let outcome = compare_after_command(
        fixture.path(),
        command_words("git", ["add", "--chmod=+x", "nested/tracked.txt"]),
        command_words(rit_binary(), ["add", "--chmod=+x", "nested/tracked.txt"]),
    );

    assert_eq!(outcome.git_command_stdout, outcome.rit_command_stdout);
    assert_eq!(outcome.git_command_stderr, outcome.rit_command_stderr);
    assert_eq!(outcome.git_status, outcome.rit_status);

    let env = [
        ("GIT_AUTHOR_DATE", "1700000000 +0900"),
        ("GIT_COMMITTER_DATE", "1700000000 +0900"),
    ];
    run_command(
        &command_words_with_env("git", ["commit", "-m", "mode"], &env),
        &outcome.git_repo,
    );
    run_command(
        &command_words_with_env(rit_binary(), ["commit", "-m", "mode"], &env),
        &outcome.rit_repo,
    );

    assert_eq!(
        run_capture(
            "git",
            ["ls-tree", "HEAD", "nested/tracked.txt"],
            &outcome.git_repo
        )
        .0,
        run_capture(
            "git",
            ["ls-tree", "HEAD", "nested/tracked.txt"],
            &outcome.rit_repo
        )
        .0
    );
}

#[test]
fn restore_directory_pathspec_matches_git_status_and_files() {
    let fixture = LocalWriteFixture::new("restore-directory", LocalWriteFixtureKind::NestedTracked)
        .expect("fixture should build");
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
fn restore_wildcard_pathspec_matches_git_status_and_files() {
    let fixture = LocalWriteFixture::new("restore-wildcard", LocalWriteFixtureKind::NestedTracked)
        .expect("fixture should build");
    fs::write(
        fixture.path().join("nested").join("tracked.txt"),
        "changed\n",
    )
    .expect("tracked file should be modified");

    let outcome = compare_after_command(
        fixture.path(),
        command_words("git", ["restore", "nested/[tn]*.txt"]),
        command_words(rit_binary(), ["restore", "nested/[tn]*.txt"]),
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
fn restore_magic_pathspec_matches_git_status_and_files() {
    let fixture = LocalWriteFixture::new("restore-magic", LocalWriteFixtureKind::NestedTracked)
        .expect("fixture should build");
    fs::write(
        fixture.path().join("nested").join("tracked.txt"),
        "changed\n",
    )
    .expect("tracked file should be modified");

    let outcome = compare_after_command(
        fixture.path(),
        command_words("git", ["restore", ":(top)nested/tracked.txt"]),
        command_words(rit_binary(), ["restore", ":(top)nested/tracked.txt"]),
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
    let fixture = LocalWriteFixture::new("reset-directory", LocalWriteFixtureKind::NestedTracked)
        .expect("fixture should build");
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
fn reset_wildcard_pathspec_matches_git_status() {
    let fixture = LocalWriteFixture::new("reset-wildcard", LocalWriteFixtureKind::NestedTracked)
        .expect("fixture should build");
    fs::write(
        fixture.path().join("nested").join("tracked.txt"),
        "changed\n",
    )
    .expect("tracked file should be modified");
    fs::write(fixture.path().join("nested").join("new.txt"), "new\n")
        .expect("new file should be written");
    run_git(fixture.path(), ["add", "nested"]);

    let outcome = compare_after_command(
        fixture.path(),
        command_words("git", ["reset", "nested/[tn]*.txt"]),
        command_words(rit_binary(), ["reset", "nested/[tn]*.txt"]),
    );

    assert_eq!(outcome.git_command_stdout, outcome.rit_command_stdout);
    assert_eq!(outcome.git_command_stderr, outcome.rit_command_stderr);
    assert_eq!(outcome.git_status, outcome.rit_status);
}

#[test]
fn reset_magic_pathspec_matches_git_status() {
    let fixture = LocalWriteFixture::new("reset-magic", LocalWriteFixtureKind::NestedTracked)
        .expect("fixture should build");
    fs::write(
        fixture.path().join("nested").join("tracked.txt"),
        "changed\n",
    )
    .expect("tracked file should be modified");
    run_git(fixture.path(), ["add", "nested"]);

    let outcome = compare_after_command(
        fixture.path(),
        command_words("git", ["reset", ":(top)nested/tracked.txt"]),
        command_words(rit_binary(), ["reset", ":(top)nested/tracked.txt"]),
    );

    assert_eq!(outcome.git_command_stdout, outcome.rit_command_stdout);
    assert_eq!(outcome.git_command_stderr, outcome.rit_command_stderr);
    assert_eq!(outcome.git_status, outcome.rit_status);
}

#[test]
fn commit_author_and_date_overrides_match_git_object() {
    let fixture =
        LocalWriteFixture::new("commit-author-date", LocalWriteFixtureKind::NestedTracked)
            .expect("fixture should build");
    let workspace = temp_path("commit-author-date-compare");
    let git_repo = workspace.join("git");
    let rit_repo = workspace.join("rit");
    copy_directory(fixture.path(), &git_repo);
    copy_directory(fixture.path(), &rit_repo);

    fs::write(git_repo.join("nested").join("tracked.txt"), "changed\n")
        .expect("git file should be changed");
    fs::write(rit_repo.join("nested").join("tracked.txt"), "changed\n")
        .expect("rit file should be changed");
    run_git(&git_repo, ["add", "nested/tracked.txt"]);
    run_git(&rit_repo, ["add", "nested/tracked.txt"]);

    let env = [
        ("GIT_COMMITTER_NAME", "C O Mitter"),
        ("GIT_COMMITTER_EMAIL", "c@example.test"),
        ("GIT_COMMITTER_DATE", "1700000001 +0900"),
    ];
    run_command(
        &command_words_with_env(
            "git",
            [
                "commit",
                "-m",
                "authored",
                "--author=A U Thor <a@example.test>",
                "--date=1700000000 +0900",
            ],
            &env,
        ),
        &git_repo,
    );
    run_command(
        &command_words_with_env(
            rit_binary(),
            [
                "commit",
                "-m",
                "authored",
                "--author=A U Thor <a@example.test>",
                "--date=1700000000 +0900",
            ],
            &env,
        ),
        &rit_repo,
    );

    assert_eq!(
        run_capture("git", ["rev-parse", "HEAD"], &git_repo).0,
        run_capture("git", ["rev-parse", "HEAD"], &rit_repo).0
    );
    assert_eq!(
        run_capture("git", ["cat-file", "-p", "HEAD"], &git_repo).0,
        run_capture("git", ["cat-file", "-p", "HEAD"], &rit_repo).0
    );
    let _ = fs::remove_dir_all(workspace);
}

#[test]
fn commit_msg_hook_modifies_message_like_git() {
    let fixture = LocalWriteFixture::new("commit-msg-hook", LocalWriteFixtureKind::NestedTracked)
        .expect("fixture should build");
    let workspace = temp_path("commit-msg-hook-compare");
    let git_repo = workspace.join("git");
    let rit_repo = workspace.join("rit");
    copy_directory(fixture.path(), &git_repo);
    copy_directory(fixture.path(), &rit_repo);
    write_hook(
        &git_repo,
        "commit-msg",
        "#!/bin/sh\nprintf '\\nHooked-by: test\\n' >> \"$1\"\n",
    );
    write_hook(
        &rit_repo,
        "commit-msg",
        "#!/bin/sh\nprintf '\\nHooked-by: test\\n' >> \"$1\"\n",
    );

    fs::write(git_repo.join("nested").join("tracked.txt"), "hooked\n")
        .expect("git file should be changed");
    fs::write(rit_repo.join("nested").join("tracked.txt"), "hooked\n")
        .expect("rit file should be changed");
    run_git(&git_repo, ["add", "nested/tracked.txt"]);
    run_git(&rit_repo, ["add", "nested/tracked.txt"]);

    let env = [
        ("GIT_AUTHOR_DATE", "1700000010 +0900"),
        ("GIT_COMMITTER_DATE", "1700000011 +0900"),
    ];
    run_command(
        &command_words_with_env("git", ["commit", "-m", "hooked"], &env),
        &git_repo,
    );
    run_command(
        &command_words_with_env(rit_binary(), ["commit", "-m", "hooked"], &env),
        &rit_repo,
    );

    assert_eq!(
        run_capture("git", ["cat-file", "-p", "HEAD"], &git_repo).0,
        run_capture("git", ["cat-file", "-p", "HEAD"], &rit_repo).0
    );
    let _ = fs::remove_dir_all(workspace);
}

#[test]
fn pre_commit_hook_blocks_commit_like_git() {
    let fixture = LocalWriteFixture::new("pre-commit-hook", LocalWriteFixtureKind::NestedTracked)
        .expect("fixture should build");
    let workspace = temp_path("pre-commit-hook-compare");
    let git_repo = workspace.join("git");
    let rit_repo = workspace.join("rit");
    copy_directory(fixture.path(), &git_repo);
    copy_directory(fixture.path(), &rit_repo);
    write_hook(
        &git_repo,
        "pre-commit",
        "#!/bin/sh\necho blocked >&2\nexit 1\n",
    );
    write_hook(
        &rit_repo,
        "pre-commit",
        "#!/bin/sh\necho blocked >&2\nexit 1\n",
    );

    fs::write(git_repo.join("nested").join("tracked.txt"), "blocked\n")
        .expect("git file should be changed");
    fs::write(rit_repo.join("nested").join("tracked.txt"), "blocked\n")
        .expect("rit file should be changed");
    run_git(&git_repo, ["add", "nested/tracked.txt"]);
    run_git(&rit_repo, ["add", "nested/tracked.txt"]);

    let env = [
        ("GIT_AUTHOR_DATE", "1700000020 +0900"),
        ("GIT_COMMITTER_DATE", "1700000021 +0900"),
    ];
    let git = run_command_allow_failure(
        &command_words_with_env("git", ["commit", "-m", "blocked"], &env),
        &git_repo,
    );
    let rit = run_command_allow_failure(
        &command_words_with_env(rit_binary(), ["commit", "-m", "blocked"], &env),
        &rit_repo,
    );

    assert!(!git.success);
    assert!(!rit.success);
    assert_eq!(
        run_capture("git", ["rev-parse", "HEAD"], &git_repo).0,
        run_capture("git", ["rev-parse", "HEAD"], &rit_repo).0
    );
    let _ = fs::remove_dir_all(workspace);
}

#[test]
fn no_verify_bypasses_commit_hooks_like_git() {
    let fixture = LocalWriteFixture::new("no-verify-hook", LocalWriteFixtureKind::NestedTracked)
        .expect("fixture should build");
    let workspace = temp_path("no-verify-hook-compare");
    let git_repo = workspace.join("git");
    let rit_repo = workspace.join("rit");
    copy_directory(fixture.path(), &git_repo);
    copy_directory(fixture.path(), &rit_repo);
    write_hook(
        &git_repo,
        "pre-commit",
        "#!/bin/sh\necho blocked >&2\nexit 1\n",
    );
    write_hook(
        &rit_repo,
        "pre-commit",
        "#!/bin/sh\necho blocked >&2\nexit 1\n",
    );

    fs::write(git_repo.join("nested").join("tracked.txt"), "allowed\n")
        .expect("git file should be changed");
    fs::write(rit_repo.join("nested").join("tracked.txt"), "allowed\n")
        .expect("rit file should be changed");
    run_git(&git_repo, ["add", "nested/tracked.txt"]);
    run_git(&rit_repo, ["add", "nested/tracked.txt"]);

    let env = [
        ("GIT_AUTHOR_DATE", "1700000030 +0900"),
        ("GIT_COMMITTER_DATE", "1700000031 +0900"),
    ];
    run_command(
        &command_words_with_env("git", ["commit", "--no-verify", "-m", "allowed"], &env),
        &git_repo,
    );
    run_command(
        &command_words_with_env(
            rit_binary(),
            ["commit", "--no-verify", "-m", "allowed"],
            &env,
        ),
        &rit_repo,
    );

    assert_eq!(
        run_capture("git", ["cat-file", "-p", "HEAD"], &git_repo).0,
        run_capture("git", ["cat-file", "-p", "HEAD"], &rit_repo).0
    );
    let _ = fs::remove_dir_all(workspace);
}

#[test]
fn post_commit_hook_runs_after_success_like_git() {
    let fixture = LocalWriteFixture::new("post-commit-hook", LocalWriteFixtureKind::NestedTracked)
        .expect("fixture should build");
    let workspace = temp_path("post-commit-hook-compare");
    let git_repo = workspace.join("git");
    let rit_repo = workspace.join("rit");
    copy_directory(fixture.path(), &git_repo);
    copy_directory(fixture.path(), &rit_repo);
    write_hook(
        &git_repo,
        "post-commit",
        "#!/bin/sh\nprintf done > post.txt\n",
    );
    write_hook(
        &rit_repo,
        "post-commit",
        "#!/bin/sh\nprintf done > post.txt\n",
    );

    fs::write(git_repo.join("nested").join("tracked.txt"), "post\n")
        .expect("git file should be changed");
    fs::write(rit_repo.join("nested").join("tracked.txt"), "post\n")
        .expect("rit file should be changed");
    run_git(&git_repo, ["add", "nested/tracked.txt"]);
    run_git(&rit_repo, ["add", "nested/tracked.txt"]);

    let env = [
        ("GIT_AUTHOR_DATE", "1700000040 +0900"),
        ("GIT_COMMITTER_DATE", "1700000041 +0900"),
    ];
    run_command(
        &command_words_with_env("git", ["commit", "-m", "post"], &env),
        &git_repo,
    );
    run_command(
        &command_words_with_env(rit_binary(), ["commit", "-m", "post"], &env),
        &rit_repo,
    );

    assert_eq!(
        fs::read_to_string(git_repo.join("post.txt")).expect("git post marker should read"),
        fs::read_to_string(rit_repo.join("post.txt")).expect("rit post marker should read")
    );
    let _ = fs::remove_dir_all(workspace);
}

#[test]
fn checkout_detached_commit_matches_git_state() {
    let fixture =
        LocalWriteFixture::new("detached-checkout", LocalWriteFixtureKind::DetachedCheckout)
            .expect("fixture should build");
    let base = fixture
        .base_commit()
        .expect("detached checkout fixture should expose base commit")
        .to_owned();

    let outcome = compare_after_command(
        fixture.path(),
        CommandSpec {
            program: OsString::from("git"),
            args: vec![OsString::from("checkout"), OsString::from(base.clone())],
            env: Vec::new(),
        },
        CommandSpec {
            program: rit_binary(),
            args: vec![OsString::from("checkout"), OsString::from(base)],
            env: Vec::new(),
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
    let fixture = LocalWriteFixture::new(
        "branch-delete-unmerged",
        LocalWriteFixtureKind::UnmergedBranch,
    )
    .expect("fixture should build");
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
    let fixture =
        LocalWriteFixture::new("branch-delete-merged", LocalWriteFixtureKind::MergedBranch)
            .expect("fixture should build");
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

#[test]
fn clone_local_no_checkout_copies_head_objects_and_refs() {
    let workspace = temp_path("clone-local-no-checkout");
    let source = workspace.join("source");
    let git_target = workspace.join("git-target");
    let rit_target = workspace.join("rit-target");
    fs::create_dir_all(&source).expect("source should be created");
    run_git(&source, ["init", "--quiet"]);
    run_git(&source, ["config", "user.name", "Rit Test"]);
    run_git(&source, ["config", "user.email", "rit@example.test"]);
    run_git(&source, ["config", "core.autocrlf", "false"]);
    fs::write(source.join("a.txt"), "base\n").expect("source file should be written");
    run_git(&source, ["add", "a.txt"]);
    run_git(&source, ["commit", "--quiet", "-m", "base"]);

    let git_output = Command::new("git")
        .args(["clone", "-q", "--local", "--no-checkout"])
        .arg(&source)
        .arg(&git_target)
        .output()
        .expect("git clone should start");
    assert!(
        git_output.status.success(),
        "git clone failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&git_output.stdout),
        String::from_utf8_lossy(&git_output.stderr)
    );

    let rit_output = Command::new(rit_binary())
        .args(["clone", "-q", "--local", "--no-checkout"])
        .arg(&source)
        .arg(&rit_target)
        .output()
        .expect("rit clone should start");
    assert!(
        rit_output.status.success(),
        "rit clone failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&rit_output.stdout),
        String::from_utf8_lossy(&rit_output.stderr)
    );
    assert_eq!(git_output.stdout, rit_output.stdout);
    assert_eq!(git_output.stderr, rit_output.stderr);

    let git_head = run_capture("git", ["rev-parse", "HEAD"], &git_target).0;
    let rit_head = run_capture(rit_binary(), ["rev-parse", "HEAD"], &rit_target).0;
    assert_eq!(git_head, rit_head);

    let git_commit = run_capture("git", ["cat-file", "-p", "HEAD"], &git_target).0;
    let rit_commit = run_capture(rit_binary(), ["cat-file", "-p", "HEAD"], &rit_target).0;
    assert_eq!(git_commit, rit_commit);
    assert!(!rit_target.join("a.txt").exists());

    let _ = fs::remove_dir_all(workspace);
}

#[test]
fn fetch_local_copies_objects_and_writes_fetch_head() {
    let workspace = temp_path("fetch-local");
    let source = workspace.join("source");
    let git_target = workspace.join("git-target");
    let rit_target = workspace.join("rit-target");
    fs::create_dir_all(&source).expect("source should be created");
    fs::create_dir_all(&git_target).expect("git target should be created");
    fs::create_dir_all(&rit_target).expect("rit target should be created");
    run_git(&source, ["init", "--quiet"]);
    run_git(&source, ["config", "user.name", "Rit Test"]);
    run_git(&source, ["config", "user.email", "rit@example.test"]);
    run_git(&source, ["config", "core.autocrlf", "false"]);
    fs::write(source.join("a.txt"), "base\n").expect("source file should be written");
    run_git(&source, ["add", "a.txt"]);
    run_git(&source, ["commit", "--quiet", "-m", "base"]);
    run_git(&git_target, ["init", "--quiet"]);
    run_git(&rit_target, ["init", "--quiet"]);

    let git_output = Command::new("git")
        .arg("fetch")
        .arg("-q")
        .arg(&source)
        .current_dir(&git_target)
        .output()
        .expect("git fetch should start");
    assert!(
        git_output.status.success(),
        "git fetch failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&git_output.stdout),
        String::from_utf8_lossy(&git_output.stderr)
    );

    let rit_output = Command::new(rit_binary())
        .arg("fetch")
        .arg("-q")
        .arg(&source)
        .current_dir(&rit_target)
        .output()
        .expect("rit fetch should start");
    assert!(
        rit_output.status.success(),
        "rit fetch failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&rit_output.stdout),
        String::from_utf8_lossy(&rit_output.stderr)
    );
    assert_eq!(git_output.stdout, rit_output.stdout);
    assert_eq!(git_output.stderr, rit_output.stderr);

    let git_fetch_head =
        fs::read_to_string(git_target.join(".git").join("FETCH_HEAD")).expect("git FETCH_HEAD");
    let rit_fetch_head =
        fs::read_to_string(rit_target.join(".git").join("FETCH_HEAD")).expect("rit FETCH_HEAD");
    assert_eq!(git_fetch_head, rit_fetch_head);

    let git_commit = run_capture("git", ["cat-file", "-p", "FETCH_HEAD"], &git_target).0;
    let rit_commit = run_capture(rit_binary(), ["cat-file", "-p", "FETCH_HEAD"], &rit_target).0;
    assert_eq!(git_commit, rit_commit);

    let _ = fs::remove_dir_all(workspace);
}

#[test]
fn fetch_local_refspec_updates_destination_ref() {
    let workspace = temp_path("fetch-local-refspec");
    let source = workspace.join("source");
    let git_target = workspace.join("git-target");
    let rit_target = workspace.join("rit-target");
    fs::create_dir_all(&source).expect("source should be created");
    fs::create_dir_all(&git_target).expect("git target should be created");
    fs::create_dir_all(&rit_target).expect("rit target should be created");
    run_git(&source, ["init", "--quiet"]);
    run_git(&source, ["config", "user.name", "Rit Test"]);
    run_git(&source, ["config", "user.email", "rit@example.test"]);
    run_git(&source, ["config", "core.autocrlf", "false"]);
    fs::write(source.join("a.txt"), "base\n").expect("source file should be written");
    run_git(&source, ["add", "a.txt"]);
    run_git(&source, ["commit", "--quiet", "-m", "base"]);
    run_git(&git_target, ["init", "--quiet"]);
    run_git(&rit_target, ["init", "--quiet"]);
    let refspec = "refs/heads/master:refs/remotes/origin/master";

    let git_output = Command::new("git")
        .arg("fetch")
        .arg("-q")
        .arg(&source)
        .arg(refspec)
        .current_dir(&git_target)
        .output()
        .expect("git fetch should start");
    assert!(
        git_output.status.success(),
        "git fetch failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&git_output.stdout),
        String::from_utf8_lossy(&git_output.stderr)
    );

    let rit_output = Command::new(rit_binary())
        .arg("fetch")
        .arg("-q")
        .arg(&source)
        .arg(refspec)
        .current_dir(&rit_target)
        .output()
        .expect("rit fetch should start");
    assert!(
        rit_output.status.success(),
        "rit fetch failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&rit_output.stdout),
        String::from_utf8_lossy(&rit_output.stderr)
    );
    assert_eq!(git_output.stdout, rit_output.stdout);
    assert_eq!(git_output.stderr, rit_output.stderr);

    let git_fetch_head =
        fs::read_to_string(git_target.join(".git").join("FETCH_HEAD")).expect("git FETCH_HEAD");
    let rit_fetch_head =
        fs::read_to_string(rit_target.join(".git").join("FETCH_HEAD")).expect("rit FETCH_HEAD");
    assert_eq!(git_fetch_head, rit_fetch_head);

    let git_ref = run_capture(
        "git",
        ["rev-parse", "refs/remotes/origin/master"],
        &git_target,
    )
    .0;
    let rit_ref = run_capture(
        rit_binary(),
        ["rev-parse", "refs/remotes/origin/master"],
        &rit_target,
    )
    .0;
    assert_eq!(git_ref, rit_ref);

    let git_commit = run_capture(
        "git",
        ["cat-file", "-p", "refs/remotes/origin/master"],
        &git_target,
    )
    .0;
    let rit_commit = run_capture(
        rit_binary(),
        ["cat-file", "-p", "refs/remotes/origin/master"],
        &rit_target,
    )
    .0;
    assert_eq!(git_commit, rit_commit);

    let _ = fs::remove_dir_all(workspace);
}

struct CommandSpec {
    program: OsString,
    args: Vec<OsString>,
    env: Vec<(OsString, OsString)>,
}

struct AttrPathspecWriteFixture {
    path: PathBuf,
}

impl AttrPathspecWriteFixture {
    fn new(name: &str) -> Self {
        let path = temp_path(name);
        fs::create_dir_all(&path).expect("fixture should be created");
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

impl Drop for AttrPathspecWriteFixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
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
        env: Vec::new(),
    }
}

fn command_words_with_env<const N: usize, const M: usize>(
    program: impl Into<OsString>,
    args: [&str; N],
    env: &[(&str, &str); M],
) -> CommandSpec {
    CommandSpec {
        program: program.into(),
        args: args.into_iter().map(OsString::from).collect(),
        env: env
            .iter()
            .map(|(name, value)| (OsString::from(name), OsString::from(value)))
            .collect(),
    }
}

fn run_command(spec: &CommandSpec, cwd: &Path) -> (String, String) {
    let mut command = Command::new(&spec.program);
    command.args(&spec.args).current_dir(cwd);
    for (name, value) in &spec.env {
        command.env(name, value);
    }
    let output = command.output().expect("command should start");
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
    let mut command = Command::new(&spec.program);
    command.args(&spec.args).current_dir(cwd);
    for (name, value) in &spec.env {
        command.env(name, value);
    }
    let output = command.output().expect("command should start");
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

fn write_hook(repository: &Path, name: &str, contents: &str) {
    let path = repository.join(".git").join("hooks").join(name);
    fs::write(&path, contents).expect("hook should be written");
    make_hook_executable(&path);
}

#[cfg(unix)]
fn make_hook_executable(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    let mut permissions = fs::metadata(path)
        .expect("hook metadata should read")
        .permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions).expect("hook permissions should be set");
}

#[cfg(not(unix))]
fn make_hook_executable(_path: &Path) {}

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
