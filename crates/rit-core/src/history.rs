use crate::{Commit, ObjectId, ObjectKind, Repository, Result, RitError, parse_commit};

/// A commit and the object ID it was read from.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LogEntry {
    /// Commit object ID.
    pub object_id: ObjectId,
    /// Parsed commit data.
    pub commit: Commit,
}

impl Repository {
    /// Reads commits from `HEAD` following the first parent.
    pub fn log_first_parent(&self) -> Result<Vec<LogEntry>> {
        let mut entries = Vec::new();
        let mut next = self.resolve_head()?;

        while let Some(object_id) = next {
            let object = self.read_object(object_id)?;
            if object.kind != ObjectKind::Commit {
                return Err(RitError::invalid_input(format!(
                    "object {object_id} is {}, not commit",
                    object.kind
                )));
            }
            let commit = parse_commit(&object.data)?;
            next = commit.parents.first().copied();
            entries.push(LogEntry { object_id, commit });
        }

        Ok(entries)
    }
}
