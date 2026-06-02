use crate::Repository;
use std::fs;
use std::path::{Path, PathBuf};

/// Read-only repository size and object-shape summary for `rit doctor --sizer`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DoctorSizerReport {
    /// Repository `.git` directory.
    pub git_dir: String,
    /// Shared Git directory used by linked worktrees.
    pub common_dir: String,
    /// Git object storage summary.
    pub objects: DoctorObjectSizer,
    /// Loose refs summary.
    pub refs: DoctorDirectorySizer,
    /// Rit sidecar metadata summary.
    pub rit_metadata: DoctorDirectorySizer,
    /// Non-fatal inspection warnings.
    pub warnings: Vec<String>,
}

/// Object database size and shape summary.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DoctorObjectSizer {
    /// Number of fanout directories that contain loose object names.
    pub loose_fanout_directories: u64,
    /// Number of loose object files.
    pub loose_objects: u64,
    /// Total on-disk bytes used by loose object files.
    pub loose_object_bytes: u64,
    /// Largest loose object file, if one exists.
    pub largest_loose_object: Option<DoctorSizedPath>,
    /// Number of `.pack` files.
    pub pack_files: u64,
    /// Total bytes used by `.pack` files.
    pub pack_bytes: u64,
    /// Number of `.idx` pack index files.
    pub pack_indexes: u64,
    /// Total bytes used by `.idx` pack index files.
    pub pack_index_bytes: u64,
    /// Number of auxiliary pack files such as `.bitmap`, `.rev`, or `.mtimes`.
    pub auxiliary_pack_files: u64,
    /// Total bytes used by auxiliary pack files.
    pub auxiliary_pack_bytes: u64,
}

/// Recursive directory size summary.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DoctorDirectorySizer {
    /// Directory path that was inspected.
    pub path: String,
    /// Whether the directory exists.
    pub exists: bool,
    /// Number of regular files under the directory.
    pub files: u64,
    /// Number of child directories under the directory.
    pub directories: u64,
    /// Total regular-file bytes under the directory.
    pub bytes: u64,
}

/// One path with an associated byte size.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DoctorSizedPath {
    /// Path that was measured.
    pub path: String,
    /// File size in bytes.
    pub bytes: u64,
}

impl Repository {
    /// Builds a read-only repository size and object-shape audit.
    pub fn doctor_sizer(&self) -> DoctorSizerReport {
        let mut warnings = Vec::new();
        let objects_dir = self.common_dir().join("objects");
        let pack_dir = objects_dir.join("pack");
        let refs_dir = self.common_dir().join("refs");
        let rit_metadata_dir = self.git_dir().join("rit");

        let objects = scan_loose_objects(&objects_dir, &mut warnings)
            .with_pack_summary(scan_pack_directory(&pack_dir, &mut warnings));
        let refs = scan_directory_tree(&refs_dir, &mut warnings, "refs");
        let rit_metadata = scan_directory_tree(&rit_metadata_dir, &mut warnings, "rit metadata");

        DoctorSizerReport {
            git_dir: path_to_string(self.git_dir()),
            common_dir: path_to_string(self.common_dir()),
            objects,
            refs,
            rit_metadata,
            warnings,
        }
    }
}

