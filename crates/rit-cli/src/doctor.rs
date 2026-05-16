use std::io::{self, Write};
use std::process::ExitCode;

pub fn doctor_command(
    args: &[String],
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> io::Result<ExitCode> {
    let json = match args {
        [] => false,
        [flag] if flag == "--json" => true,
        [flag, ..] => {
            writeln!(stderr, "rit: unsupported doctor option '{flag}'")?;
            return Ok(ExitCode::from(129));
        }
    };

    let repository = match rit_core::Repository::discover(".") {
        Ok(repository) => repository,
        Err(error) => {
            writeln!(stderr, "rit: {error}")?;
            return Ok(ExitCode::from(128));
        }
    };
    let report = repository.doctor();
    if json {
        write_report_json(&report, stdout)?;
    } else {
        print_report(&report, stdout)?;
    }

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

fn write_report_json(report: &rit_core::DoctorReport, stdout: &mut dyn Write) -> io::Result<()> {
    writeln!(stdout, "{{")?;
    writeln!(
        stdout,
        "  \"worktree\": {},",
        json_optional_string(report.worktree.as_deref())
    )?;
    writeln!(
        stdout,
        "  \"git_dir\": \"{}\",",
        crate::op::json_escape(&report.git_dir)
    )?;
    writeln!(
        stdout,
        "  \"common_dir\": \"{}\",",
        crate::op::json_escape(&report.common_dir)
    )?;
    writeln!(stdout, "  \"bare\": {},", report.bare)?;
    writeln!(
        stdout,
        "  \"status\": \"{}\",",
        if report.has_errors() { "error" } else { "ok" }
    )?;
    writeln!(stdout, "  \"checks\": [")?;
    for (index, check) in report.checks.iter().enumerate() {
        if index > 0 {
            writeln!(stdout, ",")?;
        }
        write!(
            stdout,
            "    {{\"name\": \"{}\", \"severity\": \"{}\", \"detail\": \"{}\"}}",
            crate::op::json_escape(&check.name),
            json_severity_label(check.severity),
            crate::op::json_escape(&check.detail)
        )?;
    }
    writeln!(stdout)?;
    writeln!(stdout, "  ]")?;
    writeln!(stdout, "}}")
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

fn json_severity_label(severity: rit_core::DoctorSeverity) -> &'static str {
    match severity {
        rit_core::DoctorSeverity::Ok => "ok",
        rit_core::DoctorSeverity::Warning => "warning",
        rit_core::DoctorSeverity::Error => "error",
    }
}

fn json_optional_string(value: Option<&str>) -> String {
    value
        .map(|value| format!("\"{}\"", crate::op::json_escape(value)))
        .unwrap_or_else(|| "null".to_owned())
}

#[cfg(test)]
mod tests {
    use super::write_report_json;
    use rit_core::{DoctorCheck, DoctorReport, DoctorSeverity};

    #[test]
    fn doctor_json_escapes_paths_and_checks() {
        let report = DoctorReport {
            worktree: Some("repo\"root".to_owned()),
            git_dir: ".git".to_owned(),
            common_dir: ".git".to_owned(),
            bare: false,
            checks: vec![DoctorCheck {
                name: "head-object".to_owned(),
                severity: DoctorSeverity::Warning,
                detail: "HEAD is unborn\nnew branch".to_owned(),
            }],
        };
        let mut output = Vec::new();

        write_report_json(&report, &mut output).expect("json should be written");
        let text = String::from_utf8(output).expect("json should be utf-8");

        assert!(text.contains("\"worktree\": \"repo\\\"root\""));
        assert!(text.contains("\"severity\": \"warning\""));
        assert!(text.contains("HEAD is unborn\\nnew branch"));
    }
}
