use std::fs;
use std::io::Write;
use std::path::Path;

use crate::{DiffSummary, ObjectId, ObjectKind, PathspecSet, Repository, Result, RitError};

const ZERO_OBJECT_ID: &str = "0000000000000000000000000000000000000000";

/// One entry from `refs/stash` reflog, ordered as Git displays it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StashListEntry {
    /// Display index, where the newest stash is `stash@{0}`.
    pub index: usize,
    /// Reflog message displayed after `stash@{n}: `.
    pub message: String,
}

/// Result of dropping one stash entry.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StashDropResult {
    /// Display name of the dropped stash, such as `refs/stash@{0}`.
    pub name: String,
    /// Commit ID that was stored by the dropped reflog entry.
    pub object_id: ObjectId,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct StashReflogEntry {
    old_id: ObjectId,
    new_id: ObjectId,
    rest: String,
}

impl Repository {
    /// Lists stashes by reading the Git-compatible `refs/stash` reflog.
    pub fn stash_list(&self) -> Result<Vec<StashListEntry>> {
        let mut messages = self
            .read_stash_reflog()?
            .into_iter()
            .filter_map(|entry| {
                entry
                    .rest
                    .split_once('\t')
                    .map(|(_, message)| message.to_owned())
            })
            .collect::<Vec<_>>();
        messages.reverse();

        Ok(messages
            .into_iter()
            .enumerate()
            .map(|(index, message)| StashListEntry { index, message })
            .collect())
    }

    /// Clears the loose `refs/stash` ref and its reflog.
    pub fn stash_clear(&self) -> Result<()> {
        remove_file_if_exists(&self.common_dir().join("refs").join("stash"))?;
        remove_file_if_exists(&self.common_dir().join("logs").join("refs").join("stash"))
    }

    /// Drops one stash reflog entry and updates loose `refs/stash`.
    pub fn stash_drop(&self, display_index: usize, name: String) -> Result<StashDropResult> {
        let mut entries = self.read_stash_reflog()?;
        if entries.is_empty() {
            return Err(RitError::invalid_input("No stash entries found."));
        }
        if display_index >= entries.len() {
            return Err(RitError::invalid_input(format!(
                "log for 'stash' only has {} entries",
                entries.len()
            )));
        }

        let storage_index = entries.len() - 1 - display_index;
        let dropped = entries.remove(storage_index);
        if entries.is_empty() {
            self.stash_clear()?;
        } else {
            relink_stash_reflog_entries(&mut entries)?;
            self.write_stash_reflog(&entries)?;
            self.write_stash_ref(entries.last().expect("entries not empty").new_id)?;
        }

        Ok(StashDropResult {
            name,
            object_id: dropped.new_id,
        })
    }

    /// Shows the changes recorded by one stash against its first parent.
    pub fn stash_show(&self, display_index: usize, pathspecs: &PathspecSet) -> Result<DiffSummary> {
        let stash_id = self.stash_id_at(display_index)?;
        let stash_object = self.read_object(stash_id)?;
        if stash_object.kind != ObjectKind::Commit {
            return Err(RitError::invalid_input(format!(
                "stash entry {stash_id} is {}, not commit",
                stash_object.kind
            )));
        }
        let stash_commit = crate::parse_commit(&stash_object.data)?;
        let base_id = stash_commit
            .parents
            .first()
            .copied()
            .ok_or_else(|| RitError::invalid_input("stash commit has no parent"))?;
        self.diff_commits_with_pathspecs(base_id, stash_id, pathspecs)
    }

    fn read_stash_reflog(&self) -> Result<Vec<StashReflogEntry>> {
        let path = self.common_dir().join("logs").join("refs").join("stash");
        let text = match fs::read_to_string(&path) {
            Ok(text) => text,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(source) => return Err(RitError::io(&path, source)),
        };

        text.lines().map(parse_stash_reflog_entry).collect()
    }

    fn stash_id_at(&self, display_index: usize) -> Result<ObjectId> {
        let entries = self.read_stash_reflog()?;
        if entries.is_empty() {
            return Err(RitError::invalid_input("No stash entries found."));
        }
        if display_index >= entries.len() {
            return Err(RitError::invalid_input(format!(
                "log for 'stash' only has {} entries",
                entries.len()
            )));
        }
        Ok(entries[entries.len() - 1 - display_index].new_id)
    }

    fn write_stash_reflog(&self, entries: &[StashReflogEntry]) -> Result<()> {
        let path = self.common_dir().join("logs").join("refs").join("stash");
        write_file_atomically(&path, |file| {
            for entry in entries {
                writeln!(file, "{} {} {}", entry.old_id, entry.new_id, entry.rest)?;
            }
            Ok(())
        })
    }

    fn write_stash_ref(&self, target: ObjectId) -> Result<()> {
        let path = self.common_dir().join("refs").join("stash");
        write_file_atomically(&path, |file| writeln!(file, "{target}"))
    }
}

fn parse_stash_reflog_entry(line: &str) -> Result<StashReflogEntry> {
    let mut parts = line.splitn(3, ' ');
    let old_id = parts
        .next()
        .ok_or_else(|| RitError::invalid_input("malformed stash reflog entry"))?;
    let new_id = parts
        .next()
        .ok_or_else(|| RitError::invalid_input("malformed stash reflog entry"))?;
    let rest = parts
        .next()
        .ok_or_else(|| RitError::invalid_input("malformed stash reflog entry"))?;

    Ok(StashReflogEntry {
        old_id: ObjectId::from_hex(old_id)?,
        new_id: ObjectId::from_hex(new_id)?,
        rest: rest.to_owned(),
    })
}

fn relink_stash_reflog_entries(entries: &mut [StashReflogEntry]) -> Result<()> {
    let mut previous = ObjectId::from_hex(ZERO_OBJECT_ID)?;
    for entry in entries {
        entry.old_id = previous;
        previous = entry.new_id;
    }
    Ok(())
}

fn remove_file_if_exists(path: &Path) -> Result<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(source) => Err(RitError::io(path, source)),
    }
}

fn write_file_atomically(
    path: &Path,
    write_contents: impl FnOnce(&mut fs::File) -> std::io::Result<()>,
) -> Result<()> {
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
        write_contents(&mut file).map_err(|source| RitError::io(&lock_path, source))?;
        file.sync_all()
            .map_err(|source| RitError::io(&lock_path, source))?;
    }
    fs::rename(&lock_path, path).map_err(|source| RitError::io(path, source))
}
