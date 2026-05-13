use crate::index::join_slash_path;
use crate::write::{
    MergeConflictStageEntry, MergeConflictStagePlan, write_worktree_entry_atomically,
};
use crate::{ObjectKind, Repository, Result, RitError};
use std::path::Path;

pub(crate) fn write_conflict_markers(
    repository: &Repository,
    worktree: &Path,
    conflict_stages: &[MergeConflictStagePlan],
    target_label: &str,
    symlinks_enabled: bool,
) -> Result<()> {
    for conflict in conflict_stages {
        let Some(head) = conflict.head else {
            continue;
        };
        let Some(target) = conflict.target else {
            continue;
        };
        if !is_regular_file_mode(head.mode) || !is_regular_file_mode(target.mode) {
            continue;
        }
        let Some(head_text) = read_text_blob(repository, head)? else {
            continue;
        };
        let Some(target_text) = read_text_blob(repository, target)? else {
            continue;
        };
        let marker_text = conflict_marker_text(&head_text, &target_text, target_label);
        write_worktree_entry_atomically(
            &join_slash_path(worktree, &conflict.path),
            marker_text.as_bytes(),
            head.mode,
            symlinks_enabled,
        )?;
    }
    Ok(())
}

fn read_text_blob(
    repository: &Repository,
    entry: MergeConflictStageEntry,
) -> Result<Option<String>> {
    let object = repository.read_object(entry.object_id)?;
    if object.kind != ObjectKind::Blob {
        return Err(RitError::invalid_input(format!(
            "object {} is {}, not blob",
            entry.object_id, object.kind
        )));
    }
    Ok(String::from_utf8(object.data).ok())
}

fn is_regular_file_mode(mode: u32) -> bool {
    matches!(mode, 0o100644 | 0o100755)
}

fn conflict_marker_text(head_text: &str, target_text: &str, target_label: &str) -> String {
    let mut text = String::new();
    text.push_str("<<<<<<< HEAD\n");
    push_conflict_side(&mut text, head_text);
    text.push_str("=======\n");
    push_conflict_side(&mut text, target_text);
    text.push_str(">>>>>>> ");
    text.push_str(target_label);
    text.push('\n');
    text
}

fn push_conflict_side(output: &mut String, side_text: &str) {
    output.push_str(side_text);
    if !side_text.ends_with('\n') {
        output.push('\n');
    }
}

#[cfg(test)]
mod tests {
    use super::conflict_marker_text;

    #[test]
    fn conflict_marker_text_adds_boundaries_and_preserves_content() {
        assert_eq!(
            conflict_marker_text("head\n", "target", "topic"),
            "<<<<<<< HEAD\nhead\n=======\ntarget\n>>>>>>> topic\n"
        );
    }
}
