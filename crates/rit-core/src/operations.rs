use crate::{ObjectId, Repository, Result, RitError, object::sha1_bytes};
use std::fs;
use std::io::Write;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

/// A lightweight snapshot of repository state before or after one user action.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct OperationSnapshot {
    /// Commit currently reachable from HEAD, if HEAD is not unborn.
    pub head: Option<ObjectId>,
    /// Current branch name when HEAD is symbolic under `refs/heads`.
    pub branch: Option<String>,
    /// SHA-1 checksum of the raw index file, when an index exists.
    pub index_checksum: Option<String>,
}

/// One operation journal record stored under `.git/rit/ops.log`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OperationRecord {
    /// Stable operation identifier.
    pub id: String,
    /// Short command name, such as `commit`, `checkout`, or `merge`.
    pub command: String,
    /// Human-readable summary supplied by the caller.
    pub summary: String,
    /// Repository state captured before the operation.
    pub before: OperationSnapshot,
    /// Repository state captured after the operation.
    pub after: OperationSnapshot,
}

/// Result of restoring a journal entry.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OperationRestoreResult {
    /// Restored operation ID.
    pub id: String,
    /// Commit restored into HEAD and the working tree.
    pub restored_head: ObjectId,
}

/// Entry point for operation-journal APIs.
pub struct RepositoryOperations<'a> {
    repository: &'a Repository,
}

impl Repository {
    /// Returns the operation journal for this repository.
    pub fn operations(&self) -> RepositoryOperations<'_> {
        RepositoryOperations { repository: self }
    }
}

impl RepositoryOperations<'_> {
    /// Captures the current HEAD, branch, and index checksum.
    pub fn snapshot(&self) -> Result<OperationSnapshot> {
        Ok(OperationSnapshot {
            head: self.repository.resolve_head()?,
            branch: self.repository.current_branch_name()?,
            index_checksum: index_checksum(self.repository.git_dir().join("index").as_path())?,
        })
    }

    /// Appends one operation record to `.git/rit/ops.log`.
    pub fn record(
        &self,
        command: &str,
        summary: &str,
        before: OperationSnapshot,
        after: OperationSnapshot,
    ) -> Result<OperationRecord> {
        let record = OperationRecord {
            id: next_operation_id(),
            command: command.to_owned(),
            summary: summary.to_owned(),
            before,
            after,
        };
        append_record(self.repository, &record)?;
        Ok(record)
    }

    /// Reads operation records in append order.
    pub fn log(&self) -> Result<Vec<OperationRecord>> {
        let path = log_path(self.repository);
        if !path.exists() {
            return Ok(Vec::new());
        }
        let contents = fs::read_to_string(&path).map_err(|source| RitError::io(&path, source))?;
        contents
            .lines()
            .filter(|line| !line.trim().is_empty())
            .map(parse_record_line)
            .collect()
    }

    /// Restores the state captured before the operation with `id`.
    pub fn restore(&self, id: &str) -> Result<OperationRestoreResult> {
        let record = self
            .log()?
            .into_iter()
            .find(|record| record.id == id)
            .ok_or_else(|| RitError::invalid_input(format!("operation not found: {id}")))?;
        restore_snapshot(self.repository, &record.before)?;
        Ok(OperationRestoreResult {
            id: record.id,
            restored_head: record
                .before
                .head
                .expect("restore_snapshot requires a before HEAD"),
        })
    }

    /// Restores the state captured before the last operation in the journal.
    pub fn undo_last(&self) -> Result<OperationRestoreResult> {
        let last = self
            .log()?
            .into_iter()
            .last()
            .ok_or_else(|| RitError::invalid_input("operation journal is empty"))?;
        self.restore(&last.id)
    }
}

fn restore_snapshot(repository: &Repository, snapshot: &OperationSnapshot) -> Result<()> {
    let head = snapshot
        .head
        .ok_or_else(|| RitError::invalid_input("operation has no restorable HEAD"))?;
    repository.checkout_commit_tree(head)?;
    if let Some(branch) = &snapshot.branch {
        let ref_path = repository
            .common_dir()
            .join("refs")
            .join("heads")
            .join(branch);
        write_text_atomically(&ref_path, &format!("{head}\n"))?;
        write_text_atomically(
            &repository.git_dir().join("HEAD"),
            &format!("ref: refs/heads/{branch}\n"),
        )?;
    } else {
        write_text_atomically(&repository.git_dir().join("HEAD"), &format!("{head}\n"))?;
    }
    Ok(())
}

