//! Core library for `rit`.
//!
//! This crate owns Git-compatible data models and repository operations. The
//! CLI crate should format these structured results instead of embedding Git
//! behavior directly in argument handling.

pub mod attributes;
pub mod auth;
pub mod commit;
pub mod config;
pub mod diff;
pub mod error;
pub mod history;
pub mod index;
#[cfg(any(feature = "lfs", feature = "xet"))]
pub mod large_files;
pub mod merge_state;
pub mod object;
pub mod odb;
pub mod partial_clone;
pub mod pathspec;
pub mod refs;
pub mod repository;
pub mod sparse;
pub mod status;
pub mod transport;
pub mod workspace_profile;
pub mod write;

pub use error::{Result, RitError};
pub use history::LogEntry;
pub use index::{
    CacheTree, CacheTreeNode, EndOfIndexEntry, EwahBitmap, FsMonitor, FsMonitorToken, Index,
    IndexEntry, IndexEntryOffset, IndexEntryOffsetTable, IndexExtension, IndexExtensionKind,
    ResolveUndo, ResolveUndoEntry, ResolveUndoStage, SparseDirectory, SplitIndexLink,
    UntrackedCache, UntrackedCacheDirectoryBlock, UntrackedCacheStat, UntrackedCacheTail,
};
#[cfg(feature = "lfs")]
pub use large_files::{
    GitLfsBackend, LFS_BATCH_MEDIA_TYPE, LfsBatchAction, LfsBatchObject, LfsBatchObjectError,
    LfsBatchObjectResponse, LfsBatchOperation, LfsBatchRef, LfsBatchRequest, LfsBatchResponse,
    LfsLocalCache, encode_lfs_pointer, parse_lfs_pointer,
};
#[cfg(any(feature = "lfs", feature = "xet"))]
pub use large_files::{
    LargeFileBackend, LargeFileBackendKind, LargeFilePointer, LargeFileTrackRule,
};
#[cfg(feature = "xet")]
pub use large_files::{
    XetChunkRange, XetDetection, XetFileReconstruction, XetHash, XetLocalCache,
    XetReconstructionTerm, detect_xet_storage, parse_xet_pointer_hash,
};
pub use merge_state::{MergeState, RebaseState};
pub use object::{GitObject, ObjectId, ObjectKind, TreeEntry, hash_object};
pub use odb::{IngestedPack, LooseObjectDb, StoredPack, StoredPackIndex};
pub use partial_clone::{PartialClonePolicy, PromisorRemote};
pub use pathspec::PathspecSet;
pub use refs::{Branch, Tag};
pub use repository::{
    InitOptions, LocalCloneOptions, LocalFetchOptions, LocalFetchResult, RemoteFetchOptions,
    RemoteFetchResult, RemotePushOptions, RemotePushResult, Repository,
};
pub use sparse::{SparseCheckout, SparseCheckoutMode, SparseCheckoutPattern};
pub use status::{
    PorcelainStatus, StatusBranchHeader, StatusEntry, StatusOptions, UntrackedFilesMode,
};
pub use transport::{
    AdvertisedRef, BlockingSmartHttpClient, FetchRefSpec, ReceivePackCommand,
    ReceivePackCommandStatus, ReceivePackRequest, ReceivePackStatus, RemotePackNegotiation,
    SmartHttpAdvertisement, SmartHttpPostRequest, SmartHttpRequest, SmartHttpResponse,
    SmartHttpService, SshServiceCommand, TransportLocation, TransportProtocol, UploadPackAckStatus,
    UploadPackAcknowledgement, UploadPackRequest, UploadPackResponse, UploadPackSideBand,
};
pub use workspace_profile::{
    LazyMaterializationPolicy, RitConfig, WorkspacePrefetchPlan, WorkspaceProfile,
};
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
pub use auth::{
    AuthInteractionPolicy, Credential, CredentialKind, CredentialProvider, CredentialRequest,
    DEFAULT_TOKEN_ENV_VARS, EnvironmentToken, EnvironmentTokenProvider, GitCredentialHelper,
    GitCredentialMessage, KeychainProviderKind, SecretString, SshAgentConfig, SystemKeychainConfig,
};
pub use commit::{Commit, Signature, parse_commit};
pub use config::{GitConfig, GitConfigEntry};
pub use diff::{DiffFileStat, DiffOptions, DiffPatch, DiffPatchFile, DiffSummary};
