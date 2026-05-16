use std::io::{self, Write};
use std::process::ExitCode;

pub fn impact_command(
    args: &[String],
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> io::Result<ExitCode> {
    let range = match args {
        [range] if !range.starts_with('-') => range,
        [] => {
            writeln!(stderr, "rit: impact requires a range")?;
            return Ok(ExitCode::from(129));
        }
        [unsupported, ..] => {
            writeln!(stderr, "rit: unsupported impact option '{unsupported}'")?;
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
    match repository.impact_report(range) {
        Ok(report) => {
            print_impact_report(stdout, &report)?;
            Ok(ExitCode::SUCCESS)
        }
        Err(error) => {
            writeln!(stderr, "rit: {error}")?;
            Ok(ExitCode::from(1))
        }
    }
}

fn print_impact_report(stdout: &mut dyn Write, report: &rit_core::ImpactReport) -> io::Result<()> {
    writeln!(stdout, "impact: {}", report.range)?;
    writeln!(stdout, "base: {}", report.base)?;
    writeln!(stdout, "target: {}", report.target)?;
    writeln!(stdout, "docs-only: {}", report.docs_only)?;
    writeln!(
        stdout,
        "indexdb-acceleration: {}",
        report.indexdb_acceleration_available
    )?;
    for path in &report.changed_paths {
        writeln!(stdout, "changed: {path}")?;
    }
    for package in &report.changed_packages {
        writeln!(stdout, "package: {package}")?;
    }
    for test in &report.affected_tests {
        writeln!(stdout, "affected-test: {test}")?;
    }
    for path in &report.public_api_changes {
        writeln!(stdout, "public-api: {path}")?;
    }
    for large_file in &report.large_file_changes {
        writeln!(
            stdout,
            "large-file: {} {}",
            large_file.path, large_file.size
        )?;
    }
    for file in &report.semantic.files {
        writeln!(stdout, "semantic: {} {:?}", file.path, file.category)?;
    }
    for hint in &report.reviewer_hints {
        writeln!(
            stdout,
            "reviewer-hint: {} {} {}",
            hint.kind, hint.path, hint.detail
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::impact_command;
    use std::process::ExitCode;

    #[test]
    fn impact_requires_range() {
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        let code = impact_command(&[], &mut stdout, &mut stderr).expect("command should write");

        assert_eq!(code, ExitCode::from(129));
        assert!(stdout.is_empty());
        assert!(
            String::from_utf8(stderr)
                .expect("stderr should be utf-8")
                .contains("impact requires a range")
        );
    }
}
