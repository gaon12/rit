//! Core library for `rit`.
//!
//! This crate owns Git-compatible data models and repository operations. The
//! CLI crate should format these structured results instead of embedding Git
//! behavior directly in argument handling.

pub mod attributes;
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
pub mod transport;
pub mod write;

pub use error::{Result, RitError};
pub use history::LogEntry;
pub use index::{
    CacheTree, CacheTreeNode, EndOfIndexEntry, EwahBitmap, FsMonitor, FsMonitorToken, Index,
    IndexEntry, IndexEntryOffset, IndexEntryOffsetTable, IndexExtension, IndexExtensionKind,
    ResolveUndo, ResolveUndoEntry, ResolveUndoStage, SparseDirectory, SplitIndexLink,
    UntrackedCache, UntrackedCacheDirectoryBlock, UntrackedCacheStat, UntrackedCacheTail,
};
pub use object::{GitObject, ObjectId, ObjectKind, TreeEntry, hash_object};
pub use odb::LooseObjectDb;
pub use pathspec::PathspecSet;
pub use refs::{Branch, Tag};
pub use repository::{
    InitOptions, LocalCloneOptions, LocalFetchOptions, LocalFetchResult, Repository,
};
pub use status::{
    PorcelainStatus, StatusBranchHeader, StatusEntry, StatusOptions, UntrackedFilesMode,
};
pub use transport::{TransportLocation, TransportProtocol};
pub use write::{
    AddOptions, CommitOptions, CommitResult, FileModeOverride, SignatureIdentity, SignatureTime,
};

/// Returns the crate version used by the CLI and tests.
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}
pub use attributes::{
    AttributeAssignment, AttributeMacro, AttributeRule, AttributeState, GitAttributes,
};
pub use commit::{Commit, Signature, parse_commit};
pub use config::{GitConfig, GitConfigEntry};
pub use diff::{DiffFileStat, DiffPatch, DiffPatchFile, DiffSummary};