fn append_record(repository: &Repository, record: &OperationRecord) -> Result<()> {
    let path = log_path(repository);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| RitError::io(parent, source))?;
    }
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|source| RitError::io(&path, source))?;
    writeln!(file, "{}", format_record_line(record)).map_err(|source| RitError::io(&path, source))
}

fn log_path(repository: &Repository) -> std::path::PathBuf {
    repository.git_dir().join("rit").join("ops.log")
}

fn index_checksum(path: &Path) -> Result<Option<String>> {
    if !path.exists() {
        return Ok(None);
    }
    let bytes = fs::read(path).map_err(|source| RitError::io(path, source))?;
    Ok(Some(hex(&sha1_bytes(&bytes))))
}

fn next_operation_id() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    format!("op-{nanos}")
}

fn format_record_line(record: &OperationRecord) -> String {
    [
        escape(&record.id),
        escape(&record.command),
        escape(&record.summary),
        format_snapshot(&record.before),
        format_snapshot(&record.after),
    ]
    .join("\t")
}

fn parse_record_line(line: &str) -> Result<OperationRecord> {
    let fields = split_escaped_tabs(line);
    if fields.len() != 5 {
        return Err(RitError::invalid_input(format!(
            "operation journal line has {} fields, expected 5",
            fields.len()
        )));
    }
    Ok(OperationRecord {
        id: unescape(&fields[0])?,
        command: unescape(&fields[1])?,
        summary: unescape(&fields[2])?,
        before: parse_snapshot(&fields[3])?,
        after: parse_snapshot(&fields[4])?,
    })
}

fn format_snapshot(snapshot: &OperationSnapshot) -> String {
    format!(
        "head={};branch={};index={}",
        snapshot
            .head
            .map(|head| head.to_hex())
            .unwrap_or_else(|| "-".to_owned()),
        snapshot
            .branch
            .as_ref()
            .map(|branch| hex(branch.as_bytes()))
            .unwrap_or_else(|| "-".to_owned()),
        snapshot.index_checksum.as_deref().unwrap_or("-")
    )
}

fn parse_snapshot(input: &str) -> Result<OperationSnapshot> {
    let mut head = None;
    let mut branch = None;
    let mut index_checksum = None;
    for part in input.split(';') {
        let Some((key, value)) = part.split_once('=') else {
            return Err(RitError::invalid_input(format!(
                "operation snapshot field is malformed: {part}"
            )));
        };
        match key {
            "head" if value != "-" => head = Some(ObjectId::from_hex(value)?),
            "head" => {}
            "branch" if value != "-" => branch = Some(unhex_utf8(value)?),
            "branch" => {}
            "index" if value != "-" => index_checksum = Some(value.to_owned()),
            "index" => {}
            _ => {
                return Err(RitError::invalid_input(format!(
                    "unknown operation snapshot field: {key}"
                )));
            }
        }
    }
    Ok(OperationSnapshot {
        head,
        branch,
        index_checksum,
    })
}

fn split_escaped_tabs(line: &str) -> Vec<String> {
    let mut fields = Vec::new();
    let mut current = String::new();
    let mut escaped = false;
    for character in line.chars() {
        if escaped {
            current.push('\\');
            current.push(character);
            escaped = false;
        } else if character == '\\' {
            escaped = true;
        } else if character == '\t' {
            fields.push(current);
            current = String::new();
        } else {
            current.push(character);
        }
    }
    if escaped {
        current.push('\\');
    }
    fields.push(current);
    fields
}

fn escape(input: &str) -> String {
    input
        .replace('\\', "\\\\")
        .replace('\t', "\\t")
        .replace('\n', "\\n")
}

