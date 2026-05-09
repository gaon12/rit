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
rit rev-parse [--git-dir] [--show-toplevel] [--is-inside-work-tree]

Print selected paths or repository facts for the current repository.
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
            unsupported => {
                writeln!(stderr, "rit: unsupported rev-parse option '{unsupported}'")?;
                return Ok(ExitCode::from(129));
            }
        }
    }

    Ok(ExitCode::SUCCESS)
}

#[cfg(test)]
mod tests {
    use super::run;
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
}
