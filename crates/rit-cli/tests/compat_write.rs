use rit_testkit::{LocalWriteFixture, LocalWriteFixtureKind};
use std::ffi::{OsStr, OsString};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn init_initial_branch_equals_form_matches_git_head() {
    let workspace = temp_path("init-initial-branch-equals");
    let git_target = workspace.join("git-target");
    let rit_target = workspace.join("rit-target");
    fs::create_dir_all(&workspace).expect("workspace should be created");

    let git_output = Command::new(git_program())
        .args(["init", "-q", "--initial-branch=topic"])
        .arg(&git_target)
        .output()
        .expect("git init should start");
    assert!(
        git_output.status.success(),
        "git init failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&git_output.stdout),
        String::from_utf8_lossy(&git_output.stderr)
    );

    let rit_output = Command::new(rit_binary())
        .args(["init", "-q", "--initial-branch=topic"])
        .arg(&rit_target)
        .output()
        .expect("rit init should start");
    assert!(
        rit_output.status.success(),
        "rit init failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&rit_output.stdout),
        String::from_utf8_lossy(&rit_output.stderr)
    );
    assert_eq!(git_output.stdout, rit_output.stdout);
    assert_eq!(git_output.stderr, rit_output.stderr);

    let git_head = fs::read_to_string(git_target.join(".git").join("HEAD"))
        .expect("git HEAD should be readable");
    let rit_head = fs::read_to_string(rit_target.join(".git").join("HEAD"))
        .expect("rit HEAD should be readable");
    assert_eq!(git_head, "ref: refs/heads/topic\n");
    assert_eq!(git_head, rit_head);

    let _ = fs::remove_dir_all(workspace);
}

#[test]
fn init_no_option_forms_match_git_state() {
    let workspace = temp_path("init-no-option-forms");
    fs::create_dir_all(&workspace).expect("workspace should be created");

    let no_bare_git = workspace.join("git-no-bare");
    let no_bare_rit = workspace.join("rit-no-bare");
    let git_output = Command::new(git_program())
        .args(["init", "-q", "--bare", "--no-bare"])
        .arg(&no_bare_git)
        .output()
        .expect("git init should start");
    assert!(git_output.status.success());
    let rit_output = Command::new(rit_binary())
        .args(["init", "-q", "--bare", "--no-bare"])
        .arg(&no_bare_rit)
        .output()
        .expect("rit init should start");
    assert!(rit_output.status.success());
    assert_eq!(git_output.stdout, rit_output.stdout);
    assert_eq!(git_output.stderr, rit_output.stderr);
    assert_eq!(
        fs::read_to_string(no_bare_git.join(".git").join("HEAD"))
            .expect("git HEAD should be readable"),
        fs::read_to_string(no_bare_rit.join(".git").join("HEAD"))
            .expect("rit HEAD should be readable")
    );

    let no_initial_git = workspace.join("git-no-initial");
    let no_initial_rit = workspace.join("rit-no-initial");
    let git_output = Command::new(git_program())
        .args([
            "init",
            "-q",
            "--initial-branch=topic",
            "--no-initial-branch",
        ])
        .arg(&no_initial_git)
        .output()
        .expect("git init should start");
    assert!(git_output.status.success());
    let rit_output = Command::new(rit_binary())
        .args([
            "init",
            "-q",
            "--initial-branch=topic",
            "--no-initial-branch",
        ])
        .arg(&no_initial_rit)
        .output()
        .expect("rit init should start");
    assert!(rit_output.status.success());
    assert_eq!(git_output.stdout, rit_output.stdout);
    assert_eq!(git_output.stderr, rit_output.stderr);
    let git_head = fs::read_to_string(no_initial_git.join(".git").join("HEAD"))
        .expect("git HEAD should be readable");
    let rit_head = fs::read_to_string(no_initial_rit.join(".git").join("HEAD"))
        .expect("rit HEAD should be readable");
    assert_eq!(git_head, "ref: refs/heads/master\n");
    assert_eq!(git_head, rit_head);

    let no_quiet_rit = workspace.join("rit-no-quiet");
    let rit_output = Command::new(rit_binary())
        .args(["init", "-q", "--no-quiet"])
        .arg(&no_quiet_rit)
        .output()
        .expect("rit init should start");
    assert!(rit_output.status.success());
    assert!(!rit_output.stdout.is_empty());
    assert!(rit_output.stderr.is_empty());

    let _ = fs::remove_dir_all(workspace);
}

#[test]
fn init_default_format_options_match_git_state() {
    let workspace = temp_path("init-default-format-options");
    fs::create_dir_all(&workspace).expect("workspace should be created");

    for (name, args) in [
        ("object-equals", vec!["--object-format=sha1"]),
        ("object-value", vec!["--object-format", "sha1"]),
        (
            "object-reset",
            vec!["--object-format=sha256", "--no-object-format"],
        ),
        ("ref-equals", vec!["--ref-format=files"]),
        ("ref-value", vec!["--ref-format", "files"]),
        (
            "ref-reset",
            vec!["--ref-format=reftable", "--no-ref-format"],
        ),
    ] {
        let git_target = workspace.join(format!("git-{name}"));
        let rit_target = workspace.join(format!("rit-{name}"));

        let mut git_args = vec!["init", "-q"];
        git_args.extend(args.iter().copied());
        let git_output = Command::new(git_program())
            .args(&git_args)
            .arg(&git_target)
            .output()
            .expect("git init should start");
        assert!(
            git_output.status.success(),
            "git init failed for {name}\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&git_output.stdout),
            String::from_utf8_lossy(&git_output.stderr)
        );

        let mut rit_args = vec!["init", "-q"];
        rit_args.extend(args.iter().copied());
        let rit_output = Command::new(rit_binary())
            .args(&rit_args)
            .arg(&rit_target)
            .output()
            .expect("rit init should start");
        assert!(
            rit_output.status.success(),
            "rit init failed for {name}\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&rit_output.stdout),
            String::from_utf8_lossy(&rit_output.stderr)
        );
        assert_eq!(git_output.stdout, rit_output.stdout);
        assert_eq!(git_output.stderr, rit_output.stderr);

        let git_repository_format = run_capture(
            "git",
            ["config", "--get", "core.repositoryformatversion"],
            &git_target,
        )
        .0;
        let rit_repository_format = run_capture(
            "git",
            ["config", "--get", "core.repositoryformatversion"],
            &rit_target,
        )
        .0;
        assert_eq!(git_repository_format, "0\n");
        assert_eq!(git_repository_format, rit_repository_format);

        let git_config = fs::read_to_string(git_target.join(".git").join("config"))
            .expect("git config should be readable");
        let rit_config = fs::read_to_string(rit_target.join(".git").join("config"))
            .expect("rit config should be readable");
        assert!(!git_config.contains("objectformat"));
        assert!(!git_config.contains("refstorage"));
        assert!(!rit_config.contains("objectformat"));
        assert!(!rit_config.contains("refstorage"));
    }

    let _ = fs::remove_dir_all(workspace);
}

#[test]
fn init_no_template_resets_template_selection_like_git() {
    let workspace = temp_path("init-no-template");
    fs::create_dir_all(&workspace).expect("workspace should be created");

    for (name, args) in [
        ("no-template", vec!["--no-template"]),
        (
            "template-reset",
            vec!["--template", "missing-template", "--no-template"],
        ),
        (
            "template-equals-reset",
            vec!["--template=missing-template", "--no-template"],
        ),
    ] {
        let git_target = workspace.join(format!("git-{name}"));
        let rit_target = workspace.join(format!("rit-{name}"));

        let mut git_args = vec!["init", "-q"];
        git_args.extend(args.iter().copied());
        let git_output = Command::new(git_program())
            .args(&git_args)
            .arg(&git_target)
            .output()
            .expect("git init should start");
        assert!(
            git_output.status.success(),
            "git init failed for {name}\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&git_output.stdout),
            String::from_utf8_lossy(&git_output.stderr)
        );

        let mut rit_args = vec!["init", "-q"];
        rit_args.extend(args.iter().copied());
        let rit_output = Command::new(rit_binary())
            .args(&rit_args)
            .arg(&rit_target)
            .output()
            .expect("rit init should start");
        assert!(
            rit_output.status.success(),
            "rit init failed for {name}\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&rit_output.stdout),
            String::from_utf8_lossy(&rit_output.stderr)
        );
        assert_eq!(git_output.stdout, rit_output.stdout);
        assert_eq!(git_output.stderr, rit_output.stderr);
        assert_eq!(
            fs::read_to_string(git_target.join(".git").join("HEAD"))
                .expect("git HEAD should be readable"),
            fs::read_to_string(rit_target.join(".git").join("HEAD"))
                .expect("rit HEAD should be readable")
        );
    }

    let _ = fs::remove_dir_all(workspace);
}

#[test]
fn branch_list_option_matches_default_branch_list() {
    let fixture =
        LocalWriteFixture::new("branch-list-option", LocalWriteFixtureKind::NestedTracked)
            .expect("fixture should build");
    run_git(fixture.path(), ["branch", "topic"]);

    let git_default = run_capture("git", ["branch"], fixture.path()).0;
    let git_list = run_capture("git", ["branch", "--list"], fixture.path()).0;
    let rit_list = run_capture(rit_binary(), ["branch", "--list"], fixture.path()).0;

    assert_eq!(git_list, git_default);
    assert_eq!(rit_list, git_list);
}

#[test]
fn branch_short_list_option_matches_git_branch_list() {
    let fixture = LocalWriteFixture::new(
        "branch-short-list-option",
        LocalWriteFixtureKind::NestedTracked,
    )
    .expect("fixture should build");
    run_git(fixture.path(), ["branch", "topic"]);

    let git_output = run_capture("git", ["branch", "-l"], fixture.path()).0;
    let rit_output = run_capture(rit_binary(), ["branch", "-l"], fixture.path()).0;

    assert_eq!(rit_output, git_output);
}

#[test]
fn branch_list_patterns_match_git_branch_names() {
    let fixture =
        LocalWriteFixture::new("branch-list-patterns", LocalWriteFixtureKind::NestedTracked)
            .expect("fixture should build");
    run_git(fixture.path(), ["branch", "topic-one"]);
    run_git(fixture.path(), ["branch", "topic/two"]);
    run_git(fixture.path(), ["branch", "feature/one"]);
    run_git(fixture.path(), ["branch", "release"]);

    let git_output = run_capture(
        "git",
        ["branch", "--list", "topic*", "*/one", "release"],
        fixture.path(),
    )
    .0;
    let rit_output = run_capture(
        rit_binary(),
        ["branch", "--list", "topic*", "*/one", "release"],
        fixture.path(),
    )
    .0;

    assert_eq!(rit_output, git_output);
}

#[test]
fn branch_short_list_patterns_match_git_branch_names() {
    let fixture = LocalWriteFixture::new(
        "branch-short-list-patterns",
        LocalWriteFixtureKind::NestedTracked,
    )
    .expect("fixture should build");
    run_git(fixture.path(), ["branch", "topic-one"]);
    run_git(fixture.path(), ["branch", "topic/two"]);
    run_git(fixture.path(), ["branch", "feature/one"]);
    run_git(fixture.path(), ["branch", "release"]);

    let git_output = run_capture(
        "git",
        ["branch", "-l", "topic*", "*/one", "release"],
        fixture.path(),
    )
    .0;
    let rit_output = run_capture(
        rit_binary(),
        ["branch", "-l", "topic*", "*/one", "release"],
        fixture.path(),
    )
    .0;

    assert_eq!(rit_output, git_output);
}

#[test]
fn branch_delete_multiple_names_matches_git_branches() {
    let fixture = LocalWriteFixture::new(
        "branch-delete-multiple",
        LocalWriteFixtureKind::NestedTracked,
    )
    .expect("fixture should build");
    run_git(fixture.path(), ["branch", "one"]);
    run_git(fixture.path(), ["branch", "two"]);
    run_git(fixture.path(), ["branch", "three"]);

    let workspace = temp_path("branch-delete-multiple");
    let git_repo = workspace.join("git");
    let rit_repo = workspace.join("rit");
    copy_directory(fixture.path(), &git_repo);
    copy_directory(fixture.path(), &rit_repo);

    let git_output = run_capture("git", ["branch", "-d", "one", "two"], &git_repo);
    let rit_output = run_capture(rit_binary(), ["branch", "-d", "one", "two"], &rit_repo);

    assert_eq!(rit_output, git_output);
    let rit_branches = run_capture("git", ["branch", "--list"], &rit_repo).0;
    assert!(rit_branches.contains("  three\n"));
    assert!(!rit_branches.contains("  one\n"));
    assert!(!rit_branches.contains("  two\n"));
    assert_eq!(
        rit_branches,
        run_capture("git", ["branch", "--list"], &git_repo).0
    );
    let _ = fs::remove_dir_all(workspace);
}

#[test]
fn branch_force_delete_unmerged_branch_matches_git() {
    let fixture = LocalWriteFixture::new(
        "branch-force-delete-unmerged",
        LocalWriteFixtureKind::UnmergedBranch,
    )
    .expect("fixture should build");
    let workspace = temp_path("branch-force-delete-unmerged");
    let git_repo = workspace.join("git");
    let rit_repo = workspace.join("rit");
    copy_directory(fixture.path(), &git_repo);
    copy_directory(fixture.path(), &rit_repo);

    let git_output = run_capture("git", ["branch", "-D", "topic"], &git_repo);
    let rit_output = run_capture(rit_binary(), ["branch", "-D", "topic"], &rit_repo);

    assert_eq!(rit_output, git_output);
    assert!(
        !rit_repo
            .join(".git")
            .join("refs")
            .join("heads")
            .join("topic")
            .exists()
    );
    assert_eq!(
        run_capture("git", ["branch", "--list"], &rit_repo).0,
        run_capture("git", ["branch", "--list"], &git_repo).0
    );
    let _ = fs::remove_dir_all(workspace);
}

#[test]
fn tag_list_option_matches_git_tag_list() {
    let fixture = LocalWriteFixture::new("tag-list-option", LocalWriteFixtureKind::NestedTracked)
        .expect("fixture should build");
    run_git(fixture.path(), ["tag", "v1.0"]);
    run_git(fixture.path(), ["tag", "release/one"]);

    let git_default = run_capture("git", ["tag"], fixture.path()).0;
    let git_list = run_capture("git", ["tag", "--list"], fixture.path()).0;
    let rit_list = run_capture(rit_binary(), ["tag", "--list"], fixture.path()).0;

    assert_eq!(git_list, git_default);
    assert_eq!(rit_list, git_list);
}

#[test]
fn tag_list_patterns_match_git_tag_names() {
    let fixture = LocalWriteFixture::new("tag-list-patterns", LocalWriteFixtureKind::NestedTracked)
        .expect("fixture should build");
    run_git(fixture.path(), ["tag", "v1.0"]);
    run_git(fixture.path(), ["tag", "v2.0"]);
    run_git(fixture.path(), ["tag", "release/one"]);
    run_git(fixture.path(), ["tag", "feature-one"]);

    let git_output = run_capture(
        "git",
        ["tag", "--list", "v*", "release/*", "*one"],
        fixture.path(),
    )
    .0;
    let rit_output = run_capture(
        rit_binary(),
        ["tag", "--list", "v*", "release/*", "*one"],
        fixture.path(),
    )
    .0;

    assert_eq!(rit_output, git_output);
}

#[test]
fn tag_short_list_patterns_match_git_tag_names() {
    let fixture = LocalWriteFixture::new(
        "tag-short-list-patterns",
        LocalWriteFixtureKind::NestedTracked,
    )
    .expect("fixture should build");
    run_git(fixture.path(), ["tag", "v1.0"]);
    run_git(fixture.path(), ["tag", "v2.0"]);
    run_git(fixture.path(), ["tag", "release/one"]);
    run_git(fixture.path(), ["tag", "feature-one"]);

    let git_output = run_capture(
        "git",
        ["tag", "-l", "v*", "release/*", "*one"],
        fixture.path(),
    )
    .0;
    let rit_output = run_capture(
        rit_binary(),
        ["tag", "-l", "v*", "release/*", "*one"],
        fixture.path(),
    )
    .0;

    assert_eq!(rit_output, git_output);
}

#[test]
fn tag_delete_multiple_names_matches_git_tags() {
    let fixture =
        LocalWriteFixture::new("tag-delete-multiple", LocalWriteFixtureKind::NestedTracked)
            .expect("fixture should build");
    run_git(fixture.path(), ["tag", "one"]);
    run_git(fixture.path(), ["tag", "two"]);
    run_git(fixture.path(), ["tag", "three"]);

    let workspace = temp_path("tag-delete-multiple");
    let git_repo = workspace.join("git");
    let rit_repo = workspace.join("rit");
    copy_directory(fixture.path(), &git_repo);
    copy_directory(fixture.path(), &rit_repo);

    let git_output = run_capture("git", ["tag", "-d", "one", "two"], &git_repo);
    let rit_output = run_capture(rit_binary(), ["tag", "-d", "one", "two"], &rit_repo);

    assert_eq!(rit_output, git_output);
    assert_eq!(
        run_capture("git", ["tag", "--list"], &rit_repo).0,
        "three\n"
    );
    assert_eq!(
        run_capture("git", ["tag", "--list"], &rit_repo).0,
        run_capture("git", ["tag", "--list"], &git_repo).0
    );
    let _ = fs::remove_dir_all(workspace);
}

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

