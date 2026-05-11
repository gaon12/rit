use std::io::{self, Write};
use std::process::ExitCode;

pub fn doctor_command(
    args: &[String],
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> io::Result<ExitCode> {
    if !args.is_empty() {
        writeln!(stderr, "rit: doctor does not accept options yet")?;
        return Ok(ExitCode::from(129));
    }

    let repository = match rit_core::Repository::discover(".") {
        Ok(repository) => repository,
        Err(error) => {
            writeln!(stderr, "rit: {error}")?;
            return Ok(ExitCode::from(128));
        }
    };
    let report = repository.doctor();
    print_report(&report, stdout)?;

    if report.has_errors() {
        Ok(ExitCode::from(1))
    } else {
        Ok(ExitCode::SUCCESS)
    }
}

fn print_report(report: &rit_core::DoctorReport, stdout: &mut dyn Write) -> io::Result<()> {
    writeln!(stdout, "repository: {}", repository_label(report))?;
    writeln!(stdout, "git-dir: {}", report.git_dir)?;
    writeln!(stdout, "common-dir: {}", report.common_dir)?;
    writeln!(stdout, "bare: {}", report.bare)?;
    writeln!(
        stdout,
        "status: {}",
        if report.has_errors() { "error" } else { "ok" }
    )?;

    for check in &report.checks {
        writeln!(
            stdout,
            "[{}] {}: {}",
            severity_label(check.severity),
            check.name,
            check.detail
        )?;
    }

    Ok(())
}

fn repository_label(report: &rit_core::DoctorReport) -> &str {
    report.worktree.as_deref().unwrap_or(&report.git_dir)
}

fn severity_label(severity: rit_core::DoctorSeverity) -> &'static str {
    match severity {
        rit_core::DoctorSeverity::Ok => "ok",
        rit_core::DoctorSeverity::Warning => "warn",
        rit_core::DoctorSeverity::Error => "error",
    }
}
