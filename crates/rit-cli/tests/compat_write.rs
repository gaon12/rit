use rit_testkit::{LocalWriteFixture, LocalWriteFixtureKind};
use std::ffi::{OsStr, OsString};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::OnceLock;
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
fn add_plan_prints_paths_without_writing_index() {
    let fixture = LocalWriteFixture::new("add-plan", LocalWriteFixtureKind::NestedTracked)
        .expect("fixture should build");
    fs::write(
        fixture.path().join("nested").join("tracked.txt"),
        "changed\n",
    )
    .expect("tracked file should be modified");
    fs::write(fixture.path().join("nested").join("new.txt"), "new\n")
        .expect("new file should be written");

    let output = run_capture(rit_binary(), ["add", "--plan", "nested"], fixture.path()).0;

    assert!(output.contains("add: plan\n"));
    assert!(output.contains("add: nested/new.txt\n"));
    assert!(output.contains("add: nested/tracked.txt\n"));
    assert_eq!(
        run_capture(rit_binary(), ["status", "--porcelain=v1"], fixture.path()).0,
        " M nested/tracked.txt\n?? nested/new.txt\n"
    );
}

#[test]
fn ignore_explain_prints_matching_rules() {
    let fixture = LocalWriteFixture::new("ignore-explain", LocalWriteFixtureKind::NestedTracked)
        .expect("fixture should build");
    fs::write(fixture.path().join(".gitignore"), "*.log\n!important.log\n")
        .expect("gitignore should be written");

    let ignored = run_capture(
        rit_binary(),
        ["ignore", "explain", "debug.log"],
        fixture.path(),
    )
    .0;
    let negated = run_capture(
        rit_binary(),
        ["ignore", "explain", "important.log"],
        fixture.path(),
    )
    .0;

    assert!(ignored.contains("ignore: explain\n"));
    assert!(ignored.contains("path: debug.log\n"));
    assert!(ignored.contains("ignored: true\n"));
    assert!(ignored.contains("pattern=*.log negated=false\n"));
    assert!(negated.contains("ignored: false\n"));
    assert!(negated.contains("pattern=important.log negated=true\n"));
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
fn add_posix_bracket_pathspec_matches_git_status() {
    let fixture = LocalWriteFixture::new("add-posix-bracket", LocalWriteFixtureKind::NestedTracked)
        .expect("fixture should build");
    fs::write(fixture.path().join("1.txt"), "number\n").expect("number file should be written");
    fs::write(fixture.path().join("a.txt"), "letter\n").expect("letter file should be written");

    let outcome = compare_after_command(
        fixture.path(),
        command_words("git", ["add", "[[:digit:]].txt"]),
        command_words(rit_binary(), ["add", "[[:digit:]].txt"]),
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
fn add_pathspec_from_file_matches_git_status() {
    let fixture = LocalWriteFixture::new("add-pathspec-file", LocalWriteFixtureKind::NestedTracked)
        .expect("fixture should build");
    fs::write(
        fixture.path().join("nested").join("tracked.txt"),
        "changed\n",
    )
    .expect("tracked file should be modified");
    fs::write(fixture.path().join("nested").join("new.txt"), "new\n")
        .expect("new file should be written");
    fs::write(
        fixture.path().join("pathspecs.txt"),
        "nested/tracked.txt\nnested/new.txt\n",
    )
    .expect("pathspec file should be written");

    let outcome = compare_after_command(
        fixture.path(),
        command_words("git", ["add", "--pathspec-from-file", "pathspecs.txt"]),
        command_words(
            rit_binary(),
            ["add", "--pathspec-from-file", "pathspecs.txt"],
        ),
    );

    assert_eq!(outcome.git_command_stdout, outcome.rit_command_stdout);
    assert_eq!(outcome.git_command_stderr, outcome.rit_command_stderr);
    assert_eq!(outcome.git_status, outcome.rit_status);
}

#[test]
fn add_pathspec_from_stdin_matches_git_status() {
    let fixture =
        LocalWriteFixture::new("add-pathspec-stdin", LocalWriteFixtureKind::NestedTracked)
            .expect("fixture should build");
    fs::write(
        fixture.path().join("nested").join("tracked.txt"),
        "changed\n",
    )
    .expect("tracked file should be modified");
    fs::write(fixture.path().join("nested").join("new.txt"), "new\n")
        .expect("new file should be written");
    let stdin = b"nested/tracked.txt\nnested/new.txt\n";

    let outcome = compare_after_command(
        fixture.path(),
        command_words_with_stdin("git", ["add", "--pathspec-from-file", "-"], stdin),
        command_words_with_stdin(rit_binary(), ["add", "--pathspec-from-file", "-"], stdin),
    );

    assert_eq!(outcome.git_command_stdout, outcome.rit_command_stdout);
    assert_eq!(outcome.git_command_stderr, outcome.rit_command_stderr);
    assert_eq!(outcome.git_status, outcome.rit_status);
}

#[test]
fn add_nul_pathspec_from_stdin_matches_git_status() {
    let fixture = LocalWriteFixture::new(
        "add-pathspec-stdin-nul",
        LocalWriteFixtureKind::NestedTracked,
    )
    .expect("fixture should build");
    fs::write(
        fixture.path().join("nested").join("tracked.txt"),
        "changed\n",
    )
    .expect("tracked file should be modified");
    fs::write(fixture.path().join("nested").join("new.txt"), "new\n")
        .expect("new file should be written");
    let stdin = b"nested/tracked.txt\0nested/new.txt\0";

    let outcome = compare_after_command(
        fixture.path(),
        command_words_with_stdin(
            "git",
            ["add", "--pathspec-from-file", "-", "--pathspec-file-nul"],
            stdin,
        ),
        command_words_with_stdin(
            rit_binary(),
            ["add", "--pathspec-from-file", "-", "--pathspec-file-nul"],
            stdin,
        ),
    );

    assert_eq!(outcome.git_command_stdout, outcome.rit_command_stdout);
    assert_eq!(outcome.git_command_stderr, outcome.rit_command_stderr);
    assert_eq!(outcome.git_status, outcome.rit_status);
}

#[test]
fn add_nul_pathspec_from_file_matches_git_status() {
    let fixture = LocalWriteFixture::new(
        "add-pathspec-file-nul",
        LocalWriteFixtureKind::NestedTracked,
    )
    .expect("fixture should build");
    fs::write(
        fixture.path().join("nested").join("tracked.txt"),
        "changed\n",
    )
    .expect("tracked file should be modified");
    fs::write(fixture.path().join("nested").join("new.txt"), "new\n")
        .expect("new file should be written");
    fs::write(
        fixture.path().join("pathspecs.nul"),
        b"nested/tracked.txt\0nested/new.txt\0",
    )
    .expect("pathspec file should be written");

    let outcome = compare_after_command(
        fixture.path(),
        command_words(
            "git",
            [
                "add",
                "--pathspec-from-file",
                "pathspecs.nul",
                "--pathspec-file-nul",
            ],
        ),
        command_words(
            rit_binary(),
            [
                "add",
                "--pathspec-from-file",
                "pathspecs.nul",
                "--pathspec-file-nul",
            ],
        ),
    );

    assert_eq!(outcome.git_command_stdout, outcome.rit_command_stdout);
    assert_eq!(outcome.git_command_stderr, outcome.rit_command_stderr);
    assert_eq!(outcome.git_status, outcome.rit_status);
}

#[test]
fn add_quoted_pathspec_from_file_matches_git_status() {
    let fixture = LocalWriteFixture::new(
        "add-quoted-pathspec-file",
        LocalWriteFixtureKind::NestedTracked,
    )
    .expect("fixture should build");
    fs::write(fixture.path().join("space name.txt"), "space\n")
        .expect("space file should be written");
    fs::write(fixture.path().join("pathspecs.txt"), "\"space name.txt\"\n")
        .expect("pathspec file should be written");

    let outcome = compare_after_command(
        fixture.path(),
        command_words("git", ["add", "--pathspec-from-file", "pathspecs.txt"]),
        command_words(
            rit_binary(),
            ["add", "--pathspec-from-file", "pathspecs.txt"],
        ),
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
fn restore_pathspec_from_file_matches_git_status_and_files() {
    let fixture = LocalWriteFixture::new(
        "restore-pathspec-file",
        LocalWriteFixtureKind::NestedTracked,
    )
    .expect("fixture should build");
    fs::write(
        fixture.path().join("nested").join("tracked.txt"),
        "changed\n",
    )
    .expect("tracked file should be modified");
    fs::write(fixture.path().join("pathspecs.txt"), "nested/tracked.txt\n")
        .expect("pathspec file should be written");

    let outcome = compare_after_command(
        fixture.path(),
        command_words("git", ["restore", "--pathspec-from-file", "pathspecs.txt"]),
        command_words(
            rit_binary(),
            ["restore", "--pathspec-from-file", "pathspecs.txt"],
        ),
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
fn restore_pathspec_from_stdin_matches_git_status_and_files() {
    let fixture = LocalWriteFixture::new(
        "restore-pathspec-stdin",
        LocalWriteFixtureKind::NestedTracked,
    )
    .expect("fixture should build");
    fs::write(
        fixture.path().join("nested").join("tracked.txt"),
        "changed\n",
    )
    .expect("tracked file should be modified");
    let stdin = b"nested/tracked.txt\n";

    let outcome = compare_after_command(
        fixture.path(),
        command_words_with_stdin("git", ["restore", "--pathspec-from-file", "-"], stdin),
        command_words_with_stdin(
            rit_binary(),
            ["restore", "--pathspec-from-file", "-"],
            stdin,
        ),
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
fn restore_nul_pathspec_from_stdin_matches_git_status_and_files() {
    let fixture = LocalWriteFixture::new(
        "restore-pathspec-stdin-nul",
        LocalWriteFixtureKind::NestedTracked,
    )
    .expect("fixture should build");
    fs::write(
        fixture.path().join("nested").join("tracked.txt"),
        "changed\n",
    )
    .expect("tracked file should be modified");
    let stdin = b"nested/tracked.txt\0";

    let outcome = compare_after_command(
        fixture.path(),
        command_words_with_stdin(
            "git",
            [
                "restore",
                "--pathspec-from-file",
                "-",
                "--pathspec-file-nul",
            ],
            stdin,
        ),
        command_words_with_stdin(
            rit_binary(),
            [
                "restore",
                "--pathspec-from-file",
                "-",
                "--pathspec-file-nul",
            ],
            stdin,
        ),
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
fn reset_plan_prints_index_changes_without_writing_index() {
    let fixture = LocalWriteFixture::new("reset-plan", LocalWriteFixtureKind::NestedTracked)
        .expect("fixture should build");
    fs::write(
        fixture.path().join("nested").join("tracked.txt"),
        "changed\n",
    )
    .expect("tracked file should be modified");
    fs::write(fixture.path().join("nested").join("new.txt"), "new\n")
        .expect("new file should be written");
    run_capture(rit_binary(), ["add", "nested"], fixture.path());

    let output = run_capture(rit_binary(), ["reset", "--plan", "nested"], fixture.path()).0;

    assert!(output.contains("reset: plan\n"));
    assert!(output.contains("restore-index: nested/tracked.txt\n"));
    assert!(output.contains("remove-index: nested/new.txt\n"));
    assert_eq!(
        run_capture(rit_binary(), ["status", "--porcelain=v1"], fixture.path()).0,
        "A  nested/new.txt\nM  nested/tracked.txt\n"
    );
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
fn reset_pathspec_from_file_matches_git_status() {
    let fixture =
        LocalWriteFixture::new("reset-pathspec-file", LocalWriteFixtureKind::NestedTracked)
            .expect("fixture should build");
    fs::write(
        fixture.path().join("nested").join("tracked.txt"),
        "changed\n",
    )
    .expect("tracked file should be modified");
    fs::write(fixture.path().join("pathspecs.txt"), "nested/tracked.txt\n")
        .expect("pathspec file should be written");
    run_git(fixture.path(), ["add", "nested"]);

    let outcome = compare_after_command(
        fixture.path(),
        command_words("git", ["reset", "--pathspec-from-file", "pathspecs.txt"]),
        command_words(
            rit_binary(),
            ["reset", "--pathspec-from-file", "pathspecs.txt"],
        ),
    );

    assert_eq!(outcome.git_command_stdout, outcome.rit_command_stdout);
    assert_eq!(outcome.git_command_stderr, outcome.rit_command_stderr);
    assert_eq!(outcome.git_status, outcome.rit_status);
}

#[test]
fn reset_pathspec_from_stdin_matches_git_status() {
    let fixture =
        LocalWriteFixture::new("reset-pathspec-stdin", LocalWriteFixtureKind::NestedTracked)
            .expect("fixture should build");
    fs::write(
        fixture.path().join("nested").join("tracked.txt"),
        "changed\n",
    )
    .expect("tracked file should be modified");
    run_git(fixture.path(), ["add", "nested"]);
    let stdin = b"nested/tracked.txt\n";

    let outcome = compare_after_command(
        fixture.path(),
        command_words_with_stdin("git", ["reset", "--pathspec-from-file", "-"], stdin),
        command_words_with_stdin(rit_binary(), ["reset", "--pathspec-from-file", "-"], stdin),
    );

    assert_eq!(outcome.git_command_stdout, outcome.rit_command_stdout);
    assert_eq!(outcome.git_command_stderr, outcome.rit_command_stderr);
    assert_eq!(outcome.git_status, outcome.rit_status);
}

#[test]
fn reset_nul_pathspec_from_stdin_matches_git_status() {
    let fixture = LocalWriteFixture::new(
        "reset-pathspec-stdin-nul",
        LocalWriteFixtureKind::NestedTracked,
    )
    .expect("fixture should build");
    fs::write(
        fixture.path().join("nested").join("tracked.txt"),
        "changed\n",
    )
    .expect("tracked file should be modified");
    run_git(fixture.path(), ["add", "nested"]);
    let stdin = b"nested/tracked.txt\0";

    let outcome = compare_after_command(
        fixture.path(),
        command_words_with_stdin(
            "git",
            ["reset", "--pathspec-from-file", "-", "--pathspec-file-nul"],
            stdin,
        ),
        command_words_with_stdin(
            rit_binary(),
            ["reset", "--pathspec-from-file", "-", "--pathspec-file-nul"],
            stdin,
        ),
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
fn commit_plan_prints_staged_paths_without_advancing_head() {
    let fixture = LocalWriteFixture::new("commit-plan", LocalWriteFixtureKind::NestedTracked)
        .expect("fixture should build");
    let base = run_capture("git", ["rev-parse", "HEAD"], fixture.path()).0;
    fs::write(
        fixture.path().join("nested").join("tracked.txt"),
        "planned\n",
    )
    .expect("tracked file should be modified");
    run_capture(rit_binary(), ["add", "nested/tracked.txt"], fixture.path());

    let output = run_capture(
        rit_binary(),
        ["commit", "--plan", "--no-verify", "-m", "planned"],
        fixture.path(),
    )
    .0;

    assert!(output.contains("commit: plan\n"));
    assert!(output.contains("message: planned\n"));
    assert!(output.contains("hooks: no-verify\n"));
    assert!(output.contains("files: 1\n"));
    assert!(output.contains("path: nested/tracked.txt\n"));
    assert_eq!(
        run_capture("git", ["rev-parse", "HEAD"], fixture.path()).0,
        base
    );
    assert_eq!(
        run_capture(rit_binary(), ["status", "--porcelain=v1"], fixture.path()).0,
        "M  nested/tracked.txt\n"
    );
    let log = run_capture(rit_binary(), ["op", "log"], fixture.path()).0;
    assert!(log.contains(" add "));
    assert!(!log.contains(" commit "));
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
fn operation_journal_records_commit_and_undo_restores_head() {
    let fixture = LocalWriteFixture::new(
        "operation-journal-commit",
        LocalWriteFixtureKind::NestedTracked,
    )
    .expect("fixture should build");
    let base = run_capture("git", ["rev-parse", "HEAD"], fixture.path()).0;
    fs::write(
        fixture.path().join("nested").join("tracked.txt"),
        "journaled\n",
    )
    .expect("tracked file should be modified");

    run_capture(rit_binary(), ["add", "nested/tracked.txt"], fixture.path());
    run_capture(rit_binary(), ["commit", "-m", "journaled"], fixture.path());

    let log = run_capture(rit_binary(), ["op", "log"], fixture.path()).0;
    assert!(log.contains(" commit "));
    assert!(log.contains("journaled"));
    assert!(log.contains("paths: nested/tracked.txt"));
    assert!(log.contains("objects: "));
    {
        let mut log_file = fs::OpenOptions::new()
            .append(true)
            .open(fixture.path().join(".git").join("rit").join("ops.log"))
            .expect("operation log should open");
        writeln!(log_file, "not an operation record").expect("malformed line should append");
    }
    let (log_after_bad_line, warning) = run_capture(rit_binary(), ["op", "log"], fixture.path());
    assert!(log_after_bad_line.contains("journaled"));
    assert!(warning.contains("skipped malformed operation journal line"));
    assert_ne!(
        run_capture("git", ["rev-parse", "HEAD"], fixture.path()).0,
        base
    );

    run_capture(rit_binary(), ["undo"], fixture.path());

    assert_eq!(
        run_capture("git", ["rev-parse", "HEAD"], fixture.path()).0,
        base
    );
    assert_eq!(
        fs::read_to_string(fixture.path().join("nested").join("tracked.txt"))
            .expect("tracked file should read"),
        "base\n"
    );
    assert_eq!(
        run_capture(rit_binary(), ["status", "--porcelain=v1"], fixture.path()).0,
        ""
    );
}

#[test]
fn operation_journal_records_index_and_worktree_commands() {
    let fixture = LocalWriteFixture::new(
        "operation-journal-index",
        LocalWriteFixtureKind::NestedTracked,
    )
    .expect("fixture should build");
    fs::write(
        fixture.path().join("nested").join("tracked.txt"),
        "staged\n",
    )
    .expect("tracked file should be modified");

    run_capture(rit_binary(), ["add", "nested/tracked.txt"], fixture.path());
    let log = run_capture(rit_binary(), ["op", "log"], fixture.path()).0;
    assert!(log.contains(" add "));
    assert!(log.contains("paths: nested/tracked.txt"));

    run_capture(
        rit_binary(),
        ["reset", "nested/tracked.txt"],
        fixture.path(),
    );
    let log = run_capture(rit_binary(), ["op", "log"], fixture.path()).0;
    assert!(log.contains(" reset "));
    assert!(log.contains("paths: nested/tracked.txt"));

    run_capture(
        rit_binary(),
        ["restore", "nested/tracked.txt"],
        fixture.path(),
    );
    let log = run_capture(rit_binary(), ["op", "log"], fixture.path()).0;
    assert!(log.contains(" restore "));
    assert!(log.contains("paths: nested/tracked.txt"));
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
            stdin: None,
        },
        CommandSpec {
            program: rit_binary(),
            args: vec![OsString::from("checkout"), OsString::from(base)],
            env: Vec::new(),
            stdin: None,
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
fn merge_ff_only_matches_git_final_state() {
    let fixture = temp_path("merge-ff-only-fixture");
    fs::create_dir_all(&fixture).expect("fixture should be created");
    run_git(&fixture, ["init", "--quiet"]);
    run_git(&fixture, ["config", "user.name", "Rit Test"]);
    run_git(&fixture, ["config", "user.email", "rit@example.test"]);
    run_git(&fixture, ["config", "core.autocrlf", "false"]);
    run_git(&fixture, ["config", "core.eol", "lf"]);
    fs::write(fixture.join("tracked.txt"), "base\n").expect("base file should be written");
    run_git(&fixture, ["add", "tracked.txt"]);
    run_git(&fixture, ["commit", "--quiet", "-m", "base"]);
    run_git(&fixture, ["checkout", "--quiet", "-b", "topic"]);
    fs::write(fixture.join("tracked.txt"), "topic\n").expect("topic file should be written");
    run_git(&fixture, ["add", "tracked.txt"]);
    run_git(&fixture, ["commit", "--quiet", "-m", "topic"]);
    run_git(&fixture, ["checkout", "--quiet", "master"]);

    let outcome = compare_after_command(
        &fixture,
        command_words("git", ["merge", "--ff-only", "topic"]),
        command_words(rit_binary(), ["merge", "--ff-only", "topic"]),
    );

    assert_eq!(outcome.git_status, outcome.rit_status);
    assert_eq!(
        run_capture("git", ["rev-parse", "HEAD"], &outcome.git_repo).0,
        run_capture("git", ["rev-parse", "HEAD"], &outcome.rit_repo).0
    );
    assert_eq!(
        fs::read_to_string(outcome.git_repo.join("tracked.txt")).expect("git file should read"),
        fs::read_to_string(outcome.rit_repo.join("tracked.txt")).expect("rit file should read")
    );
    let _ = fs::remove_dir_all(fixture);
}

#[test]
fn merge_plan_prints_fast_forward_without_changing_head() {
    let fixture = temp_path("merge-plan-fixture");
    fs::create_dir_all(&fixture).expect("fixture should be created");
    run_git(&fixture, ["init", "--quiet"]);
    run_git(&fixture, ["config", "user.name", "Rit Test"]);
    run_git(&fixture, ["config", "user.email", "rit@example.test"]);
    run_git(&fixture, ["config", "core.autocrlf", "false"]);
    fs::write(fixture.join("tracked.txt"), "base\n").expect("base file should be written");
    run_git(&fixture, ["add", "tracked.txt"]);
    run_git(&fixture, ["commit", "--quiet", "-m", "base"]);
    let base = run_capture("git", ["rev-parse", "HEAD"], &fixture).0;
    run_git(&fixture, ["checkout", "--quiet", "-b", "topic"]);
    fs::write(fixture.join("tracked.txt"), "topic\n").expect("topic file should be written");
    run_git(&fixture, ["add", "tracked.txt"]);
    run_git(&fixture, ["commit", "--quiet", "-m", "topic"]);
    run_git(&fixture, ["checkout", "--quiet", "master"]);

    let output = run_capture(
        rit_binary(),
        ["merge", "--plan", "--ff-only", "topic"],
        &fixture,
    )
    .0;

    assert!(output.contains("merge: plan\n"));
    assert!(output.contains("action: fast-forward\n"));
    assert!(output.contains("update: tracked.txt\n"));
    assert_eq!(run_capture("git", ["rev-parse", "HEAD"], &fixture).0, base);
    assert_eq!(
        fs::read_to_string(fixture.join("tracked.txt")).expect("file should read"),
        "base\n"
    );
    assert_eq!(run_capture(rit_binary(), ["op", "log"], &fixture).0, "");
    let _ = fs::remove_dir_all(fixture);
}

#[test]
fn merge_plan_prints_non_fast_forward_without_changing_head() {
    let fixture = temp_path("merge-plan-non-ff-fixture");
    fs::create_dir_all(&fixture).expect("fixture should be created");
    run_git(&fixture, ["init", "--quiet"]);
    run_git(&fixture, ["config", "user.name", "Rit Test"]);
    run_git(&fixture, ["config", "user.email", "rit@example.test"]);
    run_git(&fixture, ["config", "core.autocrlf", "false"]);
    fs::write(fixture.join("tracked.txt"), "base\n").expect("base file should be written");
    run_git(&fixture, ["add", "tracked.txt"]);
    run_git(&fixture, ["commit", "--quiet", "-m", "base"]);
    run_git(&fixture, ["checkout", "--quiet", "-b", "topic"]);
    fs::write(fixture.join("tracked.txt"), "topic\n").expect("topic file should be written");
    run_git(&fixture, ["add", "tracked.txt"]);
    run_git(&fixture, ["commit", "--quiet", "-m", "topic"]);
    run_git(&fixture, ["checkout", "--quiet", "master"]);
    fs::write(fixture.join("tracked.txt"), "master\n").expect("master file should be written");
    run_git(&fixture, ["add", "tracked.txt"]);
    run_git(&fixture, ["commit", "--quiet", "-m", "master"]);
    let master = run_capture("git", ["rev-parse", "HEAD"], &fixture).0;

    let output = run_capture(rit_binary(), ["merge", "--plan", "topic"], &fixture).0;

    assert!(output.contains("merge: plan\n"));
    assert!(output.contains("action: non-fast-forward\n"));
    assert!(output.contains("merge-base: "));
    assert!(output.contains("head-change: tracked.txt\n"));
    assert!(output.contains("target-change: tracked.txt\n"));
    assert!(output.contains("conflict-candidate: tracked.txt\n"));
    assert!(output.contains("conflict-stage: tracked.txt base=100644:"));
    assert!(output.contains(" head=100644:"));
    assert!(output.contains(" target=100644:"));
    assert!(output.contains("requires: merge-commit\n"));
    assert_eq!(
        run_capture("git", ["rev-parse", "HEAD"], &fixture).0,
        master
    );
    assert_eq!(
        fs::read_to_string(fixture.join("tracked.txt")).expect("file should read"),
        "master\n"
    );
    assert_eq!(run_capture(rit_binary(), ["op", "log"], &fixture).0, "");
    let _ = fs::remove_dir_all(fixture);
}

#[test]
fn merge_conflict_writes_index_stages_and_operation_record() {
    let fixture = temp_path("merge-conflict-stage-fixture");
    fs::create_dir_all(&fixture).expect("fixture should be created");
    run_git(&fixture, ["init", "--quiet"]);
    run_git(&fixture, ["config", "user.name", "Rit Test"]);
    run_git(&fixture, ["config", "user.email", "rit@example.test"]);
    run_git(&fixture, ["config", "core.autocrlf", "false"]);
    fs::write(fixture.join("tracked.txt"), "base\n").expect("base file should be written");
    run_git(&fixture, ["add", "tracked.txt"]);
    run_git(&fixture, ["commit", "--quiet", "-m", "base"]);
    run_git(&fixture, ["checkout", "--quiet", "-b", "topic"]);
    fs::write(fixture.join("tracked.txt"), "topic\n").expect("topic file should be written");
    run_git(&fixture, ["add", "tracked.txt"]);
    run_git(&fixture, ["commit", "--quiet", "-m", "topic"]);
    run_git(&fixture, ["checkout", "--quiet", "master"]);
    fs::write(fixture.join("tracked.txt"), "master\n").expect("master file should be written");
    run_git(&fixture, ["add", "tracked.txt"]);
    run_git(&fixture, ["commit", "--quiet", "-m", "master"]);

    let output = Command::new(rit_binary())
        .args(["merge", "topic"])
        .current_dir(&fixture)
        .output()
        .expect("rit merge should start");

    assert!(!output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("CONFLICT (content): Merge conflict in tracked.txt\n"));
    assert!(stdout.contains("Automatic merge failed; fix conflicts and then commit the result.\n"));
    assert!(fixture.join(".git").join("MERGE_HEAD").exists());
    let ls_files = run_capture(rit_binary(), ["ls-files", "--stage"], &fixture).0;
    assert!(ls_files.contains(" 1\ttracked.txt\n"));
    assert!(ls_files.contains(" 2\ttracked.txt\n"));
    assert!(ls_files.contains(" 3\ttracked.txt\n"));
    let conflict_text =
        fs::read_to_string(fixture.join("tracked.txt")).expect("conflict file should read");
    assert!(conflict_text.contains("<<<<<<< HEAD\nmaster\n=======\ntopic\n>>>>>>> topic\n"));
    let op_log = run_capture(rit_binary(), ["op", "log"], &fixture).0;
    assert!(op_log.contains("merge"));
    assert!(op_log.contains("tracked.txt"));
    let abort_output = run_capture(rit_binary(), ["merge", "--abort"], &fixture).0;
    assert!(abort_output.contains("Aborted merge; restored "));
    assert!(!fixture.join(".git").join("MERGE_HEAD").exists());
    assert_eq!(
        fs::read_to_string(fixture.join("tracked.txt")).expect("file should read"),
        "master\n"
    );
    assert_eq!(
        run_capture(rit_binary(), ["status", "--porcelain=v1"], &fixture).0,
        ""
    );
    let _ = fs::remove_dir_all(fixture);
}

#[test]
fn merge_delete_modify_conflict_leaves_target_file_when_head_deleted() {
    let fixture = temp_path("merge-delete-modify-fixture");
    fs::create_dir_all(&fixture).expect("fixture should be created");
    run_git(&fixture, ["init", "--quiet"]);
    run_git(&fixture, ["config", "user.name", "Rit Test"]);
    run_git(&fixture, ["config", "user.email", "rit@example.test"]);
    run_git(&fixture, ["config", "core.autocrlf", "false"]);
    fs::write(fixture.join("a.txt"), "base\n").expect("base file should be written");
    run_git(&fixture, ["add", "a.txt"]);
    run_git(&fixture, ["commit", "--quiet", "-m", "base"]);
    run_git(&fixture, ["checkout", "--quiet", "-b", "topic"]);
    fs::write(fixture.join("a.txt"), "topic\n").expect("topic file should be written");
    run_git(&fixture, ["add", "a.txt"]);
    run_git(&fixture, ["commit", "--quiet", "-m", "topic"]);
    run_git(&fixture, ["checkout", "--quiet", "master"]);
    fs::remove_file(fixture.join("a.txt")).expect("head file should be removed");
    run_git(&fixture, ["add", "a.txt"]);
    run_git(&fixture, ["commit", "--quiet", "-m", "delete"]);

    let merge = Command::new(rit_binary())
        .args(["merge", "topic"])
        .current_dir(&fixture)
        .output()
        .expect("rit merge should start");

    assert!(!merge.status.success());
    let stdout = String::from_utf8_lossy(&merge.stdout);
    assert!(stdout.contains(
        "CONFLICT (modify/delete): a.txt deleted in HEAD and modified in topic.  Version topic of a.txt left in tree.\n"
    ));
    assert_eq!(
        fs::read_to_string(fixture.join("a.txt")).expect("target file should exist"),
        "topic\n"
    );
    assert_eq!(
        run_capture(rit_binary(), ["status", "--porcelain=v1"], &fixture).0,
        "DU a.txt\n"
    );
    let _ = fs::remove_dir_all(fixture);
}

#[test]
fn merge_delete_modify_conflict_leaves_head_file_when_target_deleted() {
    let fixture = temp_path("merge-target-delete-modify-fixture");
    fs::create_dir_all(&fixture).expect("fixture should be created");
    run_git(&fixture, ["init", "--quiet"]);
    run_git(&fixture, ["config", "user.name", "Rit Test"]);
    run_git(&fixture, ["config", "user.email", "rit@example.test"]);
    run_git(&fixture, ["config", "core.autocrlf", "false"]);
    fs::write(fixture.join("a.txt"), "base\n").expect("base file should be written");
    run_git(&fixture, ["add", "a.txt"]);
    run_git(&fixture, ["commit", "--quiet", "-m", "base"]);
    run_git(&fixture, ["checkout", "--quiet", "-b", "topic"]);
    fs::remove_file(fixture.join("a.txt")).expect("target file should be removed");
    run_git(&fixture, ["add", "a.txt"]);
    run_git(&fixture, ["commit", "--quiet", "-m", "delete"]);
    run_git(&fixture, ["checkout", "--quiet", "master"]);
    fs::write(fixture.join("a.txt"), "head\n").expect("head file should be written");
    run_git(&fixture, ["add", "a.txt"]);
    run_git(&fixture, ["commit", "--quiet", "-m", "head"]);

    let merge = Command::new(rit_binary())
        .args(["merge", "topic"])
        .current_dir(&fixture)
        .output()
        .expect("rit merge should start");

    assert!(!merge.status.success());
    let stdout = String::from_utf8_lossy(&merge.stdout);
    assert!(stdout.contains(
        "CONFLICT (modify/delete): a.txt deleted in topic and modified in HEAD.  Version HEAD of a.txt left in tree.\n"
    ));
    assert_eq!(
        fs::read_to_string(fixture.join("a.txt")).expect("head file should exist"),
        "head\n"
    );
    assert_eq!(
        run_capture(rit_binary(), ["status", "--porcelain=v1"], &fixture).0,
        "UD a.txt\n"
    );
    let _ = fs::remove_dir_all(fixture);
}

#[test]
fn merge_binary_conflict_reports_warning_and_keeps_head_file() {
    let fixture = temp_path("merge-binary-conflict-fixture");
    fs::create_dir_all(&fixture).expect("fixture should be created");
    run_git(&fixture, ["init", "--quiet"]);
    run_git(&fixture, ["config", "user.name", "Rit Test"]);
    run_git(&fixture, ["config", "user.email", "rit@example.test"]);
    run_git(&fixture, ["config", "core.autocrlf", "false"]);
    fs::write(fixture.join("blob.bin"), [0, 1, 2, 3, 4]).expect("base blob should be written");
    run_git(&fixture, ["add", "blob.bin"]);
    run_git(&fixture, ["commit", "--quiet", "-m", "base"]);
    run_git(&fixture, ["checkout", "--quiet", "-b", "topic"]);
    fs::write(fixture.join("blob.bin"), [0, 1, 2, 9, 4]).expect("topic blob should be written");
    run_git(&fixture, ["add", "blob.bin"]);
    run_git(&fixture, ["commit", "--quiet", "-m", "topic"]);
    run_git(&fixture, ["checkout", "--quiet", "master"]);
    fs::write(fixture.join("blob.bin"), [0, 1, 2, 8, 4]).expect("head blob should be written");
    run_git(&fixture, ["add", "blob.bin"]);
    run_git(&fixture, ["commit", "--quiet", "-m", "master"]);

    let merge = Command::new(rit_binary())
        .args(["merge", "topic"])
        .current_dir(&fixture)
        .output()
        .expect("rit merge should start");

    assert!(!merge.status.success());
    let stdout = String::from_utf8_lossy(&merge.stdout);
    assert!(stdout.contains("warning: Cannot merge binary files: blob.bin (HEAD vs. topic)\n"));
    assert!(stdout.contains("CONFLICT (content): Merge conflict in blob.bin\n"));
    assert_eq!(
        fs::read(fixture.join("blob.bin")).expect("head blob should exist"),
        vec![0, 1, 2, 8, 4]
    );
    assert_eq!(
        run_capture(rit_binary(), ["status", "--porcelain=v1"], &fixture).0,
        "UU blob.bin\n"
    );
    let _ = fs::remove_dir_all(fixture);
}

#[test]
fn merge_content_conflict_preserves_mode_stage_entries() {
    let fixture = temp_path("merge-content-mode-conflict-fixture");
    fs::create_dir_all(&fixture).expect("fixture should be created");
    run_git(&fixture, ["init", "--quiet"]);
    run_git(&fixture, ["config", "user.name", "Rit Test"]);
    run_git(&fixture, ["config", "user.email", "rit@example.test"]);
    run_git(&fixture, ["config", "core.autocrlf", "false"]);
    run_git(&fixture, ["config", "core.filemode", "true"]);
    fs::write(fixture.join("a.sh"), "base\n").expect("base file should be written");
    run_git(&fixture, ["add", "a.sh"]);
    run_git(&fixture, ["commit", "--quiet", "-m", "base"]);
    run_git(&fixture, ["checkout", "--quiet", "-b", "topic"]);
    fs::write(fixture.join("a.sh"), "topic\n").expect("topic file should be written");
    run_git(&fixture, ["add", "a.sh"]);
    run_git(&fixture, ["update-index", "--chmod=+x", "a.sh"]);
    run_git(&fixture, ["commit", "--quiet", "-m", "topic"]);
    run_git(&fixture, ["checkout", "--force", "--quiet", "master"]);
    fs::write(fixture.join("a.sh"), "head\n").expect("head file should be written");
    run_git(&fixture, ["add", "a.sh"]);
    run_git(&fixture, ["commit", "--quiet", "-m", "head"]);

    let merge = Command::new(rit_binary())
        .args(["merge", "topic"])
        .current_dir(&fixture)
        .output()
        .expect("rit merge should start");

    assert!(!merge.status.success());
    let stdout = String::from_utf8_lossy(&merge.stdout);
    assert!(stdout.contains("CONFLICT (content): Merge conflict in a.sh\n"));
    assert_eq!(
        run_capture(rit_binary(), ["status", "--porcelain=v1"], &fixture).0,
        "UU a.sh\n"
    );
    let ls_files = run_capture(rit_binary(), ["ls-files", "--stage"], &fixture).0;
    assert!(ls_files.contains("100644 "));
    assert!(ls_files.contains(" 1\ta.sh\n"));
    assert!(ls_files.contains(" 2\ta.sh\n"));
    assert!(ls_files.contains("100755 "));
    assert!(ls_files.contains(" 3\ta.sh\n"));
    assert_eq!(
        fs::read_to_string(fixture.join("a.sh")).expect("conflict file should read"),
        "<<<<<<< HEAD\nhead\n=======\ntopic\n>>>>>>> topic\n"
    );
    let _ = fs::remove_dir_all(fixture);
}

#[test]
fn merge_add_add_conflict_reports_add_add_message() {
    let fixture = temp_path("merge-add-add-conflict-fixture");
    fs::create_dir_all(&fixture).expect("fixture should be created");
    run_git(&fixture, ["init", "--quiet"]);
    run_git(&fixture, ["config", "user.name", "Rit Test"]);
    run_git(&fixture, ["config", "user.email", "rit@example.test"]);
    run_git(&fixture, ["config", "core.autocrlf", "false"]);
    fs::write(fixture.join("base.txt"), "base\n").expect("base file should be written");
    run_git(&fixture, ["add", "base.txt"]);
    run_git(&fixture, ["commit", "--quiet", "-m", "base"]);
    run_git(&fixture, ["checkout", "--quiet", "-b", "topic"]);
    fs::write(fixture.join("a.txt"), "topic\n").expect("topic file should be written");
    run_git(&fixture, ["add", "a.txt"]);
    run_git(&fixture, ["commit", "--quiet", "-m", "topic"]);
    run_git(&fixture, ["checkout", "--quiet", "master"]);
    fs::write(fixture.join("a.txt"), "head\n").expect("head file should be written");
    run_git(&fixture, ["add", "a.txt"]);
    run_git(&fixture, ["commit", "--quiet", "-m", "head"]);

    let merge = Command::new(rit_binary())
        .args(["merge", "topic"])
        .current_dir(&fixture)
        .output()
        .expect("rit merge should start");

    assert!(!merge.status.success());
    let stdout = String::from_utf8_lossy(&merge.stdout);
    assert!(stdout.contains("CONFLICT (add/add): Merge conflict in a.txt\n"));
    assert_eq!(
        run_capture(rit_binary(), ["status", "--porcelain=v1"], &fixture).0,
        "AA a.txt\n"
    );
    let conflict_text = fs::read_to_string(fixture.join("a.txt")).expect("file should read");
    assert!(conflict_text.contains("<<<<<<< HEAD\nhead\n=======\ntopic\n>>>>>>> topic\n"));
    let _ = fs::remove_dir_all(fixture);
}

#[test]
fn merge_distinct_type_conflict_splits_regular_file_and_symlink_paths() {
    let fixture = temp_path("merge-distinct-type-conflict-fixture");
    fs::create_dir_all(&fixture).expect("fixture should be created");
    run_git(&fixture, ["init", "--quiet"]);
    run_git(&fixture, ["config", "user.name", "Rit Test"]);
    run_git(&fixture, ["config", "user.email", "rit@example.test"]);
    run_git(&fixture, ["config", "core.autocrlf", "false"]);
    run_git(&fixture, ["config", "core.symlinks", "false"]);
    fs::write(fixture.join("a.txt"), "base\n").expect("base file should be written");
    run_git(&fixture, ["add", "a.txt"]);
    run_git(&fixture, ["commit", "--quiet", "-m", "base"]);
    run_git(&fixture, ["checkout", "--quiet", "-b", "topic"]);
    let target_blob = write_git_blob_from_stdin(&fixture, b"target");
    let cacheinfo = format!("120000,{target_blob},a.txt");
    let update = Command::new(git_program())
        .args(["update-index", "--add", "--cacheinfo", &cacheinfo])
        .current_dir(&fixture)
        .output()
        .expect("git update-index should start");
    assert!(update.status.success());
    run_git(&fixture, ["commit", "--quiet", "-m", "symlink"]);
    run_git(&fixture, ["checkout", "--force", "--quiet", "master"]);
    fs::write(fixture.join("a.txt"), "head\n").expect("head file should be written");
    run_git(&fixture, ["add", "a.txt"]);
    run_git(&fixture, ["commit", "--quiet", "-m", "content"]);

    let merge = Command::new(rit_binary())
        .args(["merge", "topic"])
        .current_dir(&fixture)
        .output()
        .expect("rit merge should start");

    assert!(!merge.status.success());
    let stdout = String::from_utf8_lossy(&merge.stdout);
    assert!(stdout.contains(
        "CONFLICT (distinct types): a.txt had different types on each side; renamed one of them so each can be recorded somewhere.\n"
    ));
    assert_eq!(
        run_capture(rit_binary(), ["status", "--porcelain=v1"], &fixture).0,
        "UA a.txt\nUD a.txt~HEAD\n"
    );
    let ls_files = run_capture(rit_binary(), ["ls-files", "--stage"], &fixture).0;
    assert!(ls_files.contains("120000 "));
    assert!(ls_files.contains(" 3\ta.txt\n"));
    assert!(ls_files.contains(" 1\ta.txt~HEAD\n"));
    assert!(ls_files.contains(" 2\ta.txt~HEAD\n"));
    assert_eq!(
        fs::read_to_string(fixture.join("a.txt")).expect("target side should read"),
        "target"
    );
    assert_eq!(
        fs::read_to_string(fixture.join("a.txt~HEAD")).expect("head side should read"),
        "head\n"
    );
    let _ = fs::remove_dir_all(fixture);
}

#[test]
fn merge_distinct_type_conflict_splits_symlink_head_and_regular_target_paths() {
    let fixture = temp_path("merge-distinct-type-inverse-fixture");
    fs::create_dir_all(&fixture).expect("fixture should be created");
    run_git(&fixture, ["init", "--quiet"]);
    run_git(&fixture, ["config", "user.name", "Rit Test"]);
    run_git(&fixture, ["config", "user.email", "rit@example.test"]);
    run_git(&fixture, ["config", "core.autocrlf", "false"]);
    run_git(&fixture, ["config", "core.symlinks", "false"]);
    fs::write(fixture.join("a.txt"), "base\n").expect("base file should be written");
    run_git(&fixture, ["add", "a.txt"]);
    run_git(&fixture, ["commit", "--quiet", "-m", "base"]);
    run_git(&fixture, ["checkout", "--quiet", "-b", "topic"]);
    fs::write(fixture.join("a.txt"), "target\n").expect("target file should be written");
    run_git(&fixture, ["add", "a.txt"]);
    run_git(&fixture, ["commit", "--quiet", "-m", "content"]);
    run_git(&fixture, ["checkout", "--force", "--quiet", "master"]);
    let head_blob = write_git_blob_from_stdin(&fixture, b"head-target");
    let cacheinfo = format!("120000,{head_blob},a.txt");
    let update = Command::new(git_program())
        .args(["update-index", "--add", "--cacheinfo", &cacheinfo])
        .current_dir(&fixture)
        .output()
        .expect("git update-index should start");
    assert!(update.status.success());
    run_git(&fixture, ["commit", "--quiet", "-m", "symlink"]);
    fs::write(fixture.join("a.txt"), "head-target").expect("head side should be materialized");

    let merge = Command::new(rit_binary())
        .args(["merge", "topic"])
        .current_dir(&fixture)
        .output()
        .expect("rit merge should start");

    assert!(!merge.status.success());
    let stdout = String::from_utf8_lossy(&merge.stdout);
    assert!(stdout.contains(
        "CONFLICT (distinct types): a.txt had different types on each side; renamed one of them so each can be recorded somewhere.\n"
    ));
    assert_eq!(
        run_capture(rit_binary(), ["status", "--porcelain=v1"], &fixture).0,
        "AU a.txt\nDU a.txt~topic\n"
    );
    let ls_files = run_capture(rit_binary(), ["ls-files", "--stage"], &fixture).0;
    assert!(ls_files.contains("120000 "));
    assert!(ls_files.contains(" 2\ta.txt\n"));
    assert!(ls_files.contains(" 1\ta.txt~topic\n"));
    assert!(ls_files.contains(" 3\ta.txt~topic\n"));
    assert_eq!(
        fs::read_to_string(fixture.join("a.txt")).expect("head side should read"),
        "head-target"
    );
    assert_eq!(
        fs::read_to_string(fixture.join("a.txt~topic")).expect("target side should read"),
        "target\n"
    );
    let _ = fs::remove_dir_all(fixture);
}

#[test]
fn merge_ff_only_rejects_conflict_without_writing_merge_state() {
    let fixture = temp_path("merge-ff-only-conflict-fixture");
    fs::create_dir_all(&fixture).expect("fixture should be created");
    run_git(&fixture, ["init", "--quiet"]);
    run_git(&fixture, ["config", "user.name", "Rit Test"]);
    run_git(&fixture, ["config", "user.email", "rit@example.test"]);
    run_git(&fixture, ["config", "core.autocrlf", "false"]);
    fs::write(fixture.join("tracked.txt"), "base\n").expect("base file should be written");
    run_git(&fixture, ["add", "tracked.txt"]);
    run_git(&fixture, ["commit", "--quiet", "-m", "base"]);
    run_git(&fixture, ["checkout", "--quiet", "-b", "topic"]);
    fs::write(fixture.join("tracked.txt"), "topic\n").expect("topic file should be written");
    run_git(&fixture, ["add", "tracked.txt"]);
    run_git(&fixture, ["commit", "--quiet", "-m", "topic"]);
    run_git(&fixture, ["checkout", "--quiet", "master"]);
    fs::write(fixture.join("tracked.txt"), "master\n").expect("master file should be written");
    run_git(&fixture, ["add", "tracked.txt"]);
    run_git(&fixture, ["commit", "--quiet", "-m", "master"]);

    let output = Command::new(rit_binary())
        .args(["merge", "--ff-only", "topic"])
        .current_dir(&fixture)
        .output()
        .expect("rit merge should start");

    assert!(!output.status.success());
    assert!(!fixture.join(".git").join("MERGE_HEAD").exists());
    assert_eq!(run_capture(rit_binary(), ["op", "log"], &fixture).0, "");
    let _ = fs::remove_dir_all(fixture);
}

#[test]
fn merge_continue_commits_resolved_conflict() {
    let fixture = temp_path("merge-continue-fixture");
    fs::create_dir_all(&fixture).expect("fixture should be created");
    run_git(&fixture, ["init", "--quiet"]);
    run_git(&fixture, ["config", "user.name", "Rit Test"]);
    run_git(&fixture, ["config", "user.email", "rit@example.test"]);
    run_git(&fixture, ["config", "core.autocrlf", "false"]);
    fs::write(fixture.join("tracked.txt"), "base\n").expect("base file should be written");
    run_git(&fixture, ["add", "tracked.txt"]);
    run_git(&fixture, ["commit", "--quiet", "-m", "base"]);
    run_git(&fixture, ["checkout", "--quiet", "-b", "topic"]);
    fs::write(fixture.join("tracked.txt"), "topic\n").expect("topic file should be written");
    run_git(&fixture, ["add", "tracked.txt"]);
    run_git(&fixture, ["commit", "--quiet", "-m", "topic"]);
    run_git(&fixture, ["checkout", "--quiet", "master"]);
    fs::write(fixture.join("tracked.txt"), "master\n").expect("master file should be written");
    run_git(&fixture, ["add", "tracked.txt"]);
    run_git(&fixture, ["commit", "--quiet", "-m", "master"]);
    let topic = run_capture("git", ["rev-parse", "topic"], &fixture).0;
    let master = run_capture("git", ["rev-parse", "master"], &fixture).0;
    let merge = Command::new(rit_binary())
        .args(["merge", "topic"])
        .current_dir(&fixture)
        .output()
        .expect("rit merge should start");
    assert!(!merge.status.success());
    fs::write(fixture.join("tracked.txt"), "resolved\n").expect("resolution should be written");
    run_capture(rit_binary(), ["add", "tracked.txt"], &fixture);

    let output = run_capture(rit_binary(), ["merge", "--continue"], &fixture).0;

    assert!(output.contains("merge commit"));
    assert!(!fixture.join(".git").join("MERGE_HEAD").exists());
    assert_eq!(
        run_capture(rit_binary(), ["status", "--porcelain=v1"], &fixture).0,
        ""
    );
    let parents = run_capture(
        "git",
        ["rev-list", "--parents", "-n", "1", "HEAD"],
        &fixture,
    )
    .0;
    assert!(parents.contains(master.trim()));
    assert!(parents.contains(topic.trim()));
    let _ = fs::remove_dir_all(fixture);
}

#[test]
fn merge_creates_clean_non_fast_forward_commit() {
    let fixture = temp_path("merge-clean-non-ff-fixture");
    fs::create_dir_all(&fixture).expect("fixture should be created");
    run_git(&fixture, ["init", "--quiet"]);
    run_git(&fixture, ["config", "user.name", "Rit Test"]);
    run_git(&fixture, ["config", "user.email", "rit@example.test"]);
    run_git(&fixture, ["config", "core.autocrlf", "false"]);
    fs::write(fixture.join("a.txt"), "base\n").expect("base file should be written");
    run_git(&fixture, ["add", "a.txt"]);
    run_git(&fixture, ["commit", "--quiet", "-m", "base"]);
    run_git(&fixture, ["checkout", "--quiet", "-b", "topic"]);
    fs::write(fixture.join("a.txt"), "topic\n").expect("topic file should be written");
    run_git(&fixture, ["add", "a.txt"]);
    run_git(&fixture, ["commit", "--quiet", "-m", "topic"]);
    run_git(&fixture, ["checkout", "--quiet", "master"]);
    fs::write(fixture.join("b.txt"), "master\n").expect("master file should be written");
    run_git(&fixture, ["add", "b.txt"]);
    run_git(&fixture, ["commit", "--quiet", "-m", "master"]);
    let topic = run_capture("git", ["rev-parse", "topic"], &fixture).0;
    let master = run_capture("git", ["rev-parse", "master"], &fixture).0;

    let output = run_capture(rit_binary(), ["merge", "topic"], &fixture).0;

    assert!(output.contains("Merged "));
    assert_eq!(
        fs::read_to_string(fixture.join("a.txt")).unwrap(),
        "topic\n"
    );
    assert_eq!(
        fs::read_to_string(fixture.join("b.txt")).unwrap(),
        "master\n"
    );
    assert_eq!(
        run_capture(rit_binary(), ["status", "--porcelain=v1"], &fixture).0,
        ""
    );
    let parents = run_capture(
        "git",
        ["rev-list", "--parents", "-n", "1", "HEAD"],
        &fixture,
    )
    .0;
    assert!(parents.contains(master.trim()));
    assert!(parents.contains(topic.trim()));
    assert!(
        run_capture(rit_binary(), ["op", "log"], &fixture)
            .0
            .contains("merge")
    );
    let _ = fs::remove_dir_all(fixture);
}

#[test]
fn merge_combines_mode_only_and_content_only_changes_cleanly() {
    let fixture = temp_path("merge-mode-content-clean-fixture");
    fs::create_dir_all(&fixture).expect("fixture should be created");
    run_git(&fixture, ["init", "--quiet"]);
    run_git(&fixture, ["config", "user.name", "Rit Test"]);
    run_git(&fixture, ["config", "user.email", "rit@example.test"]);
    run_git(&fixture, ["config", "core.autocrlf", "false"]);
    run_git(&fixture, ["config", "core.filemode", "true"]);
    fs::write(fixture.join("script.sh"), "base\n").expect("base file should be written");
    run_git(&fixture, ["add", "script.sh"]);
    run_git(&fixture, ["commit", "--quiet", "-m", "base"]);
    run_git(&fixture, ["checkout", "--quiet", "-b", "topic"]);
    run_git(&fixture, ["update-index", "--chmod=+x", "script.sh"]);
    run_git(&fixture, ["commit", "--quiet", "-m", "mode"]);
    run_git(&fixture, ["checkout", "--force", "--quiet", "master"]);
    fs::write(fixture.join("script.sh"), "head\n").expect("head file should be written");
    run_git(&fixture, ["add", "script.sh"]);
    run_git(&fixture, ["commit", "--quiet", "-m", "content"]);

    let output = run_capture(rit_binary(), ["merge", "topic"], &fixture).0;

    assert!(output.contains("Merged "));
    assert!(!fixture.join(".git").join("MERGE_HEAD").exists());
    let tree_entry = run_capture("git", ["ls-tree", "HEAD", "script.sh"], &fixture).0;
    assert!(
        tree_entry.starts_with("100755 blob "),
        "tree entry should combine executable mode: {tree_entry:?}"
    );
    assert_eq!(
        run_capture("git", ["show", "HEAD:script.sh"], &fixture).0,
        "head\n"
    );
    assert!(
        run_capture(rit_binary(), ["op", "log"], &fixture)
            .0
            .contains("merge")
    );
    let _ = fs::remove_dir_all(fixture);
}

#[test]
fn merge_explain_prints_fast_forward_reason_without_changing_head() {
    let fixture = temp_path("merge-explain-fixture");
    fs::create_dir_all(&fixture).expect("fixture should be created");
    run_git(&fixture, ["init", "--quiet"]);
    run_git(&fixture, ["config", "user.name", "Rit Test"]);
    run_git(&fixture, ["config", "user.email", "rit@example.test"]);
    run_git(&fixture, ["config", "core.autocrlf", "false"]);
    fs::write(fixture.join("tracked.txt"), "base\n").expect("base file should be written");
    run_git(&fixture, ["add", "tracked.txt"]);
    run_git(&fixture, ["commit", "--quiet", "-m", "base"]);
    let base = run_capture("git", ["rev-parse", "HEAD"], &fixture).0;
    run_git(&fixture, ["checkout", "--quiet", "-b", "topic"]);
    fs::write(fixture.join("tracked.txt"), "topic\n").expect("topic file should be written");
    run_git(&fixture, ["add", "tracked.txt"]);
    run_git(&fixture, ["commit", "--quiet", "-m", "topic"]);
    run_git(&fixture, ["checkout", "--quiet", "master"]);

    let output = run_capture(rit_binary(), ["merge", "explain", "topic"], &fixture).0;

    assert!(output.contains("merge: explain\n"));
    assert!(output.contains("target: topic\n"));
    assert!(output.contains("action: fast-forward\n"));
    assert!(output.contains("reason: HEAD is an ancestor of the target commit\n"));
    assert!(output.contains("update: tracked.txt\n"));
    assert_eq!(run_capture("git", ["rev-parse", "HEAD"], &fixture).0, base);
    assert_eq!(
        fs::read_to_string(fixture.join("tracked.txt")).expect("file should read"),
        "base\n"
    );
    assert_eq!(run_capture(rit_binary(), ["op", "log"], &fixture).0, "");
    let _ = fs::remove_dir_all(fixture);
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

    let git_output = Command::new(git_program())
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

    let git_output = Command::new(git_program())
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

    let git_output = Command::new(git_program())
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
    stdin: Option<Vec<u8>>,
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
    let program = normalize_test_program(program.into());
    CommandSpec {
        program,
        args: args.into_iter().map(OsString::from).collect(),
        env: Vec::new(),
        stdin: None,
    }
}

fn command_words_with_stdin<const N: usize>(
    program: impl Into<OsString>,
    args: [&str; N],
    stdin: &[u8],
) -> CommandSpec {
    let program = normalize_test_program(program.into());
    CommandSpec {
        program,
        args: args.into_iter().map(OsString::from).collect(),
        env: Vec::new(),
        stdin: Some(stdin.to_vec()),
    }
}

fn command_words_with_env<const N: usize, const M: usize>(
    program: impl Into<OsString>,
    args: [&str; N],
    env: &[(&str, &str); M],
) -> CommandSpec {
    let program = normalize_test_program(program.into());
    CommandSpec {
        program,
        args: args.into_iter().map(OsString::from).collect(),
        env: env
            .iter()
            .map(|(name, value)| (OsString::from(name), OsString::from(value)))
            .collect(),
        stdin: None,
    }
}

fn run_command(spec: &CommandSpec, cwd: &Path) -> (String, String) {
    let mut command = Command::new(&spec.program);
    command.args(&spec.args).current_dir(cwd);
    for (name, value) in &spec.env {
        command.env(name, value);
    }
    if spec.stdin.is_some() {
        command.stdin(Stdio::piped());
    }
    let mut child = command.spawn().expect("command should start");
    if let Some(stdin) = &spec.stdin {
        child
            .stdin
            .take()
            .expect("stdin should be piped")
            .write_all(stdin)
            .expect("stdin should be written");
    }
    let output = child.wait_with_output().expect("command should finish");
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
    if spec.stdin.is_some() {
        command.stdin(Stdio::piped());
    }
    let mut child = command.spawn().expect("command should start");
    if let Some(stdin) = &spec.stdin {
        child
            .stdin
            .take()
            .expect("stdin should be piped")
            .write_all(stdin)
            .expect("stdin should be written");
    }
    let output = child.wait_with_output().expect("command should finish");
    CommandRun {
        success: output.status.success(),
    }
}

fn run_capture<const N: usize>(
    program: impl AsRef<OsStr>,
    args: [&str; N],
    cwd: &Path,
) -> (String, String) {
    let program = normalize_test_program(program.as_ref().to_os_string());
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
    let output = Command::new(git_program())
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

fn write_git_blob_from_stdin(cwd: &Path, contents: &[u8]) -> String {
    let mut child = Command::new(git_program())
        .args(["hash-object", "-w", "--stdin"])
        .current_dir(cwd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("git hash-object should start");
    child
        .stdin
        .as_mut()
        .expect("hash stdin should open")
        .write_all(contents)
        .expect("hash stdin should write");
    let output = child
        .wait_with_output()
        .expect("git hash-object should finish");
    assert!(output.status.success());
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

fn normalize_test_program(program: OsString) -> OsString {
    if program == OsStr::new("git") {
        git_program()
    } else {
        program
    }
}

fn git_program() -> OsString {
    static GIT_PROGRAM: OnceLock<OsString> = OnceLock::new();
    GIT_PROGRAM.get_or_init(discover_git_program).clone()
}

fn discover_git_program() -> OsString {
    let locator = if cfg!(windows) { "where.exe" } else { "which" };
    if let Ok(output) = Command::new(locator).arg("git").output()
        && output.status.success()
    {
        let stdout = String::from_utf8_lossy(&output.stdout);
        if let Some(path) = stdout.lines().find(|line| !line.trim().is_empty()) {
            return OsString::from(path.trim());
        }
    }
    OsString::from("git")
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
