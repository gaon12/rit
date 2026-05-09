use rit_testkit::{CommandSpec, CompareOptions};
use std::ffi::OsString;
use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    match run(std::env::args_os().skip(1).collect()) {
        Ok(code) => code,
        Err(error) => {
            eprintln!("rit-testkit: {error}");
            ExitCode::from(1)
        }
    }
}

fn run(args: Vec<OsString>) -> Result<ExitCode, Box<dyn std::error::Error>> {
    match args.first().and_then(|arg| arg.to_str()) {
        Some("compare") => compare_command(&args[1..]),
        Some("help") | Some("-h") | Some("--help") | None => {
            print_help();
            Ok(ExitCode::SUCCESS)
        }
        Some(command) => {
            eprintln!("rit-testkit: unknown command '{command}'");
            Ok(ExitCode::from(129))
        }
    }
}

fn compare_command(args: &[OsString]) -> Result<ExitCode, Box<dyn std::error::Error>> {
    let mut fixture = None;
    let mut git = Vec::new();
    let mut rit = Vec::new();
    let mut state = ParserState::Options;
    let mut compare_repository_state = true;
    let mut index = 0;

    while index < args.len() {
        let arg = &args[index];
        match (state, arg.to_str()) {
            (ParserState::Options, Some("--fixture")) => {
                index += 1;
                let Some(path) = args.get(index) else {
                    return Err("--fixture requires a path".into());
                };
                fixture = Some(PathBuf::from(path));
            }
            (ParserState::Options, Some("--no-state")) => {
                compare_repository_state = false;
            }
            (_, Some("--git")) => state = ParserState::Git,
            (_, Some("--rit")) => state = ParserState::Rit,
            (ParserState::Git, _) => git.push(arg.clone()),
            (ParserState::Rit, _) => rit.push(arg.clone()),
            (ParserState::Options, _) => {
                return Err(
                    format!("unexpected compare argument: {}", arg.to_string_lossy()).into(),
                );
            }
        }
        index += 1;
    }

    let fixture = fixture.ok_or("--fixture is required")?;
    let git = CommandSpec::from_words("git", git)?;
    let rit = CommandSpec::from_words("rit", rit)?;
    let mut options = CompareOptions::new(fixture, git, rit);
    options.compare_repository_state = compare_repository_state;

    let outcome = rit_testkit::compare(&options)?;
    print!("{}", outcome.report());
    if outcome.is_match() {
        Ok(ExitCode::SUCCESS)
    } else {
        Ok(ExitCode::from(1))
    }
}

#[derive(Clone, Copy)]
enum ParserState {
    Options,
    Git,
    Rit,
}

fn print_help() {
    println!(
        "\
rit-testkit

Usage:
  rit-testkit compare --fixture <repo> --git <program> [args...] --rit <program> [args...]

Options:
  --no-state   Compare only stdout, stderr, and exit code.
"
    );
}
