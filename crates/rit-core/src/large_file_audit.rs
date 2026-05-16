use crate::{ObjectId, ObjectKind, Repository, Result, RitError, object::parse_tree_entries};
use std::collections::{BTreeMap, BTreeSet, HashSet};

/// Conservative default threshold for large regular Git blobs.
pub const DEFAULT_LARGE_FILE_AUDIT_THRESHOLD: usize = 10 * 1024 * 1024;

/// Read-only report for large blobs reachable from HEAD history.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LargeFilesAuditReport {
    /// Blob size threshold used by the audit.
    pub threshold_bytes: usize,
    /// Large blobs found in commits reachable from HEAD.
    pub large_blobs: Vec<LargeBlobFinding>,
    /// Suggested tracking rules based on found paths.
    pub recommended_tracking: Vec<LargeFileTrackingRecommendation>,
    /// Safe migration plan. These are instructions only; audit never rewrites.
    pub migration_plan: Vec<LargeFileMigrationStep>,
}

/// One large regular Git blob found in history.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LargeBlobFinding {
    /// Repository-relative path where the blob appears.
    pub path: String,
    /// Blob object ID.
    pub object_id: ObjectId,
    /// Blob size in bytes.
    pub size: usize,
    /// Commit where the audit first observed this path/blob pair.
    pub first_seen_commit: ObjectId,
}

/// One recommended large-file tracking rule.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LargeFileTrackingRecommendation {
    /// Glob pattern to consider.
    pub pattern: String,
    /// Suggested backend name, such as `lfs` or `xet`.
    pub backend: String,
    /// Human-readable reason for the recommendation.
    pub reason: String,
}

/// One safe migration planning step.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LargeFileMigrationStep {
    /// One-based step number.
    pub step: usize,
    /// Action to review or perform.
    pub action: String,
    /// Whether this step rewrites repository history.
    pub rewrites_history: bool,
}

impl Repository {
    /// Audits HEAD history for large regular Git blobs using the default
    /// threshold.
    pub fn large_files_audit(&self) -> Result<LargeFilesAuditReport> {
        self.large_files_audit_with_threshold(DEFAULT_LARGE_FILE_AUDIT_THRESHOLD)
    }

    /// Audits HEAD history for large regular Git blobs using `threshold_bytes`.
    pub fn large_files_audit_with_threshold(
        &self,
        threshold_bytes: usize,
    ) -> Result<LargeFilesAuditReport> {
        let mut large_blobs = Vec::new();
        let mut seen_commits = HashSet::new();
        let mut seen_path_blobs = HashSet::new();
        let mut stack = self.resolve_head()?.into_iter().collect::<Vec<_>>();

        while let Some(commit_id) = stack.pop() {
            if !seen_commits.insert(commit_id) {
                continue;
            }
            let commit_object = self.read_object(commit_id)?;
            if commit_object.kind != ObjectKind::Commit {
                continue;
            }
            let commit = crate::parse_commit(&commit_object.data)?;
            stack.extend(commit.parents.iter().copied());
            let entries = self.audit_tree_blob_entries(commit.tree)?;
            for (path, object_id) in entries {
                if !seen_path_blobs.insert((path.clone(), object_id)) {
                    continue;
                }
                let object = self.read_object(object_id)?;
                if object.kind != ObjectKind::Blob {
                    return Err(RitError::invalid_input(format!(
                        "tree entry {path} points at {}, not blob",
                        object.kind
                    )));
                }
                if object.data.len() >= threshold_bytes {
                    large_blobs.push(LargeBlobFinding {
                        path,
                        object_id,
                        size: object.data.len(),
                        first_seen_commit: commit_id,
                    });
                }
            }
        }

        large_blobs.sort_by(|left, right| {
            right
                .size
                .cmp(&left.size)
                .then_with(|| left.path.cmp(&right.path))
                .then_with(|| left.object_id.as_bytes().cmp(right.object_id.as_bytes()))
        });
        let recommended_tracking = recommended_tracking(&large_blobs);
        let migration_plan = migration_plan(&large_blobs, &recommended_tracking);
        Ok(LargeFilesAuditReport {
            threshold_bytes,
            large_blobs,
            recommended_tracking,
            migration_plan,
        })
    }

    fn audit_tree_blob_entries(&self, tree_id: ObjectId) -> Result<BTreeMap<String, ObjectId>> {
        let mut entries = BTreeMap::new();
        self.collect_audit_tree_blob_entries("", tree_id, &mut entries)?;
        Ok(entries)
    }

    fn collect_audit_tree_blob_entries(
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
        for entry in parse_tree_entries(&tree.data)? {
            let entry_name = String::from_utf8_lossy(&entry.name);
            let path = if prefix.is_empty() {
                entry_name.into_owned()
            } else {
                format!("{prefix}/{entry_name}")
            };
            if entry.kind == ObjectKind::Tree {
                self.collect_audit_tree_blob_entries(&path, entry.object_id, output)?;
            } else {
                output.insert(path, entry.object_id);
            }
        }
        Ok(())
    }
}

