use std::env;
use std::io::{self, Write};
use std::process::ExitCode;

mod auth;
mod doctor;
mod file_history;
mod help;
mod indexdb;
mod op;
mod pathspec_args;
mod remote;
mod repair;

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
            stdout.write_all(help::GENERAL_HELP.as_bytes())?;
            Ok(ExitCode::SUCCESS)
        }
        [flag] if flag == "-h" || flag == "--help" => {
            stdout.write_all(help::GENERAL_HELP.as_bytes())?;
            Ok(ExitCode::SUCCESS)
        }
        [flag] if flag == "--version" => print_version(stdout),
        [command] if command == "version" => print_version(stdout),
        [command, rest @ ..] if command == "init" => init_command(rest, stdout, stderr),
        [command, rest @ ..] if command == "clone" => remote::clone_command(rest, stdout, stderr),
        [command, rest @ ..] if command == "fetch" => remote::fetch_command(rest, stdout, stderr),
        [command, rest @ ..] if command == "push" => remote::push_command(rest, stdout, stderr),
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
        [command, rest @ ..] if command == "merge" => merge_command(rest, stdout, stderr),
        [command, rest @ ..] if command == "cherry-pick" => {
            cherry_pick_command(rest, stdout, stderr)
        }
        [command, rest @ ..] if command == "auth" => auth::auth_command(rest, stdout, stderr),
        [command, rest @ ..] if command == "indexdb" => {
            indexdb::indexdb_command(rest, stdout, stderr)
        }
        [command, rest @ ..] if command == "file-history" => {
            file_history::file_history_command(rest, stdout, stderr)
        }
        [command, rest @ ..] if command == "show" => show_command(rest, stdout, stderr),
        [command, rest @ ..] if command == "ls-files" => ls_files_command(rest, stdout, stderr),
        [command, rest @ ..] if command == "ignore" => ignore_command(rest, stdout, stderr),
        [command, rest @ ..] if command == "pathspec" => pathspec_command(rest, stdout, stderr),
        [command, rest @ ..] if command == "workspace" => workspace_command(rest, stdout, stderr),
        [command, rest @ ..] if command == "doctor" => doctor::doctor_command(rest, stdout, stderr),
        [command, rest @ ..] if command == "repair" => repair::repair_command(rest, stdout, stderr),
        [command, rest @ ..] if command == "op" => op_command(rest, stdout, stderr),
        [command, rest @ ..] if command == "undo" => undo_command(rest, stdout, stderr),
        [command] if command == "help" => {
            stdout.write_all(help::GENERAL_HELP.as_bytes())?;
            Ok(ExitCode::SUCCESS)
        }
        [command, topic] if command == "help" => help::print_command_help(topic, stdout, stderr),
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

