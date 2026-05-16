use std::fs;
use std::path::Path;

use crate::{Repository, Result, RitError};

/// One entry from `refs/stash` reflog, ordered as Git displays it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StashListEntry {
    /// Display index, where the newest stash is `stash@{0}`.
    pub index: usize,
    /// Reflog message displayed after `stash@{n}: `.
    pub message: String,
}

impl Repository {
    /// Lists stashes by reading the Git-compatible `refs/stash` reflog.
    pub fn stash_list(&self) -> Result<Vec<StashListEntry>> {
        let path = self.common_dir().join("logs").join("refs").join("stash");
        let text = match fs::read_to_string(&path) {
            Ok(text) => text,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(source) => return Err(RitError::io(&path, source)),
        };

        let mut messages = text
            .lines()
            .filter_map(|line| line.split_once('\t').map(|(_, message)| message.to_owned()))
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
}

fn remove_file_if_exists(path: &Path) -> Result<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(source) => Err(RitError::io(path, source)),
    }
}
