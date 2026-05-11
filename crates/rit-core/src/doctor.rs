use crate::{GitConfig, ObjectKind, Repository, RitError};
use std::path::Path;

/// Severity of one `rit doctor` check.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DoctorSeverity {
    /// The checked item is healthy.
    Ok,
    /// The checked item may need attention but does not prove corruption.
    Warning,
    /// The checked item is invalid or unreadable.
    Error,
}

/// One repository health check result.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DoctorCheck {
    /// Stable short check name.
    pub name: String,
    /// Check severity.
    pub severity: DoctorSeverity,
    /// Human-readable detail.
    pub detail: String,
}

/// Structured `rit doctor` report.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DoctorReport {
    /// Worktree root for non-bare repositories.
    pub worktree: Option<String>,
    /// Repository `.git` directory.
    pub git_dir: String,
    /// Shared Git directory used by linked worktrees.
    pub common_dir: String,
    /// Whether the repository is bare.
    pub bare: bool,
    /// Individual check results.
    pub checks: Vec<DoctorCheck>,
}

impl DoctorReport {
    /// Returns true when the report contains one or more error checks.
    pub fn has_errors(&self) -> bool {
        self.checks
            .iter()
            .any(|check| check.severity == DoctorSeverity::Error)
    }
}

impl Repository {
    /// Runs read-only repository health checks.
    pub fn doctor(&self) -> DoctorReport {
        let mut report = DoctorReport {
            worktree: self.worktree().map(path_to_string),
            git_dir: path_to_string(self.git_dir()),
            common_dir: path_to_string(self.common_dir()),
            bare: self.is_bare(),
            checks: Vec::new(),
        };

        check_directory(&mut report, "git-dir", self.git_dir());
        check_directory(&mut report, "common-dir", self.common_dir());
        check_directory(
            &mut report,
            "objects-dir",
            &self.common_dir().join("objects"),
        );
        check_directory(
            &mut report,
            "pack-dir",
            &self.common_dir().join("objects").join("pack"),
        );
        check_file(&mut report, "head-file", &self.git_dir().join("HEAD"));
        check_git_config(&mut report, &self.common_dir().join("config"));
        check_rit_config(&mut report, self);
        check_head_object(&mut report, self);

        report
    }
}

fn check_directory(report: &mut DoctorReport, name: &str, path: &Path) {
    if path.is_dir() {
        ok(report, name, format!("{} exists", path.display()));
    } else {
        push_error(report, name, format!("{} is missing", path.display()));
    }
}

fn check_file(report: &mut DoctorReport, name: &str, path: &Path) {
    if path.is_file() {
        ok(report, name, format!("{} exists", path.display()));
    } else {
        push_error(report, name, format!("{} is missing", path.display()));
    }
}

fn check_git_config(report: &mut DoctorReport, config_path: &Path) {
    if !config_path.exists() {
        warning(report, "git-config", "Git config is missing".to_owned());
        return;
    }
    match GitConfig::read(config_path) {
        Ok(_) => ok(report, "git-config", "Git config is readable".to_owned()),
        Err(error) => push_error(report, "git-config", error.to_string()),
    }
}

fn check_rit_config(report: &mut DoctorReport, repository: &Repository) {
    if repository.worktree().is_none() {
        warning(
            report,
            "rit-config",
            "rit config is skipped for bare repositories".to_owned(),
        );
        return;
    }
    match repository.rit_config() {
        Ok(_) => ok(report, "rit-config", "rit config is readable".to_owned()),
        Err(error) => push_error(report, "rit-config", error.to_string()),
    }
}

fn check_head_object(report: &mut DoctorReport, repository: &Repository) {
    match repository.resolve_head() {
        Ok(Some(object_id)) => match repository.read_object(object_id) {
            Ok(object) if object.kind == ObjectKind::Commit => {
                ok(
                    report,
                    "head-object",
                    format!("HEAD points to commit {object_id}"),
                );
            }
            Ok(object) => warning(
                report,
                "head-object",
                format!("HEAD points to {}, not commit", object.kind),
            ),
            Err(error) => push_error(report, "head-object", error.to_string()),
        },
        Ok(None) => warning(
            report,
            "head-object",
            "HEAD points to an unborn branch".to_owned(),
        ),
        Err(RitError::InvalidInput { message }) if message.contains("object id") => {
            push_error(
                report,
                "head-object",
                format!("invalid HEAD target: {message}"),
            );
        }
        Err(error) => push_error(report, "head-object", error.to_string()),
    }
}

fn ok(report: &mut DoctorReport, name: &str, detail: String) {
    push_check(report, name, DoctorSeverity::Ok, detail);
}

fn warning(report: &mut DoctorReport, name: &str, detail: String) {
    push_check(report, name, DoctorSeverity::Warning, detail);
}

fn push_error(report: &mut DoctorReport, name: &str, detail: String) {
    push_check(report, name, DoctorSeverity::Error, detail);
}

fn push_check(report: &mut DoctorReport, name: &str, severity: DoctorSeverity, detail: String) {
    report.checks.push(DoctorCheck {
        name: name.to_owned(),
        severity,
        detail,
    });
}

fn path_to_string(path: &Path) -> String {
    path.display().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::InitOptions;
    use std::fs;

    #[test]
    fn doctor_reports_new_repository_without_errors() {
        let root = temp_path("doctor-ok");
        let repository = Repository::init(&InitOptions::new(&root)).expect("repo should init");

        let report = repository.doctor();

        assert!(!report.has_errors());
        assert!(report.checks.iter().any(|check| check.name == "git-dir"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn doctor_reports_invalid_head_as_error() {
        let root = temp_path("doctor-bad-head");
        let repository = Repository::init(&InitOptions::new(&root)).expect("repo should init");
        fs::write(repository.git_dir().join("HEAD"), "not an object id\n")
            .expect("HEAD should be writable");

        let report = repository.doctor();

        assert!(report.has_errors());
        assert!(report.checks.iter().any(|check| {
            check.name == "head-object" && check.severity == DoctorSeverity::Error
        }));
        let _ = fs::remove_dir_all(root);
    }

    fn temp_path(name: &str) -> std::path::PathBuf {
        let suffix = std::process::id();
        let path = std::env::temp_dir().join(format!("rit-{name}-{suffix}"));
        let _ = fs::remove_dir_all(&path);
        path
    }
}
