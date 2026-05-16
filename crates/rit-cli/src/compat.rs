use std::ffi::OsString;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::time::{SystemTime, UNIX_EPOCH};

const DEFAULT_REPORT_COMMANDS: &[&[&str]] = &[
    &["status", "--porcelain=v1"],
    &["status", "-b", "--porcelain=v1"],
    &["diff", "--name-only"],
    &["diff", "--stat"],
];

pub fn compat_command(
    args: &[String],
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> io::Result<ExitCode> {
    match args {
        [] => {
            writeln!(stderr, "rit: compat requires a subcommand")?;
            Ok(ExitCode::from(129))
        }
        [subcommand, rest @ ..] if subcommand == "check" => check_command(rest, stdout, stderr),
        [subcommand, rest @ ..] if subcommand == "report" => report_command(rest, stdout, stderr),
        [subcommand, rest @ ..] if subcommand == "fixture" => fixture_command(rest, stdout, stderr),
        [subcommand, ..] => {
            writeln!(stderr, "rit: unsupported compat subcommand '{subcommand}'")?;
            Ok(ExitCode::from(129))
        }
    }
}

fn check_command(
    args: &[String],
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> io::Result<ExitCode> {
    let command = normalize_check_command(args, stderr)?;
    if command.is_empty() {
        return Ok(ExitCode::from(129));
    }
    if !is_read_only_command(&command[0]) {
        writeln!(
            stderr,
            "rit: compat check only runs read-only commands in the current repository"
        )?;
        return Ok(ExitCode::from(129));
    }
    let comparison = run_compat_comparison(&command)?;
    print_comparison(stdout, &command, &comparison)?;
    Ok(if comparison.is_match() {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    })
}

fn report_command(
    args: &[String],
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> io::Result<ExitCode> {
    let Some(since) = parse_since(args, stderr)? else {
        return Ok(ExitCode::from(129));
    };
    let changed_paths = changed_paths_since(&since)?;
    writeln!(stdout, "compat-report")?;
    writeln!(stdout, "since: {since}")?;
    for path in changed_paths {
        writeln!(stdout, "changed: {path}")?;
    }

    let mut all_match = true;
    for command in DEFAULT_REPORT_COMMANDS {
        let command = command
            .iter()
            .map(|value| value.to_string())
            .collect::<Vec<_>>();
        let comparison = run_compat_comparison(&command)?;
        all_match &= comparison.is_match();
        print_comparison(stdout, &command, &comparison)?;
    }
    Ok(if all_match {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    })
}

fn fixture_command(
    args: &[String],
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> io::Result<ExitCode> {
    match args {
        [subcommand] if subcommand == "generate" => {
            let path = default_fixture_path()?;
            generate_fixture(&path)?;
            writeln!(stdout, "fixture: {}", path.display())?;
            Ok(ExitCode::SUCCESS)
        }
        [subcommand, path] if subcommand == "generate" => {
            let path = PathBuf::from(path);
            generate_fixture(&path)?;
            writeln!(stdout, "fixture: {}", path.display())?;
            Ok(ExitCode::SUCCESS)
        }
        [] => {
            writeln!(stderr, "rit: compat fixture requires a subcommand")?;
            Ok(ExitCode::from(129))
        }
        [subcommand, ..] => {
            writeln!(
                stderr,
                "rit: unsupported compat fixture subcommand '{subcommand}'"
            )?;
            Ok(ExitCode::from(129))
        }
    }
}

fn normalize_check_command(args: &[String], stderr: &mut dyn Write) -> io::Result<Vec<String>> {
    match args {
        [] => {
            writeln!(stderr, "rit: compat check requires a command")?;
            Ok(Vec::new())
        }
        [separator, rest @ ..] if separator == "--" => Ok(rest.to_vec()),
        _ => Ok(args.to_vec()),
    }
}

fn parse_since(args: &[String], stderr: &mut dyn Write) -> io::Result<Option<String>> {
    match args {
        [flag, rev] if flag == "--since" => Ok(Some(rev.clone())),
        [] => {
            writeln!(stderr, "rit: compat report requires --since <rev>")?;
            Ok(None)
        }
        _ => {
            writeln!(stderr, "rit: compat report accepts only --since <rev>")?;
            Ok(None)
        }
    }
}

fn is_read_only_command(command: &str) -> bool {
    matches!(
        command,
        "status" | "diff" | "log" | "show" | "cat-file" | "ls-tree" | "rev-parse" | "ls-files"
    )
}

fn run_compat_comparison(command: &[String]) -> io::Result<CompatComparison> {
    let git = run_program("git", command)?;
    let rit = run_program(std::env::current_exe()?, command)?;
    Ok(CompatComparison { git, rit })
}

fn run_program(program: impl Into<OsString>, args: &[String]) -> io::Result<CompatOutput> {
    let program = program.into();
    let output = Command::new(&program).args(args).output()?;
    Ok(CompatOutput {
        exit_code: output.status.code(),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    })
}

fn changed_paths_since(since: &str) -> io::Result<Vec<String>> {
    let output = Command::new("git")
        .args(["diff", "--name-only", &format!("{since}..HEAD")])
        .output()?;
    if !output.status.success() {
        return Ok(vec![format!(
            "<could not resolve {since}: {}>",
            String::from_utf8_lossy(&output.stderr).trim()
        )]);
    }
    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(|line| line.to_owned())
        .collect())
}

fn print_comparison(
    stdout: &mut dyn Write,
    command: &[String],
    comparison: &CompatComparison,
) -> io::Result<()> {
    writeln!(stdout, "command: {}", command.join(" "))?;
    writeln!(
        stdout,
        "exit-code: {}",
        same_label(comparison.git.exit_code == comparison.rit.exit_code)
    )?;
    writeln!(
        stdout,
        "stdout: {}",
        same_label(comparison.git.stdout == comparison.rit.stdout)
    )?;
    writeln!(
        stdout,
        "stderr: {}",
        same_label(comparison.git.stderr == comparison.rit.stderr)
    )?;
    if comparison.git.stdout != comparison.rit.stdout {
        writeln!(
            stdout,
            "stdout-diff: {}",
            first_difference(&comparison.git.stdout, &comparison.rit.stdout)
        )?;
    }
    if comparison.git.stderr != comparison.rit.stderr {
        writeln!(
            stdout,
            "stderr-diff: {}",
            first_difference(&comparison.git.stderr, &comparison.rit.stderr)
        )?;
    }
    Ok(())
}

fn same_label(matches: bool) -> &'static str {
    if matches { "same" } else { "different" }
}

fn first_difference(left: &str, right: &str) -> String {
    let left_lines = left.lines().collect::<Vec<_>>();
    let right_lines = right.lines().collect::<Vec<_>>();
    let max = left_lines.len().max(right_lines.len());
    for index in 0..max {
        let left_line = left_lines.get(index).copied().unwrap_or("<missing>");
        let right_line = right_lines.get(index).copied().unwrap_or("<missing>");
        if left_line != right_line {
            return format!("line {} git={left_line:?} rit={right_line:?}", index + 1);
        }
    }
    "outputs differ outside line text".to_owned()
}

fn default_fixture_path() -> io::Result<PathBuf> {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(io::Error::other)?
        .as_nanos();
    Ok(std::env::current_dir()?
        .join("target")
        .join("rit-compat-fixtures")
        .join(format!("basic-{unique}")))
}

fn generate_fixture(path: &Path) -> io::Result<()> {
    if path.exists() {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!("fixture path already exists: {}", path.display()),
        ));
    }
    fs::create_dir_all(path)?;
    run_fixture_git(path, ["init", "--quiet"])?;
    run_fixture_git(path, ["config", "user.name", "Rit Compat"])?;
    run_fixture_git(path, ["config", "user.email", "rit-compat@example.test"])?;
    fs::write(path.join("README.md"), "# rit compat fixture\n")?;
    fs::create_dir_all(path.join("src"))?;
    fs::write(path.join("src").join("main.rs"), "fn main() {}\n")?;
    run_fixture_git(path, ["add", "."])?;
    run_fixture_git(path, ["commit", "--quiet", "-m", "initial fixture"])?;
    Ok(())
}

