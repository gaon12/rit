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
        unknown => {
            writeln!(stderr, "rit: no help for unknown command '{unknown}'")?;
            return Ok(ExitCode::from(129));
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
}
