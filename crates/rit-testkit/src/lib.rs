//! Compatibility testing helpers for comparing Git and `rit`.
//!
//! This crate is allowed to execute the system `git` binary because it is test
//! infrastructure. Production `rit` command implementations must not depend on
//! this crate and must not shell out to Git.

use rit_core::object::sha1_bytes;
use std::collections::BTreeMap;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus};
use std::time::{SystemTime, UNIX_EPOCH};

/// Result type used by `rit-testkit`.
pub type Result<T> = std::result::Result<T, TestkitError>;

/// A recoverable compatibility harness error.
#[derive(Debug)]
pub enum TestkitError {
    /// A command specification was empty.
    EmptyCommand { role: &'static str },
    /// The fixture repository path does not exist.
    MissingFixture { path: PathBuf },
    /// A file-system operation failed.
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    /// A child process failed to start.
    Spawn {
        program: OsString,
        source: std::io::Error,
    },
    /// System time could not be converted to a monotonic-ish suffix.
    Clock(std::time::SystemTimeError),
}

impl std::fmt::Display for TestkitError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyCommand { role } => write!(formatter, "{role} command is empty"),
            Self::MissingFixture { path } => {
                write!(
                    formatter,
                    "fixture repository does not exist: {}",
                    path.display()
                )
            }
            Self::Io { path, source } => {
                write!(
                    formatter,
                    "I/O error while accessing {}: {source}",
                    path.display()
                )
            }
            Self::Spawn { program, source } => {
                write!(
                    formatter,
                    "failed to run command {}: {source}",
                    program.to_string_lossy()
                )
            }
            Self::Clock(source) => write!(formatter, "system clock error: {source}"),
        }
    }
}

impl std::error::Error for TestkitError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } | Self::Spawn { source, .. } => Some(source),
            Self::Clock(source) => Some(source),
            _ => None,
        }
    }
}

/// A command with program and arguments.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommandSpec {
    /// Program to execute.
    pub program: OsString,
    /// Arguments passed to the program.
    pub args: Vec<OsString>,
}

impl CommandSpec {
    /// Creates a command specification.
    pub fn new(program: impl Into<OsString>, args: Vec<OsString>) -> Self {
        Self {
            program: program.into(),
            args,
        }
    }

    /// Creates a command specification from a non-empty vector.
    pub fn from_words(role: &'static str, words: Vec<OsString>) -> Result<Self> {
        let mut words = words.into_iter();
        let Some(program) = words.next() else {
            return Err(TestkitError::EmptyCommand { role });
        };
        Ok(Self::new(program, words.collect()))
    }
}

/// Options for one compatibility comparison.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompareOptions {
    /// Fixture repository copied for each side of the comparison.
    pub fixture: PathBuf,
    /// Reference Git command.
    pub git: CommandSpec,
    /// Candidate rit command.
    pub rit: CommandSpec,
    /// Compare repository snapshots after commands finish.
    pub compare_repository_state: bool,
}

impl CompareOptions {
    /// Builds options with repository-state comparison enabled.
    pub fn new(fixture: impl Into<PathBuf>, git: CommandSpec, rit: CommandSpec) -> Self {
        Self {
            fixture: fixture.into(),
            git,
            rit,
            compare_repository_state: true,
        }
    }
}

/// Captured command output.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommandOutput {
    /// Process exit code, or `None` when the platform did not provide one.
    pub exit_code: Option<i32>,
    /// Captured stdout bytes decoded lossily as UTF-8.
    pub stdout: String,
    /// Captured stderr bytes decoded lossily as UTF-8.
    pub stderr: String,
}

impl CommandOutput {
    fn from_process_output(status: ExitStatus, stdout: Vec<u8>, stderr: Vec<u8>) -> Self {
        Self {
            exit_code: status.code(),
            stdout: String::from_utf8_lossy(&stdout).into_owned(),
            stderr: String::from_utf8_lossy(&stderr).into_owned(),
        }
    }
}

/// Final comparison result.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompareOutcome {
    /// Captured Git output.
    pub git: CommandOutput,
    /// Captured rit output.
    pub rit: CommandOutput,
    /// Repository snapshot comparison, if requested.
    pub repository_state: Option<StateComparison>,
}

impl CompareOutcome {
    /// Returns true when outputs and requested repository snapshots match.
    pub fn is_match(&self) -> bool {
        self.git == self.rit
            && self
                .repository_state
                .as_ref()
                .is_none_or(StateComparison::is_match)
    }

