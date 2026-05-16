use crate::{GitConfig, ObjectKind, Repository, RitError};
use std::fs;
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
        check_loose_objects(&mut report, &self.common_dir().join("objects"));
        check_pack_index_state(&mut report, &self.common_dir().join("objects").join("pack"));
        check_commit_graph(&mut report, &self.common_dir().join("objects").join("info"));
        check_rit_metadata(&mut report, self);
        #[cfg(feature = "indexdb")]
        check_indexdb(&mut report, self);

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

fn check_loose_objects(report: &mut DoctorReport, objects_dir: &Path) {
    match count_loose_objects(objects_dir) {
        Ok(count) if count > 1_000 => warning(
            report,
            "loose-objects",
            format!("{count} loose objects; consider packing during maintenance"),
        ),
        Ok(count) => ok(report, "loose-objects", format!("{count} loose objects")),
        Err(error) => warning(
            report,
            "loose-objects",
            format!("could not count loose objects: {error}"),
        ),
    }
}

fn check_pack_index_state(report: &mut DoctorReport, pack_dir: &Path) {
    match count_pack_files(pack_dir) {
        Ok((packs, indexes)) if packs == indexes => ok(
            report,
            "pack-index-state",
            format!("{packs} pack files and {indexes} pack indexes"),
        ),
        Ok((packs, indexes)) => warning(
            report,
            "pack-index-state",
            format!("{packs} pack files but {indexes} pack indexes"),
        ),
        Err(error) => warning(
            report,
            "pack-index-state",
            format!("could not inspect pack directory: {error}"),
        ),
    }
}

fn check_commit_graph(report: &mut DoctorReport, objects_info_dir: &Path) {
    let commit_graph = objects_info_dir.join("commit-graph");
    if commit_graph.is_file() {
        ok(
            report,
            "commit-graph",
            format!("{} exists", commit_graph.display()),
        );
    } else {
        warning(
            report,
            "commit-graph",
            "commit graph is not present; history walks use object traversal".to_owned(),
        );
    }
}

fn check_rit_metadata(report: &mut DoctorReport, repository: &Repository) {
    let rit_dir = repository.git_dir().join("rit");
    if !rit_dir.exists() {
        ok(
            report,
            "rit-metadata",
            "no rit metadata directory is present".to_owned(),
        );
        return;
    }
    match repository.operations().log_with_warnings() {
        Ok(log) if log.warnings.is_empty() => ok(
            report,
            "rit-metadata",
            "rit operation journal is readable".to_owned(),
        ),
        Ok(log) => warning(
            report,
            "rit-metadata",
            format!(
                "rit operation journal has {} malformed line(s)",
                log.warnings.len()
            ),
        ),
        Err(error) => warning(
            report,
            "rit-metadata",
            format!("could not inspect rit metadata: {error}"),
        ),
    }
}

#[cfg(feature = "indexdb")]
fn check_indexdb(report: &mut DoctorReport, repository: &Repository) {
    match repository.indexdb().status() {
        Ok(status) if !status.exists => ok(
            report,
            "indexdb-state",
            format!(
                "indexdb is not built at {}; optional acceleration is disabled",
                status.storage.database_path.display()
            ),
        ),
        Ok(status) if status.healthy && !status.stale => ok(
            report,
            "indexdb-state",
            format!(
                "indexdb schema version {} is healthy and fresh",
                status.schema_version.unwrap_or_default()
            ),
        ),
        Ok(status) => warning(
            report,
            "indexdb-state",
            format!(
                "indexdb needs attention at {}: {}",
                status.storage.database_path.display(),
                status.stale_reasons.join("; ")
            ),
        ),
        Err(error) => warning(
            report,
            "indexdb-state",
            format!("could not inspect indexdb: {error}"),
        ),
    }
}

fn count_loose_objects(objects_dir: &Path) -> std::io::Result<usize> {
    let mut count = 0;
    for entry in fs::read_dir(objects_dir)? {
        let entry = entry?;
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if !path.is_dir()
            || name.len() != 2
            || !name.chars().all(|character| character.is_ascii_hexdigit())
        {
            continue;
        }
        for object_entry in fs::read_dir(path)? {
            let object_entry = object_entry?;
            if object_entry.file_type()?.is_file() {
                count += 1;
            }
        }
    }
    Ok(count)
}

fn count_pack_files(pack_dir: &Path) -> std::io::Result<(usize, usize)> {
    let mut packs = 0;
    let mut indexes = 0;
    for entry in fs::read_dir(pack_dir)? {
        let entry = entry?;
        let path = entry.path();
        match path.extension().and_then(|extension| extension.to_str()) {
            Some("pack") => packs += 1,
            Some("idx") => indexes += 1,
            _ => {}
        }
    }
    Ok((packs, indexes))
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
        assert!(
            report
                .checks
                .iter()
                .any(|check| check.name == "loose-objects")
        );
        assert!(
            report
                .checks
                .iter()
                .any(|check| check.name == "pack-index-state")
        );
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

    #[cfg(feature = "indexdb")]
    #[test]
    fn doctor_reports_optional_missing_indexdb() {
        let root = temp_path("doctor-indexdb-missing");
        let repository = Repository::init(&InitOptions::new(&root)).expect("repo should init");

        let report = repository.doctor();

        assert!(report.checks.iter().any(|check| {
            check.name == "indexdb-state"
                && check.severity == DoctorSeverity::Ok
                && check.detail.contains("optional acceleration is disabled")
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
