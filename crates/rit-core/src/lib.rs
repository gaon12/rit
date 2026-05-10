//! Core library for `rit`.
//!
//! This crate owns Git-compatible data models and repository operations. The
//! CLI crate should format these structured results instead of embedding Git
//! behavior directly in argument handling.

pub mod commit;
pub mod config;
pub mod diff;
pub mod error;
pub mod history;
pub mod index;
pub mod object;
pub mod odb;
pub mod pathspec;
pub mod refs;
pub mod repository;
pub mod status;
pub mod write;

pub use error::{Result, RitError};
pub use history::LogEntry;
pub use index::{Index, IndexEntry};
pub use object::{GitObject, ObjectId, ObjectKind, TreeEntry, hash_object};
pub use odb::LooseObjectDb;
pub use pathspec::PathspecSet;
pub use refs::{Branch, Tag};
pub use repository::{InitOptions, Repository};
pub use status::{
    PorcelainStatus, StatusBranchHeader, StatusEntry, StatusOptions, UntrackedFilesMode,
};
pub use write::{
    AddOptions, CommitOptions, CommitResult, FileModeOverride, SignatureIdentity, SignatureTime,
};

/// Returns the crate version used by the CLI and tests.
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}
pub use commit::{Commit, Signature, parse_commit};
pub use config::{GitConfig, GitConfigEntry};
pub use diff::{DiffFileStat, DiffPatch, DiffPatchFile, DiffSummary};
