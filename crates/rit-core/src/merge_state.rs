use crate::{ObjectId, Repository, Result, RitError};
use std::fs;
use std::path::{Path, PathBuf};

/// Repository operation state recorded by Git state files.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct MergeState {
    /// Object IDs listed in `.git/MERGE_HEAD`.
    pub merge_heads: Vec<ObjectId>,
    /// Object ID listed in `.git/CHERRY_PICK_HEAD`.
    pub cherry_pick_head: Option<ObjectId>,
    /// Object ID listed in `.git/REVERT_HEAD`.
    pub revert_head: Option<ObjectId>,
    /// Rebase state stored in `.git/rebase-apply`.
    pub rebase_apply: Option<RebaseState>,
    /// Rebase state stored in `.git/rebase-merge`.
    pub rebase_merge: Option<RebaseState>,
    /// Merge message draft from `.git/MERGE_MSG`.
    pub merge_message: Option<String>,
    /// Squash message draft from `.git/SQUASH_MSG`.
    pub squash_message: Option<String>,
}

impl MergeState {
    /// Returns true when no known operation state files are present.
    pub fn is_clean(&self) -> bool {
        self.merge_heads.is_empty()
            && self.cherry_pick_head.is_none()
            && self.revert_head.is_none()
            && self.rebase_apply.is_none()
            && self.rebase_merge.is_none()
            && self.merge_message.is_none()
            && self.squash_message.is_none()
    }
}

/// Minimal rebase directory metadata.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RebaseState {
    /// Path to the rebase state directory.
    pub directory: PathBuf,
    /// Branch name from `head-name`, when Git recorded one.
    pub head_name: Option<String>,
    /// Target commit from `onto`, when present.
    pub onto: Option<ObjectId>,
    /// Original HEAD from `orig-head`, when present.
    pub original_head: Option<ObjectId>,
    /// Current patch number from `msgnum`, when present.
    pub current_step: Option<u32>,
    /// Total patch count from `end`, when present.
    pub total_steps: Option<u32>,
}

impl Repository {
    /// Reads Git-compatible merge, cherry-pick, revert, and rebase state files.
    pub fn merge_state(&self) -> Result<MergeState> {
        let git_dir = self.git_dir();
        Ok(MergeState {
            merge_heads: read_object_id_list(&git_dir.join("MERGE_HEAD"))?,
            cherry_pick_head: read_optional_object_id(&git_dir.join("CHERRY_PICK_HEAD"))?,
            revert_head: read_optional_object_id(&git_dir.join("REVERT_HEAD"))?,
            rebase_apply: read_rebase_state(&git_dir.join("rebase-apply"))?,
            rebase_merge: read_rebase_state(&git_dir.join("rebase-merge"))?,
            merge_message: read_optional_text(&git_dir.join("MERGE_MSG"))?,
            squash_message: read_optional_text(&git_dir.join("SQUASH_MSG"))?,
        })
    }
}

fn read_object_id_list(path: &Path) -> Result<Vec<ObjectId>> {
    let Some(text) = read_optional_text(path)? else {
        return Ok(Vec::new());
    };
    let mut object_ids = Vec::new();
    for line in text.lines().map(str::trim).filter(|line| !line.is_empty()) {
        object_ids.push(parse_state_object_id(path, line)?);
    }
    Ok(object_ids)
}

fn read_optional_object_id(path: &Path) -> Result<Option<ObjectId>> {
    let Some(text) = read_optional_text(path)? else {
        return Ok(None);
    };
    let Some(line) = text.lines().map(str::trim).find(|line| !line.is_empty()) else {
        return Ok(None);
    };
    Ok(Some(parse_state_object_id(path, line)?))
}

fn parse_state_object_id(path: &Path, value: &str) -> Result<ObjectId> {
    ObjectId::from_hex(value).map_err(|_| {
        RitError::invalid_input(format!(
            "invalid object id in operation state file {}: {value}",
            path.display()
        ))
    })
}

fn read_rebase_state(path: &Path) -> Result<Option<RebaseState>> {
    if !path.is_dir() {
        return Ok(None);
    }
    Ok(Some(RebaseState {
        directory: path.to_path_buf(),
        head_name: read_optional_trimmed_text(&path.join("head-name"))?,
        onto: read_optional_object_id(&path.join("onto"))?,
        original_head: read_optional_object_id(&path.join("orig-head"))?,
        current_step: read_optional_u32(&path.join("msgnum"))?,
        total_steps: read_optional_u32(&path.join("end"))?,
    }))
}