fn recommended_tracking(large_blobs: &[LargeBlobFinding]) -> Vec<LargeFileTrackingRecommendation> {
    let mut by_pattern = BTreeMap::<String, (String, BTreeSet<String>)>::new();
    for blob in large_blobs {
        let pattern = recommended_pattern(&blob.path);
        let backend = recommended_backend(&blob.path).to_owned();
        by_pattern
            .entry(pattern)
            .or_insert_with(|| (backend, BTreeSet::new()))
            .1
            .insert(blob.path.clone());
    }
    by_pattern
        .into_iter()
        .map(
            |(pattern, (backend, examples))| LargeFileTrackingRecommendation {
                pattern,
                backend: backend.clone(),
                reason: format!(
                    "large blobs found at {}",
                    examples.into_iter().collect::<Vec<_>>().join(", ")
                ),
            },
        )
        .collect()
}

fn recommended_pattern(path: &str) -> String {
    path.rsplit_once('.')
        .map(|(_, extension)| format!("*.{extension}"))
        .unwrap_or_else(|| path.to_owned())
}

fn recommended_backend(path: &str) -> &'static str {
    match path
        .rsplit_once('.')
        .map(|(_, extension)| extension.to_ascii_lowercase())
    {
        Some(extension)
            if matches!(
                extension.as_str(),
                "safetensors" | "parquet" | "bin" | "onnx" | "pt" | "ckpt"
            ) =>
        {
            "xet"
        }
        _ => "lfs",
    }
}

fn migration_plan(
    large_blobs: &[LargeBlobFinding],
    recommendations: &[LargeFileTrackingRecommendation],
) -> Vec<LargeFileMigrationStep> {
    if large_blobs.is_empty() {
        return vec![LargeFileMigrationStep {
            step: 1,
            action: "No large regular Git blobs were found; no migration is needed.".to_owned(),
            rewrites_history: false,
        }];
    }
    let patterns = recommendations
        .iter()
        .map(|recommendation| format!("{} via {}", recommendation.pattern, recommendation.backend))
        .collect::<Vec<_>>()
        .join(", ");
    vec![
        LargeFileMigrationStep {
            step: 1,
            action: format!("Review proposed tracking rules: {patterns}."),
            rewrites_history: false,
        },
        LargeFileMigrationStep {
            step: 2,
            action: "Add tracking rules in .gitattributes only after review.".to_owned(),
            rewrites_history: false,
        },
        LargeFileMigrationStep {
            step: 3,
            action: "Plan any history rewrite separately with backups and team coordination."
                .to_owned(),
            rewrites_history: true,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::DEFAULT_LARGE_FILE_AUDIT_THRESHOLD;
    use crate::{InitOptions, Repository};
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn audit_reports_large_blobs_and_recommends_backend() {
        let root = temp_path("large-file-audit");
        let repository = Repository::init(&InitOptions::new(&root)).expect("repo should init");
        write_identity(&repository);
        fs::write(root.join("model.bin"), b"0123456789abc").expect("model should write");
        repository
            .add_paths(&["model.bin".to_owned()])
            .expect("file should add");
        repository
            .commit_index("model")
            .expect("commit should work");

        let report = repository
            .large_files_audit_with_threshold(10)
            .expect("audit should work");

        assert_eq!(report.threshold_bytes, 10);
        assert_eq!(report.large_blobs.len(), 1);
        assert_eq!(report.large_blobs[0].path, "model.bin");
        assert_eq!(report.recommended_tracking[0].pattern, "*.bin");
        assert_eq!(report.recommended_tracking[0].backend, "xet");
        assert!(
            report
                .migration_plan
                .iter()
                .any(|step| step.rewrites_history)
        );
        remove_dir_all(&root);
    }

    #[test]
    fn audit_empty_history_reports_no_migration_needed() {
        let root = temp_path("large-file-audit-empty");
        let repository = Repository::init(&InitOptions::new(&root)).expect("repo should init");

        let report = repository
            .large_files_audit()
            .expect("empty audit should work");

        assert_eq!(report.threshold_bytes, DEFAULT_LARGE_FILE_AUDIT_THRESHOLD);
        assert!(report.large_blobs.is_empty());
        assert!(!report.migration_plan[0].rewrites_history);
        remove_dir_all(&root);
    }

    fn write_identity(repository: &Repository) {
        fs::write(
            repository.git_dir().join("config"),
            "[core]\n\trepositoryformatversion = 0\n\tbare = false\n[user]\n\tname = Rit Test\n\temail = rit@example.test\n",
        )
        .expect("config should write");
    }

    fn temp_path(name: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("rit-{name}-{unique}"))
    }

    fn remove_dir_all(path: &Path) {
        if path.exists() {
            fs::remove_dir_all(path).expect("temporary directory should be removed");
        }
    }
}