fn run_fixture_git<const N: usize>(path: &Path, args: [&str; N]) -> io::Result<()> {
    let output = Command::new("git").args(args).current_dir(path).output()?;
    if output.status.success() {
        Ok(())
    } else {
        Err(io::Error::other(format!(
            "git fixture command failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CompatOutput {
    exit_code: Option<i32>,
    stdout: String,
    stderr: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CompatComparison {
    git: CompatOutput,
    rit: CompatOutput,
}

impl CompatComparison {
    fn is_match(&self) -> bool {
        self.git == self.rit
    }
}

#[cfg(test)]
mod tests {
    use super::{first_difference, is_read_only_command, normalize_check_command};

    #[test]
    fn read_only_command_filter_allows_safe_queries() {
        assert!(is_read_only_command("status"));
        assert!(is_read_only_command("diff"));
        assert!(!is_read_only_command("commit"));
        assert!(!is_read_only_command("add"));
    }

    #[test]
    fn check_command_accepts_optional_separator() {
        let mut stderr = Vec::new();
        let command = normalize_check_command(
            &[
                "--".to_owned(),
                "status".to_owned(),
                "--porcelain=v1".to_owned(),
            ],
            &mut stderr,
        )
        .expect("command should parse");

        assert_eq!(command, vec!["status", "--porcelain=v1"]);
        assert!(stderr.is_empty());
    }

    #[test]
    fn first_difference_reports_line_number() {
        assert_eq!(
            first_difference("a\nb\n", "a\nc\n"),
            "line 2 git=\"b\" rit=\"c\""
        );
    }
}