fn workspace_command(
    args: &[String],
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> io::Result<ExitCode> {
    let profile_name = match args {
        [subcommand, profile] if subcommand == "prefetch" && !profile.starts_with('-') => profile,
        [subcommand] if subcommand == "prefetch" => {
            writeln!(stderr, "rit: workspace prefetch requires a profile name")?;
            return Ok(ExitCode::from(129));
        }
        [subcommand, ..] => {
            writeln!(
                stderr,
                "rit: unsupported workspace subcommand '{subcommand}'"
            )?;
            return Ok(ExitCode::from(129));
        }
        [] => {
            writeln!(stderr, "rit: workspace requires a subcommand")?;
            return Ok(ExitCode::from(129));
        }
    };

    let repository = match discover_repository(stderr)? {
        Some(repository) => repository,
        None => return Ok(ExitCode::from(128)),
    };
    let config = match repository.rit_config() {
        Ok(config) => config,
        Err(error) => {
            writeln!(stderr, "rit: {error}")?;
            return Ok(ExitCode::from(1));
        }
    };
    let Some(profile) = config.workspace_profile(profile_name) else {
        writeln!(stderr, "rit: workspace profile not found: {profile_name}")?;
        return Ok(ExitCode::from(1));
    };
    let partial_clone = match repository.partial_clone_policy() {
        Ok(policy) => policy,
        Err(error) => {
            writeln!(stderr, "rit: {error}")?;
            return Ok(ExitCode::from(1));
        }
    };
    let plan = profile.prefetch_plan(&partial_clone);

    writeln!(stdout, "workspace: {}", plan.workspace)?;
    writeln!(stdout, "prefetch: planned")?;
    writeln!(stdout, "partial-clone: {}", plan.partial_clone)?;
    writeln!(stdout, "lazy-files: {}", plan.lazy_files)?;
    if let Some(remote) = plan.promisor_remote {
        writeln!(stdout, "promisor-remote: {remote}")?;
    }
    if let Some(filter) = plan.partial_clone_filter {
        writeln!(stdout, "partial-clone-filter: {filter}")?;
    }
    for path in plan.include {
        writeln!(stdout, "include: {path}")?;
    }

    Ok(ExitCode::SUCCESS)
}

fn pathspec_command(
    args: &[String],
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> io::Result<ExitCode> {
    let pathspecs = match args {
        [subcommand, pathspecs @ ..] if subcommand == "explain" && !pathspecs.is_empty() => {
            pathspecs.to_vec()
        }
        [subcommand] if subcommand == "explain" => {
            writeln!(
                stderr,
                "rit: pathspec explain requires at least one pathspec"
            )?;
            return Ok(ExitCode::from(129));
        }
        [subcommand, ..] => {
            writeln!(
                stderr,
                "rit: unsupported pathspec subcommand '{subcommand}'"
            )?;
            return Ok(ExitCode::from(129));
        }
        [] => {
            writeln!(stderr, "rit: pathspec requires a subcommand")?;
            return Ok(ExitCode::from(129));
        }
    };
    let set = match rit_core::PathspecSet::from_args(&pathspecs) {
        Ok(set) => set,
        Err(error) => return write_command_error(stderr, error),
    };
    let explanation = set.explain();
    writeln!(stdout, "pathspec: explain")?;
    writeln!(stdout, "matches-all: {}", explanation.matches_all)?;
    for pattern in explanation.patterns {
        writeln!(stdout, "pattern: {}", pattern.pattern)?;
        writeln!(stdout, "mode: {}", pattern.mode)?;
        writeln!(stdout, "ignore-case: {}", pattern.ignore_case)?;
        writeln!(stdout, "exclude: {}", pattern.exclude)?;
        writeln!(stdout, "wildcard: {}", pattern.has_wildcard)?;
        for attribute in pattern.attributes {
            writeln!(stdout, "attr: {attribute}")?;
        }
    }
    Ok(ExitCode::SUCCESS)
}

fn ignore_command(
    args: &[String],
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> io::Result<ExitCode> {
    let path = match args {
        [subcommand, path] if subcommand == "explain" && !path.starts_with('-') => path,
        [subcommand] if subcommand == "explain" => {
            writeln!(stderr, "rit: ignore explain requires one path")?;
            return Ok(ExitCode::from(129));
        }
        [subcommand, ..] => {
            writeln!(stderr, "rit: unsupported ignore subcommand '{subcommand}'")?;
            return Ok(ExitCode::from(129));
        }
        [] => {
            writeln!(stderr, "rit: ignore requires a subcommand")?;
            return Ok(ExitCode::from(129));
        }
    };
    let repository = match discover_repository(stderr)? {
        Some(repository) => repository,
        None => return Ok(ExitCode::from(128)),
    };
    match repository.explain_ignore_path(path) {
        Ok(explanation) => {
            writeln!(stdout, "ignore: explain")?;
            writeln!(stdout, "path: {}", explanation.path)?;
            writeln!(stdout, "ignored: {}", explanation.ignored)?;
            if explanation.matching_rules.is_empty() {
                writeln!(stdout, "reason: no matching ignore rules")?;
            }
            for rule in explanation.matching_rules {
                writeln!(
                    stdout,
                    "rule: {}:{} pattern={} negated={}",
                    rule.source, rule.line_number, rule.pattern, rule.negated
                )?;
            }
            Ok(ExitCode::SUCCESS)
        }
        Err(error) => write_command_error(stderr, error),
    }
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
        Err(error) => write_command_error(stderr, error),
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
            if pathspec.is_exclude()
                || pathspec.has_wildcard()
                || pathspec.has_attribute_requirements()
            {
                continue;
            }
            match find_tree_entry_by_path(&repository, object_id, pathspec.pattern()) {
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
    if let Some(path) = status_args.explain_path {
        return match repository.explain_status_path(&path) {
            Ok(explanation) => {
                writeln!(stdout, "status: explain")?;
                writeln!(stdout, "path: {}", explanation.path)?;
                writeln!(
                    stdout,
                    "status: {}{}",
                    explanation.index_status, explanation.worktree_status
                )?;
                writeln!(stdout, "in-head: {}", explanation.in_head)?;
                writeln!(stdout, "in-index: {}", explanation.in_index)?;
                writeln!(stdout, "in-worktree: {}", explanation.in_worktree)?;
                writeln!(stdout, "ignored: {}", explanation.ignored)?;
                for reason in explanation.reasons {
                    writeln!(stdout, "reason: {reason}")?;
                }
                Ok(ExitCode::SUCCESS)
            }
            Err(error) => {
                writeln!(stderr, "rit: {error}")?;
                Ok(ExitCode::from(1))
            }
        };
    }
    match repository.status_porcelain_v1_with_options(
        &pathspecs,
        rit_core::StatusOptions {
            untracked_files: status_args.untracked_files,
            include_branch_header: status_args.include_branch_header,
            include_ignored: status_args.include_ignored,
        },
    ) {
        Ok(status) => {
            if status_args.null_terminated {
                stdout.write_all(status.to_porcelain_v1_null_terminated().as_bytes())?;
            } else {
                stdout.write_all(status.to_porcelain_v1().as_bytes())?;
            }
            Ok(ExitCode::SUCCESS)
        }
        Err(error) => write_command_error(stderr, error),
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct StatusArgs {
    pathspecs: Vec<String>,
    untracked_files: rit_core::UntrackedFilesMode,
    null_terminated: bool,
    include_branch_header: bool,
    include_ignored: bool,
    explain_path: Option<String>,
}

fn parse_status_args(args: &[String], stderr: &mut dyn Write) -> io::Result<Option<StatusArgs>> {
    let mut has_porcelain = false;
    let mut pathspecs = Vec::new();
    let mut after_separator = false;
    let mut untracked_files = rit_core::UntrackedFilesMode::Normal;
    let mut null_terminated = false;
    let mut include_branch_header = false;
    let mut include_ignored = false;
    let mut explain_path = None;
    let mut index = 0;

    while index < args.len() {
        let arg = &args[index];
        match arg.as_str() {
            "--" if !after_separator => after_separator = true,
            "--porcelain" | "--porcelain=v1" | "-s" if !after_separator => has_porcelain = true,
            "-z" if !after_separator => null_terminated = true,
            "-b" | "--branch" if !after_separator => include_branch_header = true,
            "--explain" if !after_separator => {
                index += 1;
                let Some(path) = args.get(index) else {
                    writeln!(stderr, "rit: status --explain requires one path")?;
                    return Ok(None);
                };
                if path.starts_with('-') {
                    writeln!(stderr, "rit: status --explain requires one path")?;
                    return Ok(None);
                }
                explain_path = Some(path.to_owned());
            }
            value if value.starts_with("--explain=") && !after_separator => {
                let path = value.trim_start_matches("--explain=");
                if path.is_empty() {
                    writeln!(stderr, "rit: status --explain requires one path")?;
                    return Ok(None);
                }
                explain_path = Some(path.to_owned());
            }
            "--ignored" | "--ignored=traditional" | "--ignored=matching" if !after_separator => {
                include_ignored = true;
            }
            "--no-ignored" | "--ignored=no" if !after_separator => {
                include_ignored = false;
            }
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
        index += 1;
    }

    if explain_path.is_some() || has_porcelain {
        Ok(Some(StatusArgs {
            pathspecs,
            untracked_files,
            null_terminated,
            include_branch_header,
            include_ignored,
            explain_path,
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
    let mut find_renames = false;
    let mut find_copies = false;
    let mut find_copies_harder = false;
    let mut rename_similarity_threshold = 50;
    let mut copy_similarity_threshold = 50;
    let mut rename_limit = None;
    let mut output_mode = None;
    let mut nul_terminated = false;
    let mut pathspec_args = Vec::new();
    let mut after_separator = false;

    for arg in args {
        match arg.as_str() {
            "--" if !after_separator => after_separator = true,
            "--cached" | "--staged" if !after_separator => cached = true,
            "-z" if !after_separator => nul_terminated = true,
            "-M" | "--find-renames" if !after_separator => find_renames = true,
            "-C" | "--find-copies" if !after_separator => find_copies = true,
            "--find-copies-harder" if !after_separator => {
                find_copies = true;
                find_copies_harder = true;
            }
            option if option.starts_with("-l") && !after_separator => {
                match parse_rename_limit_option(option) {
                    Ok(limit) => rename_limit = Some(limit),
                    Err(error) => {
                        writeln!(stderr, "rit: {error}")?;
                        return Ok(ExitCode::from(129));
                    }
                }
            }
            option
                if (option.starts_with("-M") || option.starts_with("--find-renames="))
                    && !after_separator =>
            {
                match parse_similarity_option(option, "-M", "--find-renames=") {
                    Ok(threshold) => {
                        find_renames = true;
                        rename_similarity_threshold = threshold;
                    }
                    Err(error) => {
                        writeln!(stderr, "rit: {error}")?;
                        return Ok(ExitCode::from(129));
                    }
                }
            }
            option
                if (option.starts_with("-C") || option.starts_with("--find-copies="))
                    && !after_separator =>
            {
                match parse_similarity_option(option, "-C", "--find-copies=") {
                    Ok(threshold) => {
                        find_copies = true;
                        copy_similarity_threshold = threshold;
                    }
                    Err(error) => {
                        writeln!(stderr, "rit: {error}")?;
                        return Ok(ExitCode::from(129));
                    }
                }
            }
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
    let diff_options = rit_core::DiffOptions {
        find_renames,
        find_copies,
        find_copies_harder,
        rename_similarity_threshold,
        copy_similarity_threshold,
        rename_limit,
    };
    if output_mode == "--patch" {
        let patch_result = if cached {
            repository.diff_index_to_head_patch_with_options(&pathspecs, &diff_options)
        } else {
            repository.diff_worktree_to_index_patch_with_options(&pathspecs, &diff_options)
        };
        match patch_result {
            Ok(patch) => match patch.to_patch_text() {
                Ok(text) => {
                    write_diff_warnings(stderr, &patch.warnings)?;
                    stdout.write_all(text.as_bytes())?;
                    return Ok(ExitCode::SUCCESS);
                }
                Err(error) => {
                    writeln!(stderr, "rit: {error}")?;
                    return Ok(ExitCode::from(1));
                }
            },
            Err(error) => {
                return write_diff_error(stderr, &error);
            }
        }
    }

    let diff_result = if cached {
        repository.diff_index_to_head_with_options(&pathspecs, &diff_options)
    } else {
        repository.diff_worktree_to_index_with_options(&pathspecs, &diff_options)
    };
    let diff = match diff_result {
        Ok(diff) => diff,
        Err(error) => {
            return write_diff_error(stderr, &error);
        }
    };
    write_diff_warnings(stderr, &diff.warnings)?;

    match output_mode {
        "--name-only" => {
            if nul_terminated {
                stdout.write_all(&diff.to_name_only_z())?;
            } else {
                for path in diff.name_only() {
                    writeln!(stdout, "{path}")?;
                }
            }
        }
        "--name-status" => {
            if nul_terminated {
                stdout.write_all(&diff.to_name_status_z())?;
            } else {
                stdout.write_all(diff.to_name_status_text().as_bytes())?;
            }
        }
        "--numstat" => {
            if nul_terminated {
                stdout.write_all(&diff.to_numstat_z())?;
            } else {
                stdout.write_all(diff.to_numstat_text().as_bytes())?;
            }
        }
        "--stat" => stdout.write_all(diff.to_stat_text().as_bytes())?,
        _ => unreachable!("validated above"),
    }

    Ok(ExitCode::SUCCESS)
}

fn write_diff_error(stderr: &mut dyn Write, error: &rit_core::RitError) -> io::Result<ExitCode> {
    let message = error.to_string();
    if message.starts_with("bad numeric config value ") {
        writeln!(stderr, "fatal: {message}")?;
        return Ok(ExitCode::from(128));
    }
    writeln!(stderr, "rit: {message}")?;
    Ok(ExitCode::from(1))
}

fn write_diff_warnings(stderr: &mut dyn Write, warnings: &[String]) -> io::Result<()> {
    for warning in warnings {
        writeln!(stderr, "{warning}")?;
    }
    Ok(())
}

fn parse_similarity_option(
    option: &str,
    short_prefix: &str,
    long_prefix: &str,
) -> Result<u32, String> {
    let raw_value = if let Some(value) = option.strip_prefix(long_prefix) {
        value
    } else if let Some(value) = option.strip_prefix(short_prefix) {
        value
    } else {
        return Err(format!("invalid similarity option '{option}'"));
    };
    let (value, is_percent) = raw_value
        .strip_suffix('%')
        .map(|value| (value, true))
        .unwrap_or((raw_value, false));
    if value.is_empty() {
        return Err(format!("missing similarity threshold in '{option}'"));
    }
    let threshold = value
        .parse::<u32>()
        .map_err(|_| format!("invalid similarity threshold in '{option}'"))?;
    if is_percent {
        return Ok(threshold);
    }

    // Git treats a percent-less -M/-C value as a fraction written without the
    // leading `0.`: -M5 is 50%, -M05 is 5%, and -M400 is 40%.
    let denominator = 10u128.checked_pow(value.len() as u32).unwrap_or(u128::MAX);
    let numerator = u128::from(threshold) * 100;
    let fractional_percent = numerator.div_ceil(denominator);
    Ok(fractional_percent.min(u128::from(u32::MAX)) as u32)
}

fn parse_rename_limit_option(option: &str) -> Result<usize, String> {
    let raw_value = if let Some(value) = option.strip_prefix("-l") {
        value
    } else {
        return Err(format!("invalid rename limit option '{option}'"));
    };
    if raw_value.is_empty() {
        return Err(format!("missing rename limit in '{option}'"));
    }
    raw_value
        .parse::<usize>()
        .map_err(|_| format!("invalid rename limit in '{option}'"))
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
    let attributes = match repository.root_attributes() {
        Ok(attributes) => attributes,
        Err(error) => {
            writeln!(stderr, "rit: {error}")?;
            return Ok(ExitCode::from(1));
        }
    };
    for entry in index.entries {
        if !pathspecs.matches_with_attributes(&entry.path, Some(&attributes)) {
            continue;
        }
        if stage {
            writeln!(
                stdout,
                "{:06o} {} {}\t{}",
                entry.mode, entry.object_id, entry.stage, entry.path
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
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> io::Result<ExitCode> {
    let Some(add_args) = parse_add_args(args, stderr)? else {
        return Ok(ExitCode::from(129));
    };
    if let Some(exit_code) = add_args.exit_code {
        return Ok(ExitCode::from(exit_code));
    }
    if add_args.paths.is_empty() {
        writeln!(
            stderr,
            "Nothing specified, nothing added.\nhint: Maybe you wanted to say 'git add .'?\nhint: Disable this message with \"git config set advice.addEmptyPathspec false\""
        )?;
        return Ok(ExitCode::SUCCESS);
    }

    let repository = match discover_repository(stderr)? {
        Some(repository) => repository,
        None => return Ok(ExitCode::from(128)),
    };
    if add_args.plan {
        return match repository.plan_add_paths_with_options(&add_args.paths, &add_args.options) {
            Ok(plan) => {
                writeln!(stdout, "add: plan")?;
                if let Some(mode) = plan.mode_override {
                    writeln!(stdout, "chmod: {}", chmod_mode_text(mode))?;
                }
                for path in plan.paths_to_add {
                    writeln!(stdout, "add: {path}")?;
                }
                for path in plan.paths_to_remove {
                    writeln!(stdout, "remove: {path}")?;
                }
                Ok(ExitCode::SUCCESS)
            }
            Err(error) => write_command_error(stderr, error),
        };
    }
    let before = capture_operation_snapshot(&repository, stderr)?;
    let planned_paths = repository
        .plan_add_paths_with_options(&add_args.paths, &add_args.options)
        .ok()
        .map(|plan| merge_changed_paths(plan.paths_to_add, plan.paths_to_remove))
        .unwrap_or_else(|| add_args.paths.clone());
    match repository.add_paths_with_options(&add_args.paths, &add_args.options) {
        Ok(_) => {
            record_operation_with_changed_paths(
                &repository,
                "add",
                &format!("paths {}", add_args.paths.join(" ")),
                before,
                planned_paths,
                Vec::new(),
                stderr,
            )?;
            Ok(ExitCode::SUCCESS)
        }
        Err(error) => write_command_error(stderr, error),
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ParsedAddArgs {
    paths: Vec<String>,
    options: rit_core::AddOptions,
    plan: bool,
    exit_code: Option<u8>,
}

impl ParsedAddArgs {
    fn exit(exit_code: u8) -> Self {
        Self {
            paths: Vec::new(),
            options: rit_core::AddOptions::default(),
            plan: false,
            exit_code: Some(exit_code),
        }
    }
}

fn parse_add_args(args: &[String], stderr: &mut dyn Write) -> io::Result<Option<ParsedAddArgs>> {
    let mut paths = Vec::new();
    let mut options = rit_core::AddOptions::default();
    let mut plan = false;
    let mut pathspec_file = None;
    let mut pathspec_file_nul = false;
    let mut after_separator = false;
    let mut index = 0;

    while index < args.len() {
        let arg = &args[index];
        if pathspec_from_file_missing_value(args, index, after_separator) {
            write_pathspec_from_file_requires_value(stderr)?;
            return Ok(Some(ParsedAddArgs::exit(129)));
        }
        if arg == "--" && !after_separator {
            after_separator = true;
        } else if pathspec_args::handle_pathspec_file_option(
            args,
            &mut index,
            after_separator,
            &mut pathspec_file,
            &mut pathspec_file_nul,
        )? {
        } else if arg == "--plan" && !after_separator {
            plan = true;
        } else if arg == "--chmod" && !after_separator {
            index += 1;
            let Some(value) = args.get(index) else {
                writeln!(stderr, "rit: option requires an argument: --chmod")?;
                return Ok(None);
            };
            let Some(mode) = parse_chmod_mode(value) else {
                writeln!(stderr, "rit: unsupported add chmod mode '{value}'")?;
                return Ok(None);
            };
            options.mode_override = Some(mode);
        } else if let Some(value) = arg.strip_prefix("--chmod=").filter(|_| !after_separator) {
            let Some(mode) = parse_chmod_mode(value) else {
                writeln!(stderr, "rit: unsupported add chmod mode '{value}'")?;
                return Ok(None);
            };
            options.mode_override = Some(mode);
        } else if arg.starts_with('-') && !after_separator {
            writeln!(stderr, "rit: unsupported add option '{arg}'")?;
            return Ok(None);
        } else {
            paths.push(arg.clone());
        }
        index += 1;
    }

    if pathspec_file_nul && pathspec_file.is_none() {
        write_pathspec_file_nul_requires_file(stderr)?;
        return Ok(Some(ParsedAddArgs::exit(128)));
    }
    if pathspec_file.is_some() && !paths.is_empty() {
        write_pathspec_file_cannot_mix_with_args(stderr)?;
        return Ok(Some(ParsedAddArgs::exit(128)));
    }
    if let Some(file_name) = pathspec_file {
        match pathspec_args::read_pathspecs_from_file(&file_name, pathspec_file_nul, "add", stderr)?
        {
            pathspec_args::PathspecFileRead::Pathspecs(file_pathspecs) => {
                paths.extend(file_pathspecs);
            }
            pathspec_args::PathspecFileRead::Error { exit_code } => {
                return Ok(Some(ParsedAddArgs::exit(exit_code)));
            }
        }
    }

    Ok(Some(ParsedAddArgs {
        paths,
        options,
        plan,
        exit_code: None,
    }))
}

fn parse_chmod_mode(value: &str) -> Option<rit_core::FileModeOverride> {
    match value {
        "+x" => Some(rit_core::FileModeOverride::Executable),
        "-x" => Some(rit_core::FileModeOverride::Regular),
        _ => None,
    }
}

fn chmod_mode_text(mode: rit_core::FileModeOverride) -> &'static str {
    match mode {
        rit_core::FileModeOverride::Executable => "+x",
        rit_core::FileModeOverride::Regular => "-x",
    }
}

fn commit_command(
    args: &[String],
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> io::Result<ExitCode> {
    let commit_args = match parse_commit_args(args) {
        Ok(Some(commit_args)) => commit_args,
        Ok(None) => {
            writeln!(
                stderr,
                "rit: commit currently supports -m <message>, --plan, --author, --date, and --no-verify"
            )?;
            return Ok(ExitCode::from(129));
        }
        Err(error) => {
            writeln!(stderr, "rit: {error}")?;
            return Ok(ExitCode::from(129));
        }
    };

    let repository = match discover_repository(stderr)? {
        Some(repository) => repository,
        None => return Ok(ExitCode::from(128)),
    };
    if commit_args.plan {
        return match repository
            .plan_commit_index_with_options(&commit_args.message, &commit_args.options)
        {
            Ok(plan) => {
                writeln!(stdout, "commit: plan")?;
                writeln!(stdout, "parent: {}", short_head(plan.parent_id))?;
                writeln!(stdout, "message: {}", plan.message_summary)?;
                writeln!(
                    stdout,
                    "hooks: {}",
                    if plan.verify { "verify" } else { "no-verify" }
                )?;
                if let Some(author) = plan.author {
                    writeln!(stdout, "author: {} <{}>", author.name, author.email)?;
                }
                if let Some(author_date) = plan.author_date {
                    writeln!(
                        stdout,
                        "author-date: {} {}",
                        author_date.timestamp, author_date.offset
                    )?;
                }
                writeln!(stdout, "files: {}", plan.file_count)?;
                for path in plan.paths_to_commit {
                    writeln!(stdout, "path: {path}")?;
                }
                Ok(ExitCode::SUCCESS)
            }
            Err(error) => write_command_error(stderr, error),
        };
    }
    let before = capture_operation_snapshot(&repository, stderr)?;
    match repository.commit_index_with_options(&commit_args.message, &commit_args.options) {
        Ok(result) => {
            record_operation(
                &repository,
                "commit",
                first_message_line(&commit_args.message),
                before,
                vec![result.commit_id],
                stderr,
            )?;
            writeln!(
                stdout,
                "[{}] {}",
                &result.commit_id.to_hex()[..7],
                first_message_line(&commit_args.message)
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
            let before = capture_operation_snapshot(&repository, stderr)?;
            match repository.delete_branch(name) {
                Ok(target) => {
                    record_operation(
                        &repository,
                        "branch",
                        &format!("delete branch {name}"),
                        before,
                        Vec::new(),
                        stderr,
                    )?;
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
        [name] if !name.starts_with('-') => {
            let before = capture_operation_snapshot(&repository, stderr)?;
            match repository.create_branch(name) {
                Ok(_) => {
                    record_operation(
                        &repository,
                        "branch",
                        &format!("create branch {name}"),
                        before,
                        Vec::new(),
                        stderr,
                    )?;
                    Ok(ExitCode::SUCCESS)
                }
                Err(error) => write_command_error(stderr, error),
            }
        }
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
        [flag, name] if flag == "-d" || flag == "--delete" => {
            let before = capture_operation_snapshot(&repository, stderr)?;
            match repository.delete_tag(name) {
                Ok(target) => {
                    record_operation(
                        &repository,
                        "tag",
                        &format!("delete tag {name}"),
                        before,
                        Vec::new(),
                        stderr,
                    )?;
                    writeln!(
                        stdout,
                        "Deleted tag '{name}' (was {}).",
                        &target.to_hex()[..7]
                    )?;
                    Ok(ExitCode::SUCCESS)
                }
                Err(error) => write_command_error(stderr, error),
            }
        }
        [name] if !name.starts_with('-') => {
            let before = capture_operation_snapshot(&repository, stderr)?;
            match repository.create_tag(name) {
                Ok(_) => {
                    record_operation(
                        &repository,
                        "tag",
                        &format!("create tag {name}"),
                        before,
                        Vec::new(),
                        stderr,
                    )?;
                    Ok(ExitCode::SUCCESS)
                }
                Err(error) => write_command_error(stderr, error),
            }
        }
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
    let Some(restore_args) = parse_restore_args(args, stderr)? else {
        return Ok(ExitCode::from(129));
    };
    if let Some(exit_code) = restore_args.exit_code {
        return Ok(ExitCode::from(exit_code));
    }
    let staged = restore_args.staged;
    let paths = restore_args.paths;
    if paths.is_empty() {
        writeln!(stderr, "fatal: you must specify path(s) to restore")?;
        return Ok(ExitCode::from(128));
    }

    let before = capture_operation_snapshot(&repository, stderr)?;
    let planned_paths = if staged {
        repository
            .plan_restore_staged_paths_from_head(&paths)
            .ok()
            .map(|plan| merge_changed_paths(plan.paths_to_restore, plan.paths_to_remove))
            .unwrap_or_else(|| paths.clone())
    } else {
        paths.clone()
    };
    let result = if staged {
        repository
            .restore_staged_paths_from_head(&paths)
            .map(|_| ())
    } else {
        repository.restore_worktree_paths(&paths)
    };
    match result {
        Ok(()) => {
            record_operation_with_changed_paths(
                &repository,
                "restore",
                if staged {
                    "staged paths"
                } else {
                    "worktree paths"
                },
                before,
                planned_paths,
                Vec::new(),
                stderr,
            )?;
            Ok(ExitCode::SUCCESS)
        }
        Err(error) => write_command_error(stderr, error),
    }
}

struct ParsedRestoreArgs {
    staged: bool,
    paths: Vec<String>,
    exit_code: Option<u8>,
}

impl ParsedRestoreArgs {
    fn exit(exit_code: u8) -> Self {
        Self {
            staged: false,
            paths: Vec::new(),
            exit_code: Some(exit_code),
        }
    }
}

fn parse_restore_args(
    args: &[String],
    stderr: &mut dyn Write,
) -> io::Result<Option<ParsedRestoreArgs>> {
    let mut staged = false;
    let mut paths = Vec::new();
    let mut pathspec_file = None;
    let mut pathspec_file_nul = false;
    let mut after_separator = false;
    let mut index = 0;
    while index < args.len() {
        let arg = &args[index];
        if pathspec_from_file_missing_value(args, index, after_separator) {
            write_pathspec_from_file_requires_value(stderr)?;
            return Ok(Some(ParsedRestoreArgs::exit(129)));
        }
        if arg == "--" && !after_separator {
            after_separator = true;
        } else if pathspec_args::handle_pathspec_file_option(
            args,
            &mut index,
            after_separator,
            &mut pathspec_file,
            &mut pathspec_file_nul,
        )? {
        } else if (arg == "--staged" || arg == "-S") && !after_separator {
            staged = true;
        } else if arg.starts_with('-') && !after_separator {
            writeln!(stderr, "rit: unsupported restore option '{arg}'")?;
            return Ok(None);
        } else {
            paths.push(arg.clone());
        }
        index += 1;
    }
    if pathspec_file_nul && pathspec_file.is_none() {
        write_pathspec_file_nul_requires_file(stderr)?;
        return Ok(Some(ParsedRestoreArgs::exit(128)));
    }
    if pathspec_file.is_some() && !paths.is_empty() {
        write_pathspec_file_cannot_mix_with_args(stderr)?;
        return Ok(Some(ParsedRestoreArgs::exit(128)));
    }
    if let Some(file_name) = pathspec_file {
        match pathspec_args::read_pathspecs_from_file(
            &file_name,
            pathspec_file_nul,
            "restore",
            stderr,
        )? {
            pathspec_args::PathspecFileRead::Pathspecs(file_pathspecs) => {
                paths.extend(file_pathspecs);
            }
            pathspec_args::PathspecFileRead::Error { exit_code } => {
                return Ok(Some(ParsedRestoreArgs::exit(exit_code)));
            }
        }
    }
    Ok(Some(ParsedRestoreArgs {
        staged,
        paths,
        exit_code: None,
    }))
}

fn reset_command(
    args: &[String],
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> io::Result<ExitCode> {
    let Some(reset_args) = parse_reset_args(args, stderr)? else {
        return Ok(ExitCode::from(129));
    };
    if let Some(exit_code) = reset_args.exit_code {
        return Ok(ExitCode::from(exit_code));
    }
    let reset_paths = if reset_args.paths.is_empty() && reset_args.from_pathspec_file {
        vec![".".to_owned()]
    } else {
        reset_args.paths
    };
    if reset_paths.is_empty() {
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
    if reset_args.plan {
        return match repository.plan_restore_staged_paths_from_head(&reset_paths) {
            Ok(plan) => {
                writeln!(stdout, "reset: plan")?;
                for path in plan.paths_to_restore {
                    writeln!(stdout, "restore-index: {path}")?;
                }
                for path in plan.paths_to_remove {
                    writeln!(stdout, "remove-index: {path}")?;
                }
                Ok(ExitCode::SUCCESS)
            }
            Err(error) if is_reset_noop_pathspec_error(&error) => Ok(ExitCode::SUCCESS),
            Err(error) => write_command_error(stderr, error),
        };
    }
    let before = capture_operation_snapshot(&repository, stderr)?;
    let planned_paths = repository
        .plan_restore_staged_paths_from_head(&reset_paths)
        .ok()
        .map(|plan| merge_changed_paths(plan.paths_to_restore, plan.paths_to_remove))
        .unwrap_or_else(|| reset_paths.clone());
    match repository.restore_staged_paths_from_head(&reset_paths) {
        Ok(unstaged) => {
            if !unstaged.is_empty() {
                writeln!(stdout, "Unstaged changes after reset:")?;
                for line in unstaged {
                    writeln!(stdout, "{line}")?;
                }
            }
            record_operation_with_changed_paths(
                &repository,
                "reset",
                &format!("paths {}", reset_paths.join(" ")),
                before,
                planned_paths,
                Vec::new(),
                stderr,
            )?;
            Ok(ExitCode::SUCCESS)
        }
        Err(error) if is_reset_noop_pathspec_error(&error) => Ok(ExitCode::SUCCESS),
        Err(error) => write_command_error(stderr, error),
    }
}

struct ParsedResetArgs {
    paths: Vec<String>,
    plan: bool,
    exit_code: Option<u8>,
    from_pathspec_file: bool,
}

impl ParsedResetArgs {
    fn exit(exit_code: u8) -> Self {
        Self {
            paths: Vec::new(),
            plan: false,
            exit_code: Some(exit_code),
            from_pathspec_file: false,
        }
    }
}

fn parse_reset_args(
    args: &[String],
    stderr: &mut dyn Write,
) -> io::Result<Option<ParsedResetArgs>> {
    let mut paths = Vec::new();
    let mut plan = false;
    let mut pathspec_file = None;
    let mut pathspec_file_nul = false;
    let mut from_pathspec_file = false;
    let mut after_separator = false;
    let mut index = 0;
    while index < args.len() {
        let arg = &args[index];
        if pathspec_from_file_missing_value(args, index, after_separator) {
            write_pathspec_from_file_requires_value(stderr)?;
            return Ok(Some(ParsedResetArgs::exit(129)));
        }
        if arg == "--" && !after_separator {
            after_separator = true;
        } else if pathspec_args::handle_pathspec_file_option(
            args,
            &mut index,
            after_separator,
            &mut pathspec_file,
            &mut pathspec_file_nul,
        )? {
        } else if arg == "--plan" && !after_separator {
            plan = true;
        } else if arg.starts_with('-') && !after_separator {
            writeln!(stderr, "rit: unsupported reset option '{arg}'")?;
            return Ok(None);
        } else {
            paths.push(arg.clone());
        }
        index += 1;
    }
    if pathspec_file_nul && pathspec_file.is_none() {
        write_pathspec_file_nul_requires_file(stderr)?;
        return Ok(Some(ParsedResetArgs::exit(128)));
    }
    if pathspec_file.is_some() && !paths.is_empty() {
        write_pathspec_file_cannot_mix_with_args(stderr)?;
        return Ok(Some(ParsedResetArgs::exit(128)));
    }
    if let Some(file_name) = pathspec_file {
        from_pathspec_file = true;
        match pathspec_args::read_pathspecs_from_file(
            &file_name,
            pathspec_file_nul,
            "reset",
            stderr,
        )? {
            pathspec_args::PathspecFileRead::Pathspecs(file_pathspecs) => {
                paths.extend(file_pathspecs);
            }
            pathspec_args::PathspecFileRead::Error { exit_code } => {
                return Ok(Some(ParsedResetArgs::exit(exit_code)));
            }
        }
    }
    Ok(Some(ParsedResetArgs {
        paths,
        plan,
        exit_code: None,
        from_pathspec_file,
    }))
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
    let before = capture_operation_snapshot(&repository, stderr)?;
    match repository.checkout_branch(branch) {
        Ok(_) => {
            record_operation(
                &repository,
                "checkout",
                &format!("branch {branch}"),
                before,
                Vec::new(),
                stderr,
            )?;
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
    let before = capture_operation_snapshot(&repository, stderr)?;
    if repository.branch_target(target).is_ok() {
        match repository.checkout_branch(target) {
            Ok(_) => {
                record_operation(
                    &repository,
                    "checkout",
                    &format!("branch {target}"),
                    before,
                    Vec::new(),
                    stderr,
                )?;
                writeln!(stdout, "Switched to branch '{target}'")?;
                Ok(ExitCode::SUCCESS)
            }
            Err(error) => write_command_error(stderr, error),
        }
    } else {
        match repository.checkout_detached(target) {
            Ok(commit_id) => {
                record_operation(
                    &repository,
                    "checkout",
                    &format!("detach {}", &commit_id.to_hex()[..7]),
                    before,
                    Vec::new(),
                    stderr,
                )?;
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
    let before = capture_operation_snapshot(&repository, stderr)?;
    match repository.checkout_new_branch(branch) {
        Ok(_) => {
            record_operation(
                &repository,
                "checkout",
                &format!("new branch {branch}"),
                before,
                Vec::new(),
                stderr,
            )?;
            writeln!(stdout, "{message_prefix} '{branch}'")?;
            Ok(ExitCode::SUCCESS)
        }
        Err(error) => write_command_error(stderr, error),
    }
}

fn merge_command(
    args: &[String],
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> io::Result<ExitCode> {
    let Some(merge_args) = parse_merge_args(args, stderr)? else {
        return Ok(ExitCode::from(129));
    };
    let repository = match discover_repository(stderr)? {
        Some(repository) => repository,
        None => return Ok(ExitCode::from(128)),
    };
    if merge_args.abort {
        let before = capture_operation_snapshot(&repository, stderr)?;
        return match repository.abort_merge() {
            Ok(original_head) => {
                record_operation(
                    &repository,
                    "merge",
                    "abort merge",
                    before,
                    Vec::new(),
                    stderr,
                )?;
                writeln!(
                    stdout,
                    "Aborted merge; restored {}",
                    &original_head.to_hex()[..7]
                )?;
                Ok(ExitCode::SUCCESS)
            }
            Err(error) => write_command_error(stderr, error),
        };
    }
    if merge_args.quit {
        let before = capture_operation_snapshot(&repository, stderr)?;
        return match repository.quit_merge() {
            Ok(()) => {
                record_operation(
                    &repository,
                    "merge",
                    "quit merge",
                    before,
                    Vec::new(),
                    stderr,
                )?;
                Ok(ExitCode::SUCCESS)
            }
            Err(error) => write_command_error(stderr, error),
        };
    }
    if merge_args.continue_merge {
        let before = capture_operation_snapshot(&repository, stderr)?;
        return match repository.continue_merge(&rit_core::CommitOptions {
            verify: merge_args.verify,
            ..rit_core::CommitOptions::default()
        }) {
            Ok(result) => {
                record_operation(
                    &repository,
                    "merge",
                    "continue merge",
                    before,
                    vec![result.commit_id],
                    stderr,
                )?;
                writeln!(stdout, "[{}] merge commit", &result.commit_id.to_hex()[..7])?;
                Ok(ExitCode::SUCCESS)
            }
            Err(error) => write_command_error(stderr, error),
        };
    }
    let Some(target) = merge_args.target.as_deref() else {
        writeln!(stderr, "rit: merge requires a target revision")?;
        return Ok(ExitCode::from(129));
    };
    if merge_args.plan {
        return match repository.plan_merge_ff_only(target) {
            Ok(rit_core::MergePlan::AlreadyUpToDate { commit_id }) => {
                writeln!(stdout, "merge: plan")?;
                writeln!(stdout, "action: already-up-to-date")?;
                writeln!(stdout, "head: {}", &commit_id.to_hex()[..7])?;
                Ok(ExitCode::SUCCESS)
            }
            Ok(rit_core::MergePlan::FastForward {
                old_id,
                new_id,
                paths_to_update,
                paths_to_remove,
            }) => {
                writeln!(stdout, "merge: plan")?;
                writeln!(stdout, "action: fast-forward")?;
                writeln!(stdout, "old: {}", &old_id.to_hex()[..7])?;
                writeln!(stdout, "new: {}", &new_id.to_hex()[..7])?;
                for path in paths_to_update {
                    writeln!(stdout, "update: {path}")?;
                }
                for path in paths_to_remove {
                    writeln!(stdout, "remove: {path}")?;
                }
                Ok(ExitCode::SUCCESS)
            }
            Ok(rit_core::MergePlan::NonFastForward {
                head_id,
                target_id,
                merge_base,
                head_changed_paths,
                target_changed_paths,
                conflict_paths,
                conflict_stages,
            }) => {
                writeln!(stdout, "merge: plan")?;
                writeln!(stdout, "action: non-fast-forward")?;
                writeln!(stdout, "head: {}", &head_id.to_hex()[..7])?;
                writeln!(stdout, "target: {}", &target_id.to_hex()[..7])?;
                match merge_base {
                    Some(commit_id) => {
                        writeln!(stdout, "merge-base: {}", &commit_id.to_hex()[..7])?
                    }
                    None => writeln!(stdout, "merge-base: <none>")?,
                }
                for path in head_changed_paths {
                    writeln!(stdout, "head-change: {path}")?;
                }
                for path in target_changed_paths {
                    writeln!(stdout, "target-change: {path}")?;
                }
                for path in conflict_paths {
                    writeln!(stdout, "conflict-candidate: {path}")?;
                }
                for stage in conflict_stages {
                    writeln!(
                        stdout,
                        "conflict-stage: {} base={} head={} target={}",
                        stage.path,
                        format_merge_stage(stage.base),
                        format_merge_stage(stage.head),
                        format_merge_stage(stage.target)
                    )?;
                }
                writeln!(stdout, "requires: merge-commit")?;
                Ok(ExitCode::SUCCESS)
            }
            Err(error) => write_command_error(stderr, error),
        };
    }
    if merge_args.explain {
        writeln!(stdout, "merge: explain")?;
        writeln!(stdout, "target: {target}")?;
        return match repository.plan_merge_ff_only(target) {
            Ok(rit_core::MergePlan::AlreadyUpToDate { commit_id }) => {
                writeln!(stdout, "action: already-up-to-date")?;
                writeln!(stdout, "head: {}", &commit_id.to_hex()[..7])?;
                writeln!(stdout, "reason: HEAD already equals the target commit")?;
                Ok(ExitCode::SUCCESS)
            }
            Ok(rit_core::MergePlan::FastForward {
                old_id,
                new_id,
                paths_to_update,
                paths_to_remove,
            }) => {
                writeln!(stdout, "action: fast-forward")?;
                writeln!(stdout, "old: {}", &old_id.to_hex()[..7])?;
                writeln!(stdout, "new: {}", &new_id.to_hex()[..7])?;
                writeln!(stdout, "reason: HEAD is an ancestor of the target commit")?;
                for path in paths_to_update {
                    writeln!(stdout, "update: {path}")?;
                }
                for path in paths_to_remove {
                    writeln!(stdout, "remove: {path}")?;
                }
                Ok(ExitCode::SUCCESS)
            }
            Ok(rit_core::MergePlan::NonFastForward {
                head_id,
                target_id,
                merge_base,
                head_changed_paths,
                target_changed_paths,
                conflict_paths,
                conflict_stages,
            }) => {
                writeln!(stdout, "action: non-fast-forward")?;
                writeln!(stdout, "head: {}", &head_id.to_hex()[..7])?;
                writeln!(stdout, "target: {}", &target_id.to_hex()[..7])?;
                match merge_base {
                    Some(commit_id) => {
                        writeln!(stdout, "merge-base: {}", &commit_id.to_hex()[..7])?
                    }
                    None => writeln!(stdout, "merge-base: <none>")?,
                }
                writeln!(
                    stdout,
                    "reason: HEAD is not an ancestor of the target commit"
                )?;
                for path in head_changed_paths {
                    writeln!(stdout, "head-change: {path}")?;
                }
                for path in target_changed_paths {
                    writeln!(stdout, "target-change: {path}")?;
                }
                for path in conflict_paths {
                    writeln!(stdout, "conflict-candidate: {path}")?;
                }
                for stage in conflict_stages {
                    writeln!(
                        stdout,
                        "conflict-stage: {} base={} head={} target={}",
                        stage.path,
                        format_merge_stage(stage.base),
                        format_merge_stage(stage.head),
                        format_merge_stage(stage.target)
                    )?;
                }
                writeln!(stdout, "requires: merge-commit")?;
                Ok(ExitCode::SUCCESS)
            }
            Err(error) => {
                writeln!(stdout, "action: unsupported")?;
                writeln!(stdout, "reason: {error}")?;
                Ok(ExitCode::SUCCESS)
            }
        };
    }
    let before = capture_operation_snapshot(&repository, stderr)?;
    let merge_result = if merge_args.ff_only {
        repository.merge_ff_only(target)
    } else {
        repository.merge_with_options(
            target,
            &rit_core::MergeOptions {
                verify: merge_args.verify,
            },
        )
    };
    match merge_result {
        Ok(rit_core::MergeResult::AlreadyUpToDate { .. }) => {
            record_operation(
                &repository,
                "merge",
                &format!("already up to date with {target}"),
                before,
                Vec::new(),
                stderr,
            )?;
            writeln!(stdout, "Already up to date.")?;
            Ok(ExitCode::SUCCESS)
        }
        Ok(rit_core::MergeResult::FastForward { old_id, new_id }) => {
            record_operation(
                &repository,
                "merge",
                &format!("fast-forward {target}"),
                before,
                Vec::new(),
                stderr,
            )?;
            writeln!(
                stdout,
                "Updating {}..{}",
                &old_id.to_hex()[..7],
                &new_id.to_hex()[..7]
            )?;
            writeln!(stdout, "Fast-forward")?;
            Ok(ExitCode::SUCCESS)
        }
        Ok(rit_core::MergeResult::MergeCommit {
            old_id,
            target_id,
            commit_id,
        }) => {
            record_operation(
                &repository,
                "merge",
                &format!("merge commit {target}"),
                before,
                vec![commit_id],
                stderr,
            )?;
            writeln!(
                stdout,
                "Merged {} and {} into {}",
                &old_id.to_hex()[..7],
                &target_id.to_hex()[..7],
                &commit_id.to_hex()[..7]
            )?;
            Ok(ExitCode::SUCCESS)
        }
        Ok(rit_core::MergeResult::Conflicts {
            conflict_paths,
            conflict_reports,
            ..
        }) => {
            record_operation_with_changed_paths(
                &repository,
                "merge",
                &format!("conflicted merge {target}"),
                before,
                conflict_paths.clone(),
                Vec::new(),
                stderr,
            )?;
            for report in &conflict_reports {
                write_merge_conflict_report(stdout, report, target)?;
            }
            writeln!(
                stdout,
                "Automatic merge failed; fix conflicts and then commit the result."
            )?;
            Ok(ExitCode::from(1))
        }
        Err(error) => write_command_error(stderr, error),
    }
}

fn cherry_pick_command(
    args: &[String],
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> io::Result<ExitCode> {
    let mut commit = true;
    let mut target = None;
    for arg in args {
        if arg == "-n" || arg == "--no-commit" {
            commit = false;
        } else if arg == "--commit" {
            commit = true;
        } else if arg.starts_with('-') {
            writeln!(stderr, "rit: unsupported cherry-pick option '{arg}'")?;
            return Ok(ExitCode::from(129));
        } else if target.is_some() {
            writeln!(
                stderr,
                "rit: cherry-pick currently supports only one target revision, unexpected '{arg}'"
            )?;
            return Ok(ExitCode::from(129));
        } else {
            target = Some(arg.as_str());
        }
    }
    let Some(target) = target else {
        writeln!(stderr, "rit: cherry-pick requires a target revision")?;
        return Ok(ExitCode::from(129));
    };
    let repository = match discover_repository(stderr)? {
        Some(repository) => repository,
        None => return Ok(ExitCode::from(128)),
    };
    let before = capture_operation_snapshot(&repository, stderr)?;
    match repository.cherry_pick_with_options(target, &rit_core::CherryPickOptions { commit }) {
        Ok(result) => {
            let created_objects = result.commit_id.into_iter().collect::<Vec<_>>();
            record_operation(
                &repository,
                "cherry-pick",
                &format!("cherry-pick {target}"),
                before,
                created_objects,
                stderr,
            )?;
            if let Some(commit_id) = result.commit_id {
                writeln!(stdout, "[{}] {}", &commit_id.to_hex()[..7], target)?;
            }
            Ok(ExitCode::SUCCESS)
        }
        Err(error) => write_command_error(stderr, error),
    }
}

struct ParsedMergeArgs {
    target: Option<String>,
    plan: bool,
    explain: bool,
    ff_only: bool,
    abort: bool,
    quit: bool,
    continue_merge: bool,
    verify: bool,
}

fn parse_merge_args(
    args: &[String],
    stderr: &mut dyn Write,
) -> io::Result<Option<ParsedMergeArgs>> {
    let mut plan = false;
    let mut explain = false;
    let mut ff_only = false;
    let mut abort = false;
    let mut quit = false;
    let mut continue_merge = false;
    let mut verify = true;
    let mut target = None;
    let mut index = 0;
    while index < args.len() {
        let arg = &args[index];
        if arg == "--plan" {
            plan = true;
        } else if arg == "--ff-only" {
            ff_only = true;
        } else if arg == "--abort" {
            abort = true;
        } else if arg == "--quit" {
            quit = true;
        } else if arg == "--continue" {
            continue_merge = true;
        } else if arg == "-n" || arg == "--no-verify" {
            verify = false;
        } else if arg == "--verify" {
            verify = true;
        } else if arg == "explain" && target.is_none() {
            explain = true;
            index += 1;
            let Some(next_target) = args.get(index) else {
                writeln!(stderr, "rit: merge explain requires a target revision")?;
                return Ok(None);
            };
            if next_target.starts_with('-') {
                writeln!(stderr, "rit: merge explain requires a target revision")?;
                return Ok(None);
            }
            target = Some(next_target.clone());
        } else if arg.starts_with('-') {
            writeln!(stderr, "rit: unsupported merge option '{arg}'")?;
            return Ok(None);
        } else if target.is_some() {
            writeln!(
                stderr,
                "rit: merge currently supports only one target revision"
            )?;
            return Ok(None);
        } else {
            target = Some(arg.clone());
        }
        index += 1;
    }
    let state_option_count = [abort, quit, continue_merge]
        .into_iter()
        .filter(|selected| *selected)
        .count();
    if state_option_count > 1 {
        writeln!(
            stderr,
            "rit: merge can use only one of --abort, --quit, and --continue"
        )?;
        return Ok(None);
    }
    if (abort || quit || continue_merge) && target.is_some() {
        let option = if abort {
            "--abort"
        } else if quit {
            "--quit"
        } else {
            "--continue"
        };
        writeln!(
            stderr,
            "rit: merge {option} does not take a target revision"
        )?;
        return Ok(None);
    }
    if !(abort || quit || continue_merge) && target.is_none() {
        writeln!(stderr, "rit: merge requires a target revision")?;
        return Ok(None);
    }
    Ok(Some(ParsedMergeArgs {
        target,
        plan,
        explain,
        ff_only,
        abort,
        quit,
        continue_merge,
        verify,
    }))
}

fn op_command(
    args: &[String],
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> io::Result<ExitCode> {
    let repository = match discover_repository(stderr)? {
        Some(repository) => repository,
        None => return Ok(ExitCode::from(128)),
    };
    match args {
        [subcommand] if subcommand == "log" => match repository.operations().log_with_warnings() {
            Ok(log) => {
                for warning in &log.warnings {
                    writeln!(
                        stderr,
                        "rit: warning: skipped malformed operation journal line {}: {}",
                        warning.line_number, warning.message
                    )?;
                }
                for record in log.records.iter().rev() {
                    writeln!(
                        stdout,
                        "{} {} {} -> {} {}",
                        record.id,
                        record.command,
                        short_head(record.before.head),
                        short_head(record.after.head),
                        record.summary
                    )?;
                    if !record.changed_paths.is_empty() {
                        writeln!(stdout, "  paths: {}", record.changed_paths.join(", "))?;
                    }
                    if !record.created_object_ids.is_empty() {
                        let object_ids = record
                            .created_object_ids
                            .iter()
                            .map(|object_id| object_id.to_hex()[..7].to_owned())
                            .collect::<Vec<_>>()
                            .join(", ");
                        writeln!(stdout, "  objects: {object_ids}")?;
                    }
                }
                Ok(ExitCode::SUCCESS)
            }
            Err(error) => write_command_error(stderr, error),
        },
        [subcommand, flag] if subcommand == "log" && flag == "--json" => {
            match repository.operations().log_with_warnings() {
                Ok(log) => {
                    op::write_operation_log_json(stdout, &log)?;
                    Ok(ExitCode::SUCCESS)
                }
                Err(error) => write_command_error(stderr, error),
            }
        }
        [subcommand, flag] if subcommand == "log" => {
            writeln!(stderr, "rit: unsupported op log option '{flag}'")?;
            Ok(ExitCode::from(129))
        }
        [subcommand, id] if subcommand == "restore" => match repository.operations().restore(id) {
            Ok(result) => {
                write_restore_result(stdout, "Restored operation", &result)?;
                Ok(ExitCode::SUCCESS)
            }
            Err(error) => write_command_error(stderr, error),
        },
        [subcommand, ..] => {
            writeln!(stderr, "rit: unsupported op subcommand '{subcommand}'")?;
            Ok(ExitCode::from(129))
        }
        [] => {
            writeln!(stderr, "rit: op requires a subcommand")?;
            Ok(ExitCode::from(129))
        }
    }
}

fn undo_command(
    args: &[String],
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> io::Result<ExitCode> {
    if !args.is_empty() {
        writeln!(stderr, "rit: undo does not accept arguments yet")?;
        return Ok(ExitCode::from(129));
    }
    let repository = match discover_repository(stderr)? {
        Some(repository) => repository,
        None => return Ok(ExitCode::from(128)),
    };
    match repository.operations().undo_last() {
        Ok(result) => {
            write_restore_result(stdout, "Undid operation", &result)?;
            Ok(ExitCode::SUCCESS)
        }
        Err(error) => write_command_error(stderr, error),
    }
}

fn write_restore_result(
    stdout: &mut dyn Write,
    prefix: &str,
    result: &rit_core::OperationRestoreResult,
) -> io::Result<()> {
    match result.restored_head {
        Some(head) if result.restored_worktree => writeln!(
            stdout,
            "{prefix} {} and restored {}",
            result.id,
            &head.to_hex()[..7]
        ),
        Some(head) if result.restored_index => writeln!(
            stdout,
            "{prefix} {} and restored index at {}",
            result.id,
            &head.to_hex()[..7]
        ),
        None if result.restored_index => {
            writeln!(stdout, "{prefix} {} and restored index", result.id)
        }
        _ => writeln!(stdout, "{prefix} {}", result.id),
    }
}

fn capture_operation_snapshot(
    repository: &rit_core::Repository,
    stderr: &mut dyn Write,
) -> io::Result<Option<rit_core::OperationSnapshot>> {
    match repository.operations().snapshot() {
        Ok(snapshot) => Ok(Some(snapshot)),
        Err(error) => {
            writeln!(
                stderr,
                "rit: warning: could not snapshot operation: {error}"
            )?;
            Ok(None)
        }
    }
}

fn record_operation(
    repository: &rit_core::Repository,
    command: &str,
    summary: &str,
    before: Option<rit_core::OperationSnapshot>,
    created_object_ids: Vec<rit_core::ObjectId>,
    stderr: &mut dyn Write,
) -> io::Result<()> {
    let Some(before) = before else {
        return Ok(());
    };
    let after = match repository.operations().snapshot() {
        Ok(snapshot) => snapshot,
        Err(error) => {
            writeln!(
                stderr,
                "rit: warning: could not snapshot operation result: {error}"
            )?;
            return Ok(());
        }
    };
    let changed_paths = match repository
        .operations()
        .changed_paths_between(&before, &after)
    {
        Ok(paths) => paths,
        Err(error) => {
            writeln!(
                stderr,
                "rit: warning: could not compute operation paths: {error}"
            )?;
            Vec::new()
        }
    };
    if let Err(error) = repository.operations().record_with_details(
        command,
        summary,
        before,
        after,
        changed_paths,
        created_object_ids,
    ) {
        writeln!(stderr, "rit: warning: could not record operation: {error}")?;
    }
    Ok(())
}

fn record_operation_with_changed_paths(
    repository: &rit_core::Repository,
    command: &str,
    summary: &str,
    before: Option<rit_core::OperationSnapshot>,
    changed_paths: Vec<String>,
    created_object_ids: Vec<rit_core::ObjectId>,
    stderr: &mut dyn Write,
) -> io::Result<()> {
    let Some(before) = before else {
        return Ok(());
    };
    let after = match repository.operations().snapshot() {
        Ok(snapshot) => snapshot,
        Err(error) => {
            writeln!(
                stderr,
                "rit: warning: could not snapshot operation result: {error}"
            )?;
            return Ok(());
        }
    };
    if let Err(error) = repository.operations().record_with_details(
        command,
        summary,
        before,
        after,
        changed_paths,
        created_object_ids,
    ) {
        writeln!(stderr, "rit: warning: could not record operation: {error}")?;
    }
    Ok(())
}

fn merge_changed_paths(left: Vec<String>, right: Vec<String>) -> Vec<String> {
    let mut paths = left.into_iter().chain(right).collect::<Vec<_>>();
    paths.sort();
    paths.dedup();
    paths
}

fn short_head(head: Option<rit_core::ObjectId>) -> String {
    head.map(|object_id| object_id.to_hex()[..7].to_owned())
        .unwrap_or_else(|| "-".to_owned())
}

fn format_merge_stage(stage: Option<rit_core::MergeConflictStageEntry>) -> String {
    stage
        .map(|stage| format!("{:o}:{}", stage.mode, &stage.object_id.to_hex()[..7]))
        .unwrap_or_else(|| "-".to_owned())
}

fn write_merge_conflict_report(
    stdout: &mut dyn Write,
    report: &rit_core::MergeConflictReport,
    target: &str,
) -> io::Result<()> {
    match report.kind {
        rit_core::MergeConflictKind::Content => {
            writeln!(stdout, "Auto-merging {}", report.path)?;
            writeln!(
                stdout,
                "CONFLICT (content): Merge conflict in {}",
                report.path
            )
        }
        rit_core::MergeConflictKind::BinaryContent => {
            writeln!(
                stdout,
                "warning: Cannot merge binary files: {} (HEAD vs. {target})",
                report.path
            )?;
            writeln!(stdout, "Auto-merging {}", report.path)?;
            writeln!(
                stdout,
                "CONFLICT (content): Merge conflict in {}",
                report.path
            )
        }
        rit_core::MergeConflictKind::AddAdd => {
            writeln!(stdout, "Auto-merging {}", report.path)?;
            writeln!(
                stdout,
                "CONFLICT (add/add): Merge conflict in {}",
                report.path
            )
        }
        rit_core::MergeConflictKind::DistinctTypes => writeln!(
            stdout,
            "CONFLICT (distinct types): {} had different types on each side; renamed one of them so each can be recorded somewhere.",
            report.path
        ),
        rit_core::MergeConflictKind::ModifyDelete {
            deleted_side,
            modified_side,
            worktree_side,
        } => writeln!(
            stdout,
            "CONFLICT (modify/delete): {} deleted in {} and modified in {}.  Version {} of {} left in tree.",
            report.path,
            format_merge_side(deleted_side, target),
            format_merge_side(modified_side, target),
            format_merge_side(worktree_side, target),
            report.path
        ),
    }
}

fn format_merge_side(side: rit_core::MergeConflictSide, target: &str) -> &str {
    match side {
        rit_core::MergeConflictSide::Head => "HEAD",
        rit_core::MergeConflictSide::Target => target,
    }
}

fn write_command_error(stderr: &mut dyn Write, error: rit_core::RitError) -> io::Result<ExitCode> {
    if let rit_core::RitError::InvalidInput { message } = &error {
        if let Some(pathspec) = message.strip_prefix("pathspec did not match any files: ") {
            writeln!(
                stderr,
                "fatal: pathspec '{}' did not match any files",
                git_error_pathspec(pathspec)
            )?;
            return Ok(ExitCode::from(128));
        }
        if let Some(pathspec) =
            message.strip_prefix("pathspec did not match any file known to git: ")
        {
            writeln!(
                stderr,
                "error: pathspec '{}' did not match any file(s) known to git",
                git_error_pathspec(pathspec)
            )?;
            return Ok(ExitCode::from(1));
        }
        if let Some(pathspec) = message.strip_prefix("pathspec did not match any indexed file: ") {
            writeln!(
                stderr,
                "error: pathspec '{}' did not match any file(s) known to git",
                git_error_pathspec(pathspec)
            )?;
            return Ok(ExitCode::from(1));
        }
    }
    writeln!(stderr, "rit: {error}")?;
    Ok(ExitCode::from(1))
}

fn is_reset_noop_pathspec_error(error: &rit_core::RitError) -> bool {
    matches!(
        error,
        rit_core::RitError::InvalidInput { message }
            if message.starts_with("pathspec did not match any file known to git: ")
    )
}

fn write_pathspec_file_nul_requires_file(stderr: &mut dyn Write) -> io::Result<()> {
    writeln!(
        stderr,
        "fatal: the option '--pathspec-file-nul' requires '--pathspec-from-file'"
    )
}

fn write_pathspec_file_cannot_mix_with_args(stderr: &mut dyn Write) -> io::Result<()> {
    writeln!(
        stderr,
        "fatal: '--pathspec-from-file' and pathspec arguments cannot be used together"
    )
}

fn pathspec_from_file_missing_value(args: &[String], index: usize, after_separator: bool) -> bool {
    !after_separator && args[index] == "--pathspec-from-file" && index + 1 >= args.len()
}

fn write_pathspec_from_file_requires_value(stderr: &mut dyn Write) -> io::Result<()> {
    writeln!(
        stderr,
        "error: option `pathspec-from-file' requires a value"
    )
}

fn git_error_pathspec(pathspec: &str) -> String {
    pathspec
        .chars()
        .map(|ch| if ch.is_control() { '?' } else { ch })
        .collect()
}

struct ParsedCommitArgs {
    message: String,
    options: rit_core::CommitOptions,
    plan: bool,
}

fn parse_commit_args(args: &[String]) -> rit_core::Result<Option<ParsedCommitArgs>> {
    let mut message = None;
    let mut options = rit_core::CommitOptions::default();
    let mut plan = false;
    let mut index = 0;

    while index < args.len() {
        let arg = &args[index];
        if arg == "-m" || arg == "--message" {
            index += 1;
            let Some(value) = args.get(index) else {
                return Ok(None);
            };
            message = Some(value.clone());
        } else if let Some(value) = arg.strip_prefix("--message=") {
            message = Some(value.to_owned());
        } else if arg == "--author" {
            index += 1;
            let Some(value) = args.get(index) else {
                return Ok(None);
            };
            options.author = Some(rit_core::SignatureIdentity::parse_author(value)?);
        } else if let Some(value) = arg.strip_prefix("--author=") {
            options.author = Some(rit_core::SignatureIdentity::parse_author(value)?);
        } else if arg == "--date" {
            index += 1;
            let Some(value) = args.get(index) else {
                return Ok(None);
            };
            options.author_date = Some(rit_core::SignatureTime::parse_git_raw(value)?);
        } else if let Some(value) = arg.strip_prefix("--date=") {
            options.author_date = Some(rit_core::SignatureTime::parse_git_raw(value)?);
        } else if arg == "-n" || arg == "--no-verify" {
            options.verify = false;
        } else if arg == "--verify" {
            options.verify = true;
        } else if arg == "--plan" {
            plan = true;
        } else {
            return Ok(None);
        }
        index += 1;
    }
    Ok(message.map(|message| ParsedCommitArgs {
        message,
        options,
        plan,
    }))
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
    fn clone_help_is_available() {
        let (code, stdout, stderr) = run_with(&["help", "clone"]);

        assert_eq!(code, ExitCode::SUCCESS);
        assert!(stdout.contains("rit clone"));
        assert_eq!(stderr, "");
    }

    #[test]
    fn fetch_help_is_available() {
        let (code, stdout, stderr) = run_with(&["help", "fetch"]);

        assert_eq!(code, ExitCode::SUCCESS);
        assert!(stdout.contains("rit fetch"));
        assert_eq!(stderr, "");
    }

    #[test]
    fn push_help_is_available() {
        let (code, stdout, stderr) = run_with(&["help", "push"]);

        assert_eq!(code, ExitCode::SUCCESS);
        assert!(stdout.contains("rit push"));
        assert_eq!(stderr, "");
    }

    #[test]
    fn clone_local_rejects_remote_locations() {
        let (code, stdout, stderr) = run_with(&[
            "clone",
            "--local",
            "--no-checkout",
            "https://example.test/repo.git",
        ]);

        assert_eq!(code, ExitCode::from(129));
        assert_eq!(stdout, "");
        assert!(stderr.contains("requires a local repository path"));
    }

    #[test]
    fn fetch_rejects_unsupported_options() {
        let (code, stdout, stderr) = run_with(&["fetch", "--depth=1", "origin"]);

        assert_eq!(code, ExitCode::from(129));
        assert_eq!(stdout, "");
        assert!(stderr.contains("unsupported fetch option"));
    }

    #[test]
    fn push_rejects_local_locations() {
        let (code, stdout, stderr) = run_with(&["push", "../repo.git", "HEAD:refs/heads/main"]);

        assert_eq!(code, ExitCode::from(129));
        assert_eq!(stdout, "");
        assert!(stderr.contains("http://, https://, or SSH smart remotes"));
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
    fn status_help_mentions_explain() {
        let (code, stdout, stderr) = run_with(&["help", "status"]);

        assert_eq!(code, ExitCode::SUCCESS);
        assert!(stdout.contains("rit status --explain <path>"));
        assert_eq!(stderr, "");
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
    fn ignore_help_is_available() {
        let (code, stdout, stderr) = run_with(&["help", "ignore"]);

        assert_eq!(code, ExitCode::SUCCESS);
        assert!(stdout.contains("rit ignore explain"));
        assert_eq!(stderr, "");
    }

    #[test]
    fn pathspec_help_is_available() {
        let (code, stdout, stderr) = run_with(&["help", "pathspec"]);

        assert_eq!(code, ExitCode::SUCCESS);
        assert!(stdout.contains("rit pathspec explain"));
        assert_eq!(stderr, "");
    }

    #[test]
    fn pathspec_explain_prints_parsed_rules() {
        let (code, stdout, stderr) =
            run_with(&["pathspec", "explain", ":(icase,glob)*.RS", ":!target"]);

        assert_eq!(code, ExitCode::SUCCESS);
        assert!(stdout.contains("pathspec: explain\n"));
        assert!(stdout.contains("pattern: *.RS\n"));
        assert!(stdout.contains("mode: glob\n"));
        assert!(stdout.contains("ignore-case: true\n"));
        assert!(stdout.contains("exclude: true\n"));
        assert_eq!(stderr, "");
    }

    #[test]
    fn cherry_pick_help_is_available() {
        let (code, stdout, stderr) = run_with(&["help", "cherry-pick"]);

        assert_eq!(code, ExitCode::SUCCESS);
        assert!(stdout.contains("rit cherry-pick"));
        assert_eq!(stderr, "");
    }

    #[test]
    fn auth_help_is_available() {
        let (code, stdout, stderr) = run_with(&["help", "auth"]);

        assert_eq!(code, ExitCode::SUCCESS);
        assert!(stdout.contains("rit auth explain"));
        assert_eq!(stderr, "");
    }

    #[test]
    fn indexdb_help_is_available() {
        let (code, stdout, stderr) = run_with(&["help", "indexdb"]);

        assert_eq!(code, ExitCode::SUCCESS);
        assert!(stdout.contains("rit indexdb"));
        assert_eq!(stderr, "");
    }

    #[test]
    fn file_history_help_is_available() {
        let (code, stdout, stderr) = run_with(&["help", "file-history"]);

        assert_eq!(code, ExitCode::SUCCESS);
        assert!(stdout.contains("rit file-history <path>"));
        assert_eq!(stderr, "");
    }

    #[cfg(not(feature = "indexdb"))]
    #[test]
    fn indexdb_command_reports_missing_feature() {
        let (code, stdout, stderr) = run_with(&["indexdb", "status"]);

        assert_eq!(code, ExitCode::from(1));
        assert_eq!(stdout, "");
        assert!(stderr.contains("does not include indexdb support"));
    }

    #[cfg(not(feature = "indexdb"))]
    #[test]
    fn file_history_command_reports_missing_feature() {
        let (code, stdout, stderr) = run_with(&["file-history", "file.txt"]);

        assert_eq!(code, ExitCode::from(1));
        assert_eq!(stdout, "");
        assert!(stderr.contains("does not include indexdb support"));
    }

    #[test]
    fn auth_explain_prints_remote_request_without_secret_values() {
        let (code, stdout, stderr) =
            run_with(&["auth", "explain", "https://alice@example.test/org/repo.git"]);

        assert_eq!(code, ExitCode::SUCCESS);
        assert!(stdout.contains("auth: explain\n"));
        assert!(stdout.contains("protocol: https\n"));
        assert!(stdout.contains("credential-lookup: true\n"));
        assert!(stdout.contains("request-host: example.test\n"));
        assert!(stdout.contains("request-path: org/repo.git\n"));
        assert!(stdout.contains("request-username: alice\n"));
        assert!(!stdout.contains("secret"));
        assert_eq!(stderr, "");
    }

    #[test]
    fn workspace_help_is_available() {
        let (code, stdout, stderr) = run_with(&["help", "workspace"]);

        assert_eq!(code, ExitCode::SUCCESS);
        assert!(stdout.contains("rit workspace prefetch"));
        assert_eq!(stderr, "");
    }

    #[test]
    fn doctor_help_is_available() {
        let (code, stdout, stderr) = run_with(&["help", "doctor"]);

        assert_eq!(code, ExitCode::SUCCESS);
        assert!(stdout.contains("rit doctor"));
        assert_eq!(stderr, "");
    }

    #[test]
    fn doctor_rejects_options_for_now() {
        let (code, stdout, stderr) = run_with(&["doctor", "--json"]);

        assert_eq!(code, ExitCode::from(129));
        assert_eq!(stdout, "");
        assert!(stderr.contains("does not accept options yet"));
    }

    #[test]
    fn repair_help_is_available() {
        let (code, stdout, stderr) = run_with(&["help", "repair"]);

        assert_eq!(code, ExitCode::SUCCESS);
        assert!(stdout.contains("rit repair"));
        assert_eq!(stderr, "");
    }

    #[test]
    fn repair_rejects_unknown_options() {
        let (code, stdout, stderr) = run_with(&["repair", "--json"]);

        assert_eq!(code, ExitCode::from(129));
        assert_eq!(stdout, "");
        assert!(stderr.contains("unsupported repair option"));
    }

    #[test]
    fn workspace_prefetch_requires_profile_name() {
        let (code, stdout, stderr) = run_with(&["workspace", "prefetch"]);

        assert_eq!(code, ExitCode::from(129));
        assert_eq!(stdout, "");
        assert!(stderr.contains("requires a profile name"));
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