#[cfg(feature = "indexdb")]
#[test]
fn file_history_command_prints_indexed_path_changes() {
    let fixture =
        LocalWriteFixture::new("file-history-command", LocalWriteFixtureKind::NestedTracked)
            .expect("fixture should build");
    std::thread::sleep(std::time::Duration::from_secs(1));
    fs::write(
        fixture.path().join("nested").join("tracked.txt"),
        "changed\n",
    )
    .expect("tracked file should be modified");

    run_capture(rit_binary(), ["add", "nested/tracked.txt"], fixture.path());
    run_capture(rit_binary(), ["commit", "-m", "changed"], fixture.path());
    let (stdout, stderr) = run_capture(
        rit_binary(),
        ["file-history", "nested/tracked.txt"],
        fixture.path(),
    );
    let lines = stdout.lines().collect::<Vec<_>>();

    assert_eq!(stderr, "");
    assert_eq!(lines.len(), 3);
    assert_eq!(lines[0], "file-history: nested/tracked.txt");
    assert!(lines[1].contains(" M 100644 "));
    assert!(lines[1].ends_with(" nested/tracked.txt"));
    assert!(lines[2].contains(" A 100644 "));
    assert!(lines[2].ends_with(" nested/tracked.txt"));
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
fn write_glob_magic_double_star_component_matches_git_status() {
    for command in ["add", "restore", "reset"] {
        let fixture = LocalWriteFixture::new(
            &format!("{command}-glob-double-star-component"),
            LocalWriteFixtureKind::NestedTracked,
        )
        .expect("fixture should build");
        fs::write(
            fixture.path().join("nested").join("tracked.txt"),
            "changed\n",
        )
        .expect("tracked file should be modified");
        if command == "reset" {
            run_git(fixture.path(), ["add", "nested/tracked.txt"]);
        }

        let workspace = temp_path(&format!("{command}-glob-double-star-component-compare"));
        let git_repo = workspace.join("git");
        let rit_repo = workspace.join("rit");
        copy_directory(fixture.path(), &git_repo);
        copy_directory(fixture.path(), &rit_repo);

        let git = run_command_allow_failure(
            &command_words("git", [command, ":(glob)**tracked.txt"]),
            &git_repo,
        );
        let rit = run_command_allow_failure(
            &command_words(rit_binary(), [command, ":(glob)**tracked.txt"]),
            &rit_repo,
        );

        assert_eq!(rit.exit_code, git.exit_code, "{command} exit code");
        assert_eq!(rit.stdout, git.stdout, "{command} stdout");
        assert_eq!(rit.stderr, git.stderr, "{command} stderr");
        assert_eq!(
            run_capture("git", ["status", "--porcelain=v1"], &git_repo).0,
            run_capture(rit_binary(), ["status", "--porcelain=v1"], &rit_repo).0,
            "{command} status"
        );
        let _ = fs::remove_dir_all(workspace);
    }
}

#[test]
fn write_glob_magic_special_double_star_forms_match_git_state() {
    for (case_name, pathspec) in [
        ("double-star-slash", ":(glob)**/*.txt"),
        ("trailing-double-star", ":(glob)nested/**"),
    ] {
        for command in ["add", "restore", "reset"] {
            let fixture = build_write_glob_special_form_fixture(&format!("{command}-{case_name}"));
            if command == "reset" {
                run_git(fixture.path(), ["add", "top.txt", "nested"]);
            }

            let outcome = compare_after_command(
                fixture.path(),
                command_words("git", [command, pathspec]),
                command_words(rit_binary(), [command, pathspec]),
            );

            assert_eq!(outcome.git_command_stdout, outcome.rit_command_stdout);
            assert_eq!(outcome.git_command_stderr, outcome.rit_command_stderr);
            assert_eq!(outcome.git_status, outcome.rit_status);
            assert_matching_file_contents(
                &outcome.git_repo.join("top.txt"),
                &outcome.rit_repo.join("top.txt"),
            );
            assert_matching_file_contents(
                &outcome.git_repo.join("nested").join("tracked.txt"),
                &outcome.rit_repo.join("nested").join("tracked.txt"),
            );
            assert_matching_file_contents(
                &outcome
                    .git_repo
                    .join("nested")
                    .join("deep")
                    .join("inner.txt"),
                &outcome
                    .rit_repo
                    .join("nested")
                    .join("deep")
                    .join("inner.txt"),
            );
            assert_matching_file_contents(
                &outcome.git_repo.join("nested").join("skip.md"),
                &outcome.rit_repo.join("nested").join("skip.md"),
            );
        }
    }
}

#[test]
fn write_glob_magic_component_local_base_matches_git_state() {
    for command in ["add", "restore", "reset"] {
        let fixture = build_write_glob_component_local_fixture(&format!("{command}-glob-base"));
        if command == "reset" {
            run_git(
                fixture.path(),
                [
                    "add",
                    "topbase.txt",
                    "nested/base.txt",
                    "nested/deep/innerbase.txt",
                ],
            );
        }

        let outcome = compare_after_command(
            fixture.path(),
            command_words("git", [command, ":(glob)**base.txt"]),
            command_words(rit_binary(), [command, ":(glob)**base.txt"]),
        );

        assert_eq!(outcome.git_command_stdout, outcome.rit_command_stdout);
        assert_eq!(outcome.git_command_stderr, outcome.rit_command_stderr);
        assert_eq!(outcome.git_status, outcome.rit_status);
        assert_matching_file_contents(
            &outcome.git_repo.join("topbase.txt"),
            &outcome.rit_repo.join("topbase.txt"),
        );
        assert_matching_file_contents(
            &outcome.git_repo.join("nested").join("base.txt"),
            &outcome.rit_repo.join("nested").join("base.txt"),
        );
        assert_matching_file_contents(
            &outcome
                .git_repo
                .join("nested")
                .join("deep")
                .join("innerbase.txt"),
            &outcome
                .rit_repo
                .join("nested")
                .join("deep")
                .join("innerbase.txt"),
        );
    }
}

#[test]
fn write_commands_resolve_pathspecs_relative_to_subdirectory_like_git() {
    for command in ["add", "restore", "reset"] {
        let fixture = LocalWriteFixture::new(
            &format!("{command}-subdirectory-relative-pathspec"),
            LocalWriteFixtureKind::NestedTracked,
        )
        .expect("fixture should build");
        fs::write(
            fixture.path().join("nested").join("tracked.txt"),
            "changed\n",
        )
        .expect("tracked file should be modified");
        if command == "reset" {
            run_git(fixture.path(), ["add", "nested/tracked.txt"]);
        }

        let workspace = temp_path(&format!("{command}-subdirectory-pathspec-compare"));
        let git_repo = workspace.join("git");
        let rit_repo = workspace.join("rit");
        copy_directory(fixture.path(), &git_repo);
        copy_directory(fixture.path(), &rit_repo);

        let git_output = run_command(
            &command_words("git", [command, "tracked.txt"]),
            &git_repo.join("nested"),
        );
        let rit_output = run_command(
            &command_words(rit_binary(), [command, "tracked.txt"]),
            &rit_repo.join("nested"),
        );

        assert_eq!(rit_output.0, git_output.0, "{command} stdout");
        assert_eq!(rit_output.1, git_output.1, "{command} stderr");
        assert_eq!(
            run_capture("git", ["status", "--porcelain=v1"], &git_repo).0,
            run_capture(rit_binary(), ["status", "--porcelain=v1"], &rit_repo).0,
            "{command} status"
        );
        let _ = fs::remove_dir_all(workspace);
    }
}

#[test]
fn write_commands_resolve_pathspecs_from_file_relative_to_subdirectory_like_git() {
    for command in ["add", "restore", "reset"] {
        let fixture = LocalWriteFixture::new(
            &format!("{command}-subdirectory-pathspec-file"),
            LocalWriteFixtureKind::NestedTracked,
        )
        .expect("fixture should build");
        fs::write(fixture.path().join("top.txt"), "top base\n")
            .expect("top file should be written");
        run_git(fixture.path(), ["add", "top.txt"]);
        run_git(
            fixture.path(),
            ["commit", "--quiet", "-m", "add top-level file"],
        );
        fs::write(fixture.path().join("top.txt"), "top changed\n").expect("top file should change");
        fs::write(
            fixture.path().join("nested").join("tracked.txt"),
            "changed\n",
        )
        .expect("tracked file should be modified");
        fs::write(
            fixture.path().join("nested").join("pathspecs.txt"),
            "tracked.txt\n:(top)top.txt\n",
        )
        .expect("pathspec file should be written");
        if command == "reset" {
            run_git(fixture.path(), ["add", "top.txt", "nested/tracked.txt"]);
        }

        let workspace = temp_path(&format!("{command}-subdirectory-pathspec-file-compare"));
        let git_repo = workspace.join("git");
        let rit_repo = workspace.join("rit");
        copy_directory(fixture.path(), &git_repo);
        copy_directory(fixture.path(), &rit_repo);

        let git_output = run_command(
            &command_words("git", [command, "--pathspec-from-file", "pathspecs.txt"]),
            &git_repo.join("nested"),
        );
        let rit_output = run_command(
            &command_words(
                rit_binary(),
                [command, "--pathspec-from-file", "pathspecs.txt"],
            ),
            &rit_repo.join("nested"),
        );

        assert_eq!(rit_output.0, git_output.0, "{command} stdout");
        assert_eq!(rit_output.1, git_output.1, "{command} stderr");
        assert_eq!(
            run_capture("git", ["status", "--porcelain=v1"], &git_repo).0,
            run_capture(rit_binary(), ["status", "--porcelain=v1"], &rit_repo).0,
            "{command} status"
        );
        assert_matching_file_contents(&git_repo.join("top.txt"), &rit_repo.join("top.txt"));
        assert_matching_file_contents(
            &git_repo.join("nested").join("tracked.txt"),
            &rit_repo.join("nested").join("tracked.txt"),
        );
        let _ = fs::remove_dir_all(workspace);
    }
}

#[test]
fn write_commands_resolve_pathspecs_from_stdin_relative_to_subdirectory_like_git() {
    for command in ["add", "restore", "reset"] {
        let fixture = LocalWriteFixture::new(
            &format!("{command}-subdirectory-pathspec-stdin"),
            LocalWriteFixtureKind::NestedTracked,
        )
        .expect("fixture should build");
        fs::write(fixture.path().join("top.txt"), "top base\n")
            .expect("top file should be written");
        run_git(fixture.path(), ["add", "top.txt"]);
        run_git(
            fixture.path(),
            ["commit", "--quiet", "-m", "add top-level file"],
        );
        fs::write(fixture.path().join("top.txt"), "top changed\n").expect("top file should change");
        fs::write(
            fixture.path().join("nested").join("tracked.txt"),
            "changed\n",
        )
        .expect("tracked file should be modified");
        if command == "reset" {
            run_git(fixture.path(), ["add", "top.txt", "nested/tracked.txt"]);
        }

        let workspace = temp_path(&format!("{command}-subdirectory-pathspec-stdin-compare"));
        let git_repo = workspace.join("git");
        let rit_repo = workspace.join("rit");
        copy_directory(fixture.path(), &git_repo);
        copy_directory(fixture.path(), &rit_repo);
        let stdin = b"tracked.txt\n:(top)top.txt\n";

        let git_output = run_command(
            &command_words_with_stdin("git", [command, "--pathspec-from-file", "-"], stdin),
            &git_repo.join("nested"),
        );
        let rit_output = run_command(
            &command_words_with_stdin(rit_binary(), [command, "--pathspec-from-file", "-"], stdin),
            &rit_repo.join("nested"),
        );

        assert_eq!(rit_output.0, git_output.0, "{command} stdout");
        assert_eq!(rit_output.1, git_output.1, "{command} stderr");
        assert_eq!(
            run_capture("git", ["status", "--porcelain=v1"], &git_repo).0,
            run_capture(rit_binary(), ["status", "--porcelain=v1"], &rit_repo).0,
            "{command} status"
        );
        assert_matching_file_contents(&git_repo.join("top.txt"), &rit_repo.join("top.txt"));
        assert_matching_file_contents(
            &git_repo.join("nested").join("tracked.txt"),
            &rit_repo.join("nested").join("tracked.txt"),
        );
        let _ = fs::remove_dir_all(workspace);
    }
}

#[test]
fn write_commands_resolve_nul_pathspecs_from_stdin_relative_to_subdirectory_like_git() {
    for command in ["add", "restore", "reset"] {
        let fixture = LocalWriteFixture::new(
            &format!("{command}-subdirectory-pathspec-stdin-nul"),
            LocalWriteFixtureKind::NestedTracked,
        )
        .expect("fixture should build");
        fs::write(fixture.path().join("top.txt"), "top base\n")
            .expect("top file should be written");
        run_git(fixture.path(), ["add", "top.txt"]);
        run_git(
            fixture.path(),
            ["commit", "--quiet", "-m", "add top-level file"],
        );
        fs::write(fixture.path().join("top.txt"), "top changed\n").expect("top file should change");
        fs::write(
            fixture.path().join("nested").join("tracked.txt"),
            "changed\n",
        )
        .expect("tracked file should be modified");
        if command == "reset" {
            run_git(fixture.path(), ["add", "top.txt", "nested/tracked.txt"]);
        }

        let workspace = temp_path(&format!(
            "{command}-subdirectory-pathspec-stdin-nul-compare"
        ));
        let git_repo = workspace.join("git");
        let rit_repo = workspace.join("rit");
        copy_directory(fixture.path(), &git_repo);
        copy_directory(fixture.path(), &rit_repo);
        let stdin = b"tracked.txt\0:(top)top.txt\0";

        let git_output = run_command(
            &command_words_with_stdin(
                "git",
                [command, "--pathspec-from-file", "-", "--pathspec-file-nul"],
                stdin,
            ),
            &git_repo.join("nested"),
        );
        let rit_output = run_command(
            &command_words_with_stdin(
                rit_binary(),
                [command, "--pathspec-from-file", "-", "--pathspec-file-nul"],
                stdin,
            ),
            &rit_repo.join("nested"),
        );

        assert_eq!(rit_output.0, git_output.0, "{command} stdout");
        assert_eq!(rit_output.1, git_output.1, "{command} stderr");
        assert_eq!(
            run_capture("git", ["status", "--porcelain=v1"], &git_repo).0,
            run_capture(rit_binary(), ["status", "--porcelain=v1"], &rit_repo).0,
            "{command} status"
        );
        assert_matching_file_contents(&git_repo.join("top.txt"), &rit_repo.join("top.txt"));
        assert_matching_file_contents(
            &git_repo.join("nested").join("tracked.txt"),
            &rit_repo.join("nested").join("tracked.txt"),
        );
        let _ = fs::remove_dir_all(workspace);
    }
}

#[test]
fn write_commands_resolve_nul_pathspecs_from_file_relative_to_subdirectory_like_git() {
    for command in ["add", "restore", "reset"] {
        let fixture = LocalWriteFixture::new(
            &format!("{command}-subdirectory-pathspec-file-nul"),
            LocalWriteFixtureKind::NestedTracked,
        )
        .expect("fixture should build");
        fs::write(fixture.path().join("top.txt"), "top base\n")
            .expect("top file should be written");
        run_git(fixture.path(), ["add", "top.txt"]);
        run_git(
            fixture.path(),
            ["commit", "--quiet", "-m", "add top-level file"],
        );
        fs::write(fixture.path().join("top.txt"), "top changed\n").expect("top file should change");
        fs::write(
            fixture.path().join("nested").join("tracked.txt"),
            "changed\n",
        )
        .expect("tracked file should be modified");
        fs::write(
            fixture.path().join("nested").join("pathspecs.nul"),
            b"tracked.txt\0:(top)top.txt\0",
        )
        .expect("pathspec file should be written");
        if command == "reset" {
            run_git(fixture.path(), ["add", "top.txt", "nested/tracked.txt"]);
        }

        let workspace = temp_path(&format!("{command}-subdirectory-pathspec-file-nul-compare"));
        let git_repo = workspace.join("git");
        let rit_repo = workspace.join("rit");
        copy_directory(fixture.path(), &git_repo);
        copy_directory(fixture.path(), &rit_repo);

        let git_output = run_command(
            &command_words(
                "git",
                [
                    command,
                    "--pathspec-from-file",
                    "pathspecs.nul",
                    "--pathspec-file-nul",
                ],
            ),
            &git_repo.join("nested"),
        );
        let rit_output = run_command(
            &command_words(
                rit_binary(),
                [
                    command,
                    "--pathspec-from-file",
                    "pathspecs.nul",
                    "--pathspec-file-nul",
                ],
            ),
            &rit_repo.join("nested"),
        );

        assert_eq!(rit_output.0, git_output.0, "{command} stdout");
        assert_eq!(rit_output.1, git_output.1, "{command} stderr");
        assert_eq!(
            run_capture("git", ["status", "--porcelain=v1"], &git_repo).0,
            run_capture(rit_binary(), ["status", "--porcelain=v1"], &rit_repo).0,
            "{command} status"
        );
        assert_matching_file_contents(&git_repo.join("top.txt"), &rit_repo.join("top.txt"));
        assert_matching_file_contents(
            &git_repo.join("nested").join("tracked.txt"),
            &rit_repo.join("nested").join("tracked.txt"),
        );
        let _ = fs::remove_dir_all(workspace);
    }
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
fn crlf_pathspec_from_file_matches_git_status() {
    for command in ["add", "restore", "reset"] {
        let fixture = LocalWriteFixture::new(
            &format!("{command}-crlf-pathspec-file"),
            LocalWriteFixtureKind::NestedTracked,
        )
        .expect("fixture should build");
        fs::write(
            fixture.path().join("nested").join("tracked.txt"),
            "changed\n",
        )
        .expect("tracked file should be modified");
        fs::write(
            fixture.path().join("pathspecs.txt"),
            b"nested/tracked.txt\r\n",
        )
        .expect("pathspec file should be written");
        if command == "reset" {
            run_git(fixture.path(), ["add", "nested/tracked.txt"]);
        }

        let outcome = compare_after_command(
            fixture.path(),
            command_words("git", [command, "--pathspec-from-file", "pathspecs.txt"]),
            command_words(
                rit_binary(),
                [command, "--pathspec-from-file", "pathspecs.txt"],
            ),
        );

        assert_eq!(outcome.git_command_stdout, outcome.rit_command_stdout);
        assert_eq!(outcome.git_command_stderr, outcome.rit_command_stderr);
        assert_eq!(outcome.git_status, outcome.rit_status);
    }
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
fn quoted_crlf_pathspec_from_file_matches_git_status() {
    for command in ["add", "restore", "reset"] {
        let fixture = LocalWriteFixture::new(
            &format!("{command}-quoted-crlf-pathspec-file"),
            LocalWriteFixtureKind::NestedTracked,
        )
        .expect("fixture should build");
        fs::write(fixture.path().join("space name.txt"), "base\n")
            .expect("space file should be written");
        run_git(fixture.path(), ["add", "space name.txt"]);
        run_git(fixture.path(), ["commit", "--quiet", "-m", "space"]);
        fs::write(fixture.path().join("space name.txt"), "changed\n")
            .expect("space file should be modified");
        fs::write(
            fixture.path().join("pathspecs.txt"),
            b"\"space name.txt\"\r\n",
        )
        .expect("pathspec file should be written");
        if command == "reset" {
            run_git(fixture.path(), ["add", "space name.txt"]);
        }

        let outcome = compare_after_command(
            fixture.path(),
            command_words("git", [command, "--pathspec-from-file", "pathspecs.txt"]),
            command_words(
                rit_binary(),
                [command, "--pathspec-from-file", "pathspecs.txt"],
            ),
        );

        assert_eq!(outcome.git_command_stdout, outcome.rit_command_stdout);
        assert_eq!(outcome.git_command_stderr, outcome.rit_command_stderr);
        assert_eq!(outcome.git_status, outcome.rit_status);
    }
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
fn nul_stdin_pathspec_option_order_matches_git_state() {
    for command in ["add", "restore", "reset"] {
        let fixture = LocalWriteFixture::new(
            &format!("{command}-pathspec-stdin-nul-option-order"),
            LocalWriteFixtureKind::NestedTracked,
        )
        .expect("fixture should build");
        fs::write(
            fixture.path().join("nested").join("tracked.txt"),
            "changed\n",
        )
        .expect("tracked file should be modified");
        if command == "add" {
            fs::write(fixture.path().join("nested").join("new.txt"), "new\n")
                .expect("new file should be written");
        }
        if command == "reset" {
            run_git(fixture.path(), ["add", "nested"]);
        }
        let stdin: &[u8] = if command == "add" {
            b"nested/tracked.txt\0nested/new.txt\0"
        } else {
            b"nested/tracked.txt\0"
        };

        let outcome = compare_after_command(
            fixture.path(),
            command_words_with_stdin(
                "git",
                [command, "--pathspec-file-nul", "--pathspec-from-file", "-"],
                stdin,
            ),
            command_words_with_stdin(
                rit_binary(),
                [command, "--pathspec-file-nul", "--pathspec-from-file", "-"],
                stdin,
            ),
        );

        assert_eq!(outcome.git_command_stdout, outcome.rit_command_stdout);
        assert_eq!(outcome.git_command_stderr, outcome.rit_command_stderr);
        assert_eq!(outcome.git_status, outcome.rit_status);
        if command == "restore" {
            assert_eq!(
                fs::read_to_string(outcome.git_repo.join("nested").join("tracked.txt"))
                    .expect("git file should read"),
                fs::read_to_string(outcome.rit_repo.join("nested").join("tracked.txt"))
                    .expect("rit file should read")
            );
        }
    }
}

#[test]
fn no_pathspec_file_nul_with_stdin_reverts_to_text_mode_like_git() {
    for command in ["add", "restore", "reset"] {
        let fixture = LocalWriteFixture::new(
            &format!("{command}-no-pathspec-file-nul-stdin"),
            LocalWriteFixtureKind::NestedTracked,
        )
        .expect("fixture should build");
        fs::write(
            fixture.path().join("nested").join("tracked.txt"),
            "changed\n",
        )
        .expect("tracked file should be modified");
        if command == "add" {
            fs::write(fixture.path().join("nested").join("new.txt"), "new\n")
                .expect("new file should be written");
        }
        if command == "reset" {
            run_git(fixture.path(), ["add", "nested"]);
        }
        let stdin: &[u8] = if command == "add" {
            b"nested/tracked.txt\nnested/new.txt\n"
        } else {
            b"nested/tracked.txt\n"
        };

        let outcome = compare_after_command(
            fixture.path(),
            command_words_with_stdin(
                "git",
                [
                    command,
                    "--pathspec-file-nul",
                    "--no-pathspec-file-nul",
                    "--pathspec-from-file",
                    "-",
                ],
                stdin,
            ),
            command_words_with_stdin(
                rit_binary(),
                [
                    command,
                    "--pathspec-file-nul",
                    "--no-pathspec-file-nul",
                    "--pathspec-from-file",
                    "-",
                ],
                stdin,
            ),
        );

        assert_eq!(outcome.git_command_stdout, outcome.rit_command_stdout);
        assert_eq!(outcome.git_command_stderr, outcome.rit_command_stderr);
        assert_eq!(outcome.git_status, outcome.rit_status);
        if command == "restore" {
            assert_eq!(
                fs::read_to_string(outcome.git_repo.join("nested").join("tracked.txt"))
                    .expect("git file should read"),
                fs::read_to_string(outcome.rit_repo.join("nested").join("tracked.txt"))
                    .expect("rit file should read")
            );
        }
    }
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
fn add_nul_pathspec_file_option_order_matches_git_status() {
    let fixture = LocalWriteFixture::new(
        "add-pathspec-file-nul-option-order",
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
                "--pathspec-file-nul",
                "--pathspec-from-file",
                "pathspecs.nul",
            ],
        ),
        command_words(
            rit_binary(),
            [
                "add",
                "--pathspec-file-nul",
                "--pathspec-from-file",
                "pathspecs.nul",
            ],
        ),
    );

    assert_eq!(outcome.git_command_stdout, outcome.rit_command_stdout);
    assert_eq!(outcome.git_command_stderr, outcome.rit_command_stderr);
    assert_eq!(outcome.git_status, outcome.rit_status);
}

#[test]
fn add_no_pathspec_file_nul_reverts_to_text_mode_like_git() {
    let fixture = LocalWriteFixture::new(
        "add-no-pathspec-file-nul",
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
        command_words(
            "git",
            [
                "add",
                "--pathspec-from-file=pathspecs.txt",
                "--pathspec-file-nul",
                "--no-pathspec-file-nul",
            ],
        ),
        command_words(
            rit_binary(),
            [
                "add",
                "--pathspec-from-file=pathspecs.txt",
                "--pathspec-file-nul",
                "--no-pathspec-file-nul",
            ],
        ),
    );

    assert_eq!(outcome.git_command_stdout, outcome.rit_command_stdout);
    assert_eq!(outcome.git_command_stderr, outcome.rit_command_stderr);
    assert_eq!(outcome.git_status, outcome.rit_status);
}

#[test]
fn add_no_pathspec_file_nul_option_order_matches_git_status() {
    let fixture = LocalWriteFixture::new(
        "add-no-pathspec-file-nul-option-order",
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
        command_words(
            "git",
            [
                "add",
                "--pathspec-file-nul",
                "--no-pathspec-file-nul",
                "--pathspec-from-file",
                "pathspecs.txt",
            ],
        ),
        command_words(
            rit_binary(),
            [
                "add",
                "--pathspec-file-nul",
                "--no-pathspec-file-nul",
                "--pathspec-from-file",
                "pathspecs.txt",
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
fn octal_quoted_pathspec_from_file_matches_git_status() {
    for command in ["add", "restore", "reset"] {
        let fixture = LocalWriteFixture::new(
            &format!("{command}-octal-quoted-pathspec-file"),
            LocalWriteFixtureKind::NestedTracked,
        )
        .expect("fixture should build");
        run_git(fixture.path(), ["config", "core.quotepath", "false"]);
        let path = "caf\u{00e9}.txt";
        fs::write(fixture.path().join(path), "base\n").expect("unicode file should be written");
        run_git(fixture.path(), ["add", path]);
        run_git(fixture.path(), ["commit", "--quiet", "-m", "unicode path"]);
        fs::write(fixture.path().join(path), "changed\n").expect("unicode file should be modified");
        fs::write(
            fixture.path().join("pathspecs.txt"),
            "\"caf\\303\\251.txt\"\n",
        )
        .expect("pathspec file should be written");
        if command == "reset" {
            run_git(fixture.path(), ["add", path]);
        }

        let outcome = compare_after_command(
            fixture.path(),
            command_words("git", [command, "--pathspec-from-file", "pathspecs.txt"]),
            command_words(
                rit_binary(),
                [command, "--pathspec-from-file", "pathspecs.txt"],
            ),
        );

        assert_eq!(outcome.git_command_stdout, outcome.rit_command_stdout);
        assert_eq!(outcome.git_command_stderr, outcome.rit_command_stderr);
        assert_eq!(outcome.git_status, outcome.rit_status);
    }
}

#[test]
fn short_octal_quoted_pathspec_from_file_is_rejected_like_git() {
    for command in ["add", "restore", "reset"] {
        let fixture = LocalWriteFixture::new(
            &format!("{command}-short-octal-quoted-pathspec-file"),
            LocalWriteFixtureKind::NestedTracked,
        )
        .expect("fixture should build");
        fs::write(
            fixture.path().join("nested").join("tracked.txt"),
            "changed\n",
        )
        .expect("tracked file should be modified");
        fs::write(
            fixture.path().join("pathspecs.txt"),
            "\"nested/tracked\\1.txt\"\n",
        )
        .expect("pathspec file should be written");
        if command == "reset" {
            run_git(fixture.path(), ["add", "nested/tracked.txt"]);
        }

        let workspace = temp_path(&format!("{command}-short-octal-pathspec-compare"));
        let git_repo = workspace.join("git");
        let rit_repo = workspace.join("rit");
        copy_directory(fixture.path(), &git_repo);
        copy_directory(fixture.path(), &rit_repo);

        let git = run_command_allow_failure(
            &command_words("git", [command, "--pathspec-from-file=pathspecs.txt"]),
            &git_repo,
        );
        let rit = run_command_allow_failure(
            &command_words(
                rit_binary(),
                [command, "--pathspec-from-file=pathspecs.txt"],
            ),
            &rit_repo,
        );

        assert_eq!(rit.exit_code, git.exit_code, "{command} exit code");
        assert_eq!(rit.stdout, git.stdout, "{command} stdout");
        assert_eq!(rit.stderr, git.stderr, "{command} stderr");
        assert_eq!(
            run_capture("git", ["status", "--porcelain=v1"], &git_repo).0,
            run_capture(rit_binary(), ["status", "--porcelain=v1"], &rit_repo).0,
            "{command} status"
        );
        let _ = fs::remove_dir_all(workspace);
    }
}

#[test]
fn alarm_quoted_pathspec_from_file_matches_git_error() {
    for command in ["add", "restore", "reset"] {
        let fixture = LocalWriteFixture::new(
            &format!("{command}-alarm-quoted-pathspec-file"),
            LocalWriteFixtureKind::NestedTracked,
        )
        .expect("fixture should build");
        fs::write(
            fixture.path().join("nested").join("tracked.txt"),
            "changed\n",
        )
        .expect("tracked file should be modified");
        fs::write(fixture.path().join("pathspecs.txt"), "\"bell\\a.txt\"\n")
            .expect("pathspec file should be written");
        if command == "reset" {
            run_git(fixture.path(), ["add", "nested/tracked.txt"]);
        }

        let workspace = temp_path(&format!("{command}-alarm-quoted-pathspec-compare"));
        let git_repo = workspace.join("git");
        let rit_repo = workspace.join("rit");
        copy_directory(fixture.path(), &git_repo);
        copy_directory(fixture.path(), &rit_repo);

        let git = run_command_allow_failure(
            &command_words("git", [command, "--pathspec-from-file", "pathspecs.txt"]),
            &git_repo,
        );
        let rit = run_command_allow_failure(
            &command_words(
                rit_binary(),
                [command, "--pathspec-from-file", "pathspecs.txt"],
            ),
            &rit_repo,
        );

        assert_eq!(rit.exit_code, git.exit_code, "{command} exit code");
        assert_eq!(rit.stdout, git.stdout, "{command} stdout");
        assert_eq!(rit.stderr, git.stderr, "{command} stderr");
        assert_eq!(
            run_capture("git", ["status", "--porcelain=v1"], &git_repo).0,
            run_capture(rit_binary(), ["status", "--porcelain=v1"], &rit_repo).0,
            "{command} status"
        );
        let _ = fs::remove_dir_all(workspace);
    }
}

#[test]
fn empty_pathspec_file_matches_git_behavior() {
    for command in ["add", "restore", "reset"] {
        let fixture = LocalWriteFixture::new(
            &format!("{command}-empty-pathspec-file-behavior"),
            LocalWriteFixtureKind::NestedTracked,
        )
        .expect("fixture should build");
        fs::write(
            fixture.path().join("nested").join("tracked.txt"),
            "changed\n",
        )
        .expect("tracked file should be modified");
        fs::write(fixture.path().join("pathspecs.txt"), "")
            .expect("pathspec file should be written");
        if command == "reset" {
            run_git(fixture.path(), ["add", "nested/tracked.txt"]);
        }

        let workspace = temp_path(&format!("{command}-empty-pathspec-file-compare"));
        let git_repo = workspace.join("git");
        let rit_repo = workspace.join("rit");
        copy_directory(fixture.path(), &git_repo);
        copy_directory(fixture.path(), &rit_repo);

        let git = run_command_allow_failure(
            &command_words("git", [command, "--pathspec-from-file", "pathspecs.txt"]),
            &git_repo,
        );
        let rit = run_command_allow_failure(
            &command_words(
                rit_binary(),
                [command, "--pathspec-from-file", "pathspecs.txt"],
            ),
            &rit_repo,
        );

        assert_eq!(rit.exit_code, git.exit_code, "{command} exit code");
        assert_eq!(rit.stdout, git.stdout, "{command} stdout");
        assert_eq!(rit.stderr, git.stderr, "{command} stderr");
        assert_eq!(
            run_capture("git", ["status", "--porcelain=v1"], &git_repo).0,
            run_capture(rit_binary(), ["status", "--porcelain=v1"], &rit_repo).0,
            "{command} status"
        );
        let _ = fs::remove_dir_all(workspace);
    }
}

#[test]
fn empty_pathspec_from_file_value_matches_git_behavior() {
    for command in ["add", "restore", "reset"] {
        let fixture = LocalWriteFixture::new(
            &format!("{command}-empty-pathspec-from-file-value"),
            LocalWriteFixtureKind::NestedTracked,
        )
        .expect("fixture should build");
        fs::write(
            fixture.path().join("nested").join("tracked.txt"),
            "changed\n",
        )
        .expect("tracked file should be modified");
        if command == "reset" {
            run_git(fixture.path(), ["add", "nested/tracked.txt"]);
        }

        let workspace = temp_path(&format!("{command}-empty-pathspec-from-file-value-compare"));
        let git_repo = workspace.join("git");
        let rit_repo = workspace.join("rit");
        copy_directory(fixture.path(), &git_repo);
        copy_directory(fixture.path(), &rit_repo);

        let git = run_command_allow_failure(
            &command_words("git", [command, "--pathspec-from-file="]),
            &git_repo,
        );
        let rit = run_command_allow_failure(
            &command_words(rit_binary(), [command, "--pathspec-from-file="]),
            &rit_repo,
        );

        assert_eq!(rit.exit_code, git.exit_code, "{command} exit code");
        assert_eq!(rit.stdout, git.stdout, "{command} stdout");
        assert_eq!(rit.stderr, git.stderr, "{command} stderr");
        assert_eq!(
            run_capture("git", ["status", "--porcelain=v1"], &git_repo).0,
            run_capture(rit_binary(), ["status", "--porcelain=v1"], &rit_repo).0,
            "{command} status"
        );
        let _ = fs::remove_dir_all(workspace);
    }
}

#[test]
fn empty_nul_pathspec_file_matches_git_behavior() {
    for command in ["add", "restore", "reset"] {
        let fixture = LocalWriteFixture::new(
            &format!("{command}-empty-nul-pathspec-file-behavior"),
            LocalWriteFixtureKind::NestedTracked,
        )
        .expect("fixture should build");
        fs::write(
            fixture.path().join("nested").join("tracked.txt"),
            "changed\n",
        )
        .expect("tracked file should be modified");
        fs::write(fixture.path().join("pathspecs.nul"), "")
            .expect("pathspec file should be written");
        if command == "reset" {
            run_git(fixture.path(), ["add", "nested/tracked.txt"]);
        }

        let workspace = temp_path(&format!("{command}-empty-nul-pathspec-file-compare"));
        let git_repo = workspace.join("git");
        let rit_repo = workspace.join("rit");
        copy_directory(fixture.path(), &git_repo);
        copy_directory(fixture.path(), &rit_repo);

        let git = run_command_allow_failure(
            &command_words(
                "git",
                [
                    command,
                    "--pathspec-from-file=pathspecs.nul",
                    "--pathspec-file-nul",
                ],
            ),
            &git_repo,
        );
        let rit = run_command_allow_failure(
            &command_words(
                rit_binary(),
                [
                    command,
                    "--pathspec-from-file=pathspecs.nul",
                    "--pathspec-file-nul",
                ],
            ),
            &rit_repo,
        );

        assert_eq!(rit.exit_code, git.exit_code, "{command} exit code");
        assert_eq!(rit.stdout, git.stdout, "{command} stdout");
        assert_eq!(rit.stderr, git.stderr, "{command} stderr");
        assert_eq!(
            run_capture("git", ["status", "--porcelain=v1"], &git_repo).0,
            run_capture(rit_binary(), ["status", "--porcelain=v1"], &rit_repo).0,
            "{command} status"
        );
        let _ = fs::remove_dir_all(workspace);
    }
}

#[test]
fn repeated_pathspec_from_file_uses_last_file_like_git() {
    for command in ["add", "restore", "reset"] {
        let fixture = LocalWriteFixture::new(
            &format!("{command}-repeated-pathspec-from-file"),
            LocalWriteFixtureKind::NestedTracked,
        )
        .expect("fixture should build");
        fs::write(fixture.path().join("other.txt"), "base\n")
            .expect("other file should be written");
        run_git(fixture.path(), ["add", "other.txt"]);
        run_git(fixture.path(), ["commit", "--quiet", "-m", "other"]);
        fs::write(
            fixture.path().join("nested").join("tracked.txt"),
            "changed\n",
        )
        .expect("tracked file should be modified");
        fs::write(fixture.path().join("other.txt"), "changed\n")
            .expect("other file should be modified");
        fs::write(fixture.path().join("one.txt"), "nested/tracked.txt\n")
            .expect("first pathspec file should be written");
        fs::write(fixture.path().join("two.txt"), "other.txt\n")
            .expect("second pathspec file should be written");
        if command == "reset" {
            run_git(fixture.path(), ["add", "nested/tracked.txt", "other.txt"]);
        }

        let outcome = compare_after_command(
            fixture.path(),
            command_words(
                "git",
                [
                    command,
                    "--pathspec-from-file=one.txt",
                    "--pathspec-from-file=two.txt",
                ],
            ),
            command_words(
                rit_binary(),
                [
                    command,
                    "--pathspec-from-file=one.txt",
                    "--pathspec-from-file=two.txt",
                ],
            ),
        );

        assert_eq!(outcome.git_command_stdout, outcome.rit_command_stdout);
        assert_eq!(outcome.git_command_stderr, outcome.rit_command_stderr);
        assert_eq!(outcome.git_status, outcome.rit_status);
    }
}

#[test]
fn repeated_pathspec_from_file_uses_later_stdin_like_git() {
    for command in ["add", "restore", "reset"] {
        let fixture = LocalWriteFixture::new(
            &format!("{command}-repeated-pathspec-from-file-later-stdin"),
            LocalWriteFixtureKind::NestedTracked,
        )
        .expect("fixture should build");
        fs::write(fixture.path().join("other.txt"), "base\n")
            .expect("other file should be written");
        run_git(fixture.path(), ["add", "other.txt"]);
        run_git(fixture.path(), ["commit", "--quiet", "-m", "other"]);
        fs::write(
            fixture.path().join("nested").join("tracked.txt"),
            "changed\n",
        )
        .expect("tracked file should be modified");
        fs::write(fixture.path().join("other.txt"), "changed\n")
            .expect("other file should be modified");
        fs::write(fixture.path().join("one.txt"), "nested/tracked.txt\n")
            .expect("first pathspec file should be written");
        if command == "reset" {
            run_git(fixture.path(), ["add", "nested/tracked.txt", "other.txt"]);
        }
        let stdin = b"other.txt\n";

        let outcome = compare_after_command(
            fixture.path(),
            command_words_with_stdin(
                "git",
                [
                    command,
                    "--pathspec-from-file=one.txt",
                    "--pathspec-from-file",
                    "-",
                ],
                stdin,
            ),
            command_words_with_stdin(
                rit_binary(),
                [
                    command,
                    "--pathspec-from-file=one.txt",
                    "--pathspec-from-file",
                    "-",
                ],
                stdin,
            ),
        );

        assert_eq!(outcome.git_command_stdout, outcome.rit_command_stdout);
        assert_eq!(outcome.git_command_stderr, outcome.rit_command_stderr);
        assert_eq!(outcome.git_status, outcome.rit_status);
    }
}

#[test]
fn repeated_pathspec_from_file_uses_later_file_after_stdin_like_git() {
    for command in ["add", "restore", "reset"] {
        let fixture = LocalWriteFixture::new(
            &format!("{command}-repeated-pathspec-from-file-later-file"),
            LocalWriteFixtureKind::NestedTracked,
        )
        .expect("fixture should build");
        fs::write(fixture.path().join("other.txt"), "base\n")
            .expect("other file should be written");
        run_git(fixture.path(), ["add", "other.txt"]);
        run_git(fixture.path(), ["commit", "--quiet", "-m", "other"]);
        fs::write(
            fixture.path().join("nested").join("tracked.txt"),
            "changed\n",
        )
        .expect("tracked file should be modified");
        fs::write(fixture.path().join("other.txt"), "changed\n")
            .expect("other file should be modified");
        fs::write(fixture.path().join("two.txt"), "other.txt\n")
            .expect("second pathspec file should be written");
        if command == "reset" {
            run_git(fixture.path(), ["add", "nested/tracked.txt", "other.txt"]);
        }
        let stdin = b"nested/tracked.txt\n";

        let outcome = compare_after_command(
            fixture.path(),
            command_words_with_stdin(
                "git",
                [
                    command,
                    "--pathspec-from-file",
                    "-",
                    "--pathspec-from-file=two.txt",
                ],
                stdin,
            ),
            command_words_with_stdin(
                rit_binary(),
                [
                    command,
                    "--pathspec-from-file",
                    "-",
                    "--pathspec-from-file=two.txt",
                ],
                stdin,
            ),
        );

        assert_eq!(outcome.git_command_stdout, outcome.rit_command_stdout);
        assert_eq!(outcome.git_command_stderr, outcome.rit_command_stderr);
        assert_eq!(outcome.git_status, outcome.rit_status);
    }
}

#[test]
fn repeated_pathspec_from_file_uses_last_nul_file_like_git() {
    for command in ["add", "restore", "reset"] {
        let fixture = LocalWriteFixture::new(
            &format!("{command}-repeated-pathspec-from-file-last-nul-file"),
            LocalWriteFixtureKind::NestedTracked,
        )
        .expect("fixture should build");
        fs::write(fixture.path().join("other.txt"), "base\n")
            .expect("other file should be written");
        run_git(fixture.path(), ["add", "other.txt"]);
        run_git(fixture.path(), ["commit", "--quiet", "-m", "other"]);
        fs::write(
            fixture.path().join("nested").join("tracked.txt"),
            "changed\n",
        )
        .expect("tracked file should be modified");
        fs::write(fixture.path().join("other.txt"), "changed\n")
            .expect("other file should be modified");
        fs::write(fixture.path().join("one.nul"), b"nested/tracked.txt\0")
            .expect("first NUL pathspec file should be written");
        fs::write(fixture.path().join("two.nul"), b"other.txt\0")
            .expect("second NUL pathspec file should be written");
        if command == "reset" {
            run_git(fixture.path(), ["add", "nested/tracked.txt", "other.txt"]);
        }

        let outcome = compare_after_command(
            fixture.path(),
            command_words(
                "git",
                [
                    command,
                    "--pathspec-from-file=one.nul",
                    "--pathspec-file-nul",
                    "--pathspec-from-file=two.nul",
                ],
            ),
            command_words(
                rit_binary(),
                [
                    command,
                    "--pathspec-from-file=one.nul",
                    "--pathspec-file-nul",
                    "--pathspec-from-file=two.nul",
                ],
            ),
        );

        assert_eq!(outcome.git_command_stdout, outcome.rit_command_stdout);
        assert_eq!(outcome.git_command_stderr, outcome.rit_command_stderr);
        assert_eq!(outcome.git_status, outcome.rit_status);
    }
}

#[test]
fn repeated_pathspec_from_file_uses_later_nul_stdin_like_git() {
    for command in ["add", "restore", "reset"] {
        let fixture = LocalWriteFixture::new(
            &format!("{command}-repeated-pathspec-from-file-later-nul-stdin"),
            LocalWriteFixtureKind::NestedTracked,
        )
        .expect("fixture should build");
        fs::write(fixture.path().join("other.txt"), "base\n")
            .expect("other file should be written");
        run_git(fixture.path(), ["add", "other.txt"]);
        run_git(fixture.path(), ["commit", "--quiet", "-m", "other"]);
        fs::write(
            fixture.path().join("nested").join("tracked.txt"),
            "changed\n",
        )
        .expect("tracked file should be modified");
        fs::write(fixture.path().join("other.txt"), "changed\n")
            .expect("other file should be modified");
        fs::write(fixture.path().join("one.txt"), "nested/tracked.txt\n")
            .expect("text pathspec file should be written");
        if command == "reset" {
            run_git(fixture.path(), ["add", "nested/tracked.txt", "other.txt"]);
        }
        let stdin = b"other.txt\0";

        let outcome = compare_after_command(
            fixture.path(),
            command_words_with_stdin(
                "git",
                [
                    command,
                    "--pathspec-from-file=one.txt",
                    "--pathspec-file-nul",
                    "--pathspec-from-file",
                    "-",
                ],
                stdin,
            ),
            command_words_with_stdin(
                rit_binary(),
                [
                    command,
                    "--pathspec-from-file=one.txt",
                    "--pathspec-file-nul",
                    "--pathspec-from-file",
                    "-",
                ],
                stdin,
            ),
        );

        assert_eq!(outcome.git_command_stdout, outcome.rit_command_stdout);
        assert_eq!(outcome.git_command_stderr, outcome.rit_command_stderr);
        assert_eq!(outcome.git_status, outcome.rit_status);
    }
}

#[test]
fn repeated_pathspec_from_file_uses_later_nul_file_after_stdin_like_git() {
    for command in ["add", "restore", "reset"] {
        let fixture = LocalWriteFixture::new(
            &format!("{command}-repeated-pathspec-from-file-later-nul-file"),
            LocalWriteFixtureKind::NestedTracked,
        )
        .expect("fixture should build");
        fs::write(fixture.path().join("other.txt"), "base\n")
            .expect("other file should be written");
        run_git(fixture.path(), ["add", "other.txt"]);
        run_git(fixture.path(), ["commit", "--quiet", "-m", "other"]);
        fs::write(
            fixture.path().join("nested").join("tracked.txt"),
            "changed\n",
        )
        .expect("tracked file should be modified");
        fs::write(fixture.path().join("other.txt"), "changed\n")
            .expect("other file should be modified");
        fs::write(fixture.path().join("two.nul"), b"other.txt\0")
            .expect("second NUL pathspec file should be written");
        if command == "reset" {
            run_git(fixture.path(), ["add", "nested/tracked.txt", "other.txt"]);
        }
        let stdin = b"nested/tracked.txt\0";

        let outcome = compare_after_command(
            fixture.path(),
            command_words_with_stdin(
                "git",
                [
                    command,
                    "--pathspec-from-file",
                    "-",
                    "--pathspec-file-nul",
                    "--pathspec-from-file=two.nul",
                ],
                stdin,
            ),
            command_words_with_stdin(
                rit_binary(),
                [
                    command,
                    "--pathspec-from-file",
                    "-",
                    "--pathspec-file-nul",
                    "--pathspec-from-file=two.nul",
                ],
                stdin,
            ),
        );

        assert_eq!(outcome.git_command_stdout, outcome.rit_command_stdout);
        assert_eq!(outcome.git_command_stderr, outcome.rit_command_stderr);
        assert_eq!(outcome.git_status, outcome.rit_status);
    }
}

#[test]
fn repeated_pathspec_from_file_uses_later_nul_file_after_text_file_like_git() {
    for command in ["add", "restore", "reset"] {
        let fixture = LocalWriteFixture::new(
            &format!("{command}-repeated-pathspec-from-file-later-nul-file-after-text"),
            LocalWriteFixtureKind::NestedTracked,
        )
        .expect("fixture should build");
        fs::write(fixture.path().join("other.txt"), "base\n")
            .expect("other file should be written");
        run_git(fixture.path(), ["add", "other.txt"]);
        run_git(fixture.path(), ["commit", "--quiet", "-m", "other"]);
        fs::write(
            fixture.path().join("nested").join("tracked.txt"),
            "changed\n",
        )
        .expect("tracked file should be modified");
        fs::write(fixture.path().join("other.txt"), "changed\n")
            .expect("other file should be modified");
        fs::write(fixture.path().join("one.txt"), "nested/tracked.txt\n")
            .expect("first text pathspec file should be written");
        fs::write(fixture.path().join("two.nul"), b"other.txt\0")
            .expect("second NUL pathspec file should be written");
        if command == "reset" {
            run_git(fixture.path(), ["add", "nested/tracked.txt", "other.txt"]);
        }

        let outcome = compare_after_command(
            fixture.path(),
            command_words(
                "git",
                [
                    command,
                    "--pathspec-from-file=one.txt",
                    "--pathspec-file-nul",
                    "--pathspec-from-file=two.nul",
                ],
            ),
            command_words(
                rit_binary(),
                [
                    command,
                    "--pathspec-from-file=one.txt",
                    "--pathspec-file-nul",
                    "--pathspec-from-file=two.nul",
                ],
            ),
        );

        assert_eq!(outcome.git_command_stdout, outcome.rit_command_stdout);
        assert_eq!(outcome.git_command_stderr, outcome.rit_command_stderr);
        assert_eq!(outcome.git_status, outcome.rit_status);
    }
}

#[test]
fn repeated_pathspec_from_file_uses_later_text_file_after_nul_mode_like_git() {
    for command in ["add", "restore", "reset"] {
        let fixture = LocalWriteFixture::new(
            &format!("{command}-repeated-pathspec-from-file-later-text-file-after-nul"),
            LocalWriteFixtureKind::NestedTracked,
        )
        .expect("fixture should build");
        fs::write(fixture.path().join("other.txt"), "base\n")
            .expect("other file should be written");
        run_git(fixture.path(), ["add", "other.txt"]);
        run_git(fixture.path(), ["commit", "--quiet", "-m", "other"]);
        fs::write(
            fixture.path().join("nested").join("tracked.txt"),
            "changed\n",
        )
        .expect("tracked file should be modified");
        fs::write(fixture.path().join("other.txt"), "changed\n")
            .expect("other file should be modified");
        fs::write(fixture.path().join("one.nul"), b"nested/tracked.txt\0")
            .expect("first NUL pathspec file should be written");
        fs::write(fixture.path().join("two.txt"), "other.txt\n")
            .expect("second text pathspec file should be written");
        if command == "reset" {
            run_git(fixture.path(), ["add", "nested/tracked.txt", "other.txt"]);
        }

        let outcome = compare_after_command(
            fixture.path(),
            command_words(
                "git",
                [
                    command,
                    "--pathspec-from-file=one.nul",
                    "--pathspec-file-nul",
                    "--no-pathspec-file-nul",
                    "--pathspec-from-file=two.txt",
                ],
            ),
            command_words(
                rit_binary(),
                [
                    command,
                    "--pathspec-from-file=one.nul",
                    "--pathspec-file-nul",
                    "--no-pathspec-file-nul",
                    "--pathspec-from-file=two.txt",
                ],
            ),
        );

        assert_eq!(outcome.git_command_stdout, outcome.rit_command_stdout);
        assert_eq!(outcome.git_command_stderr, outcome.rit_command_stderr);
        assert_eq!(outcome.git_status, outcome.rit_status);
    }
}

#[test]
fn no_pathspec_from_file_without_file_is_accepted_like_git() {
    for command in ["add", "restore", "reset"] {
        let fixture = LocalWriteFixture::new(
            &format!("{command}-no-pathspec-from-file"),
            LocalWriteFixtureKind::NestedTracked,
        )
        .expect("fixture should build");
        fs::write(fixture.path().join("other.txt"), "base\n")
            .expect("other file should be written");
        run_git(fixture.path(), ["add", "other.txt"]);
        run_git(fixture.path(), ["commit", "--quiet", "-m", "other"]);
        fs::write(
            fixture.path().join("nested").join("tracked.txt"),
            "changed\n",
        )
        .expect("tracked file should be modified");
        fs::write(fixture.path().join("other.txt"), "changed\n")
            .expect("other file should be modified");
        fs::write(fixture.path().join("pathspecs.txt"), "nested/tracked.txt\n")
            .expect("pathspec file should be written");
        if command == "reset" {
            run_git(fixture.path(), ["add", "nested/tracked.txt", "other.txt"]);
        }

        let outcome = compare_after_command(
            fixture.path(),
            command_words("git", [command, "--no-pathspec-from-file", "other.txt"]),
            command_words(
                rit_binary(),
                [command, "--no-pathspec-from-file", "other.txt"],
            ),
        );

        assert_eq!(outcome.git_command_stdout, outcome.rit_command_stdout);
        assert_eq!(outcome.git_command_stderr, outcome.rit_command_stderr);
        assert_eq!(outcome.git_status, outcome.rit_status);
    }
}

#[test]
fn no_pathspec_from_file_keeps_active_pathspec_file_selection_like_git() {
    for command in ["add", "restore", "reset"] {
        let fixture = LocalWriteFixture::new(
            &format!("{command}-no-pathspec-from-file-active-selection"),
            LocalWriteFixtureKind::NestedTracked,
        )
        .expect("fixture should build");
        fs::write(fixture.path().join("other.txt"), "base\n")
            .expect("other file should be written");
        run_git(fixture.path(), ["add", "other.txt"]);
        run_git(fixture.path(), ["commit", "--quiet", "-m", "other"]);
        fs::write(
            fixture.path().join("nested").join("tracked.txt"),
            "changed\n",
        )
        .expect("tracked file should be modified");
        fs::write(fixture.path().join("other.txt"), "changed\n")
            .expect("other file should be modified");
        fs::write(fixture.path().join("pathspecs.txt"), "nested/tracked.txt\n")
            .expect("pathspec file should be written");
        if command == "reset" {
            run_git(fixture.path(), ["add", "nested/tracked.txt", "other.txt"]);
        }

        let outcome = compare_after_command(
            fixture.path(),
            command_words(
                "git",
                [
                    command,
                    "--pathspec-from-file=pathspecs.txt",
                    "--no-pathspec-from-file",
                ],
            ),
            command_words(
                rit_binary(),
                [
                    command,
                    "--pathspec-from-file=pathspecs.txt",
                    "--no-pathspec-from-file",
                ],
            ),
        );

        assert_eq!(outcome.git_command_stdout, outcome.rit_command_stdout);
        assert_eq!(outcome.git_command_stderr, outcome.rit_command_stderr);
        assert_eq!(outcome.git_status, outcome.rit_status);
    }
}

#[test]
fn no_pathspec_from_file_keeps_active_stdin_selection_like_git() {
    for command in ["add", "restore", "reset"] {
        let fixture = LocalWriteFixture::new(
            &format!("{command}-no-pathspec-from-file-active-stdin"),
            LocalWriteFixtureKind::NestedTracked,
        )
        .expect("fixture should build");
        fs::write(fixture.path().join("other.txt"), "base\n")
            .expect("other file should be written");
        run_git(fixture.path(), ["add", "other.txt"]);
        run_git(fixture.path(), ["commit", "--quiet", "-m", "other"]);
        fs::write(
            fixture.path().join("nested").join("tracked.txt"),
            "changed\n",
        )
        .expect("tracked file should be modified");
        fs::write(fixture.path().join("other.txt"), "changed\n")
            .expect("other file should be modified");
        if command == "reset" {
            run_git(fixture.path(), ["add", "nested/tracked.txt", "other.txt"]);
        }
        let stdin = b"nested/tracked.txt\n";

        let outcome = compare_after_command(
            fixture.path(),
            command_words_with_stdin(
                "git",
                [
                    command,
                    "--pathspec-from-file",
                    "-",
                    "--no-pathspec-from-file",
                ],
                stdin,
            ),
            command_words_with_stdin(
                rit_binary(),
                [
                    command,
                    "--pathspec-from-file",
                    "-",
                    "--no-pathspec-from-file",
                ],
                stdin,
            ),
        );

        assert_eq!(outcome.git_command_stdout, outcome.rit_command_stdout);
        assert_eq!(outcome.git_command_stderr, outcome.rit_command_stderr);
        assert_eq!(outcome.git_status, outcome.rit_status);
        if command == "restore" {
            assert_eq!(
                fs::read_to_string(outcome.git_repo.join("nested").join("tracked.txt"))
                    .expect("git file should read"),
                fs::read_to_string(outcome.rit_repo.join("nested").join("tracked.txt"))
                    .expect("rit file should read")
            );
            assert_eq!(
                fs::read_to_string(outcome.git_repo.join("other.txt"))
                    .expect("git file should read"),
                fs::read_to_string(outcome.rit_repo.join("other.txt"))
                    .expect("rit file should read")
            );
        }
    }
}

#[test]
fn no_pathspec_from_file_keeps_active_nul_stdin_selection_like_git() {
    for command in ["add", "restore", "reset"] {
        let fixture = LocalWriteFixture::new(
            &format!("{command}-no-pathspec-from-file-active-nul-stdin"),
            LocalWriteFixtureKind::NestedTracked,
        )
        .expect("fixture should build");
        fs::write(fixture.path().join("other.txt"), "base\n")
            .expect("other file should be written");
        run_git(fixture.path(), ["add", "other.txt"]);
        run_git(fixture.path(), ["commit", "--quiet", "-m", "other"]);
        fs::write(
            fixture.path().join("nested").join("tracked.txt"),
            "changed\n",
        )
        .expect("tracked file should be modified");
        fs::write(fixture.path().join("other.txt"), "changed\n")
            .expect("other file should be modified");
        if command == "reset" {
            run_git(fixture.path(), ["add", "nested/tracked.txt", "other.txt"]);
        }
        let stdin = b"nested/tracked.txt\0";

        let outcome = compare_after_command(
            fixture.path(),
            command_words_with_stdin(
                "git",
                [
                    command,
                    "--pathspec-from-file",
                    "-",
                    "--pathspec-file-nul",
                    "--no-pathspec-from-file",
                ],
                stdin,
            ),
            command_words_with_stdin(
                rit_binary(),
                [
                    command,
                    "--pathspec-from-file",
                    "-",
                    "--pathspec-file-nul",
                    "--no-pathspec-from-file",
                ],
                stdin,
            ),
        );

        assert_eq!(outcome.git_command_stdout, outcome.rit_command_stdout);
        assert_eq!(outcome.git_command_stderr, outcome.rit_command_stderr);
        assert_eq!(outcome.git_status, outcome.rit_status);
        if command == "restore" {
            assert_eq!(
                fs::read_to_string(outcome.git_repo.join("nested").join("tracked.txt"))
                    .expect("git file should read"),
                fs::read_to_string(outcome.rit_repo.join("nested").join("tracked.txt"))
                    .expect("rit file should read")
            );
            assert_eq!(
                fs::read_to_string(outcome.git_repo.join("other.txt"))
                    .expect("git file should read"),
                fs::read_to_string(outcome.rit_repo.join("other.txt"))
                    .expect("rit file should read")
            );
        }
    }
}

#[test]
fn no_pathspec_from_file_before_selection_keeps_later_file_active_like_git() {
    for command in ["add", "restore", "reset"] {
        let fixture = LocalWriteFixture::new(
            &format!("{command}-no-pathspec-from-file-before-selection"),
            LocalWriteFixtureKind::NestedTracked,
        )
        .expect("fixture should build");
        fs::write(fixture.path().join("other.txt"), "base\n")
            .expect("other file should be written");
        run_git(fixture.path(), ["add", "other.txt"]);
        run_git(fixture.path(), ["commit", "--quiet", "-m", "other"]);
        fs::write(
            fixture.path().join("nested").join("tracked.txt"),
            "changed\n",
        )
        .expect("tracked file should be modified");
        fs::write(fixture.path().join("other.txt"), "changed\n")
            .expect("other file should be modified");
        fs::write(fixture.path().join("pathspecs.txt"), "nested/tracked.txt\n")
            .expect("pathspec file should be written");
        if command == "reset" {
            run_git(fixture.path(), ["add", "nested/tracked.txt", "other.txt"]);
        }

        let outcome = compare_after_command(
            fixture.path(),
            command_words(
                "git",
                [
                    command,
                    "--no-pathspec-from-file",
                    "--pathspec-from-file=pathspecs.txt",
                ],
            ),
            command_words(
                rit_binary(),
                [
                    command,
                    "--no-pathspec-from-file",
                    "--pathspec-from-file=pathspecs.txt",
                ],
            ),
        );

        assert_eq!(outcome.git_command_stdout, outcome.rit_command_stdout);
        assert_eq!(outcome.git_command_stderr, outcome.rit_command_stderr);
        assert_eq!(outcome.git_status, outcome.rit_status);
    }
}

#[test]
fn no_pathspec_from_file_before_selection_keeps_later_stdin_active_like_git() {
    for command in ["add", "restore", "reset"] {
        let fixture = LocalWriteFixture::new(
            &format!("{command}-no-pathspec-from-file-before-stdin-selection"),
            LocalWriteFixtureKind::NestedTracked,
        )
        .expect("fixture should build");
        fs::write(fixture.path().join("other.txt"), "base\n")
            .expect("other file should be written");
        run_git(fixture.path(), ["add", "other.txt"]);
        run_git(fixture.path(), ["commit", "--quiet", "-m", "other"]);
        fs::write(
            fixture.path().join("nested").join("tracked.txt"),
            "changed\n",
        )
        .expect("tracked file should be modified");
        fs::write(fixture.path().join("other.txt"), "changed\n")
            .expect("other file should be modified");
        if command == "reset" {
            run_git(fixture.path(), ["add", "nested/tracked.txt", "other.txt"]);
        }
        let stdin = b"nested/tracked.txt\n";

        let outcome = compare_after_command(
            fixture.path(),
            command_words_with_stdin(
                "git",
                [
                    command,
                    "--no-pathspec-from-file",
                    "--pathspec-from-file",
                    "-",
                ],
                stdin,
            ),
            command_words_with_stdin(
                rit_binary(),
                [
                    command,
                    "--no-pathspec-from-file",
                    "--pathspec-from-file",
                    "-",
                ],
                stdin,
            ),
        );

        assert_eq!(outcome.git_command_stdout, outcome.rit_command_stdout);
        assert_eq!(outcome.git_command_stderr, outcome.rit_command_stderr);
        assert_eq!(outcome.git_status, outcome.rit_status);
        if command == "restore" {
            assert_eq!(
                fs::read_to_string(outcome.git_repo.join("nested").join("tracked.txt"))
                    .expect("git file should read"),
                fs::read_to_string(outcome.rit_repo.join("nested").join("tracked.txt"))
                    .expect("rit file should read")
            );
            assert_eq!(
                fs::read_to_string(outcome.git_repo.join("other.txt"))
                    .expect("git file should read"),
                fs::read_to_string(outcome.rit_repo.join("other.txt"))
                    .expect("rit file should read")
            );
        }
    }
}

#[test]
fn no_pathspec_from_file_before_selection_keeps_later_nul_stdin_active_like_git() {
    for command in ["add", "restore", "reset"] {
        let fixture = LocalWriteFixture::new(
            &format!("{command}-no-pathspec-from-file-before-nul-stdin-selection"),
            LocalWriteFixtureKind::NestedTracked,
        )
        .expect("fixture should build");
        fs::write(fixture.path().join("other.txt"), "base\n")
            .expect("other file should be written");
        run_git(fixture.path(), ["add", "other.txt"]);
        run_git(fixture.path(), ["commit", "--quiet", "-m", "other"]);
        fs::write(
            fixture.path().join("nested").join("tracked.txt"),
            "changed\n",
        )
        .expect("tracked file should be modified");
        fs::write(fixture.path().join("other.txt"), "changed\n")
            .expect("other file should be modified");
        if command == "reset" {
            run_git(fixture.path(), ["add", "nested/tracked.txt", "other.txt"]);
        }
        let stdin = b"nested/tracked.txt\0";

        let outcome = compare_after_command(
            fixture.path(),
            command_words_with_stdin(
                "git",
                [
                    command,
                    "--no-pathspec-from-file",
                    "--pathspec-from-file",
                    "-",
                    "--pathspec-file-nul",
                ],
                stdin,
            ),
            command_words_with_stdin(
                rit_binary(),
                [
                    command,
                    "--no-pathspec-from-file",
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
        if command == "restore" {
            assert_eq!(
                fs::read_to_string(outcome.git_repo.join("nested").join("tracked.txt"))
                    .expect("git file should read"),
                fs::read_to_string(outcome.rit_repo.join("nested").join("tracked.txt"))
                    .expect("rit file should read")
            );
            assert_eq!(
                fs::read_to_string(outcome.git_repo.join("other.txt"))
                    .expect("git file should read"),
                fs::read_to_string(outcome.rit_repo.join("other.txt"))
                    .expect("rit file should read")
            );
        }
    }
}

#[test]
fn pathspec_from_file_and_args_are_rejected_like_git() {
    for command in ["add", "restore", "reset"] {
        let fixture = LocalWriteFixture::new(
            &format!("{command}-pathspec-from-file-and-args"),
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
        if command == "reset" {
            run_git(fixture.path(), ["add", "nested/tracked.txt"]);
        }

        let workspace = temp_path(&format!("{command}-pathspec-from-file-and-args-compare"));
        let git_repo = workspace.join("git");
        let rit_repo = workspace.join("rit");
        copy_directory(fixture.path(), &git_repo);
        copy_directory(fixture.path(), &rit_repo);

        let git = run_command_allow_failure(
            &command_words(
                "git",
                [
                    command,
                    "--pathspec-from-file=pathspecs.txt",
                    "nested/tracked.txt",
                ],
            ),
            &git_repo,
        );
        let rit = run_command_allow_failure(
            &command_words(
                rit_binary(),
                [
                    command,
                    "--pathspec-from-file=pathspecs.txt",
                    "nested/tracked.txt",
                ],
            ),
            &rit_repo,
        );

        assert_eq!(rit.exit_code, git.exit_code, "{command} exit code");
        assert_eq!(rit.stdout, git.stdout, "{command} stdout");
        assert_eq!(rit.stderr, git.stderr, "{command} stderr");
        assert_eq!(
            run_capture("git", ["status", "--porcelain=v1"], &git_repo).0,
            run_capture(rit_binary(), ["status", "--porcelain=v1"], &rit_repo).0,
            "{command} status"
        );
        let _ = fs::remove_dir_all(workspace);
    }
}

#[test]
fn missing_pathspec_from_file_is_rejected_like_git() {
    for command in ["add", "restore", "reset"] {
        let fixture = LocalWriteFixture::new(
            &format!("{command}-missing-pathspec-from-file"),
            LocalWriteFixtureKind::NestedTracked,
        )
        .expect("fixture should build");
        fs::write(
            fixture.path().join("nested").join("tracked.txt"),
            "changed\n",
        )
        .expect("tracked file should be modified");
        if command == "reset" {
            run_git(fixture.path(), ["add", "nested/tracked.txt"]);
        }

        let workspace = temp_path(&format!("{command}-missing-pathspec-from-file-compare"));
        let git_repo = workspace.join("git");
        let rit_repo = workspace.join("rit");
        copy_directory(fixture.path(), &git_repo);
        copy_directory(fixture.path(), &rit_repo);

        let git = run_command_allow_failure(
            &command_words("git", [command, "--pathspec-from-file=missing.txt"]),
            &git_repo,
        );
        let rit = run_command_allow_failure(
            &command_words(rit_binary(), [command, "--pathspec-from-file=missing.txt"]),
            &rit_repo,
        );

        assert_eq!(rit.exit_code, git.exit_code, "{command} exit code");
        assert_eq!(rit.stdout, git.stdout, "{command} stdout");
        assert_eq!(rit.stderr, git.stderr, "{command} stderr");
        assert_eq!(
            run_capture("git", ["status", "--porcelain=v1"], &git_repo).0,
            run_capture(rit_binary(), ["status", "--porcelain=v1"], &rit_repo).0,
            "{command} status"
        );
        let _ = fs::remove_dir_all(workspace);
    }
}

#[test]
fn invalid_utf8_pathspec_file_matches_git_behavior() {
    for command in ["add", "restore", "reset"] {
        let fixture = LocalWriteFixture::new(
            &format!("{command}-invalid-utf8-pathspec-file"),
            LocalWriteFixtureKind::NestedTracked,
        )
        .expect("fixture should build");
        fs::write(
            fixture.path().join("nested").join("tracked.txt"),
            "changed\n",
        )
        .expect("tracked file should be modified");
        fs::write(fixture.path().join("pathspecs.bin"), [0xff, b'\n'])
            .expect("pathspec file should be written");
        if command == "reset" {
            run_git(fixture.path(), ["add", "nested/tracked.txt"]);
        }

        let workspace = temp_path(&format!("{command}-invalid-utf8-pathspec-file-compare"));
        let git_repo = workspace.join("git");
        let rit_repo = workspace.join("rit");
        copy_directory(fixture.path(), &git_repo);
        copy_directory(fixture.path(), &rit_repo);

        let git = run_command_allow_failure(
            &command_words("git", [command, "--pathspec-from-file=pathspecs.bin"]),
            &git_repo,
        );
        let rit = run_command_allow_failure(
            &command_words(
                rit_binary(),
                [command, "--pathspec-from-file=pathspecs.bin"],
            ),
            &rit_repo,
        );

        assert_eq!(rit.exit_code, git.exit_code, "{command} exit code");
        assert_eq!(rit.stdout, git.stdout, "{command} stdout");
        assert_eq!(rit.stderr, git.stderr, "{command} stderr");
        assert_eq!(
            run_capture("git", ["status", "--porcelain=v1"], &git_repo).0,
            run_capture(rit_binary(), ["status", "--porcelain=v1"], &rit_repo).0,
            "{command} status"
        );
        let _ = fs::remove_dir_all(workspace);
    }
}

#[test]
fn text_pathspec_file_nul_bytes_match_git_behavior() {
    for command in ["add", "restore", "reset"] {
        let fixture = LocalWriteFixture::new(
            &format!("{command}-text-pathspec-file-nul-byte"),
            LocalWriteFixtureKind::NestedTracked,
        )
        .expect("fixture should build");
        fs::write(
            fixture.path().join("nested").join("tracked.txt"),
            "changed\n",
        )
        .expect("tracked file should be modified");
        fs::write(
            fixture.path().join("pathspecs.bin"),
            b"nested/tracked.txt\0other.txt\n",
        )
        .expect("pathspec file should be written");
        if command == "reset" {
            run_git(fixture.path(), ["add", "nested/tracked.txt"]);
        }

        let workspace = temp_path(&format!("{command}-text-pathspec-file-nul-byte-compare"));
        let git_repo = workspace.join("git");
        let rit_repo = workspace.join("rit");
        copy_directory(fixture.path(), &git_repo);
        copy_directory(fixture.path(), &rit_repo);

        let git = run_command_allow_failure(
            &command_words("git", [command, "--pathspec-from-file=pathspecs.bin"]),
            &git_repo,
        );
        let rit = run_command_allow_failure(
            &command_words(
                rit_binary(),
                [command, "--pathspec-from-file=pathspecs.bin"],
            ),
            &rit_repo,
        );

        assert_eq!(rit.exit_code, git.exit_code, "{command} exit code");
        assert_eq!(rit.stdout, git.stdout, "{command} stdout");
        assert_eq!(rit.stderr, git.stderr, "{command} stderr");
        assert_eq!(
            run_capture("git", ["status", "--porcelain=v1"], &git_repo).0,
            run_capture(rit_binary(), ["status", "--porcelain=v1"], &rit_repo).0,
            "{command} status"
        );
        let _ = fs::remove_dir_all(workspace);
    }
}

#[test]
fn text_pathspec_file_lone_cr_matches_git_behavior() {
    for command in ["add", "restore", "reset"] {
        let fixture = LocalWriteFixture::new(
            &format!("{command}-text-pathspec-file-lone-cr"),
            LocalWriteFixtureKind::NestedTracked,
        )
        .expect("fixture should build");
        fs::write(
            fixture.path().join("nested").join("tracked.txt"),
            "changed\n",
        )
        .expect("tracked file should be modified");
        fs::write(
            fixture.path().join("pathspecs.bin"),
            b"nested/tracked.txt\r",
        )
        .expect("pathspec file should be written");
        if command == "reset" {
            run_git(fixture.path(), ["add", "nested/tracked.txt"]);
        }

        let workspace = temp_path(&format!("{command}-text-pathspec-file-lone-cr-compare"));
        let git_repo = workspace.join("git");
        let rit_repo = workspace.join("rit");
        copy_directory(fixture.path(), &git_repo);
        copy_directory(fixture.path(), &rit_repo);

        let git = run_command_allow_failure(
            &command_words("git", [command, "--pathspec-from-file=pathspecs.bin"]),
            &git_repo,
        );
        let rit = run_command_allow_failure(
            &command_words(
                rit_binary(),
                [command, "--pathspec-from-file=pathspecs.bin"],
            ),
            &rit_repo,
        );

        assert_eq!(rit.exit_code, git.exit_code, "{command} exit code");
        assert_eq!(rit.stdout, git.stdout, "{command} stdout");
        assert_eq!(rit.stderr, git.stderr, "{command} stderr");
        assert_eq!(
            run_capture("git", ["status", "--porcelain=v1"], &git_repo).0,
            run_capture(rit_binary(), ["status", "--porcelain=v1"], &rit_repo).0,
            "{command} status"
        );
        let _ = fs::remove_dir_all(workspace);
    }
}

#[test]
fn quoted_empty_pathspec_from_file_is_rejected_like_git() {
    for command in ["add", "restore", "reset"] {
        let fixture = LocalWriteFixture::new(
            &format!("{command}-quoted-empty-pathspec-file"),
            LocalWriteFixtureKind::NestedTracked,
        )
        .expect("fixture should build");
        fs::write(
            fixture.path().join("nested").join("tracked.txt"),
            "changed\n",
        )
        .expect("tracked file should be modified");
        fs::write(fixture.path().join("pathspecs.txt"), "\"\"\n")
            .expect("pathspec file should be written");
        if command == "reset" {
            run_git(fixture.path(), ["add", "nested/tracked.txt"]);
        }

        let workspace = temp_path(&format!("{command}-quoted-empty-pathspec-file-compare"));
        let git_repo = workspace.join("git");
        let rit_repo = workspace.join("rit");
        copy_directory(fixture.path(), &git_repo);
        copy_directory(fixture.path(), &rit_repo);

        let git = run_command_allow_failure(
            &command_words("git", [command, "--pathspec-from-file=pathspecs.txt"]),
            &git_repo,
        );
        let rit = run_command_allow_failure(
            &command_words(
                rit_binary(),
                [command, "--pathspec-from-file=pathspecs.txt"],
            ),
            &rit_repo,
        );

        assert_eq!(rit.exit_code, git.exit_code, "{command} exit code");
        assert_eq!(rit.stdout, git.stdout, "{command} stdout");
        assert_eq!(rit.stderr, git.stderr, "{command} stderr");
        assert_eq!(
            run_capture("git", ["status", "--porcelain=v1"], &git_repo).0,
            run_capture(rit_binary(), ["status", "--porcelain=v1"], &rit_repo).0,
            "{command} status"
        );
        let _ = fs::remove_dir_all(workspace);
    }
}

#[test]
fn quoted_pathspec_from_file_ignores_trailing_bytes_like_git() {
    for command in ["add", "restore", "reset"] {
        let fixture = LocalWriteFixture::new(
            &format!("{command}-quoted-pathspec-trailing-bytes"),
            LocalWriteFixtureKind::NestedTracked,
        )
        .expect("fixture should build");
        fs::write(
            fixture.path().join("nested").join("tracked.txt"),
            "changed\n",
        )
        .expect("tracked file should be modified");
        fs::write(
            fixture.path().join("pathspecs.txt"),
            "\"nested/tracked.txt\" trailing ignored\n",
        )
        .expect("pathspec file should be written");
        if command == "reset" {
            run_git(fixture.path(), ["add", "nested/tracked.txt"]);
        }

        let workspace = temp_path(&format!("{command}-quoted-pathspec-trailing-bytes-compare"));
        let git_repo = workspace.join("git");
        let rit_repo = workspace.join("rit");
        copy_directory(fixture.path(), &git_repo);
        copy_directory(fixture.path(), &rit_repo);

        let git = run_command_allow_failure(
            &command_words("git", [command, "--pathspec-from-file=pathspecs.txt"]),
            &git_repo,
        );
        let rit = run_command_allow_failure(
            &command_words(
                rit_binary(),
                [command, "--pathspec-from-file=pathspecs.txt"],
            ),
            &rit_repo,
        );

        assert_eq!(rit.exit_code, git.exit_code, "{command} exit code");
        assert_eq!(rit.stdout, git.stdout, "{command} stdout");
        assert_eq!(rit.stderr, git.stderr, "{command} stderr");
        assert_eq!(
            run_capture("git", ["status", "--porcelain=v1"], &git_repo).0,
            run_capture(rit_binary(), ["status", "--porcelain=v1"], &rit_repo).0,
            "{command} status"
        );
        let _ = fs::remove_dir_all(workspace);
    }
}

#[test]
fn pathspec_file_nul_without_file_is_rejected_like_git() {
    for command in ["add", "restore", "reset"] {
        let fixture = LocalWriteFixture::new(
            &format!("{command}-pathspec-file-nul-without-file"),
            LocalWriteFixtureKind::NestedTracked,
        )
        .expect("fixture should build");
        fs::write(
            fixture.path().join("nested").join("tracked.txt"),
            "changed\n",
        )
        .expect("tracked file should be modified");
        if command == "reset" {
            run_git(fixture.path(), ["add", "nested/tracked.txt"]);
        }

        let workspace = temp_path(&format!("{command}-pathspec-file-nul-without-file-compare"));
        let git_repo = workspace.join("git");
        let rit_repo = workspace.join("rit");
        copy_directory(fixture.path(), &git_repo);
        copy_directory(fixture.path(), &rit_repo);

        let git = run_command_allow_failure(
            &command_words("git", [command, "--pathspec-file-nul"]),
            &git_repo,
        );
        let rit = run_command_allow_failure(
            &command_words(rit_binary(), [command, "--pathspec-file-nul"]),
            &rit_repo,
        );

        assert_eq!(rit.exit_code, git.exit_code, "{command} exit code");
        assert_eq!(rit.stdout, git.stdout, "{command} stdout");
        assert_eq!(rit.stderr, git.stderr, "{command} stderr");
        assert_eq!(
            run_capture("git", ["status", "--porcelain=v1"], &git_repo).0,
            run_capture(rit_binary(), ["status", "--porcelain=v1"], &rit_repo).0,
            "{command} status"
        );
        let _ = fs::remove_dir_all(workspace);
    }
}

#[test]
fn pathspec_from_file_without_value_is_rejected_like_git() {
    for command in ["add", "restore", "reset"] {
        let fixture = LocalWriteFixture::new(
            &format!("{command}-pathspec-from-file-without-value"),
            LocalWriteFixtureKind::NestedTracked,
        )
        .expect("fixture should build");
        fs::write(
            fixture.path().join("nested").join("tracked.txt"),
            "changed\n",
        )
        .expect("tracked file should be modified");
        if command == "reset" {
            run_git(fixture.path(), ["add", "nested/tracked.txt"]);
        }

        let workspace = temp_path(&format!(
            "{command}-pathspec-from-file-without-value-compare"
        ));
        let git_repo = workspace.join("git");
        let rit_repo = workspace.join("rit");
        copy_directory(fixture.path(), &git_repo);
        copy_directory(fixture.path(), &rit_repo);

        let git = run_command_allow_failure(
            &command_words("git", [command, "--pathspec-from-file"]),
            &git_repo,
        );
        let rit = run_command_allow_failure(
            &command_words(rit_binary(), [command, "--pathspec-from-file"]),
            &rit_repo,
        );

        assert_eq!(rit.exit_code, git.exit_code, "{command} exit code");
        assert_eq!(rit.stdout, git.stdout, "{command} stdout");
        assert_eq!(rit.stderr, git.stderr, "{command} stderr");
        assert_eq!(
            run_capture("git", ["status", "--porcelain=v1"], &git_repo).0,
            run_capture(rit_binary(), ["status", "--porcelain=v1"], &rit_repo).0,
            "{command} status"
        );
        let _ = fs::remove_dir_all(workspace);
    }
}

#[test]
fn empty_line_pathspec_from_file_is_rejected_like_git() {
    for command in ["add", "restore", "reset"] {
        let fixture = LocalWriteFixture::new(
            &format!("{command}-empty-pathspec-file"),
            LocalWriteFixtureKind::NestedTracked,
        )
        .expect("fixture should build");
        fs::write(
            fixture.path().join("nested").join("tracked.txt"),
            "changed\n",
        )
        .expect("tracked file should be modified");
        fs::write(
            fixture.path().join("pathspecs.txt"),
            "\nnested/tracked.txt\n",
        )
        .expect("pathspec file should be written");
        if command == "reset" {
            run_git(fixture.path(), ["add", "nested/tracked.txt"]);
        }

        let workspace = temp_path(&format!("{command}-empty-pathspec-compare"));
        let git_repo = workspace.join("git");
        let rit_repo = workspace.join("rit");
        copy_directory(fixture.path(), &git_repo);
        copy_directory(fixture.path(), &rit_repo);

        let git = run_command_allow_failure(
            &command_words("git", [command, "--pathspec-from-file", "pathspecs.txt"]),
            &git_repo,
        );
        let rit = run_command_allow_failure(
            &command_words(
                rit_binary(),
                [command, "--pathspec-from-file", "pathspecs.txt"],
            ),
            &rit_repo,
        );

        assert_eq!(git.exit_code, Some(128), "{command} git exit code");
        assert_eq!(rit.exit_code, git.exit_code, "{command} rit exit code");
        assert_eq!(rit.stdout, git.stdout, "{command} stdout");
        assert_eq!(rit.stderr, git.stderr, "{command} stderr");
        assert_eq!(
            run_capture("git", ["status", "--porcelain=v1"], &git_repo).0,
            run_capture(rit_binary(), ["status", "--porcelain=v1"], &rit_repo).0,
            "{command} status"
        );
        let _ = fs::remove_dir_all(workspace);
    }
}

#[test]
fn empty_nul_pathspec_from_file_is_rejected_like_git() {
    for command in ["add", "restore", "reset"] {
        let fixture = LocalWriteFixture::new(
            &format!("{command}-empty-nul-pathspec-file"),
            LocalWriteFixtureKind::NestedTracked,
        )
        .expect("fixture should build");
        fs::write(
            fixture.path().join("nested").join("tracked.txt"),
            "changed\n",
        )
        .expect("tracked file should be modified");
        fs::write(
            fixture.path().join("pathspecs.nul"),
            b"\0nested/tracked.txt\0",
        )
        .expect("pathspec file should be written");
        if command == "reset" {
            run_git(fixture.path(), ["add", "nested/tracked.txt"]);
        }

        let workspace = temp_path(&format!("{command}-empty-nul-pathspec-compare"));
        let git_repo = workspace.join("git");
        let rit_repo = workspace.join("rit");
        copy_directory(fixture.path(), &git_repo);
        copy_directory(fixture.path(), &rit_repo);

        let git = run_command_allow_failure(
            &command_words(
                "git",
                [
                    command,
                    "--pathspec-from-file",
                    "pathspecs.nul",
                    "--pathspec-file-nul",
                ],
            ),
            &git_repo,
        );
        let rit = run_command_allow_failure(
            &command_words(
                rit_binary(),
                [
                    command,
                    "--pathspec-from-file",
                    "pathspecs.nul",
                    "--pathspec-file-nul",
                ],
            ),
            &rit_repo,
        );

        assert_eq!(git.exit_code, Some(128), "{command} git exit code");
        assert_eq!(rit.exit_code, git.exit_code, "{command} rit exit code");
        assert_eq!(rit.stdout, git.stdout, "{command} stdout");
        assert_eq!(rit.stderr, git.stderr, "{command} stderr");
        assert_eq!(
            run_capture("git", ["status", "--porcelain=v1"], &git_repo).0,
            run_capture(rit_binary(), ["status", "--porcelain=v1"], &rit_repo).0,
            "{command} status"
        );
        let _ = fs::remove_dir_all(workspace);
    }
}

#[test]
fn badly_quoted_pathspec_from_file_is_rejected_like_git() {
    for command in ["add", "restore", "reset"] {
        let fixture = LocalWriteFixture::new(
            &format!("{command}-bad-quoted-pathspec-file"),
            LocalWriteFixtureKind::NestedTracked,
        )
        .expect("fixture should build");
        fs::write(
            fixture.path().join("nested").join("tracked.txt"),
            "changed\n",
        )
        .expect("tracked file should be modified");
        fs::write(
            fixture.path().join("pathspecs.txt"),
            "\"nested/tracked.txt\n",
        )
        .expect("pathspec file should be written");
        if command == "reset" {
            run_git(fixture.path(), ["add", "nested/tracked.txt"]);
        }

        let workspace = temp_path(&format!("{command}-bad-quoted-pathspec-compare"));
        let git_repo = workspace.join("git");
        let rit_repo = workspace.join("rit");
        copy_directory(fixture.path(), &git_repo);
        copy_directory(fixture.path(), &rit_repo);

        let git = run_command_allow_failure(
            &command_words("git", [command, "--pathspec-from-file", "pathspecs.txt"]),
            &git_repo,
        );
        let rit = run_command_allow_failure(
            &command_words(
                rit_binary(),
                [command, "--pathspec-from-file", "pathspecs.txt"],
            ),
            &rit_repo,
        );

        assert_eq!(git.exit_code, Some(128), "{command} git exit code");
        assert_eq!(rit.exit_code, git.exit_code, "{command} rit exit code");
        assert_eq!(rit.stdout, git.stdout, "{command} stdout");
        assert_eq!(rit.stderr, git.stderr, "{command} stderr");
        assert_eq!(
            run_capture("git", ["status", "--porcelain=v1"], &git_repo).0,
            run_capture(rit_binary(), ["status", "--porcelain=v1"], &rit_repo).0,
            "{command} status"
        );
        let _ = fs::remove_dir_all(workspace);
    }
}

#[test]
fn literal_and_glob_magic_pathspec_is_rejected_like_git() {
    for command in ["add", "restore", "reset"] {
        let fixture = LocalWriteFixture::new(
            &format!("{command}-literal-glob-pathspec"),
            LocalWriteFixtureKind::NestedTracked,
        )
        .expect("fixture should build");
        fs::write(
            fixture.path().join("nested").join("tracked.txt"),
            "changed\n",
        )
        .expect("tracked file should be modified");
        if command == "reset" {
            run_git(fixture.path(), ["add", "nested/tracked.txt"]);
        }

        let workspace = temp_path(&format!("{command}-literal-glob-pathspec-compare"));
        let git_repo = workspace.join("git");
        let rit_repo = workspace.join("rit");
        copy_directory(fixture.path(), &git_repo);
        copy_directory(fixture.path(), &rit_repo);

        let git = run_command_allow_failure(
            &command_words("git", [command, "--", ":(literal,glob)*.txt"]),
            &git_repo,
        );
        let rit = run_command_allow_failure(
            &command_words(rit_binary(), [command, "--", ":(literal,glob)*.txt"]),
            &rit_repo,
        );

        assert_eq!(git.exit_code, Some(128), "{command} git exit code");
        assert_eq!(rit.exit_code, git.exit_code, "{command} rit exit code");
        assert_eq!(rit.stdout, git.stdout, "{command} stdout");
        assert_eq!(rit.stderr, git.stderr, "{command} stderr");
        assert_eq!(
            run_capture("git", ["status", "--porcelain=v1"], &git_repo).0,
            run_capture(rit_binary(), ["status", "--porcelain=v1"], &rit_repo).0,
            "{command} status"
        );
        let _ = fs::remove_dir_all(workspace);
    }
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
fn restore_attr_pathspec_matches_git_status() {
    for (name, pathspec) in [
        ("set", ":(attr:text)*"),
        ("unset", ":(attr:-text)*"),
        ("value", ":(attr:diff=markdown)*"),
        ("unspecified", ":(attr:!diff)*"),
    ] {
        assert_write_attr_pathspec_matches_git("restore", name, pathspec, false);
    }
}

#[test]
fn reset_attr_pathspec_matches_git_status() {
    for (name, pathspec) in [
        ("set", ":(attr:text)*"),
        ("unset", ":(attr:-text)*"),
        ("value", ":(attr:diff=markdown)*"),
        ("unspecified", ":(attr:!diff)*"),
    ] {
        assert_write_attr_pathspec_matches_git("reset", name, pathspec, true);
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
fn add_rejects_core_ignorecase_false_mismatched_case_pathspec_like_git() {
    let fixture = temp_path("add-core-ignorecase-false-fixture");
    fs::create_dir_all(&fixture).expect("fixture should be created");
    run_git(&fixture, ["init", "--quiet"]);
    run_git(&fixture, ["config", "user.name", "Rit Test"]);
    run_git(&fixture, ["config", "user.email", "rit@example.test"]);
    run_git(&fixture, ["config", "core.autocrlf", "false"]);
    run_git(&fixture, ["config", "core.ignorecase", "false"]);
    fs::write(fixture.join("Camel.txt"), "base\n").expect("case file should be written");
    run_git(&fixture, ["add", "Camel.txt"]);
    run_git(&fixture, ["commit", "--quiet", "-m", "base"]);
    fs::write(fixture.join("Camel.txt"), "changed\n").expect("case file should be changed");

    let workspace = temp_path("add-core-ignorecase-false-compare");
    let git_repo = workspace.join("git");
    let rit_repo = workspace.join("rit");
    copy_directory(&fixture, &git_repo);
    copy_directory(&fixture, &rit_repo);

    let git = run_command_allow_failure(&command_words("git", ["add", "camel.txt"]), &git_repo);
    let rit = run_command_allow_failure(
        &command_words(rit_binary(), ["add", "camel.txt"]),
        &rit_repo,
    );
    let git_status = run_capture("git", ["status", "--porcelain=v1"], &git_repo).0;
    let rit_status = run_capture(rit_binary(), ["status", "--porcelain=v1"], &rit_repo).0;

    assert!(
        !git.success,
        "git should reject the mismatched-case pathspec"
    );
    assert_eq!(git.exit_code, rit.exit_code);
    assert_eq!(git.stdout, rit.stdout);
    assert_eq!(git.stderr, rit.stderr);
    assert_eq!(git_status, rit_status);
    let _ = fs::remove_dir_all(fixture);
    let _ = fs::remove_dir_all(workspace);
}

#[test]
fn reset_honors_core_ignorecase_for_mismatched_case_pathspec() {
    let fixture = temp_path("reset-core-ignorecase-fixture");
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
    run_git(&fixture, ["add", "Camel.txt"]);

    let outcome = compare_after_command(
        &fixture,
        command_words("git", ["reset", "camel.txt"]),
        command_words(rit_binary(), ["reset", "camel.txt"]),
    );

    assert_eq!(outcome.git_command_stdout, outcome.rit_command_stdout);
    assert_eq!(outcome.git_command_stderr, outcome.rit_command_stderr);
    assert_eq!(outcome.git_status, outcome.rit_status);
    let _ = fs::remove_dir_all(fixture);
}

#[test]
fn restore_rejects_core_ignorecase_mismatched_case_pathspec_like_git() {
    let fixture = temp_path("restore-core-ignorecase-fixture");
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

    let workspace = temp_path("restore-core-ignorecase-compare");
    let git_repo = workspace.join("git");
    let rit_repo = workspace.join("rit");
    copy_directory(&fixture, &git_repo);
    copy_directory(&fixture, &rit_repo);

    let git = run_command_allow_failure(&command_words("git", ["restore", "camel.txt"]), &git_repo);
    let rit = run_command_allow_failure(
        &command_words(rit_binary(), ["restore", "camel.txt"]),
        &rit_repo,
    );
    let git_status = run_capture("git", ["status", "--porcelain=v1"], &git_repo).0;
    let rit_status = run_capture(rit_binary(), ["status", "--porcelain=v1"], &rit_repo).0;

    assert!(
        !git.success,
        "git should reject the mismatched-case pathspec"
    );
    assert_eq!(git.exit_code, rit.exit_code);
    assert_eq!(git.stdout, rit.stdout);
    assert_eq!(git.stderr, rit.stderr);
    assert_eq!(git_status, rit_status);
    assert_eq!(
        fs::read_to_string(git_repo.join("Camel.txt")).expect("git file should read"),
        fs::read_to_string(rit_repo.join("Camel.txt")).expect("rit file should read")
    );
    let _ = fs::remove_dir_all(fixture);
    let _ = fs::remove_dir_all(workspace);
}

#[test]
fn reset_keeps_git_windows_baseline_no_op_for_core_ignorecase_false() {
    let fixture = case_pathspec_fixture("reset-core-ignorecase-false-fixture", false, true);
    let outcome = compare_after_command(
        &fixture,
        command_words("git", ["reset", "camel.txt"]),
        command_words(rit_binary(), ["reset", "camel.txt"]),
    );

    assert_eq!(outcome.git_command_stdout, outcome.rit_command_stdout);
    assert_eq!(outcome.git_command_stderr, outcome.rit_command_stderr);
    assert_eq!(outcome.git_status, outcome.rit_status);
    let _ = fs::remove_dir_all(fixture);
}

#[test]
fn restore_rejects_core_ignorecase_false_mismatched_case_pathspec_like_git() {
    let fixture = case_pathspec_fixture("restore-core-ignorecase-false-fixture", false, false);
    let workspace = temp_path("restore-core-ignorecase-false-compare");
    let git_repo = workspace.join("git");
    let rit_repo = workspace.join("rit");
    copy_directory(&fixture, &git_repo);
    copy_directory(&fixture, &rit_repo);

    let git = run_command_allow_failure(&command_words("git", ["restore", "camel.txt"]), &git_repo);
    let rit = run_command_allow_failure(
        &command_words(rit_binary(), ["restore", "camel.txt"]),
        &rit_repo,
    );
    let git_status = run_capture("git", ["status", "--porcelain=v1"], &git_repo).0;
    let rit_status = run_capture(rit_binary(), ["status", "--porcelain=v1"], &rit_repo).0;

    assert!(
        !git.success,
        "git should reject the mismatched-case pathspec"
    );
    assert_eq!(git.exit_code, rit.exit_code);
    assert_eq!(git.stdout, rit.stdout);
    assert_eq!(git.stderr, rit.stderr);
    assert_eq!(git_status, rit_status);
    assert_eq!(
        fs::read_to_string(git_repo.join("Camel.txt")).expect("git file should read"),
        fs::read_to_string(rit_repo.join("Camel.txt")).expect("rit file should read")
    );
    let _ = fs::remove_dir_all(fixture);
    let _ = fs::remove_dir_all(workspace);
}

#[test]
fn add_rejects_mismatched_case_wildcard_pathspec_like_git() {
    for core_ignorecase in [false, true] {
        let fixture = case_pathspec_fixture(
            &format!("add-core-ignorecase-wildcard-{core_ignorecase}"),
            core_ignorecase,
            false,
        );
        let workspace = temp_path(&format!(
            "add-core-ignorecase-wildcard-compare-{core_ignorecase}"
        ));
        let git_repo = workspace.join("git");
        let rit_repo = workspace.join("rit");
        copy_directory(&fixture, &git_repo);
        copy_directory(&fixture, &rit_repo);

        let git = run_command_allow_failure(&command_words("git", ["add", "camel*"]), &git_repo);
        let rit =
            run_command_allow_failure(&command_words(rit_binary(), ["add", "camel*"]), &rit_repo);
        let git_status = run_capture("git", ["status", "--porcelain=v1"], &git_repo).0;
        let rit_status = run_capture(rit_binary(), ["status", "--porcelain=v1"], &rit_repo).0;

        assert!(!git.success, "git should reject the wildcard pathspec");
        assert_eq!(git.exit_code, rit.exit_code);
        assert_eq!(git.stdout, rit.stdout);
        assert_eq!(git.stderr, rit.stderr);
        assert_eq!(git_status, rit_status);
        let _ = fs::remove_dir_all(fixture);
        let _ = fs::remove_dir_all(workspace);
    }
}

#[test]
fn reset_keeps_git_windows_baseline_no_op_for_mismatched_case_wildcard() {
    for core_ignorecase in [false, true] {
        let fixture = case_pathspec_fixture(
            &format!("reset-core-ignorecase-wildcard-{core_ignorecase}"),
            core_ignorecase,
            true,
        );
        let outcome = compare_after_command(
            &fixture,
            command_words("git", ["reset", "camel*"]),
            command_words(rit_binary(), ["reset", "camel*"]),
        );

        assert_eq!(outcome.git_command_stdout, outcome.rit_command_stdout);
        assert_eq!(outcome.git_command_stderr, outcome.rit_command_stderr);
        assert_eq!(outcome.git_status, outcome.rit_status);
        let _ = fs::remove_dir_all(fixture);
    }
}

#[test]
fn restore_rejects_mismatched_case_wildcard_pathspec_like_git() {
    for core_ignorecase in [false, true] {
        let fixture = case_pathspec_fixture(
            &format!("restore-core-ignorecase-wildcard-{core_ignorecase}"),
            core_ignorecase,
            false,
        );
        let workspace = temp_path(&format!(
            "restore-core-ignorecase-wildcard-compare-{core_ignorecase}"
        ));
        let git_repo = workspace.join("git");
        let rit_repo = workspace.join("rit");
        copy_directory(&fixture, &git_repo);
        copy_directory(&fixture, &rit_repo);

        let git =
            run_command_allow_failure(&command_words("git", ["restore", "camel*"]), &git_repo);
        let rit = run_command_allow_failure(
            &command_words(rit_binary(), ["restore", "camel*"]),
            &rit_repo,
        );
        let git_status = run_capture("git", ["status", "--porcelain=v1"], &git_repo).0;
        let rit_status = run_capture(rit_binary(), ["status", "--porcelain=v1"], &rit_repo).0;

        assert!(!git.success, "git should reject the wildcard pathspec");
        assert_eq!(git.exit_code, rit.exit_code);
        assert_eq!(git.stdout, rit.stdout);
        assert_eq!(git.stderr, rit.stderr);
        assert_eq!(git_status, rit_status);
        assert_eq!(
            fs::read_to_string(git_repo.join("Camel.txt")).expect("git file should read"),
            fs::read_to_string(rit_repo.join("Camel.txt")).expect("rit file should read")
        );
        let _ = fs::remove_dir_all(fixture);
        let _ = fs::remove_dir_all(workspace);
    }
}

#[test]
fn add_matches_git_for_mismatched_case_literal_pathspec() {
    for core_ignorecase in [false, true] {
        let fixture = case_pathspec_fixture(
            &format!("add-core-ignorecase-literal-{core_ignorecase}"),
            core_ignorecase,
            false,
        );
        let workspace = temp_path(&format!(
            "add-core-ignorecase-literal-compare-{core_ignorecase}"
        ));
        let git_repo = workspace.join("git");
        let rit_repo = workspace.join("rit");
        copy_directory(&fixture, &git_repo);
        copy_directory(&fixture, &rit_repo);

        let git = run_command_allow_failure(
            &command_words("git", ["add", ":(literal)camel.txt"]),
            &git_repo,
        );
        let rit = run_command_allow_failure(
            &command_words(rit_binary(), ["add", ":(literal)camel.txt"]),
            &rit_repo,
        );
        let git_status = run_capture("git", ["status", "--porcelain=v1"], &git_repo).0;
        let rit_status = run_capture(rit_binary(), ["status", "--porcelain=v1"], &rit_repo).0;

        assert_eq!(git.exit_code, rit.exit_code);
        assert_eq!(git.stdout, rit.stdout);
        assert_eq!(git.stderr, rit.stderr);
        assert_eq!(git_status, rit_status);
        let _ = fs::remove_dir_all(fixture);
        let _ = fs::remove_dir_all(workspace);
    }
}

#[test]
fn reset_keeps_git_windows_baseline_no_op_for_mismatched_case_literal() {
    for core_ignorecase in [false, true] {
        let fixture = case_pathspec_fixture(
            &format!("reset-core-ignorecase-literal-{core_ignorecase}"),
            core_ignorecase,
            true,
        );
        let outcome = compare_after_command(
            &fixture,
            command_words("git", ["reset", ":(literal)camel.txt"]),
            command_words(rit_binary(), ["reset", ":(literal)camel.txt"]),
        );

        assert_eq!(outcome.git_command_stdout, outcome.rit_command_stdout);
        assert_eq!(outcome.git_command_stderr, outcome.rit_command_stderr);
        assert_eq!(outcome.git_status, outcome.rit_status);
        let _ = fs::remove_dir_all(fixture);
    }
}

#[test]
fn restore_rejects_mismatched_case_literal_pathspec_like_git() {
    for core_ignorecase in [false, true] {
        let fixture = case_pathspec_fixture(
            &format!("restore-core-ignorecase-literal-{core_ignorecase}"),
            core_ignorecase,
            false,
        );
        let workspace = temp_path(&format!(
            "restore-core-ignorecase-literal-compare-{core_ignorecase}"
        ));
        let git_repo = workspace.join("git");
        let rit_repo = workspace.join("rit");
        copy_directory(&fixture, &git_repo);
        copy_directory(&fixture, &rit_repo);

        let git = run_command_allow_failure(
            &command_words("git", ["restore", ":(literal)camel.txt"]),
            &git_repo,
        );
        let rit = run_command_allow_failure(
            &command_words(rit_binary(), ["restore", ":(literal)camel.txt"]),
            &rit_repo,
        );
        let git_status = run_capture("git", ["status", "--porcelain=v1"], &git_repo).0;
        let rit_status = run_capture(rit_binary(), ["status", "--porcelain=v1"], &rit_repo).0;

        assert_eq!(git.exit_code, rit.exit_code);
        assert_eq!(git.stdout, rit.stdout);
        assert_eq!(git.stderr, rit.stderr);
        assert_eq!(git_status, rit_status);
        assert_eq!(
            fs::read_to_string(git_repo.join("Camel.txt")).expect("git file should read"),
            fs::read_to_string(rit_repo.join("Camel.txt")).expect("rit file should read")
        );
        let _ = fs::remove_dir_all(fixture);
        let _ = fs::remove_dir_all(workspace);
    }
}

#[test]
fn add_matches_git_for_mismatched_case_top_magic_pathspec() {
    for core_ignorecase in [false, true] {
        let fixture = case_pathspec_fixture(
            &format!("add-core-ignorecase-top-magic-{core_ignorecase}"),
            core_ignorecase,
            false,
        );
        let workspace = temp_path(&format!(
            "add-core-ignorecase-top-magic-compare-{core_ignorecase}"
        ));
        let git_repo = workspace.join("git");
        let rit_repo = workspace.join("rit");
        copy_directory(&fixture, &git_repo);
        copy_directory(&fixture, &rit_repo);

        let git =
            run_command_allow_failure(&command_words("git", ["add", ":(top)camel.txt"]), &git_repo);
        let rit = run_command_allow_failure(
            &command_words(rit_binary(), ["add", ":(top)camel.txt"]),
            &rit_repo,
        );
        let git_status = run_capture("git", ["status", "--porcelain=v1"], &git_repo).0;
        let rit_status = run_capture(rit_binary(), ["status", "--porcelain=v1"], &rit_repo).0;

        assert_eq!(git.exit_code, rit.exit_code);
        assert_eq!(git.stdout, rit.stdout);
        assert_eq!(git.stderr, rit.stderr);
        assert_eq!(git_status, rit_status);
        let _ = fs::remove_dir_all(fixture);
        let _ = fs::remove_dir_all(workspace);
    }
}

#[test]
fn reset_keeps_git_windows_baseline_no_op_for_mismatched_case_top_magic() {
    for core_ignorecase in [false, true] {
        let fixture = case_pathspec_fixture(
            &format!("reset-core-ignorecase-top-magic-{core_ignorecase}"),
            core_ignorecase,
            true,
        );
        let outcome = compare_after_command(
            &fixture,
            command_words("git", ["reset", ":(top)camel.txt"]),
            command_words(rit_binary(), ["reset", ":(top)camel.txt"]),
        );

        assert_eq!(outcome.git_command_stdout, outcome.rit_command_stdout);
        assert_eq!(outcome.git_command_stderr, outcome.rit_command_stderr);
        assert_eq!(outcome.git_status, outcome.rit_status);
        let _ = fs::remove_dir_all(fixture);
    }
}

#[test]
fn restore_rejects_mismatched_case_top_magic_pathspec_like_git() {
    for core_ignorecase in [false, true] {
        let fixture = case_pathspec_fixture(
            &format!("restore-core-ignorecase-top-magic-{core_ignorecase}"),
            core_ignorecase,
            false,
        );
        let workspace = temp_path(&format!(
            "restore-core-ignorecase-top-magic-compare-{core_ignorecase}"
        ));
        let git_repo = workspace.join("git");
        let rit_repo = workspace.join("rit");
        copy_directory(&fixture, &git_repo);
        copy_directory(&fixture, &rit_repo);

        let git = run_command_allow_failure(
            &command_words("git", ["restore", ":(top)camel.txt"]),
            &git_repo,
        );
        let rit = run_command_allow_failure(
            &command_words(rit_binary(), ["restore", ":(top)camel.txt"]),
            &rit_repo,
        );
        let git_status = run_capture("git", ["status", "--porcelain=v1"], &git_repo).0;
        let rit_status = run_capture(rit_binary(), ["status", "--porcelain=v1"], &rit_repo).0;

        assert_eq!(git.exit_code, rit.exit_code);
        assert_eq!(git.stdout, rit.stdout);
        assert_eq!(git.stderr, rit.stderr);
        assert_eq!(git_status, rit_status);
        assert_eq!(
            fs::read_to_string(git_repo.join("Camel.txt")).expect("git file should read"),
            fs::read_to_string(rit_repo.join("Camel.txt")).expect("rit file should read")
        );
        let _ = fs::remove_dir_all(fixture);
        let _ = fs::remove_dir_all(workspace);
    }
}

#[test]
fn add_matches_git_for_mismatched_case_root_shorthand_pathspec() {
    for core_ignorecase in [false, true] {
        let fixture = case_pathspec_fixture(
            &format!("add-core-ignorecase-root-shorthand-{core_ignorecase}"),
            core_ignorecase,
            false,
        );
        let workspace = temp_path(&format!(
            "add-core-ignorecase-root-shorthand-compare-{core_ignorecase}"
        ));
        let git_repo = workspace.join("git");
        let rit_repo = workspace.join("rit");
        copy_directory(&fixture, &git_repo);
        copy_directory(&fixture, &rit_repo);

        let git =
            run_command_allow_failure(&command_words("git", ["add", ":/camel.txt"]), &git_repo);
        let rit = run_command_allow_failure(
            &command_words(rit_binary(), ["add", ":/camel.txt"]),
            &rit_repo,
        );
        let git_status = run_capture("git", ["status", "--porcelain=v1"], &git_repo).0;
        let rit_status = run_capture(rit_binary(), ["status", "--porcelain=v1"], &rit_repo).0;

        assert_eq!(git.exit_code, rit.exit_code);
        assert_eq!(git.stdout, rit.stdout);
        assert_eq!(git.stderr, rit.stderr);
        assert_eq!(git_status, rit_status);
        let _ = fs::remove_dir_all(fixture);
        let _ = fs::remove_dir_all(workspace);
    }
}

#[test]
fn reset_keeps_git_windows_baseline_no_op_for_mismatched_case_root_shorthand() {
    for core_ignorecase in [false, true] {
        let fixture = case_pathspec_fixture(
            &format!("reset-core-ignorecase-root-shorthand-{core_ignorecase}"),
            core_ignorecase,
            true,
        );
        let outcome = compare_after_command(
            &fixture,
            command_words("git", ["reset", ":/camel.txt"]),
            command_words(rit_binary(), ["reset", ":/camel.txt"]),
        );

        assert_eq!(outcome.git_command_stdout, outcome.rit_command_stdout);
        assert_eq!(outcome.git_command_stderr, outcome.rit_command_stderr);
        assert_eq!(outcome.git_status, outcome.rit_status);
        let _ = fs::remove_dir_all(fixture);
    }
}

#[test]
fn restore_rejects_mismatched_case_root_shorthand_pathspec_like_git() {
    for core_ignorecase in [false, true] {
        let fixture = case_pathspec_fixture(
            &format!("restore-core-ignorecase-root-shorthand-{core_ignorecase}"),
            core_ignorecase,
            false,
        );
        let workspace = temp_path(&format!(
            "restore-core-ignorecase-root-shorthand-compare-{core_ignorecase}"
        ));
        let git_repo = workspace.join("git");
        let rit_repo = workspace.join("rit");
        copy_directory(&fixture, &git_repo);
        copy_directory(&fixture, &rit_repo);

        let git =
            run_command_allow_failure(&command_words("git", ["restore", ":/camel.txt"]), &git_repo);
        let rit = run_command_allow_failure(
            &command_words(rit_binary(), ["restore", ":/camel.txt"]),
            &rit_repo,
        );
        let git_status = run_capture("git", ["status", "--porcelain=v1"], &git_repo).0;
        let rit_status = run_capture(rit_binary(), ["status", "--porcelain=v1"], &rit_repo).0;

        assert_eq!(git.exit_code, rit.exit_code);
        assert_eq!(git.stdout, rit.stdout);
        assert_eq!(git.stderr, rit.stderr);
        assert_eq!(git_status, rit_status);
        assert_eq!(
            fs::read_to_string(git_repo.join("Camel.txt")).expect("git file should read"),
            fs::read_to_string(rit_repo.join("Camel.txt")).expect("rit file should read")
        );
        let _ = fs::remove_dir_all(fixture);
        let _ = fs::remove_dir_all(workspace);
    }
}

#[test]
fn add_matches_git_for_mismatched_case_glob_magic_pathspec() {
    for core_ignorecase in [false, true] {
        let fixture = case_pathspec_fixture(
            &format!("add-core-ignorecase-glob-magic-{core_ignorecase}"),
            core_ignorecase,
            false,
        );
        let workspace = temp_path(&format!(
            "add-core-ignorecase-glob-magic-compare-{core_ignorecase}"
        ));
        let git_repo = workspace.join("git");
        let rit_repo = workspace.join("rit");
        copy_directory(&fixture, &git_repo);
        copy_directory(&fixture, &rit_repo);

        let git = run_command_allow_failure(
            &command_words("git", ["add", ":(glob)camel.txt"]),
            &git_repo,
        );
        let rit = run_command_allow_failure(
            &command_words(rit_binary(), ["add", ":(glob)camel.txt"]),
            &rit_repo,
        );
        let git_status = run_capture("git", ["status", "--porcelain=v1"], &git_repo).0;
        let rit_status = run_capture(rit_binary(), ["status", "--porcelain=v1"], &rit_repo).0;

        assert_eq!(git.exit_code, rit.exit_code);
        assert_eq!(git.stdout, rit.stdout);
        assert_eq!(git.stderr, rit.stderr);
        assert_eq!(git_status, rit_status);
        let _ = fs::remove_dir_all(fixture);
        let _ = fs::remove_dir_all(workspace);
    }
}

#[test]
fn reset_keeps_git_windows_baseline_no_op_for_mismatched_case_glob_magic() {
    for core_ignorecase in [false, true] {
        let fixture = case_pathspec_fixture(
            &format!("reset-core-ignorecase-glob-magic-{core_ignorecase}"),
            core_ignorecase,
            true,
        );
        let outcome = compare_after_command(
            &fixture,
            command_words("git", ["reset", ":(glob)camel.txt"]),
            command_words(rit_binary(), ["reset", ":(glob)camel.txt"]),
        );

        assert_eq!(outcome.git_command_stdout, outcome.rit_command_stdout);
        assert_eq!(outcome.git_command_stderr, outcome.rit_command_stderr);
        assert_eq!(outcome.git_status, outcome.rit_status);
        let _ = fs::remove_dir_all(fixture);
    }
}

#[test]
fn restore_rejects_mismatched_case_glob_magic_pathspec_like_git() {
    for core_ignorecase in [false, true] {
        let fixture = case_pathspec_fixture(
            &format!("restore-core-ignorecase-glob-magic-{core_ignorecase}"),
            core_ignorecase,
            false,
        );
        let workspace = temp_path(&format!(
            "restore-core-ignorecase-glob-magic-compare-{core_ignorecase}"
        ));
        let git_repo = workspace.join("git");
        let rit_repo = workspace.join("rit");
        copy_directory(&fixture, &git_repo);
        copy_directory(&fixture, &rit_repo);

        let git = run_command_allow_failure(
            &command_words("git", ["restore", ":(glob)camel.txt"]),
            &git_repo,
        );
        let rit = run_command_allow_failure(
            &command_words(rit_binary(), ["restore", ":(glob)camel.txt"]),
            &rit_repo,
        );
        let git_status = run_capture("git", ["status", "--porcelain=v1"], &git_repo).0;
        let rit_status = run_capture(rit_binary(), ["status", "--porcelain=v1"], &rit_repo).0;

        assert_eq!(git.exit_code, rit.exit_code);
        assert_eq!(git.stdout, rit.stdout);
        assert_eq!(git.stderr, rit.stderr);
        assert_eq!(git_status, rit_status);
        assert_eq!(
            fs::read_to_string(git_repo.join("Camel.txt")).expect("git file should read"),
            fs::read_to_string(rit_repo.join("Camel.txt")).expect("rit file should read")
        );
        let _ = fs::remove_dir_all(fixture);
        let _ = fs::remove_dir_all(workspace);
    }
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
fn restore_posix_bracket_pathspec_matches_git_status_and_files() {
    let fixture = LocalWriteFixture::new(
        "restore-posix-bracket",
        LocalWriteFixtureKind::NestedTracked,
    )
    .expect("fixture should build");
    let numbered_file = fixture.path().join("1.txt");
    let letter_file = fixture.path().join("a.txt");
    fs::write(&numbered_file, "number\n").expect("number file should be written");
    fs::write(&letter_file, "letter\n").expect("letter file should be written");
    run_git(fixture.path(), ["add", "1.txt", "a.txt"]);
    run_git(fixture.path(), ["commit", "-m", "add files"]);
    fs::write(&numbered_file, "changed number\n").expect("number file should be changed");
    fs::write(&letter_file, "changed letter\n").expect("letter file should be changed");

    let outcome = compare_after_command(
        fixture.path(),
        command_words("git", ["restore", "[[:digit:]].txt"]),
        command_words(rit_binary(), ["restore", "[[:digit:]].txt"]),
    );

    assert_eq!(outcome.git_command_stdout, outcome.rit_command_stdout);
    assert_eq!(outcome.git_command_stderr, outcome.rit_command_stderr);
    assert_eq!(outcome.git_status, outcome.rit_status);
    assert_eq!(
        fs::read_to_string(outcome.git_repo.join("1.txt")).expect("git number file should read"),
        fs::read_to_string(outcome.rit_repo.join("1.txt")).expect("rit number file should read")
    );
    assert_eq!(
        fs::read_to_string(outcome.git_repo.join("a.txt")).expect("git letter file should read"),
        fs::read_to_string(outcome.rit_repo.join("a.txt")).expect("rit letter file should read")
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
fn restore_icase_magic_pathspec_matches_git_status_and_files() {
    let fixture = case_pathspec_fixture("restore-icase-magic-fixture", false, false);

    let outcome = compare_after_command(
        &fixture,
        command_words("git", ["restore", ":(icase)camel.txt"]),
        command_words(rit_binary(), ["restore", ":(icase)camel.txt"]),
    );

    assert_eq!(outcome.git_command_stdout, outcome.rit_command_stdout);
    assert_eq!(outcome.git_command_stderr, outcome.rit_command_stderr);
    assert_eq!(outcome.git_status, outcome.rit_status);
    assert_eq!(
        fs::read_to_string(outcome.git_repo.join("Camel.txt")).expect("git file should read"),
        fs::read_to_string(outcome.rit_repo.join("Camel.txt")).expect("rit file should read")
    );

    let _ = fs::remove_dir_all(fixture);
}

#[test]
fn restore_exclude_magic_pathspec_matches_git_status_and_files() {
    let fixture = LocalWriteFixture::new(
        "restore-exclude-magic",
        LocalWriteFixtureKind::NestedTracked,
    )
    .expect("fixture should build");
    fs::write(fixture.path().join("top.txt"), "top base\n").expect("top file should be written");
    fs::write(
        fixture.path().join("nested").join("skip.txt"),
        "skip base\n",
    )
    .expect("skip file should be written");
    run_git(fixture.path(), ["add", "top.txt", "nested/skip.txt"]);
    run_git(
        fixture.path(),
        ["commit", "--quiet", "-m", "extra tracked files"],
    );
    fs::write(fixture.path().join("top.txt"), "top changed\n").expect("top file should change");
    fs::write(
        fixture.path().join("nested").join("tracked.txt"),
        "tracked changed\n",
    )
    .expect("tracked file should change");
    fs::write(
        fixture.path().join("nested").join("skip.txt"),
        "skip changed\n",
    )
    .expect("skip file should change");

    let outcome = compare_after_command(
        fixture.path(),
        command_words(
            "git",
            ["restore", "*.txt", "nested/*.txt", ":!nested/skip.txt"],
        ),
        command_words(
            rit_binary(),
            ["restore", "*.txt", "nested/*.txt", ":!nested/skip.txt"],
        ),
    );

    assert_eq!(outcome.git_command_stdout, outcome.rit_command_stdout);
    assert_eq!(outcome.git_command_stderr, outcome.rit_command_stderr);
    assert_eq!(outcome.git_status, outcome.rit_status);
    for relative_path in ["top.txt", "nested/tracked.txt", "nested/skip.txt"] {
        assert_eq!(
            fs::read_to_string(outcome.git_repo.join(relative_path)).expect("git file should read"),
            fs::read_to_string(outcome.rit_repo.join(relative_path)).expect("rit file should read")
        );
    }
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
fn restore_nul_pathspec_file_option_order_matches_git_status_and_files() {
    let fixture = LocalWriteFixture::new(
        "restore-pathspec-file-nul-option-order",
        LocalWriteFixtureKind::NestedTracked,
    )
    .expect("fixture should build");
    fs::write(
        fixture.path().join("nested").join("tracked.txt"),
        "changed\n",
    )
    .expect("tracked file should be modified");
    fs::write(
        fixture.path().join("pathspecs.nul"),
        b"nested/tracked.txt\0",
    )
    .expect("pathspec file should be written");

    let outcome = compare_after_command(
        fixture.path(),
        command_words(
            "git",
            [
                "restore",
                "--pathspec-file-nul",
                "--pathspec-from-file",
                "pathspecs.nul",
            ],
        ),
        command_words(
            rit_binary(),
            [
                "restore",
                "--pathspec-file-nul",
                "--pathspec-from-file",
                "pathspecs.nul",
            ],
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
fn reset_posix_bracket_pathspec_matches_git_status() {
    let fixture =
        LocalWriteFixture::new("reset-posix-bracket", LocalWriteFixtureKind::NestedTracked)
            .expect("fixture should build");
    fs::write(fixture.path().join("1.txt"), "number\n").expect("number file should be written");
    fs::write(fixture.path().join("a.txt"), "letter\n").expect("letter file should be written");
    run_git(fixture.path(), ["add", "1.txt", "a.txt"]);

    let outcome = compare_after_command(
        fixture.path(),
        command_words("git", ["reset", "[[:digit:]].txt"]),
        command_words(rit_binary(), ["reset", "[[:digit:]].txt"]),
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
fn reset_icase_magic_pathspec_matches_git_status() {
    let fixture = case_pathspec_fixture("reset-icase-magic-fixture", false, true);

    let outcome = compare_after_command(
        &fixture,
        command_words("git", ["reset", ":(icase)camel.txt"]),
        command_words(rit_binary(), ["reset", ":(icase)camel.txt"]),
    );

    assert_eq!(outcome.git_command_stdout, outcome.rit_command_stdout);
    assert_eq!(outcome.git_command_stderr, outcome.rit_command_stderr);
    assert_eq!(outcome.git_status, outcome.rit_status);

    let _ = fs::remove_dir_all(fixture);
}

#[test]
fn reset_exclude_magic_pathspec_matches_git_status() {
    let fixture =
        LocalWriteFixture::new("reset-exclude-magic", LocalWriteFixtureKind::NestedTracked)
            .expect("fixture should build");
    fs::write(fixture.path().join("top.txt"), "top base\n").expect("top file should be written");
    fs::write(
        fixture.path().join("nested").join("skip.txt"),
        "skip base\n",
    )
    .expect("skip file should be written");
    run_git(fixture.path(), ["add", "top.txt", "nested/skip.txt"]);
    run_git(
        fixture.path(),
        ["commit", "--quiet", "-m", "extra tracked files"],
    );
    fs::write(fixture.path().join("top.txt"), "top changed\n").expect("top file should change");
    fs::write(
        fixture.path().join("nested").join("tracked.txt"),
        "tracked changed\n",
    )
    .expect("tracked file should change");
    fs::write(
        fixture.path().join("nested").join("skip.txt"),
        "skip changed\n",
    )
    .expect("skip file should change");
    run_git(
        fixture.path(),
        ["add", "top.txt", "nested/tracked.txt", "nested/skip.txt"],
    );

    let outcome = compare_after_command(
        fixture.path(),
        command_words(
            "git",
            ["reset", "*.txt", "nested/*.txt", ":!nested/skip.txt"],
        ),
        command_words(
            rit_binary(),
            ["reset", "*.txt", "nested/*.txt", ":!nested/skip.txt"],
        ),
    );

    assert_eq!(outcome.git_command_stdout, outcome.rit_command_stdout);
    assert_eq!(outcome.git_command_stderr, outcome.rit_command_stderr);
    assert_eq!(outcome.git_status, outcome.rit_status);
}

#[test]
fn restore_no_pathspec_file_nul_reverts_to_text_mode_like_git() {
    let fixture = LocalWriteFixture::new(
        "restore-no-pathspec-file-nul",
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
        command_words(
            "git",
            [
                "restore",
                "--pathspec-from-file=pathspecs.txt",
                "--pathspec-file-nul",
                "--no-pathspec-file-nul",
            ],
        ),
        command_words(
            rit_binary(),
            [
                "restore",
                "--pathspec-from-file=pathspecs.txt",
                "--pathspec-file-nul",
                "--no-pathspec-file-nul",
            ],
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
fn restore_no_pathspec_file_nul_option_order_matches_git_status_and_files() {
    let fixture = LocalWriteFixture::new(
        "restore-no-pathspec-file-nul-option-order",
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
        command_words(
            "git",
            [
                "restore",
                "--pathspec-file-nul",
                "--no-pathspec-file-nul",
                "--pathspec-from-file",
                "pathspecs.txt",
            ],
        ),
        command_words(
            rit_binary(),
            [
                "restore",
                "--pathspec-file-nul",
                "--no-pathspec-file-nul",
                "--pathspec-from-file",
                "pathspecs.txt",
            ],
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
fn reset_no_pathspec_file_nul_reverts_to_text_mode_like_git() {
    let fixture = LocalWriteFixture::new(
        "reset-no-pathspec-file-nul",
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
    run_git(fixture.path(), ["add", "nested"]);

    let outcome = compare_after_command(
        fixture.path(),
        command_words(
            "git",
            [
                "reset",
                "--pathspec-from-file=pathspecs.txt",
                "--pathspec-file-nul",
                "--no-pathspec-file-nul",
            ],
        ),
        command_words(
            rit_binary(),
            [
                "reset",
                "--pathspec-from-file=pathspecs.txt",
                "--pathspec-file-nul",
                "--no-pathspec-file-nul",
            ],
        ),
    );

    assert_eq!(outcome.git_command_stdout, outcome.rit_command_stdout);
    assert_eq!(outcome.git_command_stderr, outcome.rit_command_stderr);
    assert_eq!(outcome.git_status, outcome.rit_status);
}

#[test]
fn reset_no_pathspec_file_nul_option_order_matches_git_status() {
    let fixture = LocalWriteFixture::new(
        "reset-no-pathspec-file-nul-option-order",
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
    run_git(fixture.path(), ["add", "nested"]);

    let outcome = compare_after_command(
        fixture.path(),
        command_words(
            "git",
            [
                "reset",
                "--pathspec-file-nul",
                "--no-pathspec-file-nul",
                "--pathspec-from-file",
                "pathspecs.txt",
            ],
        ),
        command_words(
            rit_binary(),
            [
                "reset",
                "--pathspec-file-nul",
                "--no-pathspec-file-nul",
                "--pathspec-from-file",
                "pathspecs.txt",
            ],
        ),
    );

    assert_eq!(outcome.git_command_stdout, outcome.rit_command_stdout);
    assert_eq!(outcome.git_command_stderr, outcome.rit_command_stderr);
    assert_eq!(outcome.git_status, outcome.rit_status);
}

#[test]
fn reset_nul_pathspec_file_option_order_matches_git_status() {
    let fixture = LocalWriteFixture::new(
        "reset-pathspec-file-nul-option-order",
        LocalWriteFixtureKind::NestedTracked,
    )
    .expect("fixture should build");
    fs::write(
        fixture.path().join("nested").join("tracked.txt"),
        "changed\n",
    )
    .expect("tracked file should be modified");
    run_git(fixture.path(), ["add", "nested"]);
    fs::write(
        fixture.path().join("pathspecs.nul"),
        b"nested/tracked.txt\0",
    )
    .expect("pathspec file should be written");

    let outcome = compare_after_command(
        fixture.path(),
        command_words(
            "git",
            [
                "reset",
                "--pathspec-file-nul",
                "--pathspec-from-file",
                "pathspecs.nul",
            ],
        ),
        command_words(
            rit_binary(),
            [
                "reset",
                "--pathspec-file-nul",
                "--pathspec-from-file",
                "pathspecs.nul",
            ],
        ),
    );

    assert_eq!(outcome.git_command_stdout, outcome.rit_command_stdout);
    assert_eq!(outcome.git_command_stderr, outcome.rit_command_stderr);
    assert_eq!(outcome.git_status, outcome.rit_status);
}

#[test]
fn reset_deprecated_stdin_matches_git_status() {
    let fixture = LocalWriteFixture::new(
        "reset-deprecated-stdin",
        LocalWriteFixtureKind::NestedTracked,
    )
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
        command_words_with_stdin("git", ["reset", "--stdin"], stdin),
        command_words_with_stdin(rit_binary(), ["reset", "--stdin"], stdin),
    );

    assert_eq!(outcome.git_command_stdout, outcome.rit_command_stdout);
    assert_eq!(outcome.git_command_stderr, outcome.rit_command_stderr);
    assert_eq!(outcome.git_status, outcome.rit_status);
}

#[test]
fn reset_deprecated_stdin_z_matches_git_status() {
    let fixture = LocalWriteFixture::new(
        "reset-deprecated-stdin-z",
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
        command_words_with_stdin("git", ["reset", "--stdin", "-z"], stdin),
        command_words_with_stdin(rit_binary(), ["reset", "--stdin", "-z"], stdin),
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
    let (json_log, json_warning) =
        run_capture(rit_binary(), ["op", "log", "--json"], fixture.path());
    assert_eq!(json_warning, "");
    let parsed_json: serde_json::Value =
        serde_json::from_str(&json_log).expect("op log JSON should parse");
    assert_eq!(parsed_json["records"][0]["command"], "commit");
    assert_eq!(parsed_json["records"][0]["summary"], "journaled");
    assert_eq!(
        parsed_json["records"][0]["changed_paths"][0],
        "nested/tracked.txt"
    );
    assert!(
        parsed_json["records"][0]["created_object_ids"][0]
            .as_str()
            .expect("created object id should be a string")
            .len()
            >= 40
    );
    assert!(
        parsed_json["warnings"]
            .as_array()
            .expect("warnings array")
            .is_empty()
    );
    assert!(json_log.contains("\"records\": ["));
    assert!(json_log.contains("\"command\": \"commit\""));
    assert!(json_log.contains("\"summary\": \"journaled\""));
    assert!(json_log.contains("\"before\": {"));
    assert!(json_log.contains("\"after\": {"));
    assert!(json_log.contains("\"changed_paths\": [\"nested/tracked.txt\"]"));
    assert!(json_log.contains("\"created_object_ids\": [\""));
    assert!(json_log.contains("\"warnings\": ["));
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
    let (json_after_bad_line, json_warning) =
        run_capture(rit_binary(), ["op", "log", "--json"], fixture.path());
    assert_eq!(json_warning, "");
    let parsed_json: serde_json::Value =
        serde_json::from_str(&json_after_bad_line).expect("warning JSON should parse");
    assert_eq!(parsed_json["records"][0]["summary"], "journaled");
    assert_eq!(parsed_json["warnings"][0]["line_number"], 3);
    assert!(
        parsed_json["warnings"][0]["message"]
            .as_str()
            .expect("warning message should be a string")
            .contains("operation journal")
    );
    assert!(json_after_bad_line.contains("\"summary\": \"journaled\""));
    assert!(json_after_bad_line.contains("\"warnings\": ["));
    assert!(json_after_bad_line.contains("\"line_number\":"));
    assert!(json_after_bad_line.contains("operation journal"));
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
fn undo_preserve_changes_moves_head_back_without_reverting_commit_contents() {
    let fixture = LocalWriteFixture::new(
        "operation-journal-commit-preserve",
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

    let undo_output = run_capture(rit_binary(), ["undo", "--preserve-changes"], fixture.path()).0;

    assert!(undo_output.contains("moved HEAD"));
    assert!(undo_output.contains("keeping the staged and working tree changes"));
    assert_eq!(
        run_capture("git", ["rev-parse", "HEAD"], fixture.path()).0,
        base
    );
    assert_eq!(
        fs::read_to_string(fixture.path().join("nested").join("tracked.txt"))
            .expect("tracked file should read"),
        "journaled\n"
    );
    assert_eq!(
        run_capture(rit_binary(), ["status", "--porcelain=v1"], fixture.path()).0,
        "M  nested/tracked.txt\n"
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
fn undo_after_add_restores_index_without_reverting_worktree() {
    let fixture = LocalWriteFixture::new(
        "operation-journal-add-undo",
        LocalWriteFixtureKind::NestedTracked,
    )
    .expect("fixture should build");
    fs::write(
        fixture.path().join("nested").join("tracked.txt"),
        "staged\n",
    )
    .expect("tracked file should be modified");

    run_capture(rit_binary(), ["add", "nested/tracked.txt"], fixture.path());
    assert_eq!(
        run_capture(rit_binary(), ["status", "--porcelain=v1"], fixture.path()).0,
        "M  nested/tracked.txt\n"
    );

    let undo_output = run_capture(rit_binary(), ["undo"], fixture.path()).0;

    assert!(undo_output.contains("restored index"));
    assert_eq!(
        run_capture(rit_binary(), ["status", "--porcelain=v1"], fixture.path()).0,
        " M nested/tracked.txt\n"
    );
    assert_eq!(
        fs::read_to_string(fixture.path().join("nested").join("tracked.txt"))
            .expect("tracked file should read"),
        "staged\n"
    );
}

#[test]
fn operation_journal_records_branch_and_tag_ref_commands() {
    let fixture = LocalWriteFixture::new(
        "operation-journal-refs",
        LocalWriteFixtureKind::NestedTracked,
    )
    .expect("fixture should build");

    run_capture(rit_binary(), ["branch", "scratch"], fixture.path());
    run_capture(rit_binary(), ["tag", "v1"], fixture.path());
    run_capture(rit_binary(), ["tag", "-d", "v1"], fixture.path());
    run_capture(rit_binary(), ["branch", "-d", "scratch"], fixture.path());

    let log = run_capture(rit_binary(), ["op", "log"], fixture.path()).0;
    assert!(log.contains(" branch "));
    assert!(log.contains("create branch scratch"));
    assert!(log.contains("delete branch scratch"));
    assert!(log.contains(" tag "));
    assert!(log.contains("create tag v1"));
    assert!(log.contains("delete tag v1"));

    let json_log = run_capture(rit_binary(), ["op", "log", "--json"], fixture.path()).0;
    let parsed_json: serde_json::Value =
        serde_json::from_str(&json_log).expect("op log JSON should parse");
    let summaries = parsed_json["records"]
        .as_array()
        .expect("records should be an array")
        .iter()
        .map(|record| {
            record["summary"]
                .as_str()
                .expect("summary should be a string")
                .to_owned()
        })
        .collect::<Vec<_>>();
    assert!(summaries.contains(&"create branch scratch".to_owned()));
    assert!(summaries.contains(&"delete branch scratch".to_owned()));
    assert!(summaries.contains(&"create tag v1".to_owned()));
    assert!(summaries.contains(&"delete tag v1".to_owned()));
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
fn merge_no_ff_option_order_matches_git_head_shape() {
    let fixture = temp_path("merge-no-ff-option-order-fixture");
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
    run_git(&fixture, ["commit", "--quiet", "-am", "topic"]);
    run_git(&fixture, ["checkout", "--quiet", "master"]);

    for args in [
        vec!["merge", "--no-ff", "topic"],
        vec!["merge", "--ff", "--no-ff", "topic"],
        vec!["merge", "--no-ff", "--ff", "topic"],
    ] {
        let outcome = compare_after_command(
            &fixture,
            command_words_vec("git", &args),
            command_words_vec(rit_binary(), &args),
        );

        assert_eq!(outcome.git_status, outcome.rit_status);
        assert_eq!(
            run_capture(
                "git",
                ["show", "--no-patch", "--pretty=%P", "HEAD"],
                &outcome.git_repo,
            )
            .0,
            run_capture(
                "git",
                ["show", "--no-patch", "--pretty=%P", "HEAD"],
                &outcome.rit_repo,
            )
            .0,
        );
        assert_eq!(
            run_capture(
                "git",
                ["show", "--no-patch", "--pretty=%T", "HEAD"],
                &outcome.git_repo,
            )
            .0,
            run_capture(
                "git",
                ["show", "--no-patch", "--pretty=%T", "HEAD"],
                &outcome.rit_repo,
            )
            .0,
        );
    }
    let _ = fs::remove_dir_all(fixture);
}

#[test]
fn merge_no_commit_matches_git_state_and_option_order() {
    let fixture = temp_path("merge-no-commit-fixture");
    fs::create_dir_all(&fixture).expect("fixture should be created");
    run_git(&fixture, ["init", "--quiet"]);
    run_git(&fixture, ["config", "user.name", "Rit Test"]);
    run_git(&fixture, ["config", "user.email", "rit@example.test"]);
    run_git(&fixture, ["config", "core.autocrlf", "false"]);
    fs::write(fixture.join("base.txt"), "base\n").expect("base file should be written");
    run_git(&fixture, ["add", "base.txt"]);
    run_git(&fixture, ["commit", "--quiet", "-m", "base"]);
    run_git(&fixture, ["checkout", "--quiet", "-b", "topic"]);
    fs::write(fixture.join("topic.txt"), "topic\n").expect("topic file should be written");
    run_git(&fixture, ["add", "topic.txt"]);
    run_git(&fixture, ["commit", "--quiet", "-m", "topic"]);
    run_git(&fixture, ["checkout", "--quiet", "master"]);
    fs::write(fixture.join("head.txt"), "head\n").expect("head file should be written");
    run_git(&fixture, ["add", "head.txt"]);
    run_git(&fixture, ["commit", "--quiet", "-m", "head"]);

    for args in [
        vec!["merge", "--no-commit", "topic"],
        vec!["merge", "--commit", "--no-commit", "topic"],
    ] {
        let outcome = compare_after_command(
            &fixture,
            command_words_vec("git", &args),
            command_words_vec(rit_binary(), &args),
        );

        assert_eq!(outcome.git_command_stdout, outcome.rit_command_stdout);
        assert_eq!(outcome.git_status, outcome.rit_status);
        assert_eq!(
            run_capture("git", ["rev-parse", "HEAD"], &outcome.git_repo).0,
            run_capture("git", ["rev-parse", "HEAD"], &outcome.rit_repo).0,
        );
        assert_eq!(
            run_capture("git", ["ls-files", "--stage"], &outcome.git_repo).0,
            run_capture(rit_binary(), ["ls-files", "--stage"], &outcome.rit_repo).0,
        );
        assert_eq!(
            read_repo_git_file(&outcome.git_repo, "MERGE_HEAD"),
            read_repo_git_file(&outcome.rit_repo, "MERGE_HEAD"),
        );
        assert_eq!(
            read_repo_git_file(&outcome.git_repo, "MERGE_MSG"),
            read_repo_git_file(&outcome.rit_repo, "MERGE_MSG"),
        );
        assert_eq!(
            read_repo_git_file(&outcome.git_repo, "MERGE_MODE"),
            read_repo_git_file(&outcome.rit_repo, "MERGE_MODE"),
        );
    }

    let committed_outcome = compare_after_command(
        &fixture,
        command_words("git", ["merge", "--no-commit", "--commit", "topic"]),
        command_words(rit_binary(), ["merge", "--no-commit", "--commit", "topic"]),
    );
    assert_eq!(committed_outcome.git_status, committed_outcome.rit_status);
    assert_eq!(
        read_repo_git_file(&committed_outcome.git_repo, "MERGE_HEAD"),
        read_repo_git_file(&committed_outcome.rit_repo, "MERGE_HEAD"),
    );
    assert_eq!(
        run_capture(
            "git",
            ["show", "--no-patch", "--pretty=%P", "HEAD"],
            &committed_outcome.git_repo,
        )
        .0
        .split_whitespace()
        .count(),
        run_capture(
            "git",
            ["show", "--no-patch", "--pretty=%P", "HEAD"],
            &committed_outcome.rit_repo,
        )
        .0
        .split_whitespace()
        .count(),
    );
    assert_eq!(
        run_capture(
            "git",
            ["show", "--no-patch", "--pretty=%T", "HEAD"],
            &committed_outcome.git_repo,
        )
        .0,
        run_capture(
            "git",
            ["show", "--no-patch", "--pretty=%T", "HEAD"],
            &committed_outcome.rit_repo,
        )
        .0,
    );

    let fast_forward_fixture = temp_path("merge-no-commit-fast-forward-fixture");
    fs::create_dir_all(&fast_forward_fixture).expect("fixture should be created");
    run_git(&fast_forward_fixture, ["init", "--quiet"]);
    run_git(&fast_forward_fixture, ["config", "user.name", "Rit Test"]);
    run_git(
        &fast_forward_fixture,
        ["config", "user.email", "rit@example.test"],
    );
    run_git(&fast_forward_fixture, ["config", "core.autocrlf", "false"]);
    fs::write(fast_forward_fixture.join("base.txt"), "base\n")
        .expect("base file should be written");
    run_git(&fast_forward_fixture, ["add", "base.txt"]);
    run_git(&fast_forward_fixture, ["commit", "--quiet", "-m", "base"]);
    run_git(
        &fast_forward_fixture,
        ["checkout", "--quiet", "-b", "topic"],
    );
    fs::write(fast_forward_fixture.join("topic.txt"), "topic\n")
        .expect("topic file should be written");
    run_git(&fast_forward_fixture, ["add", "topic.txt"]);
    run_git(&fast_forward_fixture, ["commit", "--quiet", "-m", "topic"]);
    run_git(&fast_forward_fixture, ["checkout", "--quiet", "master"]);

    let fast_forward_outcome = compare_after_command(
        &fast_forward_fixture,
        command_words("git", ["merge", "--no-commit", "topic"]),
        command_words(rit_binary(), ["merge", "--no-commit", "topic"]),
    );
    assert_eq!(
        fast_forward_outcome.git_status,
        fast_forward_outcome.rit_status
    );
    assert_eq!(
        read_repo_git_file(&fast_forward_outcome.git_repo, "MERGE_HEAD"),
        read_repo_git_file(&fast_forward_outcome.rit_repo, "MERGE_HEAD"),
    );
    assert_eq!(
        run_capture("git", ["rev-parse", "HEAD"], &fast_forward_outcome.git_repo).0,
        run_capture("git", ["rev-parse", "HEAD"], &fast_forward_outcome.rit_repo).0,
    );

    let forced_outcome = compare_after_command(
        &fast_forward_fixture,
        command_words("git", ["merge", "--no-ff", "--no-commit", "topic"]),
        command_words(rit_binary(), ["merge", "--no-ff", "--no-commit", "topic"]),
    );
    assert_eq!(
        forced_outcome.git_command_stdout,
        forced_outcome.rit_command_stdout
    );
    assert_eq!(forced_outcome.git_status, forced_outcome.rit_status);
    assert_eq!(
        read_repo_git_file(&forced_outcome.git_repo, "MERGE_HEAD"),
        read_repo_git_file(&forced_outcome.rit_repo, "MERGE_HEAD"),
    );
    assert_eq!(
        read_repo_git_file(&forced_outcome.git_repo, "MERGE_MSG"),
        read_repo_git_file(&forced_outcome.rit_repo, "MERGE_MSG"),
    );
    assert_eq!(
        read_repo_git_file(&forced_outcome.git_repo, "MERGE_MODE"),
        read_repo_git_file(&forced_outcome.rit_repo, "MERGE_MODE"),
    );

    let _ = fs::remove_dir_all(fixture);
    let _ = fs::remove_dir_all(fast_forward_fixture);
}

#[test]
fn merge_message_option_matches_git_commit_message_and_tree() {
    let fixture = merge_clean_non_fast_forward_fixture("merge-message-option-fixture");

    let outcome = compare_after_command(
        &fixture,
        command_words("git", ["merge", "-m", "Explain this merge", "topic"]),
        command_words(rit_binary(), ["merge", "-m", "Explain this merge", "topic"]),
    );

    assert_eq!(outcome.git_status, outcome.rit_status);
    assert_eq!(
        run_capture(
            "git",
            ["show", "--no-patch", "--pretty=%B", "HEAD"],
            &outcome.git_repo
        )
        .0,
        run_capture(
            "git",
            ["show", "--no-patch", "--pretty=%B", "HEAD"],
            &outcome.rit_repo
        )
        .0
    );
    assert_eq!(
        run_capture(
            "git",
            ["show", "--no-patch", "--pretty=%T", "HEAD"],
            &outcome.git_repo
        )
        .0,
        run_capture(
            "git",
            ["show", "--no-patch", "--pretty=%T", "HEAD"],
            &outcome.rit_repo
        )
        .0
    );
    assert_eq!(
        run_capture(
            "git",
            ["show", "--no-patch", "--pretty=%P", "HEAD"],
            &outcome.git_repo
        )
        .0,
        run_capture(
            "git",
            ["show", "--no-patch", "--pretty=%P", "HEAD"],
            &outcome.rit_repo
        )
        .0
    );
    let _ = fs::remove_dir_all(fixture);
}

#[test]
fn merge_message_option_writes_stopped_merge_message() {
    let fixture = merge_clean_non_fast_forward_fixture("merge-message-no-commit-fixture");

    let outcome = compare_after_command(
        &fixture,
        command_words(
            "git",
            ["merge", "--no-commit", "-m", "Stop and explain", "topic"],
        ),
        command_words(
            rit_binary(),
            ["merge", "--no-commit", "-m", "Stop and explain", "topic"],
        ),
    );

    assert_eq!(outcome.git_status, outcome.rit_status);
    assert_eq!(
        read_repo_git_file(&outcome.git_repo, "MERGE_MSG"),
        read_repo_git_file(&outcome.rit_repo, "MERGE_MSG")
    );
    assert_eq!(
        read_repo_git_file(&outcome.rit_repo, "MERGE_MSG"),
        Some("Stop and explain\n".to_owned())
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
    assert_rit_content_conflict_output(&stdout, "tracked.txt");
    assert!(stdout.contains("rit: merge stopped because some files need your help."));
    assert!(!stdout.contains("Recorded pre-merge target"));
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

fn assert_rit_content_conflict_output(stdout: &str, path: &str) {
    assert!(
        stdout.contains(&format!("rit: merge conflict in {path}\n")),
        "stdout should name the conflicted path in rit's own words:\n{stdout}"
    );
    assert!(
        stdout.contains("Both branches changed this file"),
        "stdout should explain why the user must resolve the file:\n{stdout}"
    );
}

#[test]
fn merge_quit_clears_state_without_touching_conflict_index_or_worktree() {
    let fixture = temp_path("merge-quit-fixture");
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
    fs::write(fixture.join("a.txt"), "head\n").expect("head file should be written");
    run_git(&fixture, ["add", "a.txt"]);
    run_git(&fixture, ["commit", "--quiet", "-m", "head"]);

    let merge = Command::new(rit_binary())
        .args(["merge", "topic"])
        .current_dir(&fixture)
        .output()
        .expect("rit merge should start");
    assert!(!merge.status.success());
    assert!(fixture.join(".git").join("MERGE_HEAD").exists());
    assert!(fixture.join(".git").join("MERGE_MSG").exists());
    let before_index = run_capture(rit_binary(), ["ls-files", "--stage"], &fixture).0;
    let before_worktree = fs::read_to_string(fixture.join("a.txt")).expect("file should read");

    let quit = Command::new(rit_binary())
        .args(["merge", "--quit"])
        .current_dir(&fixture)
        .output()
        .expect("rit merge --quit should run");

    assert!(quit.status.success());
    assert_eq!(String::from_utf8_lossy(&quit.stdout), "");
    assert_eq!(String::from_utf8_lossy(&quit.stderr), "");
    assert!(!fixture.join(".git").join("MERGE_HEAD").exists());
    assert!(!fixture.join(".git").join("MERGE_MSG").exists());
    assert!(fixture.join(".git").join("ORIG_HEAD").exists());
    assert_eq!(
        run_capture(rit_binary(), ["ls-files", "--stage"], &fixture).0,
        before_index
    );
    assert_eq!(
        fs::read_to_string(fixture.join("a.txt")).expect("file should read"),
        before_worktree
    );
    assert_eq!(
        run_capture(rit_binary(), ["status", "--porcelain=v1"], &fixture).0,
        "UU a.txt\n"
    );
    let op_log = run_capture(rit_binary(), ["op", "log"], &fixture).0;
    assert!(op_log.contains("quit merge"));
    let _ = fs::remove_dir_all(fixture);
}

#[test]
fn merge_quit_without_active_merge_succeeds_without_output() {
    let fixture = LocalWriteFixture::new("merge-quit-clean", LocalWriteFixtureKind::NestedTracked)
        .expect("fixture should initialize");

    let output = Command::new(rit_binary())
        .args(["merge", "--quit"])
        .current_dir(fixture.path())
        .output()
        .expect("rit merge --quit should run");

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "");
    assert_eq!(String::from_utf8_lossy(&output.stderr), "");
    assert_eq!(
        run_capture(rit_binary(), ["status", "--porcelain=v1"], fixture.path()).0,
        ""
    );
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
        "rit: a.txt was deleted on HEAD but changed on topic. rit left the topic version in your working tree so you can decide whether to keep or remove it.\n"
    ));
    assert!(!stdout.contains("Auto-merging a.txt\n"));
    assert!(!stdout.contains("Recorded pre-merge target"));
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
        "rit: a.txt was deleted on topic but changed on HEAD. rit left the HEAD version in your working tree so you can decide whether to keep or remove it.\n"
    ));
    assert!(!stdout.contains("Auto-merging a.txt\n"));
    assert!(!stdout.contains("Recorded pre-merge target"));
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
    assert!(stdout.contains("rit: binary merge conflict in blob.bin\n"));
    assert!(stdout.contains("rit cannot combine binary files safely"));
    assert!(!stdout.contains("Recorded pre-merge target"));
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
    assert_rit_content_conflict_output(&stdout, "a.sh");
    assert!(!stdout.contains("Recorded pre-merge target"));
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
    assert!(stdout.contains("rit: both branches added a.txt\n"));
    assert!(stdout.contains("Pick the final file contents"));
    assert!(!stdout.contains("Recorded pre-merge target"));
    assert_eq!(
        run_capture(rit_binary(), ["status", "--porcelain=v1"], &fixture).0,
        "AA a.txt\n"
    );
    let conflict_text = fs::read_to_string(fixture.join("a.txt")).expect("file should read");
    assert!(conflict_text.contains("<<<<<<< HEAD\nhead\n=======\ntopic\n>>>>>>> topic\n"));
    let _ = fs::remove_dir_all(fixture);
}

#[test]
fn merge_strategy_option_theirs_resolves_text_conflict_like_git() {
    let fixture = merge_text_conflict_fixture("merge-strategy-option-theirs");

    let outcome = compare_after_command(
        &fixture,
        command_words("git", ["merge", "-Xtheirs", "topic"]),
        command_words(rit_binary(), ["merge", "-Xtheirs", "topic"]),
    );

    assert_eq!(outcome.git_status, outcome.rit_status);
    assert_eq!(outcome.rit_status, "");
    assert_eq!(
        fs::read_to_string(outcome.git_repo.join("a.txt")).expect("git file should read"),
        fs::read_to_string(outcome.rit_repo.join("a.txt")).expect("rit file should read")
    );
    assert_eq!(
        fs::read_to_string(outcome.rit_repo.join("a.txt")).expect("rit file should read"),
        "topic\n"
    );
    assert_eq!(
        run_capture(
            "git",
            ["show", "--no-patch", "--pretty=%P", "HEAD"],
            &outcome.git_repo
        )
        .0,
        run_capture(
            "git",
            ["show", "--no-patch", "--pretty=%P", "HEAD"],
            &outcome.rit_repo
        )
        .0
    );
    assert!(!outcome.rit_repo.join(".git").join("MERGE_HEAD").exists());
    let _ = fs::remove_dir_all(fixture);
}

#[test]
fn merge_strategy_option_ours_resolves_text_conflict_like_git() {
    let fixture = merge_text_conflict_fixture("merge-strategy-option-ours");

    let outcome = compare_after_command(
        &fixture,
        command_words("git", ["merge", "--strategy-option=ours", "topic"]),
        command_words(rit_binary(), ["merge", "--strategy-option=ours", "topic"]),
    );

    assert_eq!(outcome.git_status, outcome.rit_status);
    assert_eq!(outcome.rit_status, "");
    assert_eq!(
        fs::read_to_string(outcome.git_repo.join("a.txt")).expect("git file should read"),
        fs::read_to_string(outcome.rit_repo.join("a.txt")).expect("rit file should read")
    );
    assert_eq!(
        fs::read_to_string(outcome.rit_repo.join("a.txt")).expect("rit file should read"),
        "head\n"
    );
    assert_eq!(
        run_capture(
            "git",
            ["show", "--no-patch", "--pretty=%P", "HEAD"],
            &outcome.git_repo
        )
        .0,
        run_capture(
            "git",
            ["show", "--no-patch", "--pretty=%P", "HEAD"],
            &outcome.rit_repo
        )
        .0
    );
    assert!(!outcome.rit_repo.join(".git").join("MERGE_HEAD").exists());
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
        "rit: a.txt has different file types on each side. rit kept both versions with separate names so you can choose the final shape.\n"
    ));
    assert!(!stdout.contains("Auto-merging a.txt\n"));
    assert!(!stdout.contains("Recorded pre-merge target"));
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
        "rit: a.txt has different file types on each side. rit kept both versions with separate names so you can choose the final shape.\n"
    ));
    assert!(!stdout.contains("Auto-merging a.txt\n"));
    assert!(!stdout.contains("Recorded pre-merge target"));
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
fn merge_ours_strategy_matches_git_tree_and_parents() {
    let fixture = temp_path("merge-ours-strategy-fixture");
    fs::create_dir_all(&fixture).expect("fixture should be created");
    run_git(&fixture, ["init", "--quiet"]);
    run_git(&fixture, ["config", "user.name", "Rit Test"]);
    run_git(&fixture, ["config", "user.email", "rit@example.test"]);
    run_git(&fixture, ["config", "core.autocrlf", "false"]);
    fs::write(fixture.join("tracked.txt"), "base\n").expect("base file should be written");
    run_git(&fixture, ["add", "tracked.txt"]);
    run_git(&fixture, ["commit", "--quiet", "-m", "base"]);
    run_git(&fixture, ["checkout", "--quiet", "-b", "topic"]);
    fs::write(fixture.join("topic.txt"), "topic\n").expect("topic file should be written");
    run_git(&fixture, ["add", "topic.txt"]);
    run_git(&fixture, ["commit", "--quiet", "-m", "topic"]);
    run_git(&fixture, ["checkout", "--quiet", "master"]);
    fs::write(fixture.join("head.txt"), "head\n").expect("head file should be written");
    run_git(&fixture, ["add", "head.txt"]);
    run_git(&fixture, ["commit", "--quiet", "-m", "head"]);

    for args in [
        vec!["merge", "-s", "ours", "topic"],
        vec!["merge", "--strategy", "ours", "topic"],
        vec!["merge", "--strategy=ours", "topic"],
        vec!["merge", "-sours", "topic"],
    ] {
        let outcome = compare_after_command(
            &fixture,
            command_words_vec("git", &args),
            command_words_vec(rit_binary(), &args),
        );

        assert_eq!(outcome.git_status, outcome.rit_status);
        assert_eq!(
            run_capture(
                "git",
                ["show", "--no-patch", "--pretty=%P", "HEAD"],
                &outcome.git_repo,
            )
            .0,
            run_capture(
                "git",
                ["show", "--no-patch", "--pretty=%P", "HEAD"],
                &outcome.rit_repo,
            )
            .0,
        );
        assert_eq!(
            run_capture(
                "git",
                ["show", "--no-patch", "--pretty=%T", "HEAD"],
                &outcome.git_repo,
            )
            .0,
            run_capture(
                "git",
                ["show", "--no-patch", "--pretty=%T", "HEAD"],
                &outcome.rit_repo,
            )
            .0,
        );
        assert_eq!(
            run_capture("git", ["ls-tree", "--name-only", "HEAD"], &outcome.rit_repo).0,
            "head.txt\ntracked.txt\n"
        );
    }
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
    fs::create_dir_all(&source).expect("source should be created");
    run_git(&source, ["init", "--quiet"]);
    run_git(&source, ["config", "user.name", "Rit Test"]);
    run_git(&source, ["config", "user.email", "rit@example.test"]);
    run_git(&source, ["config", "core.autocrlf", "false"]);
    fs::write(source.join("a.txt"), "base\n").expect("source file should be written");
    run_git(&source, ["add", "a.txt"]);
    run_git(&source, ["commit", "--quiet", "-m", "base"]);
    run_git(&source, ["tag", "v1"]);
    run_git(&source, ["switch", "--quiet", "-c", "topic"]);
    fs::write(source.join("a.txt"), "topic\n").expect("topic file should be written");
    run_git(&source, ["commit", "--quiet", "-am", "topic"]);
    run_git(&source, ["switch", "--quiet", "master"]);

    for (name, extra_args, origin_name, branch_name, tags_expected) in [
        ("default", Vec::<&str>::new(), "origin", "master", true),
        (
            "no-hardlinks",
            vec!["--no-hardlinks"],
            "origin",
            "master",
            true,
        ),
        ("tags", vec!["--tags"], "origin", "master", true),
        ("no-tags", vec!["--no-tags"], "origin", "master", false),
        (
            "origin-short",
            vec!["-o", "upstream"],
            "upstream",
            "master",
            true,
        ),
        (
            "origin-long",
            vec!["--origin=upstream"],
            "upstream",
            "master",
            true,
        ),
        ("branch-short", vec!["-b", "topic"], "origin", "topic", true),
        (
            "branch-long",
            vec!["--branch=topic"],
            "origin",
            "topic",
            true,
        ),
    ] {
        let git_target = workspace.join(format!("git-target-{name}"));
        let rit_target = workspace.join(format!("rit-target-{name}"));
        let mut git_args = vec!["clone", "-q", "--local", "--no-checkout"];
        git_args.extend(extra_args.iter().copied());
        let git_output = Command::new(git_program())
            .args(&git_args)
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

        let mut rit_args = vec!["clone", "-q", "--local", "--no-checkout"];
        rit_args.extend(extra_args.iter().copied());
        let rit_output = Command::new(rit_binary())
            .args(&rit_args)
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

        let expected_head_ref = format!("refs/heads/{branch_name}\n");
        let git_head_ref = run_capture("git", ["symbolic-ref", "HEAD"], &git_target).0;
        let rit_head_ref = run_capture("git", ["symbolic-ref", "HEAD"], &rit_target).0;
        assert_eq!(git_head_ref, expected_head_ref);
        assert_eq!(git_head_ref, rit_head_ref);

        let git_commit = run_capture("git", ["cat-file", "-p", "HEAD"], &git_target).0;
        let rit_commit = run_capture(rit_binary(), ["cat-file", "-p", "HEAD"], &rit_target).0;
        assert_eq!(git_commit, rit_commit);
        assert!(!rit_target.join("a.txt").exists());

        let remote_section = format!("remote.{origin_name}.url");
        let git_remote_url =
            run_capture("git", ["config", "--get", &remote_section], &git_target).0;
        let rit_remote_url =
            run_capture("git", ["config", "--get", &remote_section], &rit_target).0;
        assert_eq!(git_remote_url, rit_remote_url);

        let fetch_section = format!("remote.{origin_name}.fetch");
        let git_fetch = run_capture("git", ["config", "--get", &fetch_section], &git_target).0;
        let rit_fetch = run_capture("git", ["config", "--get", &fetch_section], &rit_target).0;
        assert_eq!(git_fetch, rit_fetch);

        let branch_remote = run_capture(
            "git",
            ["config", "--get", &format!("branch.{branch_name}.remote")],
            &git_target,
        )
        .0;
        let rit_branch_remote = run_capture(
            "git",
            ["config", "--get", &format!("branch.{branch_name}.remote")],
            &rit_target,
        )
        .0;
        assert_eq!(branch_remote, rit_branch_remote);

        let git_tags = run_capture("git", ["tag", "--list"], &git_target).0;
        let rit_tags = run_capture("git", ["tag", "--list"], &rit_target).0;
        assert_eq!(git_tags, rit_tags);
        assert_eq!(git_tags.contains("v1"), tags_expected);

        let tag_opt_section = format!("remote.{origin_name}.tagOpt");
        let git_tag_opt =
            run_optional_capture("git", ["config", "--get", &tag_opt_section], &git_target);
        let rit_tag_opt =
            run_optional_capture("git", ["config", "--get", &tag_opt_section], &rit_target);
        assert_eq!(git_tag_opt, rit_tag_opt);
    }

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
    let op_log = run_capture(rit_binary(), ["op", "log"], &rit_target).0;
    assert!(op_log.contains(" fetch "));
    assert!(op_log.contains("fetch "));

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
    let op_log = run_capture(rit_binary(), ["op", "log"], &rit_target).0;
    assert!(op_log.contains(" fetch "));
    assert!(op_log.contains("fetch "));

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

fn assert_write_attr_pathspec_matches_git(
    command: &str,
    name: &str,
    pathspec: &str,
    stage_before_compare: bool,
) {
    let fixture = AttrPathspecWriteFixture::new(&format!("{command}-attr-{name}"));
    if stage_before_compare {
        run_git(
            fixture.path(),
            ["add", "main.rs", "image.bin", "docs/readme.md", "plain.txt"],
        );
    }

    let outcome = compare_after_command(
        fixture.path(),
        command_words("git", [command, pathspec]),
        command_words(rit_binary(), [command, pathspec]),
    );

    assert_eq!(outcome.git_command_stdout, outcome.rit_command_stdout);
    assert_eq!(outcome.git_command_stderr, outcome.rit_command_stderr);
    assert_eq!(outcome.git_status, outcome.rit_status);
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

fn case_pathspec_fixture(name: &str, core_ignorecase: bool, stage_change: bool) -> PathBuf {
    let fixture = temp_path(name);
    fs::create_dir_all(&fixture).expect("fixture should be created");
    run_git(&fixture, ["init", "--quiet"]);
    run_git(&fixture, ["config", "user.name", "Rit Test"]);
    run_git(&fixture, ["config", "user.email", "rit@example.test"]);
    run_git(&fixture, ["config", "core.autocrlf", "false"]);
    run_git(
        &fixture,
        [
            "config",
            "core.ignorecase",
            if core_ignorecase { "true" } else { "false" },
        ],
    );
    fs::write(fixture.join("Camel.txt"), "base\n").expect("case file should be written");
    run_git(&fixture, ["add", "Camel.txt"]);
    run_git(&fixture, ["commit", "--quiet", "-m", "base"]);
    fs::write(fixture.join("Camel.txt"), "changed\n").expect("case file should be changed");
    if stage_change {
        run_git(&fixture, ["add", "Camel.txt"]);
    }
    fixture
}

fn build_write_glob_special_form_fixture(name: &str) -> LocalWriteFixture {
    let fixture = LocalWriteFixture::new(name, LocalWriteFixtureKind::NestedTracked)
        .expect("fixture should build");
    fs::write(fixture.path().join("top.txt"), "top base\n").expect("top file should be written");
    fs::create_dir_all(fixture.path().join("nested").join("deep"))
        .expect("deep directory should be created");
    fs::write(
        fixture.path().join("nested").join("deep").join("inner.txt"),
        "deep base\n",
    )
    .expect("deep file should be written");
    fs::write(fixture.path().join("nested").join("skip.md"), "skip base\n")
        .expect("markdown file should be written");
    run_git(fixture.path(), ["add", "top.txt", "nested"]);
    run_git(
        fixture.path(),
        ["commit", "--quiet", "-m", "expand tracked files"],
    );

    fs::write(fixture.path().join("top.txt"), "top changed\n").expect("top file should change");
    fs::write(
        fixture.path().join("nested").join("tracked.txt"),
        "tracked changed\n",
    )
    .expect("tracked file should change");
    fs::write(
        fixture.path().join("nested").join("deep").join("inner.txt"),
        "deep changed\n",
    )
    .expect("deep file should change");
    fs::write(
        fixture.path().join("nested").join("skip.md"),
        "skip changed\n",
    )
    .expect("markdown file should change");
    fixture
}

fn build_write_glob_component_local_fixture(name: &str) -> LocalWriteFixture {
    let fixture = LocalWriteFixture::new(name, LocalWriteFixtureKind::NestedTracked)
        .expect("fixture should build");
    fs::write(fixture.path().join("topbase.txt"), "top base\n")
        .expect("top base file should be written");
    fs::create_dir_all(fixture.path().join("nested").join("deep"))
        .expect("deep directory should be created");
    fs::write(
        fixture.path().join("nested").join("base.txt"),
        "nested base\n",
    )
    .expect("nested base file should be written");
    fs::write(
        fixture
            .path()
            .join("nested")
            .join("deep")
            .join("innerbase.txt"),
        "inner base\n",
    )
    .expect("deep base file should be written");
    run_git(
        fixture.path(),
        [
            "add",
            "topbase.txt",
            "nested/base.txt",
            "nested/deep/innerbase.txt",
        ],
    );
    run_git(
        fixture.path(),
        ["commit", "--quiet", "-m", "add component-local base files"],
    );

    fs::write(fixture.path().join("topbase.txt"), "top changed\n")
        .expect("top base file should change");
    fs::write(
        fixture.path().join("nested").join("base.txt"),
        "nested changed\n",
    )
    .expect("nested base file should change");
    fs::write(
        fixture
            .path()
            .join("nested")
            .join("deep")
            .join("innerbase.txt"),
        "inner changed\n",
    )
    .expect("deep base file should change");
    fixture
}

fn assert_matching_file_contents(left: &Path, right: &Path) {
    assert_eq!(
        fs::read_to_string(left).expect("left file should read"),
        fs::read_to_string(right).expect("right file should read")
    );
}

fn merge_text_conflict_fixture(name: &str) -> PathBuf {
    let fixture = temp_path(name);
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
    fs::write(fixture.join("a.txt"), "head\n").expect("head file should be written");
    run_git(&fixture, ["add", "a.txt"]);
    run_git(&fixture, ["commit", "--quiet", "-m", "head"]);
    fixture
}

fn merge_clean_non_fast_forward_fixture(name: &str) -> PathBuf {
    let fixture = temp_path(name);
    fs::create_dir_all(&fixture).expect("fixture should be created");
    run_git(&fixture, ["init", "--quiet"]);
    run_git(&fixture, ["config", "user.name", "Rit Test"]);
    run_git(&fixture, ["config", "user.email", "rit@example.test"]);
    run_git(&fixture, ["config", "core.autocrlf", "false"]);
    fs::write(fixture.join("base.txt"), "base\n").expect("base file should be written");
    run_git(&fixture, ["add", "base.txt"]);
    run_git(&fixture, ["commit", "--quiet", "-m", "base"]);
    run_git(&fixture, ["checkout", "--quiet", "-b", "topic"]);
    fs::write(fixture.join("topic.txt"), "topic\n").expect("topic file should be written");
    run_git(&fixture, ["add", "topic.txt"]);
    run_git(&fixture, ["commit", "--quiet", "-m", "topic"]);
    run_git(&fixture, ["checkout", "--quiet", "master"]);
    fs::write(fixture.join("head.txt"), "head\n").expect("head file should be written");
    run_git(&fixture, ["add", "head.txt"]);
    run_git(&fixture, ["commit", "--quiet", "-m", "head"]);
    fixture
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

fn command_words_vec(program: impl Into<OsString>, args: &[&str]) -> CommandSpec {
    let program = normalize_test_program(program.into());
    CommandSpec {
        program,
        args: args.iter().map(OsString::from).collect(),
        env: Vec::new(),
        stdin: None,
    }
}

fn read_repo_git_file(repo: &Path, path: &str) -> Option<String> {
    fs::read_to_string(repo.join(".git").join(path)).ok()
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
    exit_code: Option<i32>,
    stdout: String,
    stderr: String,
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
        exit_code: output.status.code(),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
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

fn run_optional_capture<const N: usize>(
    program: impl AsRef<OsStr>,
    args: [&str; N],
    cwd: &Path,
) -> Option<String> {
    let program = normalize_test_program(program.as_ref().to_os_string());
    let output = Command::new(program)
        .args(args)
        .current_dir(cwd)
        .output()
        .expect("command should start");
    if output.status.success() {
        Some(String::from_utf8_lossy(&output.stdout).into_owned())
    } else {
        None
    }
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