fn unescape(input: &str) -> Result<String> {
    let mut output = String::new();
    let mut chars = input.chars();
    while let Some(character) = chars.next() {
        if character != '\\' {
            output.push(character);
            continue;
        }
        match chars.next() {
            Some('\\') => output.push('\\'),
            Some('t') => output.push('\t'),
            Some('n') => output.push('\n'),
            Some(other) => {
                return Err(RitError::invalid_input(format!(
                    "unknown escape in operation journal: \\{other}"
                )));
            }
            None => {
                return Err(RitError::invalid_input(
                    "trailing escape in operation journal",
                ));
            }
        }
    }
    Ok(output)
}

fn write_text_atomically(path: &Path, contents: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| RitError::io(parent, source))?;
    }
    let lock_path = path.with_extension("lock");
    {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&lock_path)
            .map_err(|source| RitError::io(&lock_path, source))?;
        file.write_all(contents.as_bytes())
            .map_err(|source| RitError::io(&lock_path, source))?;
        file.sync_all()
            .map_err(|source| RitError::io(&lock_path, source))?;
    }
    fs::rename(&lock_path, path).map_err(|source| RitError::io(path, source))
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(DIGITS[(byte >> 4) as usize] as char);
        output.push(DIGITS[(byte & 0x0f) as usize] as char);
    }
    output
}

fn unhex_utf8(input: &str) -> Result<String> {
    if !input.len().is_multiple_of(2) {
        return Err(RitError::invalid_input(
            "hex-encoded operation field has odd length",
        ));
    }
    let mut bytes = Vec::with_capacity(input.len() / 2);
    for chunk in input.as_bytes().chunks_exact(2) {
        let high = decode_hex_digit(chunk[0])?;
        let low = decode_hex_digit(chunk[1])?;
        bytes.push((high << 4) | low);
    }
    String::from_utf8(bytes)
        .map_err(|_| RitError::invalid_input("hex-encoded operation field is not UTF-8"))
}

fn decode_hex_digit(byte: u8) -> Result<u8> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        b'A'..=b'F' => Ok(byte - b'A' + 10),
        _ => Err(RitError::invalid_input(
            "hex-encoded operation field contains a non-hex digit",
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{InitOptions, Repository};
    use std::path::{Path, PathBuf};

    #[test]
    fn operation_journal_records_and_restores_previous_head() {
        let root = temp_path("operation-journal-restore");
        let repository = Repository::init(&InitOptions::new(&root)).expect("repo should init");
        write_identity(&repository);
        fs::write(root.join("tracked.txt"), "base\n").expect("base file should be written");
        repository
            .add_paths(&["tracked.txt".to_owned()])
            .expect("base file should be added");
        let base = repository
            .commit_index("base")
            .expect("base commit should work")
            .commit_id;

        let before = repository
            .operations()
            .snapshot()
            .expect("snapshot should work");
        fs::write(root.join("tracked.txt"), "next\n").expect("next file should be written");
        repository
            .add_paths(&["tracked.txt".to_owned()])
            .expect("next file should be added");
        let next = repository
            .commit_index("next")
            .expect("next commit should work")
            .commit_id;
        let after = repository
            .operations()
            .snapshot()
            .expect("snapshot should work");
        repository
            .operations()
            .record("commit", "next", before, after)
            .expect("record should append");

        assert_eq!(
            repository.resolve_head().expect("head should read"),
            Some(next)
        );

        let result = repository
            .operations()
            .undo_last()
            .expect("undo should restore");

        assert_eq!(result.restored_head, base);
        assert_eq!(
            repository.resolve_head().expect("head should read"),
            Some(base)
        );
        assert_eq!(
            fs::read_to_string(root.join("tracked.txt")).expect("file should read"),
            "base\n"
        );
        remove_dir_all(&root);
    }

    fn write_identity(repository: &Repository) {
        fs::write(
            repository.common_dir().join("config"),
            "[core]\n\trepositoryformatversion = 0\n\tbare = false\n[user]\n\tname = Rit Test\n\temail = rit@example.test\n",
        )
        .expect("config should be written");
    }

    fn temp_path(name: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("rit-{name}-{unique}"))
    }

    fn remove_dir_all(path: &Path) {
        if path.exists() {
            fs::remove_dir_all(path).expect("temporary directory should be removed");
        }
    }
}
