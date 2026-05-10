use std::env;
use std::io::{self, Write};
use std::process::ExitCode;

const GENERAL_HELP: &str = "\
rit - a readable Rust implementation of Git

Usage:
  rit <command> [<args>]

Core commands:
  version       Display rit version information
  help          Display help for rit or a command
  init          Create an empty Git repository
  rev-parse     Inspect the current repository paths
  cat-file      Inspect loose objects
  ls-tree       List entries in a tree object
  status        Show porcelain working tree status
  diff          Show working tree changes
  log           Show commit history
  add           Add file contents to the index
  commit        Record staged changes
  branch        List, create, or delete branches
  tag           List, create, or delete lightweight tags
  restore       Restore working tree or staged files
  reset         Reset staged file entries
  checkout      Switch branches
  switch        Switch branches
  show          Show one object
  ls-files      Show files in the index

Run 'rit help <command>' for command-specific notes.
";

const VERSION_HELP: &str = "\
rit version

Display version information for this rit binary.
";

const HELP_HELP: &str = "\
rit help [<command>]

Display general help or command-specific help.
";

const INIT_HELP: &str = "\
rit init [-q|--quiet] [--bare] [-b <branch>|--initial-branch <branch>] [<directory>]

Create an empty Git-compatible repository.
";

const REV_PARSE_HELP: &str = "\
rit rev-parse [--git-dir] [--show-toplevel] [--is-inside-work-tree] [<revision>...]

Print selected paths, repository facts, or resolved object IDs.
";

const CAT_FILE_HELP: &str = "\
rit cat-file (-t|-s|-p|<type>) <object>

Read a loose object and print its type, size, pretty contents, or raw contents.
";

const LS_TREE_HELP: &str = "\
rit ls-tree [--name-only|--object-only] <tree> [--] [<pathspec>...]

List entries in a loose tree object.
";

const STATUS_HELP: &str = "\
rit status --porcelain[=v1]

Show a conservative porcelain v1 status.
";

const DIFF_HELP: &str = "\
rit diff [--cached|--staged] [--name-only|--name-status|--numstat|--stat] [--] [<pathspec>...]

Show working tree changes compared with the index, or staged changes compared with HEAD.
";

const LOG_HELP: &str = "\
rit log [--oneline] [--] [<pathspec>...]

Show commits reachable from HEAD by following the first parent.
";

const ADD_HELP: &str = "\
rit add <file>...

Add explicit regular files to the index.
";

const COMMIT_HELP: &str = "\
rit commit -m <message>

Create a commit from the current index and advance HEAD.
";

const BRANCH_HELP: &str = "\
rit branch
rit branch --show-current
rit branch <branch-name>
rit branch -d <branch-name>

List, create, or delete local branches.
";

const TAG_HELP: &str = "\
rit tag
rit tag <tag-name>
rit tag -d <tag-name>

List, create, or delete lightweight tags.
";

const RESTORE_HELP: &str = "\
rit restore [--staged] <file>...

Restore working tree files from the index, or staged files from HEAD.
";

const RESET_HELP: &str = "\
rit reset <file>...

Reset staged file entries from HEAD.
";

const CHECKOUT_HELP: &str = "\
rit checkout <branch>
rit checkout <commit>
rit checkout -b <branch>

Switch to an existing branch, detach at a commit, or create and switch to a new branch.
";

const SWITCH_HELP: &str = "\
rit switch <branch>
rit switch -c <branch>

Switch to an existing branch, or create and switch to a new branch.
";

const SHOW_HELP: &str = "\
rit show [--no-patch] [<revision>] [--] [<pathspec>...]

Show one commit, tree, or blob object. Commit diffs are not emitted yet.
";

const LS_FILES_HELP: &str = "\
rit ls-files [--stage] [--] [<pathspec>...]

Show files tracked in the index.
";

fn main() -> ExitCode {
    match run(env::args().skip(1), &mut io::stdout(), &mut io::stderr()) {
        Ok(code) => code,
        Err(error) => {
            let _ = writeln!(io::stderr(), "rit: {error}");
            ExitCode::from(1)
        }
    }
}

