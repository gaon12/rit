use crate::{
    Commit, ObjectId, ObjectKind, PathspecSet, Repository, Result, RitError, parse_commit,
};
use std::collections::{BTreeMap, BTreeSet};

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
        self.log_first_parent_with_pathspecs(&PathspecSet::all())
    }

    /// Reads first-parent commits from `HEAD` that touch matching paths.
    pub fn log_first_parent_with_pathspecs(
        &self,
        pathspecs: &PathspecSet,
    ) -> Result<Vec<LogEntry>> {
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
            if pathspecs.is_all() || self.commit_touches_pathspecs(&commit, pathspecs)? {
                entries.push(LogEntry { object_id, commit });
            }
        }

        Ok(entries)
    }

    /// Returns whether one commit changes any path matched by `pathspecs`.
    pub fn commit_touches_pathspecs(
        &self,
        commit: &Commit,
        pathspecs: &PathspecSet,
    ) -> Result<bool> {
        let current_entries = self.tree_blob_entries(commit.tree)?;
        let parent_entries = if let Some(parent_id) = commit.parents.first() {
            let parent_object = self.read_object(*parent_id)?;
            if parent_object.kind != ObjectKind::Commit {
                return Err(RitError::invalid_input(format!(
                    "parent {parent_id} is {}, not commit",
                    parent_object.kind
                )));
            }
            let parent_commit = parse_commit(&parent_object.data)?;
            self.tree_blob_entries(parent_commit.tree)?
        } else {
            BTreeMap::new()
        };
        let paths = current_entries
            .keys()
            .chain(parent_entries.keys())
            .filter(|path| pathspecs.matches(path))
            .collect::<BTreeSet<_>>();

        Ok(paths
            .into_iter()
            .any(|path| current_entries.get(path) != parent_entries.get(path)))
    }

    fn tree_blob_entries(&self, tree_id: ObjectId) -> Result<BTreeMap<String, ObjectId>> {
        let mut entries = BTreeMap::new();
        self.collect_tree_blob_entries("", tree_id, &mut entries)?;
        Ok(entries)
    }

    fn collect_tree_blob_entries(
        &self,
        prefix: &str,
        tree_id: ObjectId,
        output: &mut BTreeMap<String, ObjectId>,
    ) -> Result<()> {
        let tree = self.read_object(tree_id)?;
        if tree.kind != ObjectKind::Tree {
            return Err(RitError::invalid_input(format!(
                "object {tree_id} is {}, not tree",
                tree.kind
            )));
        }
        for entry in crate::object::parse_tree_entries(&tree.data)? {
            let path = if prefix.is_empty() {
                entry.name_lossy()
            } else {
                format!("{prefix}/{}", entry.name_lossy())
            };
            if entry.kind == ObjectKind::Tree {
                self.collect_tree_blob_entries(&path, entry.object_id, output)?;
            } else {
                output.insert(path, entry.object_id);
            }
        }
        Ok(())
    }
}
