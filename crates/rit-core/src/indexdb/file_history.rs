use super::{object_id_from_snapshot_bytes, sqlite_error};
use crate::{
    ObjectId, ObjectKind, Repository, Result, RitError, object::parse_tree_entries, parse_commit,
};
use rusqlite::{Connection, params};
use std::collections::{BTreeMap, BTreeSet};

/// One path change row read from the optional SQLite auxiliary index.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IndexedFileChange {
    /// Commit that introduced the path state change.
    pub commit_id: ObjectId,
    /// Repository-relative path.
    pub path: String,
    /// Simple first-parent change kind: `A`, `M`, or `D`.
    pub change_kind: String,
    /// Blob object ID after the change, absent for deletes.
    pub object_id: Option<ObjectId>,
    /// File mode after the change, absent for deletes.
    pub mode: Option<u32>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct IndexedTreeEntry {
    object_id: ObjectId,
    mode: u32,
}

pub(super) fn refresh_file_changes_for_commit(
    connection: &Connection,
    repository: &Repository,
    object_id: ObjectId,
    commit: &crate::Commit,
) -> Result<()> {
    connection
        .execute(
            "DELETE FROM file_changes WHERE hash_kind = ?1 AND object_id = ?2",
            params!["sha1", object_id.as_bytes().to_vec()],
        )
        .map_err(sqlite_error)?;
    let current = commit_tree_entries(repository, commit.tree)?;
    let parent = commit
        .parents
        .first()
        .copied()
        .map(|parent_id| {
            let parent_object = repository.read_object(parent_id)?;
            let parent_commit = parse_commit(&parent_object.data)?;
            commit_tree_entries(repository, parent_commit.tree)
        })
        .transpose()?
        .unwrap_or_default();
    let paths = current
        .keys()
        .chain(parent.keys())
        .cloned()
        .collect::<BTreeSet<_>>();

    for path in paths {
        let before = parent.get(&path);
        let after = current.get(&path);
        let change_kind = match (before, after) {
            (None, Some(_)) => "A",
            (Some(_), None) => "D",
            (Some(before), Some(after)) if before != after => "M",
            _ => continue,
        };
        insert_file_change(connection, object_id, &path, change_kind, after)?;
    }
    Ok(())
}

pub(super) fn indexed_file_history(
    connection: &Connection,
    path: &str,
) -> Result<Vec<IndexedFileChange>> {
    let mut statement = connection
        .prepare(
            "SELECT
                file_changes.object_id, file_changes.path, file_changes.change_kind,
                file_changes.path_object_id, file_changes.mode
             FROM file_changes
             JOIN commits
               ON commits.hash_kind = file_changes.hash_kind
              AND commits.object_id = file_changes.object_id
             WHERE file_changes.hash_kind = 'sha1' AND file_changes.path = ?1
             ORDER BY commits.committer_timestamp DESC, file_changes.object_id DESC",
        )
        .map_err(sqlite_error)?;
    let rows = statement
        .query_map(params![path], |row| {
            Ok((
                row.get::<_, Vec<u8>>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Option<Vec<u8>>>(3)?,
                row.get::<_, Option<i64>>(4)?,
            ))
        })
        .map_err(sqlite_error)?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(sqlite_error)?
        .into_iter()
        .map(
            |(commit_bytes, path, change_kind, object_bytes, mode)| -> Result<_> {
                let commit_id =
                    object_id_from_snapshot_bytes(Some(&commit_bytes)).ok_or_else(|| {
                        RitError::invalid_input("indexdb file change contains an invalid commit id")
                    })?;
                let object_id = object_bytes
                    .as_deref()
                    .map(|bytes| {
                        object_id_from_snapshot_bytes(Some(bytes)).ok_or_else(|| {
                            RitError::invalid_input(
                                "indexdb file change contains an invalid object id",
                            )
                        })
                    })
                    .transpose()?;
                Ok(IndexedFileChange {
                    commit_id,
                    path,
                    change_kind,
                    object_id,
                    mode: mode.map(|value| value as u32),
                })
            },
        )
        .collect()
}

pub(super) fn canonical_file_history(
    repository: &Repository,
    path: &str,
) -> Result<Vec<IndexedFileChange>> {
    let mut history = Vec::new();
    let Some(mut commit_id) = repository.resolve_head()? else {
        return Ok(history);
    };

    loop {
        let commit_object = repository.read_object(commit_id)?;
        if commit_object.kind != ObjectKind::Commit {
            break;
        }
        let commit = parse_commit(&commit_object.data)?;
        let current = commit_tree_entries(repository, commit.tree)?;
        let parent_entries = commit
            .parents
            .first()
            .copied()
            .map(|parent_id| {
                let parent_object = repository.read_object(parent_id)?;
                let parent_commit = parse_commit(&parent_object.data)?;
                commit_tree_entries(repository, parent_commit.tree)
            })
            .transpose()?
            .unwrap_or_default();
        let before = parent_entries.get(path);
        let after = current.get(path);
        let change_kind = match (before, after) {
            (None, Some(_)) => Some("A"),
            (Some(_), None) => Some("D"),
            (Some(before), Some(after)) if before != after => Some("M"),
            _ => None,
        };
        if let Some(change_kind) = change_kind {
            history.push(IndexedFileChange {
                commit_id,
                path: path.to_owned(),
                change_kind: change_kind.to_owned(),
                object_id: after.map(|entry| entry.object_id),
                mode: after.map(|entry| entry.mode),
            });
        }

        let Some(parent_id) = commit.parents.first().copied() else {
            break;
        };
        commit_id = parent_id;
    }

    Ok(history)
}

fn commit_tree_entries(
    repository: &Repository,
    tree_id: ObjectId,
) -> Result<BTreeMap<String, IndexedTreeEntry>> {
    let mut entries = BTreeMap::new();
    collect_commit_tree_entries(repository, "", tree_id, &mut entries)?;
    Ok(entries)
}

fn collect_commit_tree_entries(
    repository: &Repository,
    prefix: &str,
    tree_id: ObjectId,
    output: &mut BTreeMap<String, IndexedTreeEntry>,
) -> Result<()> {
    let tree = repository.read_object(tree_id)?;
    for entry in parse_tree_entries(&tree.data)? {
        let path = if prefix.is_empty() {
            entry.name_lossy()
        } else {
            format!("{prefix}/{}", entry.name_lossy())
        };
        if entry.kind == ObjectKind::Tree {
            collect_commit_tree_entries(repository, &path, entry.object_id, output)?;
        } else {
            output.insert(
                path,
                IndexedTreeEntry {
                    object_id: entry.object_id,
                    mode: parse_tree_mode(&entry.mode)?,
                },
            );
        }
    }
    Ok(())
}

fn parse_tree_mode(mode: &str) -> Result<u32> {
    u32::from_str_radix(mode, 8)
        .map_err(|_| RitError::invalid_input(format!("invalid tree mode: {mode}")))
}

fn insert_file_change(
    connection: &Connection,
    commit_id: ObjectId,
    path: &str,
    change_kind: &str,
    after: Option<&IndexedTreeEntry>,
) -> Result<()> {
    connection
        .execute(
            "INSERT OR REPLACE INTO file_changes(
                hash_kind, object_id, path, change_kind, path_hash_kind,
                path_object_id, mode
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                "sha1",
                commit_id.as_bytes().to_vec(),
                path,
                change_kind,
                after.map(|_| "sha1"),
                after.map(|entry| entry.object_id.as_bytes().to_vec()),
                after.map(|entry| entry.mode as i64),
            ],
        )
        .map_err(sqlite_error)?;
    Ok(())
}
