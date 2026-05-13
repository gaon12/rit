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
        let Some(target) = conflict.target else {
            continue;
        };
        let Some(head) = conflict.head else {
            write_available_side(repository, worktree, conflict, target, symlinks_enabled)?;
            continue;
        };
        if write_distinct_type_conflict_sides(
            repository,
            worktree,
            conflict,
            head,
            target,
            target_label,
            symlinks_enabled,
        )? {
            continue;
        }
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

fn write_available_side(
    repository: &Repository,
    worktree: &Path,
    conflict: &MergeConflictStagePlan,
    target: MergeConflictStageEntry,
    symlinks_enabled: bool,
) -> Result<()> {
    let object = repository.read_object(target.object_id)?;
    if object.kind != ObjectKind::Blob {
        return Err(RitError::invalid_input(format!(
            "object {} is {}, not blob",
            target.object_id, object.kind
        )));
    }
    write_worktree_entry_atomically(
        &join_slash_path(worktree, &conflict.path),
        &object.data,
        target.mode,
        symlinks_enabled,
    )
}

fn write_distinct_type_conflict_sides(
    repository: &Repository,
    worktree: &Path,
    conflict: &MergeConflictStagePlan,
    head: MergeConflictStageEntry,
    target: MergeConflictStageEntry,
    target_label: &str,
    symlinks_enabled: bool,
) -> Result<bool> {
    if !entry_modes_have_distinct_file_types(head.mode, target.mode) {
        return Ok(false);
    }

    if is_regular_file_mode(head.mode) && !is_regular_file_mode(target.mode) {
        write_stage_entry_at_path(
            repository,
            worktree,
            &conflict.path,
            target,
            symlinks_enabled,
        )?;
        write_stage_entry_at_path(
            repository,
            worktree,
            &head_side_conflict_path(&conflict.path),
            head,
            symlinks_enabled,
        )?;
        return Ok(true);
    }

    if !is_regular_file_mode(head.mode) && is_regular_file_mode(target.mode) {
        write_stage_entry_at_path(repository, worktree, &conflict.path, head, symlinks_enabled)?;
        write_stage_entry_at_path(
            repository,
            worktree,
            &target_side_conflict_path(&conflict.path, target_label),
            target,
            symlinks_enabled,
        )?;
        return Ok(true);
    }

    Ok(false)
}

fn write_stage_entry_at_path(
    repository: &Repository,
    worktree: &Path,
    path: &str,
    entry: MergeConflictStageEntry,
    symlinks_enabled: bool,
) -> Result<()> {
    let object = repository.read_object(entry.object_id)?;
    if object.kind != ObjectKind::Blob {
        return Err(RitError::invalid_input(format!(
            "object {} is {}, not blob",
            entry.object_id, object.kind
        )));
    }
    write_worktree_entry_atomically(
        &join_slash_path(worktree, path),
        &object.data,
        entry.mode,
        symlinks_enabled,
    )
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
    if object.data.contains(&0) {
        return Ok(None);
    }
    Ok(String::from_utf8(object.data).ok())
}

fn is_regular_file_mode(mode: u32) -> bool {
    matches!(mode, 0o100644 | 0o100755)
}

fn entry_modes_have_distinct_file_types(left_mode: u32, right_mode: u32) -> bool {
    file_type_from_mode(left_mode) != file_type_from_mode(right_mode)
}

fn file_type_from_mode(mode: u32) -> FileTypeFromMode {
    if is_regular_file_mode(mode) {
        FileTypeFromMode::Regular
    } else if mode == 0o120000 {
        FileTypeFromMode::Symlink
    } else {
        FileTypeFromMode::Other
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FileTypeFromMode {
    Regular,
    Symlink,
    Other,
}

fn head_side_conflict_path(path: &str) -> String {
    format!("{path}~HEAD")
}

fn target_side_conflict_path(path: &str, target_label: &str) -> String {
    format!("{path}~{}", conflict_path_label(target_label))
}

fn conflict_path_label(label: &str) -> String {
    label
        .chars()
        .map(|character| match character {
            '/' | '\\' => '_',
            _ => character,
        })
        .collect()
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