    /// Returns a compact human-readable report.
    pub fn report(&self) -> String {
        let mut report = String::new();
        push_check(
            &mut report,
            "exit code",
            self.git.exit_code == self.rit.exit_code,
        );
        push_check(&mut report, "stdout", self.git.stdout == self.rit.stdout);
        push_check(&mut report, "stderr", self.git.stderr == self.rit.stderr);
        if self.git.stdout != self.rit.stdout {
            append_first_text_difference(&mut report, "stdout", &self.git.stdout, &self.rit.stdout);
        }
        if self.git.stderr != self.rit.stderr {
            append_first_text_difference(&mut report, "stderr", &self.git.stderr, &self.rit.stderr);
        }
        if let Some(repository_state) = &self.repository_state {
            push_check(&mut report, "repository state", repository_state.is_match());
            if !repository_state.is_match() {
                report.push_str(&repository_state.report());
            }
        }
        report
    }
}

/// Snapshot comparison for copied repositories.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StateComparison {
    /// Files that differ between the two final repository snapshots.
    pub differing_paths: Vec<PathBuf>,
    /// Files present only on the Git side.
    pub only_git_paths: Vec<PathBuf>,
    /// Files present only on the rit side.
    pub only_rit_paths: Vec<PathBuf>,
}

impl StateComparison {
    /// Returns true when both repository snapshots are identical.
    pub fn is_match(&self) -> bool {
        self.differing_paths.is_empty()
            && self.only_git_paths.is_empty()
            && self.only_rit_paths.is_empty()
    }

    fn report(&self) -> String {
        let mut report = String::new();
        append_paths(&mut report, "different", &self.differing_paths);
        append_paths(&mut report, "only git", &self.only_git_paths);
        append_paths(&mut report, "only rit", &self.only_rit_paths);
        report
    }
}

/// Runs a compatibility comparison.
pub fn compare(options: &CompareOptions) -> Result<CompareOutcome> {
    if !options.fixture.exists() {
        return Err(TestkitError::MissingFixture {
            path: options.fixture.clone(),
        });
    }

    let workspace = TemporaryWorkspace::new("rit-testkit")?;
    let git_repo = workspace.path.join("git");
    let rit_repo = workspace.path.join("rit");
    copy_directory(&options.fixture, &git_repo)?;
    copy_directory(&options.fixture, &rit_repo)?;

    let git = run_command(&options.git, &git_repo)?;
    let rit = run_command(&options.rit, &rit_repo)?;
    let repository_state = if options.compare_repository_state {
        Some(compare_state(&git_repo, &rit_repo)?)
    } else {
        None
    };

    Ok(CompareOutcome {
        git,
        rit,
        repository_state,
    })
}

fn run_command(spec: &CommandSpec, cwd: &Path) -> Result<CommandOutput> {
    let output = Command::new(&spec.program)
        .args(&spec.args)
        .current_dir(cwd)
        .output()
        .map_err(|source| TestkitError::Spawn {
            program: spec.program.clone(),
            source,
        })?;
    Ok(CommandOutput::from_process_output(
        output.status,
        output.stdout,
        output.stderr,
    ))
}

