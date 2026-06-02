use super::{object_id_from_snapshot_bytes, sqlite_error};
use crate::{
    ObjectId, ObjectKind, Repository, Result, RitError, TreeEntry, object::parse_tree_entries,
    parse_commit,
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
    let parent_tree = commit
        .parents
        .first()
        .copied()
        .map(|parent_id| {
            let parent_object = repository.read_object(parent_id)?;
            let parent_commit = parse_commit(&parent_object.data)?;
            Ok(parent_commit.tree)
        })
        .transpose()?;

    match parent_tree {
        Some(parent_tree) => {
            refresh_file_changes_between_trees(
                connection,
                repository,
                object_id,
                "",
                parent_tree,
                commit.tree,
            )?;
        }
        None => {
            insert_added_tree_changes(connection, repository, object_id, "", commit.tree)?;
        }
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

fn refresh_file_changes_between_trees(
    connection: &Connection,
    repository: &Repository,
    commit_id: ObjectId,
    prefix: &str,
    before_tree_id: ObjectId,
    after_tree_id: ObjectId,
) -> Result<()> {
    if before_tree_id == after_tree_id {
        return Ok(());
    }

    let before_entries = direct_tree_entries(repository, before_tree_id)?;
    let after_entries = direct_tree_entries(repository, after_tree_id)?;
    let entry_names = before_entries
        .keys()
        .chain(after_entries.keys())
        .cloned()
        .collect::<BTreeSet<_>>();

    for name in entry_names {
        let path = joined_tree_path(prefix, &name);
        match (before_entries.get(&name), after_entries.get(&name)) {
            (None, Some(after)) => {
                insert_added_entry_changes(connection, repository, commit_id, &path, after)?;
            }
            (Some(before), None) => {
                insert_deleted_entry_changes(connection, repository, commit_id, &path, before)?;
            }
            (Some(before), Some(after)) => {
                refresh_file_changes_for_entry_pair(
                    connection, repository, commit_id, &path, before, after,
                )?;
            }
            (None, None) => {}
        }
    }
    Ok(())
}

fn refresh_file_changes_for_entry_pair(
    connection: &Connection,
    repository: &Repository,
    commit_id: ObjectId,
    path: &str,
    before: &TreeEntry,
    after: &TreeEntry,
) -> Result<()> {
    match (before.kind, after.kind) {
        (ObjectKind::Tree, ObjectKind::Tree) => refresh_file_changes_between_trees(
            connection,
            repository,
            commit_id,
            path,
            before.object_id,
            after.object_id,
        ),
        (ObjectKind::Tree, _) => {
            insert_deleted_entry_changes(connection, repository, commit_id, path, before)?;
            insert_added_entry_changes(connection, repository, commit_id, path, after)
        }
        (_, ObjectKind::Tree) => {
            insert_deleted_entry_changes(connection, repository, commit_id, path, before)?;
            insert_added_entry_changes(connection, repository, commit_id, path, after)
        }
        (_, _) => {
            let before_index_entry = indexed_entry_from_tree_entry(before)?;
            let after_index_entry = indexed_entry_from_tree_entry(after)?;
            if before_index_entry == after_index_entry {
                return Ok(());
            }
            insert_file_change(connection, commit_id, path, "M", Some(&after_index_entry))
        }
    }
}

fn insert_added_entry_changes(
    connection: &Connection,
    repository: &Repository,
    commit_id: ObjectId,
    path: &str,
    entry: &TreeEntry,
) -> Result<()> {
    if entry.kind == ObjectKind::Tree {
        insert_added_tree_changes(connection, repository, commit_id, path, entry.object_id)
    } else {
        let after = indexed_entry_from_tree_entry(entry)?;
        insert_file_change(connection, commit_id, path, "A", Some(&after))
    }
}

fn insert_deleted_entry_changes(
    connection: &Connection,
    repository: &Repository,
    commit_id: ObjectId,
    path: &str,
    entry: &TreeEntry,
) -> Result<()> {
    if entry.kind == ObjectKind::Tree {
        insert_deleted_tree_changes(connection, repository, commit_id, path, entry.object_id)
    } else {
        insert_file_change(connection, commit_id, path, "D", None)
    }
}

fn insert_added_tree_changes(
    connection: &Connection,
    repository: &Repository,
    commit_id: ObjectId,
    prefix: &str,
    tree_id: ObjectId,
) -> Result<()> {
    for entry in direct_tree_entries(repository, tree_id)?.values() {
        let path = joined_tree_path(prefix, &entry.name_lossy());
        insert_added_entry_changes(connection, repository, commit_id, &path, entry)?;
    }
    Ok(())
}

fn insert_deleted_tree_changes(
    connection: &Connection,
    repository: &Repository,
    commit_id: ObjectId,
    prefix: &str,
    tree_id: ObjectId,
) -> Result<()> {
    for entry in direct_tree_entries(repository, tree_id)?.values() {
        let path = joined_tree_path(prefix, &entry.name_lossy());
        insert_deleted_entry_changes(connection, repository, commit_id, &path, entry)?;
    }
    Ok(())
}

fn direct_tree_entries(
    repository: &Repository,
    tree_id: ObjectId,
) -> Result<BTreeMap<String, TreeEntry>> {
    let tree = repository.read_object(tree_id)?;
    parse_tree_entries(&tree.data).map(|entries| {
        entries
            .into_iter()
            .map(|entry| (entry.name_lossy(), entry))
            .collect()
    })
}

fn indexed_entry_from_tree_entry(entry: &TreeEntry) -> Result<IndexedTreeEntry> {
    Ok(IndexedTreeEntry {
        object_id: entry.object_id,
        mode: parse_tree_mode(&entry.mode)?,
    })
}

fn joined_tree_path(prefix: &str, name: &str) -> String {
    if prefix.is_empty() {
        name.to_owned()
    } else {
        format!("{prefix}/{name}")
    }
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