fn run(
    args: impl IntoIterator<Item = String>,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> io::Result<ExitCode> {
    let args: Vec<String> = args.into_iter().collect();

    match args.as_slice() {
        [] => {
            stdout.write_all(GENERAL_HELP.as_bytes())?;
            Ok(ExitCode::SUCCESS)
        }
        [flag] if flag == "-h" || flag == "--help" => {
            stdout.write_all(GENERAL_HELP.as_bytes())?;
            Ok(ExitCode::SUCCESS)
        }
        [flag] if flag == "--version" => print_version(stdout),
        [command] if command == "version" => print_version(stdout),
        [command, rest @ ..] if command == "init" => init_command(rest, stdout, stderr),
        [command, rest @ ..] if command == "rev-parse" => rev_parse_command(rest, stdout, stderr),
        [command, rest @ ..] if command == "cat-file" => cat_file_command(rest, stdout, stderr),
        [command, rest @ ..] if command == "ls-tree" => ls_tree_command(rest, stdout, stderr),
        [command, rest @ ..] if command == "status" => status_command(rest, stdout, stderr),
        [command, rest @ ..] if command == "diff" => diff_command(rest, stdout, stderr),
        [command, rest @ ..] if command == "log" => log_command(rest, stdout, stderr),
        [command, rest @ ..] if command == "add" => add_command(rest, stdout, stderr),
        [command, rest @ ..] if command == "commit" => commit_command(rest, stdout, stderr),
        [command, rest @ ..] if command == "branch" => branch_command(rest, stdout, stderr),
        [command, rest @ ..] if command == "tag" => tag_command(rest, stdout, stderr),
        [command, rest @ ..] if command == "restore" => restore_command(rest, stderr),
        [command, rest @ ..] if command == "reset" => reset_command(rest, stdout, stderr),
        [command, rest @ ..] if command == "checkout" => checkout_command(rest, stdout, stderr),
        [command, rest @ ..] if command == "switch" => switch_command(rest, stdout, stderr),
        [command, rest @ ..] if command == "show" => show_command(rest, stdout, stderr),
        [command, rest @ ..] if command == "ls-files" => ls_files_command(rest, stdout, stderr),
        [command] if command == "help" => {
            stdout.write_all(GENERAL_HELP.as_bytes())?;
            Ok(ExitCode::SUCCESS)
        }
        [command, topic] if command == "help" => print_command_help(topic, stdout, stderr),
        [unknown, ..] => {
            writeln!(stderr, "rit: unknown command '{unknown}'")?;
            writeln!(stderr, "Run 'rit help' for usage.")?;
            Ok(ExitCode::from(129))
        }
    }
}

fn print_version(stdout: &mut dyn Write) -> io::Result<ExitCode> {
    writeln!(stdout, "rit version {}", rit_core::version())?;
    Ok(ExitCode::SUCCESS)
}

fn print_command_help(
    topic: &str,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> io::Result<ExitCode> {
    match topic {
        "version" => stdout.write_all(VERSION_HELP.as_bytes())?,
        "help" => stdout.write_all(HELP_HELP.as_bytes())?,
        "init" => stdout.write_all(INIT_HELP.as_bytes())?,
        "rev-parse" => stdout.write_all(REV_PARSE_HELP.as_bytes())?,
        "cat-file" => stdout.write_all(CAT_FILE_HELP.as_bytes())?,
        "ls-tree" => stdout.write_all(LS_TREE_HELP.as_bytes())?,
        "status" => stdout.write_all(STATUS_HELP.as_bytes())?,
        "diff" => stdout.write_all(DIFF_HELP.as_bytes())?,
        "log" => stdout.write_all(LOG_HELP.as_bytes())?,
        "add" => stdout.write_all(ADD_HELP.as_bytes())?,
        "commit" => stdout.write_all(COMMIT_HELP.as_bytes())?,
        "branch" => stdout.write_all(BRANCH_HELP.as_bytes())?,
        "tag" => stdout.write_all(TAG_HELP.as_bytes())?,
        "restore" => stdout.write_all(RESTORE_HELP.as_bytes())?,
        "reset" => stdout.write_all(RESET_HELP.as_bytes())?,
        "checkout" => stdout.write_all(CHECKOUT_HELP.as_bytes())?,
        "switch" => stdout.write_all(SWITCH_HELP.as_bytes())?,
        "show" => stdout.write_all(SHOW_HELP.as_bytes())?,
        "ls-files" => stdout.write_all(LS_FILES_HELP.as_bytes())?,
        unknown => {
            writeln!(stderr, "rit: no help for unknown command '{unknown}'")?;
            return Ok(ExitCode::from(129));
        }
    }

    Ok(ExitCode::SUCCESS)
}

fn init_command(
    args: &[String],
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> io::Result<ExitCode> {
    let mut options = rit_core::InitOptions::new(".");
    let mut index = 0;

    while index < args.len() {
        match args[index].as_str() {
            "-q" | "--quiet" => options.quiet = true,
            "--bare" => options.bare = true,
            "-b" | "--initial-branch" => {
                index += 1;
                let Some(branch_name) = args.get(index) else {
                    writeln!(
                        stderr,
                        "rit: option requires an argument: {}",
                        args[index - 1]
                    )?;
                    return Ok(ExitCode::from(129));
                };
                options.initial_branch = branch_name.clone();
            }
            unsupported if unsupported.starts_with('-') => {
                writeln!(stderr, "rit: unsupported init option '{unsupported}'")?;
                return Ok(ExitCode::from(129));
            }
            directory => {
                if options.directory != std::path::Path::new(".") {
                    writeln!(stderr, "rit: init accepts at most one directory")?;
                    return Ok(ExitCode::from(129));
                }
                options.directory = std::path::PathBuf::from(directory);
            }
        }

        index += 1;
    }

    match rit_core::Repository::init(&options) {
        Ok(repository) => {
            if !options.quiet {
                writeln!(
                    stdout,
                    "Initialized empty Git repository in {}/",
                    repository.git_dir().display()
                )?;
            }
            Ok(ExitCode::SUCCESS)
        }
        Err(error) => {
            writeln!(stderr, "rit: {error}")?;
            Ok(ExitCode::from(1))
        }
    }
}

fn rev_parse_command(
    args: &[String],
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> io::Result<ExitCode> {
    if args.is_empty() {
        writeln!(
            stderr,
            "rit: rev-parse requires at least one supported option"
        )?;
        return Ok(ExitCode::from(129));
    }

    let repository = match rit_core::Repository::discover(".") {
        Ok(repository) => repository,
        Err(error) => {
            writeln!(stderr, "rit: {error}")?;
            return Ok(ExitCode::from(128));
        }
    };

    for arg in args {
        match arg.as_str() {
            "--git-dir" => writeln!(stdout, "{}", repository.git_dir().display())?,
            "--show-toplevel" => {
                let Some(worktree) = repository.worktree() else {
                    writeln!(stderr, "rit: this operation must be run in a work tree")?;
                    return Ok(ExitCode::from(128));
                };
                writeln!(stdout, "{}", worktree.display())?;
            }
            "--is-inside-work-tree" => {
                writeln!(stdout, "{}", repository.worktree().is_some())?;
            }
            unsupported if unsupported.starts_with('-') => {
                writeln!(stderr, "rit: unsupported rev-parse option '{unsupported}'")?;
                return Ok(ExitCode::from(129));
            }
            revision => match repository.resolve_revision(revision) {
                Ok(object_id) => writeln!(stdout, "{object_id}")?,
                Err(error) => {
                    writeln!(stderr, "rit: {error}")?;
                    return Ok(ExitCode::from(1));
                }
            },
        }
    }

    Ok(ExitCode::SUCCESS)
}

fn cat_file_command(
    args: &[String],
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> io::Result<ExitCode> {
    if args.len() != 2 {
        writeln!(stderr, "rit: cat-file expects exactly two arguments")?;
        return Ok(ExitCode::from(129));
    }

    let repository = match discover_repository(stderr)? {
        Some(repository) => repository,
        None => return Ok(ExitCode::from(128)),
    };
    let object_id = match repository.resolve_revision(&args[1]) {
        Ok(object_id) => object_id,
        Err(error) => {
            writeln!(stderr, "rit: {error}")?;
            return Ok(ExitCode::from(129));
        }
    };
    let object = match repository.read_object(object_id) {
        Ok(object) => object,
        Err(error) => {
            writeln!(stderr, "rit: {error}")?;
            return Ok(ExitCode::from(1));
        }
    };

    match args[0].as_str() {
        "-t" => writeln!(stdout, "{}", object.kind)?,
        "-s" => writeln!(stdout, "{}", object.size())?,
        "-p" => pretty_print_object(&object, stdout)?,
        kind_name => {
            let expected_kind = match rit_core::ObjectKind::parse(kind_name) {
                Ok(kind) => kind,
                Err(error) => {
                    writeln!(stderr, "rit: {error}")?;
                    return Ok(ExitCode::from(129));
                }
            };
            if object.kind != expected_kind {
                writeln!(
                    stderr,
                    "rit: object {} is {}, not {}",
                    object_id, object.kind, expected_kind
                )?;
                return Ok(ExitCode::from(1));
            }
            stdout.write_all(&object.data)?;
        }
    }

    Ok(ExitCode::SUCCESS)
}

fn ls_tree_command(
    args: &[String],
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> io::Result<ExitCode> {
    let mut name_only = false;
    let mut object_only = false;
    let mut tree_id = None;
    let mut pathspec_args = Vec::new();
    let mut after_separator = false;

    for arg in args {
        match arg.as_str() {
            "--" if !after_separator => after_separator = true,
            "--name-only" if !after_separator => name_only = true,
            "--object-only" if !after_separator => object_only = true,
            unsupported if unsupported.starts_with('-') && !after_separator => {
                writeln!(stderr, "rit: unsupported ls-tree option '{unsupported}'")?;
                return Ok(ExitCode::from(129));
            }
            object => {
                if tree_id.is_none() {
                    tree_id = Some(object.to_owned());
                } else {
                    pathspec_args.push(object.to_owned());
                }
            }
        }
    }

    let Some(tree_id) = tree_id else {
        writeln!(stderr, "rit: ls-tree expects one tree object")?;
        return Ok(ExitCode::from(129));
    };
    let repository = match discover_repository(stderr)? {
        Some(repository) => repository,
        None => return Ok(ExitCode::from(128)),
    };
    let mut object_id = match repository.resolve_revision(&tree_id) {
        Ok(object_id) => object_id,
        Err(error) => {
            writeln!(stderr, "rit: {error}")?;
            return Ok(ExitCode::from(129));
        }
    };
    let object = match repository.read_object(object_id) {
        Ok(object) => object,
        Err(error) => {
            writeln!(stderr, "rit: {error}")?;
            return Ok(ExitCode::from(1));
        }
    };
    if object.kind == rit_core::ObjectKind::Commit {
        match rit_core::parse_commit(&object.data) {
            Ok(commit) => {
                object_id = commit.tree;
            }
            Err(error) => {
                writeln!(stderr, "rit: {error}")?;
                return Ok(ExitCode::from(1));
            }
        }
    }
    let object = match repository.read_object(object_id) {
        Ok(object) => object,
        Err(error) => {
            writeln!(stderr, "rit: {error}")?;
            return Ok(ExitCode::from(1));
        }
    };
    if object.kind != rit_core::ObjectKind::Tree {
        writeln!(
            stderr,
            "rit: object {object_id} is {}, not tree",
            object.kind
        )?;
        return Ok(ExitCode::from(1));
    }

    let pathspecs = match rit_core::PathspecSet::from_args(&pathspec_args) {
        Ok(pathspecs) => pathspecs,
        Err(error) => {
            writeln!(stderr, "rit: {error}")?;
            return Ok(ExitCode::from(129));
        }
    };
    if pathspecs.is_all() {
        print_tree_entries(&object.data, name_only, object_only, stdout)?;
    } else {
        for pathspec in pathspecs.patterns() {
            match find_tree_entry_by_path(&repository, object_id, pathspec) {
                Ok(Some(entry)) => print_tree_entry(&entry, name_only, object_only, stdout)?,
                Ok(None) => {}
                Err(error) => {
                    writeln!(stderr, "rit: {error}")?;
                    return Ok(ExitCode::from(1));
                }
            }
        }
    }
    Ok(ExitCode::SUCCESS)
}

fn status_command(
    args: &[String],
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> io::Result<ExitCode> {
    let Some(status_args) = parse_status_args(args, stderr)? else {
        return Ok(ExitCode::from(129));
    };
    let pathspecs = match rit_core::PathspecSet::from_args(&status_args.pathspecs) {
        Ok(pathspecs) => pathspecs,
        Err(error) => {
            writeln!(stderr, "rit: {error}")?;
            return Ok(ExitCode::from(129));
        }
    };

    let repository = match discover_repository(stderr)? {
        Some(repository) => repository,
        None => return Ok(ExitCode::from(128)),
    };
    match repository.status_porcelain_v1_with_options(&pathspecs, status_args.untracked_files) {
        Ok(status) => {
            stdout.write_all(status.to_porcelain_v1().as_bytes())?;
            Ok(ExitCode::SUCCESS)
        }
        Err(error) => {
            writeln!(stderr, "rit: {error}")?;
            Ok(ExitCode::from(1))
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct StatusArgs {
    pathspecs: Vec<String>,
    untracked_files: rit_core::UntrackedFilesMode,
}

fn parse_status_args(args: &[String], stderr: &mut dyn Write) -> io::Result<Option<StatusArgs>> {
    let mut has_porcelain = false;
    let mut pathspecs = Vec::new();
    let mut after_separator = false;
    let mut untracked_files = rit_core::UntrackedFilesMode::Normal;

    for arg in args {
        match arg.as_str() {
            "--" if !after_separator => after_separator = true,
            "--porcelain" | "--porcelain=v1" | "-s" if !after_separator => has_porcelain = true,
            "-u" | "--untracked-files" if !after_separator => {
                untracked_files = rit_core::UntrackedFilesMode::All;
            }
            "--no-untracked-files" if !after_separator => {
                untracked_files = rit_core::UntrackedFilesMode::Normal;
            }
            "-uno" | "--untracked-files=no" if !after_separator => {
                untracked_files = rit_core::UntrackedFilesMode::No;
            }
            "-unormal" | "--untracked-files=normal" if !after_separator => {
                untracked_files = rit_core::UntrackedFilesMode::Normal;
            }
            "-uall" | "--untracked-files=all" if !after_separator => {
                untracked_files = rit_core::UntrackedFilesMode::All;
            }
            unsupported if unsupported.starts_with('-') && !after_separator => {
                writeln!(stderr, "rit: unsupported status option '{unsupported}'")?;
                return Ok(None);
            }
            pathspec => pathspecs.push(pathspec.to_owned()),
        }
    }

    if has_porcelain {
        Ok(Some(StatusArgs {
            pathspecs,
            untracked_files,
        }))
    } else {
        writeln!(stderr, "rit: status currently supports only --porcelain=v1")?;
        Ok(None)
    }
}

fn diff_command(
    args: &[String],
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> io::Result<ExitCode> {
    let mut cached = false;
    let mut output_mode = None;
    let mut pathspec_args = Vec::new();
    let mut after_separator = false;

    for arg in args {
        match arg.as_str() {
            "--" if !after_separator => after_separator = true,
            "--cached" | "--staged" if !after_separator => cached = true,
            "-p" | "-u" if !after_separator => {
                if output_mode.replace("--patch").is_some() {
                    writeln!(stderr, "rit: diff accepts one output option")?;
                    return Ok(ExitCode::from(129));
                }
            }
            "--name-only" | "--name-status" | "--numstat" | "--stat" if !after_separator => {
                if output_mode.replace(arg.as_str()).is_some() {
                    writeln!(stderr, "rit: diff accepts one output option")?;
                    return Ok(ExitCode::from(129));
                }
            }
            unsupported if unsupported.starts_with('-') && !after_separator => {
                writeln!(stderr, "rit: unsupported diff option '{unsupported}'")?;
                return Ok(ExitCode::from(129));
            }
            pathspec => pathspec_args.push(pathspec.to_owned()),
        }
    }

    let output_mode = output_mode.unwrap_or("--patch");
    let pathspecs = match rit_core::PathspecSet::from_args(&pathspec_args) {
        Ok(pathspecs) => pathspecs,
        Err(error) => {
            writeln!(stderr, "rit: {error}")?;
            return Ok(ExitCode::from(129));
        }
    };

    let repository = match discover_repository(stderr)? {
        Some(repository) => repository,
        None => return Ok(ExitCode::from(128)),
    };
    if output_mode == "--patch" {
        let patch_result = if cached {
            repository.diff_index_to_head_patch_with_pathspecs(&pathspecs)
        } else {
            repository.diff_worktree_to_index_patch_with_pathspecs(&pathspecs)
        };
        match patch_result.and_then(|patch| patch.to_patch_text()) {
            Ok(text) => {
                stdout.write_all(text.as_bytes())?;
                return Ok(ExitCode::SUCCESS);
            }
            Err(error) => {
                writeln!(stderr, "rit: {error}")?;
                return Ok(ExitCode::from(1));
            }
        }
    }

    let diff_result = if cached {
        repository.diff_index_to_head_with_pathspecs(&pathspecs)
    } else {
        repository.diff_worktree_to_index_with_pathspecs(&pathspecs)
    };
    let diff = match diff_result {
        Ok(diff) => diff,
        Err(error) => {
            writeln!(stderr, "rit: {error}")?;
            return Ok(ExitCode::from(1));
        }
    };

    match output_mode {
        "--name-only" => {
            for path in diff.name_only() {
                writeln!(stdout, "{path}")?;
            }
        }
        "--name-status" => stdout.write_all(diff.to_name_status_text().as_bytes())?,
        "--numstat" => stdout.write_all(diff.to_numstat_text().as_bytes())?,
        "--stat" => stdout.write_all(diff.to_stat_text().as_bytes())?,
        _ => unreachable!("validated above"),
    }

    Ok(ExitCode::SUCCESS)
}

fn log_command(
    args: &[String],
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> io::Result<ExitCode> {
    let Some((oneline, pathspec_args)) = parse_log_args(args, stderr)? else {
        return Ok(ExitCode::from(129));
    };
    let pathspecs = match rit_core::PathspecSet::from_args(&pathspec_args) {
        Ok(pathspecs) => pathspecs,
        Err(error) => {
            writeln!(stderr, "rit: {error}")?;
            return Ok(ExitCode::from(129));
        }
    };

    let repository = match discover_repository(stderr)? {
        Some(repository) => repository,
        None => return Ok(ExitCode::from(128)),
    };
    let entries = match repository.log_first_parent_with_pathspecs(&pathspecs) {
        Ok(entries) => entries,
        Err(error) => {
            writeln!(stderr, "rit: {error}")?;
            return Ok(ExitCode::from(1));
        }
    };

    if oneline {
        for entry in entries {
            writeln!(
                stdout,
                "{} {}",
                &entry.object_id.to_hex()[..7],
                first_message_line(&entry.commit.message)
            )?;
        }
    } else {
        for (index, entry) in entries.iter().enumerate() {
            if index > 0 {
                writeln!(stdout)?;
            }
            print_commit_no_patch(entry.object_id, &entry.commit, stdout)?;
        }
    }

    Ok(ExitCode::SUCCESS)
}

fn parse_log_args(
    args: &[String],
    stderr: &mut dyn Write,
) -> io::Result<Option<(bool, Vec<String>)>> {
    let mut oneline = false;
    let mut pathspecs = Vec::new();
    let mut after_separator = false;

    for arg in args {
        match arg.as_str() {
            "--" if !after_separator => after_separator = true,
            "--oneline" if !after_separator => oneline = true,
            unsupported if unsupported.starts_with('-') && !after_separator => {
                writeln!(stderr, "rit: unsupported log option '{unsupported}'")?;
                return Ok(None);
            }
            pathspec => pathspecs.push(pathspec.to_owned()),
        }
    }

    Ok(Some((oneline, pathspecs)))
}

fn show_command(
    args: &[String],
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> io::Result<ExitCode> {
    let Some((revision, pathspec_args)) = parse_show_args(args, stderr)? else {
        return Ok(ExitCode::from(129));
    };
    let pathspecs = match rit_core::PathspecSet::from_args(&pathspec_args) {
        Ok(pathspecs) => pathspecs,
        Err(error) => {
            writeln!(stderr, "rit: {error}")?;
            return Ok(ExitCode::from(129));
        }
    };
    let repository = match discover_repository(stderr)? {
        Some(repository) => repository,
        None => return Ok(ExitCode::from(128)),
    };
    let object_id = match repository.resolve_revision(&revision) {
        Ok(object_id) => object_id,
        Err(error) => {
            writeln!(stderr, "rit: {error}")?;
            return Ok(ExitCode::from(1));
        }
    };
    let object = match repository.read_object(object_id) {
        Ok(object) => object,
        Err(error) => {
            writeln!(stderr, "rit: {error}")?;
            return Ok(ExitCode::from(1));
        }
    };
    match object.kind {
        rit_core::ObjectKind::Commit => match rit_core::parse_commit(&object.data) {
            Ok(commit) => {
                let touches_pathspecs = if pathspecs.is_all() {
                    true
                } else {
                    match repository.commit_touches_pathspecs(&commit, &pathspecs) {
                        Ok(touches) => touches,
                        Err(error) => {
                            writeln!(stderr, "rit: {error}")?;
                            return Ok(ExitCode::from(1));
                        }
                    }
                };
                if touches_pathspecs {
                    print_commit_no_patch(object_id, &commit, stdout)?;
                }
            }
            Err(error) => {
                writeln!(stderr, "rit: {error}")?;
                return Ok(ExitCode::from(1));
            }
        },
        rit_core::ObjectKind::Tree => print_tree_entries(&object.data, false, false, stdout)?,
        rit_core::ObjectKind::Blob | rit_core::ObjectKind::Tag => stdout.write_all(&object.data)?,
    }
    Ok(ExitCode::SUCCESS)
}

fn parse_show_args(
    args: &[String],
    stderr: &mut dyn Write,
) -> io::Result<Option<(String, Vec<String>)>> {
    let mut revision = None;
    let mut pathspecs = Vec::new();
    let mut after_separator = false;

    for arg in args {
        match arg.as_str() {
            "--" if !after_separator => after_separator = true,
            "--no-patch" | "-s" if !after_separator => {}
            unsupported if unsupported.starts_with('-') && !after_separator => {
                writeln!(stderr, "rit: unsupported show option '{unsupported}'")?;
                return Ok(None);
            }
            value if after_separator => pathspecs.push(value.to_owned()),
            value if revision.is_none() => revision = Some(value.to_owned()),
            extra => {
                writeln!(
                    stderr,
                    "rit: show accepts at most one revision before --: {extra}"
                )?;
                return Ok(None);
            }
        }
    }

    Ok(Some((
        revision.unwrap_or_else(|| "HEAD".to_owned()),
        pathspecs,
    )))
}

fn ls_files_command(
    args: &[String],
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> io::Result<ExitCode> {
    let Some((stage, pathspec_args)) = parse_ls_files_args(args, stderr)? else {
        return Ok(ExitCode::from(129));
    };
    let pathspecs = match rit_core::PathspecSet::from_args(&pathspec_args) {
        Ok(pathspecs) => pathspecs,
        Err(error) => {
            writeln!(stderr, "rit: {error}")?;
            return Ok(ExitCode::from(129));
        }
    };
    let repository = match discover_repository(stderr)? {
        Some(repository) => repository,
        None => return Ok(ExitCode::from(128)),
    };
    let index = match rit_core::Index::read(&repository.git_dir().join("index")) {
        Ok(index) => index,
        Err(error) => {
            writeln!(stderr, "rit: {error}")?;
            return Ok(ExitCode::from(1));
        }
    };
    for entry in index.entries {
        if !pathspecs.matches(&entry.path) {
            continue;
        }
        if stage {
            writeln!(
                stdout,
                "{:06o} {} 0\t{}",
                entry.mode, entry.object_id, entry.path
            )?;
        } else {
            writeln!(stdout, "{}", entry.path)?;
        }
    }
    Ok(ExitCode::SUCCESS)
}

fn parse_ls_files_args(
    args: &[String],
    stderr: &mut dyn Write,
) -> io::Result<Option<(bool, Vec<String>)>> {
    let mut stage = false;
    let mut pathspecs = Vec::new();
    let mut after_separator = false;

    for arg in args {
        match arg.as_str() {
            "--" if !after_separator => after_separator = true,
            "--stage" | "-s" if !after_separator => stage = true,
            unsupported if unsupported.starts_with('-') && !after_separator => {
                writeln!(stderr, "rit: unsupported ls-files option '{unsupported}'")?;
                return Ok(None);
            }
            pathspec => pathspecs.push(pathspec.to_owned()),
        }
    }

    Ok(Some((stage, pathspecs)))
}

fn print_commit_no_patch(
    object_id: rit_core::ObjectId,
    commit: &rit_core::Commit,
    stdout: &mut dyn Write,
) -> io::Result<()> {
    writeln!(stdout, "commit {object_id}")?;
    writeln!(
        stdout,
        "Author: {} <{}>",
        commit.author.name, commit.author.email
    )?;
    writeln!(stdout, "Date:   {}", format_git_date(&commit.author))?;
    writeln!(stdout)?;
    for line in commit.message.trim_end_matches('\n').lines() {
        writeln!(stdout, "    {line}")?;
    }
    Ok(())
}

fn add_command(
    args: &[String],
    _stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> io::Result<ExitCode> {
    let Some(paths) = parse_plain_path_args(args, "add", stderr)? else {
        return Ok(ExitCode::from(129));
    };
    if paths.is_empty() {
        writeln!(
            stderr,
            "rit: add currently supports only ordinary file or directory pathspecs"
        )?;
        return Ok(ExitCode::from(129));
    }

    let repository = match discover_repository(stderr)? {
        Some(repository) => repository,
        None => return Ok(ExitCode::from(128)),
    };
    match repository.add_paths(&paths) {
        Ok(_) => Ok(ExitCode::SUCCESS),
        Err(error) => {
            writeln!(stderr, "rit: {error}")?;
            Ok(ExitCode::from(1))
        }
    }
}

fn commit_command(
    args: &[String],
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> io::Result<ExitCode> {
    let Some(message) = parse_commit_message(args) else {
        writeln!(stderr, "rit: commit currently supports only -m <message>")?;
        return Ok(ExitCode::from(129));
    };

    let repository = match discover_repository(stderr)? {
        Some(repository) => repository,
        None => return Ok(ExitCode::from(128)),
    };
    match repository.commit_index(&message) {
        Ok(result) => {
            writeln!(
                stdout,
                "[{}] {}",
                &result.commit_id.to_hex()[..7],
                first_message_line(&message)
            )?;
            Ok(ExitCode::SUCCESS)
        }
        Err(error) => {
            writeln!(stderr, "rit: {error}")?;
            Ok(ExitCode::from(1))
        }
    }
}

fn branch_command(
    args: &[String],
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> io::Result<ExitCode> {
    let repository = match discover_repository(stderr)? {
        Some(repository) => repository,
        None => return Ok(ExitCode::from(128)),
    };

    match args {
        [] => match repository.list_branches() {
            Ok(branches) => {
                for branch in branches {
                    let marker = if branch.current { '*' } else { ' ' };
                    writeln!(stdout, "{marker} {}", branch.name)?;
                }
                Ok(ExitCode::SUCCESS)
            }
            Err(error) => write_command_error(stderr, error),
        },
        [flag] if flag == "--show-current" => match repository.current_branch_name() {
            Ok(Some(branch)) => {
                writeln!(stdout, "{branch}")?;
                Ok(ExitCode::SUCCESS)
            }
            Ok(None) => Ok(ExitCode::SUCCESS),
            Err(error) => write_command_error(stderr, error),
        },
        [flag, name] if flag == "-d" || flag == "--delete" => {
            match repository.delete_branch(name) {
                Ok(target) => {
                    writeln!(
                        stdout,
                        "Deleted branch {name} (was {}).",
                        &target.to_hex()[..7]
                    )?;
                    Ok(ExitCode::SUCCESS)
                }
                Err(error) => write_command_error(stderr, error),
            }
        }
        [name] if !name.starts_with('-') => match repository.create_branch(name) {
            Ok(_) => Ok(ExitCode::SUCCESS),
            Err(error) => write_command_error(stderr, error),
        },
        _ => {
            writeln!(stderr, "rit: unsupported branch arguments")?;
            Ok(ExitCode::from(129))
        }
    }
}

fn tag_command(
    args: &[String],
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> io::Result<ExitCode> {
    let repository = match discover_repository(stderr)? {
        Some(repository) => repository,
        None => return Ok(ExitCode::from(128)),
    };

    match args {
        [] => match repository.list_tags() {
            Ok(tags) => {
                for tag in tags {
                    writeln!(stdout, "{}", tag.name)?;
                }
                Ok(ExitCode::SUCCESS)
            }
            Err(error) => write_command_error(stderr, error),
        },
        [flag] if flag == "-l" || flag == "--list" => match repository.list_tags() {
            Ok(tags) => {
                for tag in tags {
                    writeln!(stdout, "{}", tag.name)?;
                }
                Ok(ExitCode::SUCCESS)
            }
            Err(error) => write_command_error(stderr, error),
        },
        [flag, name] if flag == "-d" || flag == "--delete" => match repository.delete_tag(name) {
            Ok(target) => {
                writeln!(
                    stdout,
                    "Deleted tag '{name}' (was {}).",
                    &target.to_hex()[..7]
                )?;
                Ok(ExitCode::SUCCESS)
            }
            Err(error) => write_command_error(stderr, error),
        },
        [name] if !name.starts_with('-') => match repository.create_tag(name) {
            Ok(_) => Ok(ExitCode::SUCCESS),
            Err(error) => write_command_error(stderr, error),
        },
        _ => {
            writeln!(stderr, "rit: unsupported tag arguments")?;
            Ok(ExitCode::from(129))
        }
    }
}

fn restore_command(args: &[String], stderr: &mut dyn Write) -> io::Result<ExitCode> {
    let repository = match discover_repository(stderr)? {
        Some(repository) => repository,
        None => return Ok(ExitCode::from(128)),
    };
    let Some((staged, paths)) = parse_restore_args(args, stderr)? else {
        return Ok(ExitCode::from(129));
    };
    if paths.is_empty() {
        writeln!(stderr, "rit: restore requires at least one file")?;
        return Ok(ExitCode::from(129));
    }

    let result = if staged {
        repository
            .restore_staged_paths_from_head(&paths)
            .map(|_| ())
    } else {
        repository.restore_worktree_paths(&paths)
    };
    match result {
        Ok(()) => Ok(ExitCode::SUCCESS),
        Err(error) => write_command_error(stderr, error),
    }
}

fn parse_restore_args(
    args: &[String],
    stderr: &mut dyn Write,
) -> io::Result<Option<(bool, Vec<String>)>> {
    let mut staged = false;
    let mut paths = Vec::new();
    let mut after_separator = false;
    for arg in args {
        if arg == "--" && !after_separator {
            after_separator = true;
        } else if (arg == "--staged" || arg == "-S") && !after_separator {
            staged = true;
        } else if arg.starts_with('-') && !after_separator {
            writeln!(stderr, "rit: unsupported restore option '{arg}'")?;
            return Ok(None);
        } else {
            paths.push(arg.clone());
        }
    }
    Ok(Some((staged, paths)))
}

fn reset_command(
    args: &[String],
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> io::Result<ExitCode> {
    let Some(paths) = parse_plain_path_args(args, "reset", stderr)? else {
        return Ok(ExitCode::from(129));
    };
    if paths.is_empty() {
        writeln!(
            stderr,
            "rit: reset currently supports only ordinary file or directory pathspecs"
        )?;
        return Ok(ExitCode::from(129));
    }
    let repository = match discover_repository(stderr)? {
        Some(repository) => repository,
        None => return Ok(ExitCode::from(128)),
    };
    match repository.restore_staged_paths_from_head(&paths) {
        Ok(unstaged) => {
            if !unstaged.is_empty() {
                writeln!(stdout, "Unstaged changes after reset:")?;
                for line in unstaged {
                    writeln!(stdout, "{line}")?;
                }
            }
            Ok(ExitCode::SUCCESS)
        }
        Err(error) => write_command_error(stderr, error),
    }
}

fn parse_plain_path_args(
    args: &[String],
    command: &str,
    stderr: &mut dyn Write,
) -> io::Result<Option<Vec<String>>> {
    let mut paths = Vec::new();
    let mut after_separator = false;
    for arg in args {
        if arg == "--" && !after_separator {
            after_separator = true;
        } else if arg.starts_with('-') && !after_separator {
            writeln!(stderr, "rit: unsupported {command} option '{arg}'")?;
            return Ok(None);
        } else {
            paths.push(arg.clone());
        }
    }
    Ok(Some(paths))
}

fn checkout_command(
    args: &[String],
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> io::Result<ExitCode> {
    match args {
        [branch] if !branch.starts_with('-') => {
            checkout_existing_branch_or_revision(branch, stdout, stderr)
        }
        [flag, branch] if flag == "-b" => {
            checkout_new_branch(branch, "Switched to a new branch", stdout, stderr)
        }
        _ => {
            writeln!(
                stderr,
                "rit: checkout currently supports only <branch>, <commit>, and -b <branch>"
            )?;
            Ok(ExitCode::from(129))
        }
    }
}

fn switch_command(
    args: &[String],
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> io::Result<ExitCode> {
    match args {
        [branch] if !branch.starts_with('-') => {
            checkout_existing_branch(branch, "Switched to branch", stdout, stderr)
        }
        [flag, branch] if flag == "-c" || flag == "--create" => {
            checkout_new_branch(branch, "Switched to a new branch", stdout, stderr)
        }
        _ => {
            writeln!(
                stderr,
                "rit: switch currently supports only <branch> and -c <branch>"
            )?;
            Ok(ExitCode::from(129))
        }
    }
}

fn checkout_existing_branch(
    branch: &str,
    message_prefix: &str,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> io::Result<ExitCode> {
    let repository = match discover_repository(stderr)? {
        Some(repository) => repository,
        None => return Ok(ExitCode::from(128)),
    };
    match repository.checkout_branch(branch) {
        Ok(_) => {
            writeln!(stdout, "{message_prefix} '{branch}'")?;
            Ok(ExitCode::SUCCESS)
        }
        Err(error) => write_command_error(stderr, error),
    }
}

fn checkout_existing_branch_or_revision(
    target: &str,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> io::Result<ExitCode> {
    let repository = match discover_repository(stderr)? {
        Some(repository) => repository,
        None => return Ok(ExitCode::from(128)),
    };
    if repository.branch_target(target).is_ok() {
        match repository.checkout_branch(target) {
            Ok(_) => {
                writeln!(stdout, "Switched to branch '{target}'")?;
                Ok(ExitCode::SUCCESS)
            }
            Err(error) => write_command_error(stderr, error),
        }
    } else {
        match repository.checkout_detached(target) {
            Ok(commit_id) => {
                writeln!(stdout, "HEAD is now at {}", &commit_id.to_hex()[..7])?;
                Ok(ExitCode::SUCCESS)
            }
            Err(error) => write_command_error(stderr, error),
        }
    }
}

fn checkout_new_branch(
    branch: &str,
    message_prefix: &str,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> io::Result<ExitCode> {
    let repository = match discover_repository(stderr)? {
        Some(repository) => repository,
        None => return Ok(ExitCode::from(128)),
    };
    match repository.checkout_new_branch(branch) {
        Ok(_) => {
            writeln!(stdout, "{message_prefix} '{branch}'")?;
            Ok(ExitCode::SUCCESS)
        }
        Err(error) => write_command_error(stderr, error),
    }
}

fn write_command_error(stderr: &mut dyn Write, error: rit_core::RitError) -> io::Result<ExitCode> {
    writeln!(stderr, "rit: {error}")?;
    Ok(ExitCode::from(1))
}

fn parse_commit_message(args: &[String]) -> Option<String> {
    match args {
        [flag, message] if flag == "-m" || flag == "--message" => Some(message.clone()),
        [flag] if flag.starts_with("--message=") => Some(flag["--message=".len()..].to_owned()),
        _ => None,
    }
}

fn first_message_line(message: &str) -> &str {
    message.lines().next().unwrap_or("")
}

fn format_git_date(signature: &rit_core::Signature) -> String {
    let offset_seconds = parse_timezone_offset(&signature.offset).unwrap_or(0);
    let local_seconds = signature.timestamp + offset_seconds;
    let (year, month, day, hour, minute, second, weekday) = civil_time(local_seconds);
    format!(
        "{} {} {} {:02}:{:02}:{:02} {} {}",
        weekday_name(weekday),
        month_name(month),
        day,
        hour,
        minute,
        second,
        year,
        signature.offset
    )
}

fn parse_timezone_offset(offset: &str) -> Option<i64> {
    if offset.len() != 5 {
        return None;
    }
    let sign = match &offset[..1] {
        "+" => 1,
        "-" => -1,
        _ => return None,
    };
    let hours = offset[1..3].parse::<i64>().ok()?;
    let minutes = offset[3..5].parse::<i64>().ok()?;
    Some(sign * (hours * 3600 + minutes * 60))
}

fn civil_time(seconds: i64) -> (i32, u32, u32, u32, u32, u32, u32) {
    let days = seconds.div_euclid(86_400);
    let seconds_of_day = seconds.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    let hour = (seconds_of_day / 3600) as u32;
    let minute = ((seconds_of_day % 3600) / 60) as u32;
    let second = (seconds_of_day % 60) as u32;
    let weekday = (days + 4).rem_euclid(7) as u32;
    (year, month, day, hour, minute, second, weekday)
}

fn civil_from_days(days: i64) -> (i32, u32, u32) {
    let days = days + 719_468;
    let era = if days >= 0 { days } else { days - 146_096 } / 146_097;
    let day_of_era = days - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = month_prime + if month_prime < 10 { 3 } else { -9 };
    let year = year + i64::from(month <= 2);
    (year as i32, month as u32, day as u32)
}

fn weekday_name(weekday: u32) -> &'static str {
    match weekday {
        0 => "Sun",
        1 => "Mon",
        2 => "Tue",
        3 => "Wed",
        4 => "Thu",
        5 => "Fri",
        _ => "Sat",
    }
}

fn month_name(month: u32) -> &'static str {
    match month {
        1 => "Jan",
        2 => "Feb",
        3 => "Mar",
        4 => "Apr",
        5 => "May",
        6 => "Jun",
        7 => "Jul",
        8 => "Aug",
        9 => "Sep",
        10 => "Oct",
        11 => "Nov",
        _ => "Dec",
    }
}

fn discover_repository(stderr: &mut dyn Write) -> io::Result<Option<rit_core::Repository>> {
    match rit_core::Repository::discover(".") {
        Ok(repository) => Ok(Some(repository)),
        Err(error) => {
            writeln!(stderr, "rit: {error}")?;
            Ok(None)
        }
    }
}

fn pretty_print_object(object: &rit_core::GitObject, stdout: &mut dyn Write) -> io::Result<()> {
    if object.kind == rit_core::ObjectKind::Tree {
        print_tree_entries(&object.data, false, false, stdout)
    } else {
        stdout.write_all(&object.data)
    }
}

fn print_tree_entries(
    data: &[u8],
    name_only: bool,
    object_only: bool,
    stdout: &mut dyn Write,
) -> io::Result<()> {
    let entries = rit_core::object::parse_tree_entries(data).map_err(io::Error::other)?;

    for entry in entries {
        if name_only {
            writeln!(stdout, "{}", entry.name_lossy())?;
        } else if object_only {
            writeln!(stdout, "{}", entry.object_id)?;
        } else {
            let printed_mode = if entry.kind == rit_core::ObjectKind::Tree {
                "040000".to_owned()
            } else {
                entry.mode.clone()
            };
            writeln!(
                stdout,
                "{} {} {}\t{}",
                printed_mode,
                entry.kind,
                entry.object_id,
                entry.name_lossy()
            )?;
        }
    }

    Ok(())
}

struct PrintableTreeEntry {
    mode: String,
    kind: rit_core::ObjectKind,
    object_id: rit_core::ObjectId,
    path: String,
}

fn find_tree_entry_by_path(
    repository: &rit_core::Repository,
    tree_id: rit_core::ObjectId,
    path: &str,
) -> rit_core::Result<Option<PrintableTreeEntry>> {
    let mut current_tree_id = tree_id;
    let mut traversed = Vec::new();
    let components = path
        .split('/')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();

    for (index, component) in components.iter().enumerate() {
        let tree = repository.read_object(current_tree_id)?;
        if tree.kind != rit_core::ObjectKind::Tree {
            return Ok(None);
        }
        let Some(entry) = rit_core::object::parse_tree_entries(&tree.data)?
            .into_iter()
            .find(|entry| entry.name_lossy() == *component)
        else {
            return Ok(None);
        };
        traversed.push(entry.name_lossy());

        if index + 1 == components.len() {
            return Ok(Some(PrintableTreeEntry {
                mode: entry.mode,
                kind: entry.kind,
                object_id: entry.object_id,
                path: traversed.join("/"),
            }));
        }
        if entry.kind != rit_core::ObjectKind::Tree {
            return Ok(None);
        }
        current_tree_id = entry.object_id;
    }

    Ok(None)
}

fn print_tree_entry(
    entry: &PrintableTreeEntry,
    name_only: bool,
    object_only: bool,
    stdout: &mut dyn Write,
) -> io::Result<()> {
    if name_only {
        writeln!(stdout, "{}", entry.path)
    } else if object_only {
        writeln!(stdout, "{}", entry.object_id)
    } else {
        let printed_mode = if entry.kind == rit_core::ObjectKind::Tree {
            "040000".to_owned()
        } else {
            entry.mode.clone()
        };
        writeln!(
            stdout,
            "{} {} {}\t{}",
            printed_mode, entry.kind, entry.object_id, entry.path
        )
    }
}

#[cfg(test)]
mod tests {
    use super::{format_git_date, run};
    use std::process::ExitCode;

    fn run_with(args: &[&str]) -> (ExitCode, String, String) {
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let code = run(
            args.iter().map(|arg| arg.to_string()),
            &mut stdout,
            &mut stderr,
        )
        .expect("command should write to in-memory buffers");

        (
            code,
            String::from_utf8(stdout).expect("stdout should be UTF-8"),
            String::from_utf8(stderr).expect("stderr should be UTF-8"),
        )
    }

    #[test]
    fn version_prints_current_package_version() {
        let (code, stdout, stderr) = run_with(&["version"]);

        assert_eq!(code, ExitCode::SUCCESS);
        assert_eq!(stdout, "rit version 0.1.0\n");
        assert_eq!(stderr, "");
    }

    #[test]
    fn help_prints_general_usage() {
        let (code, stdout, stderr) = run_with(&["help"]);

        assert_eq!(code, ExitCode::SUCCESS);
        assert!(stdout.contains("Usage:"));
        assert!(stdout.contains("version"));
        assert_eq!(stderr, "");
    }

    #[test]
    fn unknown_command_returns_usage_error() {
        let (code, stdout, stderr) = run_with(&["nope"]);

        assert_eq!(code, ExitCode::from(129));
        assert_eq!(stdout, "");
        assert!(stderr.contains("unknown command 'nope'"));
    }

    #[test]
    fn init_help_is_available() {
        let (code, stdout, stderr) = run_with(&["help", "init"]);

        assert_eq!(code, ExitCode::SUCCESS);
        assert!(stdout.contains("rit init"));
        assert_eq!(stderr, "");
    }

    #[test]
    fn cat_file_help_is_available() {
        let (code, stdout, stderr) = run_with(&["help", "cat-file"]);

        assert_eq!(code, ExitCode::SUCCESS);
        assert!(stdout.contains("rit cat-file"));
        assert_eq!(stderr, "");
    }

    #[test]
    fn status_rejects_long_output_for_now() {
        let (code, stdout, stderr) = run_with(&["status"]);

        assert_eq!(code, ExitCode::from(129));
        assert_eq!(stdout, "");
        assert!(stderr.contains("--porcelain=v1"));
    }

    #[test]
    fn diff_help_is_available() {
        let (code, stdout, stderr) = run_with(&["help", "diff"]);

        assert_eq!(code, ExitCode::SUCCESS);
        assert!(stdout.contains("rit diff"));
        assert_eq!(stderr, "");
    }

    #[test]
    fn log_help_is_available() {
        let (code, stdout, stderr) = run_with(&["help", "log"]);

        assert_eq!(code, ExitCode::SUCCESS);
        assert!(stdout.contains("rit log"));
        assert_eq!(stderr, "");
    }

    #[test]
    fn commit_requires_message() {
        let (code, stdout, stderr) = run_with(&["commit"]);

        assert_eq!(code, ExitCode::from(129));
        assert_eq!(stdout, "");
        assert!(stderr.contains("-m <message>"));
    }

    #[test]
    fn branch_help_is_available() {
        let (code, stdout, stderr) = run_with(&["help", "branch"]);

        assert_eq!(code, ExitCode::SUCCESS);
        assert!(stdout.contains("rit branch"));
        assert_eq!(stderr, "");
    }

    #[test]
    fn tag_help_is_available() {
        let (code, stdout, stderr) = run_with(&["help", "tag"]);

        assert_eq!(code, ExitCode::SUCCESS);
        assert!(stdout.contains("rit tag"));
        assert_eq!(stderr, "");
    }

    #[test]
    fn restore_help_is_available() {
        let (code, stdout, stderr) = run_with(&["help", "restore"]);

        assert_eq!(code, ExitCode::SUCCESS);
        assert!(stdout.contains("rit restore"));
        assert_eq!(stderr, "");
    }

    #[test]
    fn checkout_help_is_available() {
        let (code, stdout, stderr) = run_with(&["help", "checkout"]);

        assert_eq!(code, ExitCode::SUCCESS);
        assert!(stdout.contains("rit checkout"));
        assert_eq!(stderr, "");
    }

    #[test]
    fn show_help_is_available() {
        let (code, stdout, stderr) = run_with(&["help", "show"]);

        assert_eq!(code, ExitCode::SUCCESS);
        assert!(stdout.contains("rit show"));
        assert_eq!(stderr, "");
    }

    #[test]
    fn ls_files_help_is_available() {
        let (code, stdout, stderr) = run_with(&["help", "ls-files"]);

        assert_eq!(code, ExitCode::SUCCESS);
        assert!(stdout.contains("rit ls-files"));
        assert_eq!(stderr, "");
    }

    #[test]
    fn formats_git_date_with_offset() {
        let signature = rit_core::Signature {
            name: "A".to_owned(),
            email: "a@example.test".to_owned(),
            timestamp: 1_700_000_000,
            offset: "+0900".to_owned(),
        };

        assert_eq!(
            format_git_date(&signature),
            "Wed Nov 15 07:13:20 2023 +0900"
        );
    }
}