fn compare_state(git_repo: &Path, rit_repo: &Path) -> Result<StateComparison> {
    let git_snapshot = snapshot_directory(git_repo)?;
    let rit_snapshot = snapshot_directory(rit_repo)?;
    let mut differing_paths = Vec::new();
    let mut only_git_paths = Vec::new();
    let mut only_rit_paths = Vec::new();

    for (path, git_file) in &git_snapshot {
        match rit_snapshot.get(path) {
            Some(rit_file) if rit_file != git_file => differing_paths.push(path.clone()),
            None => only_git_paths.push(path.clone()),
            _ => {}
        }
    }

    for path in rit_snapshot.keys() {
        if !git_snapshot.contains_key(path) {
            only_rit_paths.push(path.clone());
        }
    }

    Ok(StateComparison {
        differing_paths,
        only_git_paths,
        only_rit_paths,
    })
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct FileFingerprint {
    len: u64,
    sha1_hex: String,
}

fn snapshot_directory(root: &Path) -> Result<BTreeMap<PathBuf, FileFingerprint>> {
    let mut files = BTreeMap::new();
    snapshot_directory_inner(root, root, &mut files)?;
    Ok(files)
}

fn snapshot_directory_inner(
    root: &Path,
    current: &Path,
    files: &mut BTreeMap<PathBuf, FileFingerprint>,
) -> Result<()> {
    for entry in fs::read_dir(current).map_err(|source| TestkitError::Io {
        path: current.to_path_buf(),
        source,
    })? {
        let entry = entry.map_err(|source| TestkitError::Io {
            path: current.to_path_buf(),
            source,
        })?;
        let path = entry.path();
        let name = entry.file_name();
        if should_skip_snapshot_path(&name) {
            continue;
        }
        let metadata = entry.metadata().map_err(|source| TestkitError::Io {
            path: path.clone(),
            source,
        })?;
        if metadata.is_dir() {
            snapshot_directory_inner(root, &path, files)?;
        } else if metadata.is_file() {
            let bytes = fs::read(&path).map_err(|source| TestkitError::Io {
                path: path.clone(),
                source,
            })?;
            let relative = path.strip_prefix(root).unwrap_or(&path).to_path_buf();
            files.insert(
                relative,
                FileFingerprint {
                    len: metadata.len(),
                    sha1_hex: hex(&sha1_bytes(&bytes)),
                },
            );
        }
    }
    Ok(())
}

fn should_skip_snapshot_path(name: &OsStr) -> bool {
    let name = name.to_string_lossy();
    name.ends_with(".lock")
}

fn copy_directory(from: &Path, to: &Path) -> Result<()> {
    fs::create_dir_all(to).map_err(|source| TestkitError::Io {
        path: to.to_path_buf(),
        source,
    })?;

    for entry in fs::read_dir(from).map_err(|source| TestkitError::Io {
        path: from.to_path_buf(),
        source,
    })? {
        let entry = entry.map_err(|source| TestkitError::Io {
            path: from.to_path_buf(),
            source,
        })?;
        let source_path = entry.path();
        let target_path = to.join(entry.file_name());
        let metadata = entry.metadata().map_err(|source| TestkitError::Io {
            path: source_path.clone(),
            source,
        })?;
        if metadata.is_dir() {
            copy_directory(&source_path, &target_path)?;
        } else if metadata.is_file() {
            fs::copy(&source_path, &target_path).map_err(|source| TestkitError::Io {
                path: target_path,
                source,
            })?;
        }
    }

    Ok(())
}

struct TemporaryWorkspace {
    path: PathBuf,
}

impl TemporaryWorkspace {
    fn new(prefix: &str) -> Result<Self> {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(TestkitError::Clock)?
            .as_nanos();
        let path = std::env::temp_dir().join(format!("{prefix}-{unique}"));
        fs::create_dir_all(&path).map_err(|source| TestkitError::Io {
            path: path.clone(),
            source,
        })?;
        Ok(Self { path })
    }
}

impl Drop for TemporaryWorkspace {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn append_paths(report: &mut String, label: &str, paths: &[PathBuf]) {
    if paths.is_empty() {
        return;
    }
    report.push_str(label);
    report.push_str(" paths:\n");
    for path in paths.iter().take(10) {
        report.push_str("  ");
        report.push_str(&path.display().to_string());
        report.push('\n');
    }
}

fn push_check(report: &mut String, name: &str, matched: bool) {
    report.push_str(name);
    report.push_str(": ");
    report.push_str(if matched { "same\n" } else { "different\n" });
}

fn append_first_text_difference(report: &mut String, label: &str, git: &str, rit: &str) {
    let git_lines = git.lines().collect::<Vec<_>>();
    let rit_lines = rit.lines().collect::<Vec<_>>();
    let max_len = git_lines.len().max(rit_lines.len());

    for index in 0..max_len {
        let git_line = git_lines.get(index).copied();
        let rit_line = rit_lines.get(index).copied();
        if git_line == rit_line {
            continue;
        }
        report.push_str("first ");
        report.push_str(label);
        report.push_str(" difference at line ");
        report.push_str(&(index + 1).to_string());
        report.push_str(":\n  git: ");
        report.push_str(git_line.unwrap_or("<missing>"));
        report.push_str("\n  rit: ");
        report.push_str(rit_line.unwrap_or("<missing>"));
        report.push('\n');
        return;
    }

    if git != rit {
        report.push_str("first ");
        report.push_str(label);
        report.push_str(" difference: text differs without line-level difference\n");
    }
}

fn hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }
    output
}

#[cfg(test)]
mod tests {
    use super::{CommandOutput, CommandSpec, CompareOptions, CompareOutcome, compare};
    use std::ffi::OsString;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn identical_commands_match_on_empty_fixture() {
        let fixture = temp_path("fixture");
        fs::create_dir_all(&fixture).expect("fixture should be created");

        let command = CommandSpec::new(git_program(), vec![OsString::from("--version")]);
        let options = CompareOptions::new(&fixture, command.clone(), command);

        let outcome = compare(&options).expect("comparison should run");

        assert!(outcome.is_match(), "{}", outcome.report());
        remove_dir_all(&fixture);
    }

    #[test]
    fn report_includes_first_stdout_difference() {
        let outcome = CompareOutcome {
            git: CommandOutput {
                exit_code: Some(0),
                stdout: "same\nleft\n".to_owned(),
                stderr: String::new(),
            },
            rit: CommandOutput {
                exit_code: Some(0),
                stdout: "same\nright\n".to_owned(),
                stderr: String::new(),
            },
            repository_state: None,
        };

        let report = outcome.report();

        assert!(report.contains("stdout: different"));
        assert!(report.contains("first stdout difference at line 2"));
        assert!(report.contains("git: left"));
        assert!(report.contains("rit: right"));
    }

    fn git_program() -> OsString {
        OsString::from("git")
    }

    fn temp_path(name: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("rit-testkit-{name}-{unique}"))
    }

    fn remove_dir_all(path: &Path) {
        if path.exists() {
            fs::remove_dir_all(path).expect("temporary directory should be removed");
        }
    }
}
