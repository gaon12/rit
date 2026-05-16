use std::io::{self, Write};
use std::process::ExitCode;

pub fn large_files_command(
    args: &[String],
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> io::Result<ExitCode> {
    match args {
        [] => {
            writeln!(stderr, "rit: large-files requires a subcommand")?;
            Ok(ExitCode::from(129))
        }
        [subcommand, rest @ ..] if subcommand == "audit" => audit_command(rest, stdout, stderr),
        [subcommand, ..] => {
            writeln!(
                stderr,
                "rit: unsupported large-files subcommand '{subcommand}'"
            )?;
            Ok(ExitCode::from(129))
        }
    }
}

fn audit_command(
    args: &[String],
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> io::Result<ExitCode> {
    let Some(threshold) = parse_audit_args(args, stderr)? else {
        return Ok(ExitCode::from(129));
    };
    let repository = match crate::discover_repository(stderr)? {
        Some(repository) => repository,
        None => return Ok(ExitCode::from(128)),
    };
    match repository.large_files_audit_with_threshold(threshold) {
        Ok(report) => {
            print_audit_report(stdout, &report)?;
            Ok(ExitCode::SUCCESS)
        }
        Err(error) => crate::write_command_error(stderr, error),
    }
}

fn parse_audit_args(args: &[String], stderr: &mut dyn Write) -> io::Result<Option<usize>> {
    match args {
        [] => Ok(Some(rit_core::DEFAULT_LARGE_FILE_AUDIT_THRESHOLD)),
        [flag, value] if flag == "--threshold" => match value.parse::<usize>() {
            Ok(value) => Ok(Some(value)),
            Err(_) => {
                writeln!(stderr, "rit: --threshold must be a byte count")?;
                Ok(None)
            }
        },
        _ => {
            writeln!(
                stderr,
                "rit: large-files audit accepts only --threshold <bytes>"
            )?;
            Ok(None)
        }
    }
}

fn print_audit_report(
    stdout: &mut dyn Write,
    report: &rit_core::LargeFilesAuditReport,
) -> io::Result<()> {
    writeln!(stdout, "large-files-audit")?;
    writeln!(stdout, "threshold-bytes: {}", report.threshold_bytes)?;
    if report.large_blobs.is_empty() {
        writeln!(stdout, "large-blob: <none>")?;
    }
    for blob in &report.large_blobs {
        writeln!(
            stdout,
            "large-blob: {} size={} object={} first-seen={}",
            blob.path, blob.size, blob.object_id, blob.first_seen_commit
        )?;
    }
    if report.recommended_tracking.is_empty() {
        writeln!(stdout, "recommendation: <none>")?;
    }
    for recommendation in &report.recommended_tracking {
        writeln!(
            stdout,
            "recommendation: {} backend={} reason={}",
            recommendation.pattern, recommendation.backend, recommendation.reason
        )?;
    }
    for step in &report.migration_plan {
        writeln!(
            stdout,
            "migration-step: {} rewrite={} action={}",
            step.step, step.rewrites_history, step.action
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::parse_audit_args;

    #[test]
    fn audit_args_default_to_safe_threshold() {
        let mut stderr = Vec::new();
        let threshold = parse_audit_args(&[], &mut stderr)
            .expect("args should parse")
            .expect("threshold should exist");

        assert_eq!(threshold, rit_core::DEFAULT_LARGE_FILE_AUDIT_THRESHOLD);
        assert!(stderr.is_empty());
    }

    #[test]
    fn audit_args_parse_threshold_bytes() {
        let mut stderr = Vec::new();
        let threshold = parse_audit_args(&["--threshold".to_owned(), "42".to_owned()], &mut stderr)
            .expect("args should parse")
            .expect("threshold should exist");

        assert_eq!(threshold, 42);
        assert!(stderr.is_empty());
    }
}