impl DoctorObjectSizer {
    fn with_pack_summary(mut self, pack_summary: PackSummary) -> Self {
        self.pack_files = pack_summary.pack_files;
        self.pack_bytes = pack_summary.pack_bytes;
        self.pack_indexes = pack_summary.pack_indexes;
        self.pack_index_bytes = pack_summary.pack_index_bytes;
        self.auxiliary_pack_files = pack_summary.auxiliary_pack_files;
        self.auxiliary_pack_bytes = pack_summary.auxiliary_pack_bytes;
        self
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct PackSummary {
    pack_files: u64,
    pack_bytes: u64,
    pack_indexes: u64,
    pack_index_bytes: u64,
    auxiliary_pack_files: u64,
    auxiliary_pack_bytes: u64,
}

fn scan_loose_objects(objects_dir: &Path, warnings: &mut Vec<String>) -> DoctorObjectSizer {
    let mut summary = DoctorObjectSizer::default();
    for fanout_dir in sorted_child_paths(objects_dir, warnings, "objects") {
        let Some(name) = fanout_dir.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if !fanout_dir.is_dir()
            || name.len() != 2
            || !name.chars().all(|character| character.is_ascii_hexdigit())
        {
            continue;
        }

        summary.loose_fanout_directories += 1;
        for object_path in sorted_child_paths(&fanout_dir, warnings, "loose objects") {
            let Ok(metadata) = fs::metadata(&object_path) else {
                warnings.push(format!(
                    "could not read metadata for {}",
                    object_path.display()
                ));
                continue;
            };
            if !metadata.is_file() {
                continue;
            }

            let object_size = metadata.len();
            summary.loose_objects += 1;
            summary.loose_object_bytes += object_size;
            update_largest_path(&mut summary.largest_loose_object, &object_path, object_size);
        }
    }
    summary
}

fn scan_pack_directory(pack_dir: &Path, warnings: &mut Vec<String>) -> PackSummary {
    let mut summary = PackSummary::default();
    for pack_path in sorted_child_paths(pack_dir, warnings, "pack directory") {
        let Ok(metadata) = fs::metadata(&pack_path) else {
            warnings.push(format!(
                "could not read metadata for {}",
                pack_path.display()
            ));
            continue;
        };
        if !metadata.is_file() {
            continue;
        }

        match pack_path
            .extension()
            .and_then(|extension| extension.to_str())
        {
            Some("pack") => {
                summary.pack_files += 1;
                summary.pack_bytes += metadata.len();
            }
            Some("idx") => {
                summary.pack_indexes += 1;
                summary.pack_index_bytes += metadata.len();
            }
            Some("bitmap" | "rev" | "mtimes") => {
                summary.auxiliary_pack_files += 1;
                summary.auxiliary_pack_bytes += metadata.len();
            }
            _ => {}
        }
    }
    summary
}

fn scan_directory_tree(
    directory: &Path,
    warnings: &mut Vec<String>,
    label: &str,
) -> DoctorDirectorySizer {
    let mut summary = DoctorDirectorySizer {
        path: path_to_string(directory),
        exists: directory.is_dir(),
        files: 0,
        directories: 0,
        bytes: 0,
    };
    if !summary.exists {
        return summary;
    }

    scan_directory_tree_into(directory, warnings, label, &mut summary);
    summary
}

fn scan_directory_tree_into(
    directory: &Path,
    warnings: &mut Vec<String>,
    label: &str,
    summary: &mut DoctorDirectorySizer,
) {
    for child_path in sorted_child_paths(directory, warnings, label) {
        let Ok(metadata) = fs::metadata(&child_path) else {
            warnings.push(format!(
                "could not read metadata for {}",
                child_path.display()
            ));
            continue;
        };
        if metadata.is_dir() {
            summary.directories += 1;
            scan_directory_tree_into(&child_path, warnings, label, summary);
        } else if metadata.is_file() {
            summary.files += 1;
            summary.bytes += metadata.len();
        }
    }
}

fn sorted_child_paths(directory: &Path, warnings: &mut Vec<String>, label: &str) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(directory) else {
        warnings.push(format!(
            "could not inspect {label} at {}",
            directory.display()
        ));
        return Vec::new();
    };

    let mut paths = Vec::new();
    for entry in entries {
        match entry {
            Ok(entry) => paths.push(entry.path()),
            Err(error) => warnings.push(format!(
                "could not read an entry in {label} at {}: {error}",
                directory.display()
            )),
        }
    }
    paths.sort();
    paths
}

fn update_largest_path(largest_path: &mut Option<DoctorSizedPath>, path: &Path, bytes: u64) {
    let should_update = largest_path
        .as_ref()
        .is_none_or(|current| bytes > current.bytes);
    if should_update {
        *largest_path = Some(DoctorSizedPath {
            path: path_to_string(path),
            bytes,
        });
    }
}

fn path_to_string(path: &Path) -> String {
    path.display().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::InitOptions;

    #[test]
    fn doctor_sizer_reports_loose_pack_and_ref_shapes() {
        let root = temp_path("doctor-sizer");
        let repository = Repository::init(&InitOptions::new(&root)).expect("repo should init");
        let loose_dir = repository.common_dir().join("objects").join("ab");
        fs::create_dir_all(&loose_dir).expect("loose dir should be created");
        fs::write(loose_dir.join("1234"), [1, 2, 3, 4]).expect("loose object should be written");
        fs::write(loose_dir.join("5678"), [1, 2]).expect("loose object should be written");
        let pack_dir = repository.common_dir().join("objects").join("pack");
        fs::write(pack_dir.join("pack-test.pack"), [1; 5]).expect("pack should be written");
        fs::write(pack_dir.join("pack-test.idx"), [1; 3]).expect("idx should be written");
        fs::write(
            repository
                .common_dir()
                .join("refs")
                .join("heads")
                .join("main"),
            "abc\n",
        )
        .expect("ref should be written");

        let report = repository.doctor_sizer();

        assert_eq!(report.objects.loose_fanout_directories, 1);
        assert_eq!(report.objects.loose_objects, 2);
        assert_eq!(report.objects.loose_object_bytes, 6);
        assert_eq!(report.objects.pack_files, 1);
        assert_eq!(report.objects.pack_bytes, 5);
        assert_eq!(report.objects.pack_indexes, 1);
        assert_eq!(report.objects.pack_index_bytes, 3);
        assert_eq!(report.refs.files, 1);
        assert_eq!(report.refs.bytes, 4);
        assert!(report.warnings.is_empty());
        let _ = fs::remove_dir_all(root);
    }

    fn temp_path(name: &str) -> std::path::PathBuf {
        let suffix = std::process::id();
        let path = std::env::temp_dir().join(format!("rit-{name}-{suffix}"));
        let _ = fs::remove_dir_all(&path);
        path
    }
}
