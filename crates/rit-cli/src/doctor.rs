use std::io::{self, Write};
use std::process::ExitCode;

pub fn doctor_command(
    args: &[String],
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> io::Result<ExitCode> {
    let mode = match args {
        [] => DoctorOutputMode::Text,
        [flag] if flag == "--json" => DoctorOutputMode::Json,
        [flag] if flag == "--explain" => DoctorOutputMode::Explain,
        [flag] if flag == "--fix-plan" => DoctorOutputMode::FixPlan,
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
    match mode {
        DoctorOutputMode::Text => print_report(&report, stdout)?,
        DoctorOutputMode::Json => write_report_json(&report, stdout)?,
        DoctorOutputMode::Explain => print_explained_report(&report, stdout)?,
        DoctorOutputMode::FixPlan => print_fix_plan_report(&report, &repository, stdout)?,
    }

    if report.has_errors() {
        Ok(ExitCode::from(1))
    } else {
        Ok(ExitCode::SUCCESS)
    }
}

enum DoctorOutputMode {
    Text,
    Json,
    Explain,
    FixPlan,
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

fn print_explained_report(
    report: &rit_core::DoctorReport,
    stdout: &mut dyn Write,
) -> io::Result<()> {
    print_report(report, stdout)?;
    writeln!(stdout, "explain:")?;
    for check in &report.checks {
        writeln!(
            stdout,
            "- {}: {}",
            check.name,
            doctor_check_explanation(check)
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

fn print_fix_plan_report(
    report: &rit_core::DoctorReport,
    repository: &rit_core::Repository,
    stdout: &mut dyn Write,
) -> io::Result<()> {
    print_report(report, stdout)?;
    let plan = repository.repair_plan();
    if plan.is_empty() {
        writeln!(stdout, "fix-plan: nothing to do")?;
        return Ok(());
    }

    writeln!(stdout, "fix-plan: dry-run")?;
    for action in plan.actions {
        writeln!(stdout, "would: {}", action.description())?;
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

fn doctor_check_explanation(check: &rit_core::DoctorCheck) -> &'static str {
    match check.name.as_str() {
        "git-dir" => "checks that the worktree points at a readable Git directory",
        "common-dir" => "checks the shared Git directory used for objects, refs, and config",
        "objects-dir" => "checks that loose objects can be stored under the common directory",
        "pack-dir" => "checks that packfiles have a standard storage directory",
        "head-file" => "checks that this worktree has a HEAD file to resolve",
        "git-config" => "checks whether .git/config exists and can be parsed",
        "rit-config" => "checks optional rit.toml or .rit.toml configuration parsing",
        "head-object" => "checks that HEAD resolves to an existing commit or an unborn branch",
        "loose-objects" => "counts loose objects that may benefit from routine packing",
        "pack-index-state" => "checks that pack files and pack indexes are paired",
        "commit-graph" => "checks whether a commit graph is available for faster history walks",
        "rit-metadata" => {
            "checks rit sidecar metadata without treating it as Git's source of truth"
        }
        _ => "checks a repository health rule and reports its current result",
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
    use super::{print_explained_report, print_fix_plan_report, write_report_json};
    use rit_core::{DoctorCheck, DoctorReport, DoctorSeverity, InitOptions, Repository};
    use std::fs;

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

    #[test]
    fn doctor_explain_prints_check_reasons() {
        let report = DoctorReport {
            worktree: Some("repo".to_owned()),
            git_dir: ".git".to_owned(),
            common_dir: ".git".to_owned(),
            bare: false,
            checks: vec![DoctorCheck {
                name: "git-config".to_owned(),
                severity: DoctorSeverity::Ok,
                detail: "Git config is readable".to_owned(),
            }],
        };
        let mut output = Vec::new();

        print_explained_report(&report, &mut output).expect("explain should be written");
        let text = String::from_utf8(output).expect("explain should be utf-8");

        assert!(text.contains("explain:\n"));
        assert!(text.contains("- git-config: checks whether .git/config exists and can be parsed"));
    }

    #[test]
    fn doctor_fix_plan_prints_repair_actions_without_applying() {
        let root = temp_path("doctor-fix-plan");
        let repository = Repository::init(&InitOptions::new(&root)).expect("repo should init");
        let pack_dir = repository.common_dir().join("objects").join("pack");
        fs::remove_dir_all(&pack_dir).expect("pack dir should be removable");
        let report = repository.doctor();
        let mut output = Vec::new();

        print_fix_plan_report(&report, &repository, &mut output)
            .expect("fix plan should be written");
        let text = String::from_utf8(output).expect("fix plan should be utf-8");

        assert!(text.contains("fix-plan: dry-run"));
        assert!(text.contains("would: create directory"));
        assert!(!pack_dir.exists());
        let _ = fs::remove_dir_all(root);
    }

    fn temp_path(name: &str) -> std::path::PathBuf {
        let suffix = std::process::id();
        let path = std::env::temp_dir().join(format!("rit-cli-{name}-{suffix}"));
        let _ = fs::remove_dir_all(&path);
        path
    }
}