fn read_optional_u32(path: &Path) -> Result<Option<u32>> {
    let Some(text) = read_optional_trimmed_text(path)? else {
        return Ok(None);
    };
    text.parse::<u32>().map(Some).map_err(|error| {
        RitError::invalid_input(format!(
            "invalid number in operation state file {}: {error}",
            path.display()
        ))
    })
}

fn read_optional_trimmed_text(path: &Path) -> Result<Option<String>> {
    Ok(read_optional_text(path)?.map(|text| text.trim().to_owned()))
}

fn read_optional_text(path: &Path) -> Result<Option<String>> {
    match fs::read_to_string(path) {
        Ok(text) => Ok(Some(text)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(source) => Err(RitError::Io {
            path: path.to_path_buf(),
            source,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{InitOptions, Repository};
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn clean_repository_has_empty_merge_state() {
        let temp = temp_path("clean-merge-state");
        let repository = Repository::init(&InitOptions::new(&temp)).expect("init should work");

        assert!(
            repository
                .merge_state()
                .expect("state should read")
                .is_clean()
        );
        remove_dir_all(&temp);
    }

    #[test]
    fn reads_merge_and_cherry_pick_state_files() {
        let temp = temp_path("operation-state");
        let repository = Repository::init(&InitOptions::new(&temp)).expect("init should work");
        let first = ObjectId::from_hex("1111111111111111111111111111111111111111")
            .expect("valid object id");
        let second = ObjectId::from_hex("2222222222222222222222222222222222222222")
            .expect("valid object id");
        fs::write(
            repository.git_dir().join("MERGE_HEAD"),
            format!("{}\n{}\n", first.to_hex(), second.to_hex()),
        )
        .expect("merge head should write");
        fs::write(
            repository.git_dir().join("CHERRY_PICK_HEAD"),
            format!("{}\n", first.to_hex()),
        )
        .expect("cherry-pick head should write");
        fs::write(repository.git_dir().join("MERGE_MSG"), "merge message\n")
            .expect("merge message should write");

        let state = repository.merge_state().expect("state should read");

        assert_eq!(state.merge_heads, vec![first, second]);
        assert_eq!(state.cherry_pick_head, Some(first));
        assert_eq!(state.merge_message, Some("merge message\n".to_owned()));
        assert!(!state.is_clean());
        remove_dir_all(&temp);
    }

    #[test]
    fn reads_rebase_state_directory() {
        let temp = temp_path("rebase-state");
        let repository = Repository::init(&InitOptions::new(&temp)).expect("init should work");
        let rebase_dir = repository.git_dir().join("rebase-merge");
        fs::create_dir_all(&rebase_dir).expect("rebase directory should be created");
        let onto = ObjectId::from_hex("3333333333333333333333333333333333333333")
            .expect("valid object id");
        let original = ObjectId::from_hex("4444444444444444444444444444444444444444")
            .expect("valid object id");
        fs::write(rebase_dir.join("head-name"), "refs/heads/topic\n")
            .expect("head-name should write");
        fs::write(rebase_dir.join("onto"), format!("{}\n", onto.to_hex()))
            .expect("onto should write");
        fs::write(
            rebase_dir.join("orig-head"),
            format!("{}\n", original.to_hex()),
        )
        .expect("orig-head should write");
        fs::write(rebase_dir.join("msgnum"), "2\n").expect("msgnum should write");
        fs::write(rebase_dir.join("end"), "5\n").expect("end should write");

        let state = repository.merge_state().expect("state should read");
        let rebase = state.rebase_merge.expect("rebase state should exist");

        assert_eq!(rebase.head_name, Some("refs/heads/topic".to_owned()));
        assert_eq!(rebase.onto, Some(onto));
        assert_eq!(rebase.original_head, Some(original));
        assert_eq!(rebase.current_step, Some(2));
        assert_eq!(rebase.total_steps, Some(5));
        remove_dir_all(&temp);
    }

    fn temp_path(name: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("rit-{name}-{unique}"))
    }

    fn remove_dir_all(path: &Path) {
        let _ = fs::remove_dir_all(path);
    }
}
