use std::fs;
use std::io::Write;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::{
    DiffPatch, DiffSummary, GitConfig, ObjectId, ObjectKind, PathspecSet, Repository, Result,
    RitError, Signature,
};

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

    /// Stores an existing commit as the newest loose stash entry.
    pub fn stash_store(&self, target: ObjectId, message: Option<&str>) -> Result<()> {
        let object = self.read_object(target)?;
        if object.kind != ObjectKind::Commit {
            return Err(RitError::invalid_input(format!(
                "stash store target {target} is {}, not commit",
                object.kind
            )));
        }

        let mut entries = self.read_stash_reflog()?;
        let old_id = entries
            .last()
            .map(|entry| entry.new_id)
            .unwrap_or(zero_object_id()?);
        entries.push(StashReflogEntry {
            old_id,
            new_id: target,
            rest: format!(
                "{}\t{}",
                format_reflog_signature(&self.reflog_committer()?),
                message.unwrap_or("Created via \"git stash store\".")
            ),
        });
        self.write_stash_reflog(&entries)?;
        self.write_stash_ref(target)
    }

    /// Shows the changes recorded by one stash against its first parent.
    pub fn stash_show(&self, display_index: usize, pathspecs: &PathspecSet) -> Result<DiffSummary> {
        let (base_id, stash_id) = self.stash_diff_pair(display_index)?;
        self.diff_commits_with_pathspecs(base_id, stash_id, pathspecs)
    }

    /// Shows patch output for the changes recorded by one stash.
    pub fn stash_show_patch(
        &self,
        display_index: usize,
        pathspecs: &PathspecSet,
    ) -> Result<DiffPatch> {
        let (base_id, stash_id) = self.stash_diff_pair(display_index)?;
        self.diff_commits_patch_with_pathspecs(base_id, stash_id, pathspecs)
    }

    fn stash_diff_pair(&self, display_index: usize) -> Result<(ObjectId, ObjectId)> {
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
        Ok((base_id, stash_id))
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

    fn reflog_committer(&self) -> Result<Signature> {
        let config_path = self.common_dir().join("config");
        let name = std::env::var("GIT_COMMITTER_NAME")
            .ok()
            .or_else(|| read_config_value(&config_path, "user", "name"))
            .ok_or_else(|| {
                RitError::invalid_input(
                    "committer identity unknown; set user.name or GIT_COMMITTER_NAME",
                )
            })?;
        let email = std::env::var("GIT_COMMITTER_EMAIL")
            .ok()
            .or_else(|| read_config_value(&config_path, "user", "email"))
            .ok_or_else(|| {
                RitError::invalid_input(
                    "committer identity unknown; set user.email or GIT_COMMITTER_EMAIL",
                )
            })?;
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| RitError::invalid_input("system time is before Unix epoch"))?
            .as_secs() as i64;
        Ok(Signature {
            name,
            email,
            timestamp,
            offset: "+0000".to_owned(),
        })
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
    let mut previous = zero_object_id()?;
    for entry in entries {
        entry.old_id = previous;
        previous = entry.new_id;
    }
    Ok(())
}

fn zero_object_id() -> Result<ObjectId> {
    ObjectId::from_hex(ZERO_OBJECT_ID)
}

fn read_config_value(path: &Path, section: &str, key: &str) -> Option<String> {
    GitConfig::read(path)
        .ok()
        .and_then(|config| config.get(section, key).map(ToOwned::to_owned))
}

fn format_reflog_signature(signature: &Signature) -> String {
    format!(
        "{} <{}> {} {}",
        signature.name, signature.email, signature.timestamp, signature.offset
    )
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
