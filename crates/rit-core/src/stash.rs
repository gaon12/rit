use std::fs;

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
}
