use crate::{
    ObjectId, ObjectKind, Repository, Result, RitError, object::parse_tree_entries,
    object::sha1_bytes, parse_commit,
};
use std::collections::{BTreeMap, BTreeSet};
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
    /// Raw index contents captured before an operation.
    ///
    /// This is not serialized into the operation log. When an operation is
    /// recorded, rit stores the bytes in a per-operation sidecar file so
    /// index-only changes can be undone without touching the working tree.
    pub index_contents: Option<Vec<u8>>,
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
    /// Repository-relative paths changed by this operation.
    pub changed_paths: Vec<String>,
    /// Git object IDs created by this operation and known to rit.
    pub created_object_ids: Vec<ObjectId>,
}

/// A malformed operation-journal line skipped while reading the log.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OperationJournalWarning {
    /// One-based line number in `.git/rit/ops.log`.
    pub line_number: usize,
    /// Human-readable parse error.
    pub message: String,
}

/// Operation records plus non-fatal journal read warnings.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct OperationLog {
    /// Valid operation records in append order.
    pub records: Vec<OperationRecord>,
    /// Malformed lines that were ignored.
    pub warnings: Vec<OperationJournalWarning>,
}

/// Result of restoring a journal entry.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OperationRestoreResult {
    /// Restored operation ID.
    pub id: String,
    /// Commit restored into HEAD and the working tree, when HEAD changed.
    pub restored_head: Option<ObjectId>,
    /// Whether the index was restored.
    pub restored_index: bool,
    /// Whether working tree files were restored from a commit tree.
    pub restored_worktree: bool,
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
        let index_contents =
            read_index_contents(self.repository.git_dir().join("index").as_path())?;
        let index_checksum = index_contents
            .as_ref()
            .map(|contents| hex(&sha1_bytes(contents)));
        Ok(OperationSnapshot {
            head: self.repository.resolve_head()?,
            branch: self.repository.current_branch_name()?,
            index_checksum,
            index_contents,
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
        self.record_with_details(command, summary, before, after, Vec::new(), Vec::new())
    }

    /// Appends one operation record with changed paths and created object IDs.
    pub fn record_with_details(
        &self,
        command: &str,
        summary: &str,
        before: OperationSnapshot,
        after: OperationSnapshot,
        changed_paths: Vec<String>,
        created_object_ids: Vec<ObjectId>,
    ) -> Result<OperationRecord> {
        let id = next_operation_id();
        if before.index_checksum != after.index_checksum {
            save_before_index(self.repository, &id, &before)?;
        }
        let record = OperationRecord {
            id,
            command: command.to_owned(),
            summary: summary.to_owned(),
            before,
            after,
            changed_paths,
            created_object_ids,
        };
        append_record(self.repository, &record)?;
        Ok(record)
    }

    /// Returns paths whose tree entries differ between two operation snapshots.
    pub fn changed_paths_between(
        &self,
        before: &OperationSnapshot,
        after: &OperationSnapshot,
    ) -> Result<Vec<String>> {
        let before_entries = match before.head {
            Some(head) => self.tree_entries_for_commit(head)?,
            None => BTreeMap::new(),
        };
        let after_entries = match after.head {
            Some(head) => self.tree_entries_for_commit(head)?,
            None => BTreeMap::new(),
        };
        let mut paths = before_entries.keys().cloned().collect::<BTreeSet<_>>();
        paths.extend(after_entries.keys().cloned());
        Ok(paths
            .into_iter()
            .filter(|path| before_entries.get(path) != after_entries.get(path))
            .collect())
    }

    fn tree_entries_for_commit(
        &self,
        commit_id: ObjectId,
    ) -> Result<BTreeMap<String, TreeEntryKey>> {
        let object = self.repository.read_object(commit_id)?;
        if object.kind != ObjectKind::Commit {
            return Err(RitError::invalid_input(format!(
                "object {commit_id} is {}, not commit",
                object.kind
            )));
        }
        let commit = parse_commit(&object.data)?;
        let mut entries = BTreeMap::new();
        collect_tree_entries(self.repository, "", commit.tree, &mut entries)?;
        Ok(entries)
    }

    /// Reads operation records in append order.
    pub fn log(&self) -> Result<Vec<OperationRecord>> {
        Ok(self.log_with_warnings()?.records)
    }

    /// Reads operation records and reports malformed lines without failing the
    /// whole journal. A broken rit metadata line must not block supported undo
    /// for earlier or later valid records.
    pub fn log_with_warnings(&self) -> Result<OperationLog> {
        let path = log_path(self.repository);
        if !path.exists() {
            return Ok(OperationLog::default());
        }
        let contents = fs::read_to_string(&path).map_err(|source| RitError::io(&path, source))?;
        let mut records = Vec::new();
        let mut warnings = Vec::new();
        for (index, line) in contents.lines().enumerate() {
            if line.trim().is_empty() {
                continue;
            }
            match parse_record_line(line) {
                Ok(record) => records.push(record),
                Err(error) => warnings.push(OperationJournalWarning {
                    line_number: index + 1,
                    message: error.to_string(),
                }),
            }
        }
        Ok(OperationLog { records, warnings })
    }

    /// Restores the state captured before the operation with `id`.
    pub fn restore(&self, id: &str) -> Result<OperationRestoreResult> {
        let record = self
            .log()?
            .into_iter()
            .find(|record| record.id == id)
            .ok_or_else(|| RitError::invalid_input(format!("operation not found: {id}")))?;
        restore_record(self.repository, &record)
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

fn restore_record(
    repository: &Repository,
    record: &OperationRecord,
) -> Result<OperationRestoreResult> {
    let head_changed =
        record.before.head != record.after.head || record.before.branch != record.after.branch;
    if head_changed {
        restore_snapshot(repository, &record.before)?;
        return Ok(OperationRestoreResult {
            id: record.id.clone(),
            restored_head: record.before.head,
            restored_index: true,
            restored_worktree: true,
        });
    }
    if record.before.index_checksum != record.after.index_checksum {
        restore_before_index(repository, record)?;
        return Ok(OperationRestoreResult {
            id: record.id.clone(),
            restored_head: record.before.head,
            restored_index: true,
            restored_worktree: false,
        });
    }
    Err(RitError::invalid_input(format!(
        "operation {} has no restorable HEAD or index change",
        record.id
    )))
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

fn save_before_index(
    repository: &Repository,
    operation_id: &str,
    snapshot: &OperationSnapshot,
) -> Result<()> {
    let Some(contents) = snapshot.index_contents.as_deref() else {
        return Ok(());
    };
    let path = operation_artifact_dir(repository, operation_id)?.join("before.index");
    write_bytes_atomically(&path, contents)
}

fn restore_before_index(repository: &Repository, record: &OperationRecord) -> Result<()> {
    let index_path = repository.git_dir().join("index");
    if record.before.index_checksum.is_none() {
        if index_path.exists() {
            fs::remove_file(&index_path).map_err(|source| RitError::io(&index_path, source))?;
        }
        return Ok(());
    }
    let backup_path = operation_artifact_dir(repository, &record.id)?.join("before.index");
    let contents = fs::read(&backup_path).map_err(|source| RitError::io(&backup_path, source))?;
    let checksum = hex(&sha1_bytes(&contents));
    if Some(checksum.as_str()) != record.before.index_checksum.as_deref() {
        return Err(RitError::invalid_input(format!(
            "operation {} index backup checksum does not match journal",
            record.id
        )));
    }
    write_bytes_atomically(&index_path, &contents)
}

fn operation_artifact_dir(
    repository: &Repository,
    operation_id: &str,
) -> Result<std::path::PathBuf> {
    if !operation_id
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || character == '-' || character == '_')
    {
        return Err(RitError::invalid_input(format!(
            "operation id is not safe for sidecar storage: {operation_id}"
        )));
    }
    Ok(repository
        .git_dir()
        .join("rit")
        .join("ops")
        .join(operation_id))
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

fn read_index_contents(path: &Path) -> Result<Option<Vec<u8>>> {
    if !path.exists() {
        return Ok(None);
    }
    fs::read(path)
        .map(Some)
        .map_err(|source| RitError::io(path, source))
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
        format_hex_string_list(&record.changed_paths),
        format_object_id_list(&record.created_object_ids),
    ]
    .join("\t")
}

fn parse_record_line(line: &str) -> Result<OperationRecord> {
    let fields = split_escaped_tabs(line);
    if fields.len() != 5 && fields.len() != 7 {
        return Err(RitError::invalid_input(format!(
            "operation journal line has {} fields, expected 5 or 7",
            fields.len()
        )));
    }
    Ok(OperationRecord {
        id: unescape(&fields[0])?,
        command: unescape(&fields[1])?,
        summary: unescape(&fields[2])?,
        before: parse_snapshot(&fields[3])?,
        after: parse_snapshot(&fields[4])?,
        changed_paths: fields
            .get(5)
            .map(|field| parse_hex_string_list(field))
            .transpose()?
            .unwrap_or_default(),
        created_object_ids: fields
            .get(6)
            .map(|field| parse_object_id_list(field))
            .transpose()?
            .unwrap_or_default(),
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
        index_contents: None,
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct TreeEntryKey {
    mode: u32,
    object_id: ObjectId,
}

fn collect_tree_entries(
    repository: &Repository,
    prefix: &str,
    tree_id: ObjectId,
    output: &mut BTreeMap<String, TreeEntryKey>,
) -> Result<()> {
    let tree = repository.read_object(tree_id)?;
    if tree.kind != ObjectKind::Tree {
        return Err(RitError::invalid_input(format!(
            "object {tree_id} is {}, not tree",
            tree.kind
        )));
    }
    for entry in parse_tree_entries(&tree.data)? {
        let path = if prefix.is_empty() {
            entry.name_lossy()
        } else {
            format!("{prefix}/{}", entry.name_lossy())
        };
        if entry.kind == ObjectKind::Tree {
            collect_tree_entries(repository, &path, entry.object_id, output)?;
        } else {
            output.insert(
                path,
                TreeEntryKey {
                    mode: parse_tree_mode(&entry.mode)?,
                    object_id: entry.object_id,
                },
            );
        }
    }
    Ok(())
}

fn parse_tree_mode(mode: &str) -> Result<u32> {
    u32::from_str_radix(mode, 8)
        .map_err(|_| RitError::invalid_input(format!("tree entry mode is invalid: {mode}")))
}

fn format_hex_string_list(values: &[String]) -> String {
    if values.is_empty() {
        return "-".to_owned();
    }
    values
        .iter()
        .map(|value| hex(value.as_bytes()))
        .collect::<Vec<_>>()
        .join(",")
}

fn parse_hex_string_list(input: &str) -> Result<Vec<String>> {
    if input == "-" {
        return Ok(Vec::new());
    }
    input.split(',').map(unhex_utf8).collect()
}

fn format_object_id_list(values: &[ObjectId]) -> String {
    if values.is_empty() {
        return "-".to_owned();
    }
    values
        .iter()
        .map(|object_id| object_id.to_hex())
        .collect::<Vec<_>>()
        .join(",")
}

fn parse_object_id_list(input: &str) -> Result<Vec<ObjectId>> {
    if input == "-" {
        return Ok(Vec::new());
    }
    input.split(',').map(ObjectId::from_hex).collect()
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
    write_bytes_atomically(path, contents.as_bytes())
}

fn write_bytes_atomically(path: &Path, contents: &[u8]) -> Result<()> {
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
        file.write_all(contents)
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
            .record_with_details(
                "commit",
                "next",
                before,
                after,
                vec!["tracked.txt".to_owned()],
                vec![next],
            )
            .expect("record should append");

        let records = repository
            .operations()
            .log()
            .expect("operation log should read");
        assert_eq!(records[0].changed_paths, vec!["tracked.txt"]);
        assert_eq!(records[0].created_object_ids, vec![next]);

        assert_eq!(
            repository.resolve_head().expect("head should read"),
            Some(next)
        );

        let result = repository
            .operations()
            .undo_last()
            .expect("undo should restore");

        assert_eq!(result.restored_head, Some(base));
        assert!(result.restored_index);
        assert!(result.restored_worktree);
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

    #[test]
    fn operation_journal_undo_restores_index_without_touching_worktree() {
        let root = temp_path("operation-journal-index-undo");
        let repository = Repository::init(&InitOptions::new(&root)).expect("repo should init");
        write_identity(&repository);
        fs::write(root.join("tracked.txt"), "base\n").expect("base file should be written");
        repository
            .add_paths(&["tracked.txt".to_owned()])
            .expect("base file should be added");
        let head = repository
            .commit_index("base")
            .expect("base commit should work")
            .commit_id;

        fs::write(root.join("tracked.txt"), "changed\n").expect("changed file should be written");
        let before = repository
            .operations()
            .snapshot()
            .expect("snapshot should work");
        repository
            .add_paths(&["tracked.txt".to_owned()])
            .expect("changed file should be staged");
        let after = repository
            .operations()
            .snapshot()
            .expect("snapshot should work");
        assert_ne!(before.index_checksum, after.index_checksum);

        repository
            .operations()
            .record_with_details(
                "add",
                "add tracked.txt",
                before.clone(),
                after,
                vec!["tracked.txt".to_owned()],
                Vec::new(),
            )
            .expect("record should append");

        let result = repository
            .operations()
            .undo_last()
            .expect("index-only undo should restore");

        assert_eq!(result.restored_head, Some(head));
        assert!(result.restored_index);
        assert!(!result.restored_worktree);
        assert_eq!(
            repository
                .operations()
                .snapshot()
                .expect("snapshot should work")
                .index_checksum,
            before.index_checksum
        );
        assert_eq!(
            fs::read_to_string(root.join("tracked.txt")).expect("file should read"),
            "changed\n"
        );
        remove_dir_all(&root);
    }

    #[test]
    fn operation_journal_skips_malformed_lines_with_warnings() {
        let root = temp_path("operation-journal-malformed");
        let repository = Repository::init(&InitOptions::new(&root)).expect("repo should init");
        write_identity(&repository);
        fs::write(root.join("tracked.txt"), "base\n").expect("base file should be written");
        repository
            .add_paths(&["tracked.txt".to_owned()])
            .expect("base file should be added");
        repository
            .commit_index("base")
            .expect("base commit should work");
        let before = repository
            .operations()
            .snapshot()
            .expect("snapshot should work");
        repository
            .operations()
            .record("commit", "base", before.clone(), before)
            .expect("record should append");
        append_raw_log_line(&repository, "not\ta\tvalid\toperation\tjournal\tline\t!");

        let log = repository
            .operations()
            .log_with_warnings()
            .expect("journal should read");

        assert_eq!(log.records.len(), 1);
        assert_eq!(log.warnings.len(), 1);
        assert_eq!(log.warnings[0].line_number, 2);
        assert!(
            log.warnings[0]
                .message
                .contains("operation snapshot field is malformed")
        );
        assert_eq!(
            repository
                .operations()
                .log()
                .expect("plain log should return valid records")
                .len(),
            1
        );
        remove_dir_all(&root);
    }

    #[test]
    fn undo_ignores_malformed_only_journal_without_changing_head() {
        let root = temp_path("operation-journal-malformed-only");
        let repository = Repository::init(&InitOptions::new(&root)).expect("repo should init");
        write_identity(&repository);
        fs::write(root.join("tracked.txt"), "base\n").expect("base file should be written");
        repository
            .add_paths(&["tracked.txt".to_owned()])
            .expect("base file should be added");
        let head = repository
            .commit_index("base")
            .expect("base commit should work")
            .commit_id;
        append_raw_log_line(&repository, "not an operation record");

        let error = repository
            .operations()
            .undo_last()
            .expect_err("malformed-only journal should not be restorable");

        assert!(error.to_string().contains("operation journal is empty"));
        assert_eq!(
            repository.resolve_head().expect("head should read"),
            Some(head)
        );
        assert_eq!(
            fs::read_to_string(root.join("tracked.txt")).expect("file should read"),
            "base\n"
        );
        remove_dir_all(&root);
    }

    #[test]
    fn linked_worktree_operation_journals_are_isolated() {
        let root = temp_path("operation-journal-linked-worktree");
        let main_worktree = root.join("main");
        let linked_worktree = root.join("linked");
        let repository =
            Repository::init(&InitOptions::new(&main_worktree)).expect("repo should init");
        write_identity(&repository);
        fs::write(main_worktree.join("tracked.txt"), "base\n")
            .expect("base file should be written");
        repository
            .add_paths(&["tracked.txt".to_owned()])
            .expect("base file should be added");
        let head = repository
            .commit_index("base")
            .expect("base commit should work")
            .commit_id;

        let linked_git_dir = repository.git_dir().join("worktrees").join("linked");
        fs::create_dir_all(&linked_git_dir).expect("linked git dir should be created");
        fs::create_dir_all(&linked_worktree).expect("linked worktree should be created");
        fs::write(
            linked_worktree.join(".git"),
            format!("gitdir: {}\n", linked_git_dir.display()),
        )
        .expect(".git file should be written");
        fs::write(linked_git_dir.join("commondir"), "../..").expect("commondir should be written");
        fs::write(linked_git_dir.join("HEAD"), "ref: refs/heads/linked\n")
            .expect("linked HEAD should be written");
        fs::create_dir_all(repository.common_dir().join("refs").join("heads"))
            .expect("heads dir should exist");
        fs::write(
            repository
                .common_dir()
                .join("refs")
                .join("heads")
                .join("linked"),
            format!("{head}\n"),
        )
        .expect("linked branch should be written");
        let linked_repository =
            Repository::open(&linked_worktree).expect("linked worktree should open");

        assert_eq!(repository.common_dir(), linked_repository.common_dir());
        assert_ne!(repository.git_dir(), linked_repository.git_dir());

        let main_snapshot = repository
            .operations()
            .snapshot()
            .expect("main snapshot should work");
        repository
            .operations()
            .record(
                "op",
                "main worktree operation",
                main_snapshot.clone(),
                main_snapshot,
            )
            .expect("main record should append");
        let linked_snapshot = linked_repository
            .operations()
            .snapshot()
            .expect("linked snapshot should work");
        linked_repository
            .operations()
            .record(
                "op",
                "linked worktree operation",
                linked_snapshot.clone(),
                linked_snapshot,
            )
            .expect("linked record should append");

        assert_ne!(log_path(&repository), log_path(&linked_repository));
        assert_eq!(
            repository
                .operations()
                .log()
                .expect("main log should read")
                .into_iter()
                .map(|record| record.summary)
                .collect::<Vec<_>>(),
            vec!["main worktree operation"]
        );
        assert_eq!(
            linked_repository
                .operations()
                .log()
                .expect("linked log should read")
                .into_iter()
                .map(|record| record.summary)
                .collect::<Vec<_>>(),
            vec!["linked worktree operation"]
        );

        remove_dir_all(&root);
    }

    fn append_raw_log_line(repository: &Repository, line: &str) {
        let path = log_path(repository);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("log parent should exist");
        }
        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .expect("log should open");
        writeln!(file, "{line}").expect("log line should write");
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
