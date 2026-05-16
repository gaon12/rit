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
pub mod doctor;
pub mod error;
pub mod history;
pub mod index;
#[cfg(feature = "indexdb")]
pub mod indexdb;
#[cfg(all(test, feature = "indexdb"))]
mod indexdb_tests;
#[cfg(any(feature = "lfs", feature = "xet"))]
pub mod large_files;
pub mod merge_conflict;
pub mod merge_state;
pub mod object;
pub mod odb;
pub mod operations;
pub mod partial_clone;
pub mod pathspec;
pub mod policy;
pub mod policy_check;
#[cfg(test)]
mod policy_check_tests;
pub mod refs;
pub mod repair;
pub mod repository;
pub mod semantic_diff;
#[cfg(test)]
mod semantic_diff_tests;
#[cfg(feature = "semantic-python")]
pub mod semantic_python;
#[cfg(feature = "semantic-rust")]
pub mod semantic_rust;
#[cfg(feature = "semantic-typescript")]
pub mod semantic_typescript;
pub mod sparse;
pub mod stash;
pub mod status;
pub mod transport;
pub mod vfs;
#[cfg(test)]
mod vfs_prefetch_tests;
#[cfg(test)]
mod vfs_tests;
pub mod workspace_profile;
pub mod write;
pub mod write_plan;
#[cfg(test)]
mod write_plan_tests;

pub use error::{Result, RitError};
pub use history::LogEntry;
pub use index::{
    CacheTree, CacheTreeNode, EndOfIndexEntry, EwahBitmap, FsMonitor, FsMonitorToken, Index,
    IndexEntry, IndexEntryOffset, IndexEntryOffsetTable, IndexExtension, IndexExtensionKind,
    ResolveUndo, ResolveUndoEntry, ResolveUndoStage, SparseDirectory, SplitIndexLink,
    UntrackedCache, UntrackedCacheDirectoryBlock, UntrackedCacheStat, UntrackedCacheTail,
};
#[cfg(feature = "indexdb")]
pub use indexdb::{
    IndexDb, IndexDbEnsureResult, IndexDbStatus, IndexDbStorage, IndexedCommit, IndexedFileChange,
    IndexedRef,
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
pub use operations::{
    OperationJournalWarning, OperationLog, OperationRecord, OperationRestoreResult,
    OperationSnapshot, RepositoryOperations,
};
pub use partial_clone::{PartialClonePolicy, PromisorRemote};
pub use pathspec::{PathspecExplanation, PathspecPatternExplanation, PathspecSet};
pub use policy::{PolicyConfig, PolicyEnforcement, parse_size_limit};
pub use policy_check::{PolicyFinding, PolicyFindingKind, PolicySeverity};
pub use refs::{Branch, Tag};
pub use repair::{RepairAction, RepairPlan, RepairResult};
pub use repository::{
    InitOptions, LocalCloneOptions, LocalFetchOptions, LocalFetchResult, RemoteFetchOptions,
    RemoteFetchResult, RemotePushOptions, RemotePushResult, Repository,
};
#[cfg(feature = "semantic-tree-sitter")]
pub use semantic_diff::TreeSitterSemanticParser;
pub use semantic_diff::{
    SemanticDiffFile, SemanticDiffReport, SemanticFileCategory, WordDiff, WordDiffOperation,
    classify_semantic_path, semantic_report_from_paths, word_diff,
};
#[cfg(feature = "semantic-python")]
pub use semantic_python::{
    PythonFunctionChange, PythonSemanticSummary, summarize_python_functions,
};
#[cfg(feature = "semantic-rust")]
pub use semantic_rust::{RustFunctionChange, RustSemanticSummary, summarize_rust_functions};
#[cfg(feature = "semantic-typescript")]
pub use semantic_typescript::{
    TypeScriptFunctionChange, TypeScriptSemanticSummary, summarize_typescript_functions,
};
pub use sparse::{SparseCheckout, SparseCheckoutMode, SparseCheckoutPattern};
pub use stash::StashListEntry;
pub use status::{
    IgnoreExplanation, IgnoreRuleExplanation, PorcelainStatus, StatusBranchHeader, StatusEntry,
    StatusExplanation, StatusOptions, UntrackedFilesMode,
};
pub use transport::{
    AdvertisedRef, BlockingSmartHttpClient, ConfiguredProcessSshServiceExecutor, FetchRefSpec,
    ProcessSshServiceExecutor, ReceivePackCommand, ReceivePackCommandStatus, ReceivePackRequest,
    ReceivePackStatus, RemotePackNegotiation, SmartHttpAdvertisement, SmartHttpPostRequest,
    SmartHttpRequest, SmartHttpResponse, SmartHttpService, SshProcessConfig, SshProcessInvocation,
    SshReceivePackExecutor, SshServiceCommand, SshServiceExecutor, SshUploadPackExecutor,
    SshVariant, TransportLocation, TransportProtocol, UploadPackAckStatus,
    UploadPackAcknowledgement, UploadPackRequest, UploadPackResponse, UploadPackSideBand,
    run_ssh_upload_pack,
};
pub use vfs::{
    FallbackMaterializedAction, FallbackMaterializedBackend, FallbackMaterializedPlan,
    VfsAvailability, VfsBackendPreference, VfsLazyMaterialization, VfsMaterializeRequest,
    VfsMaterializeResult, VfsMaterializeStatus, VfsPlan, VfsPlatformBackend,
    VfsPlatformBackendPlan, VfsPrefetchObject, VfsPrefetchRequest, VfsPrefetchResult,
    VfsPrefetchedObject,
};
pub use workspace_profile::{
    LazyMaterializationPolicy, RitConfig, WorkspacePrefetchPlan, WorkspaceProfile,
};
pub use write::{
    AddOptions, AddPlan, CherryPickOptions, CherryPickResult, CommitHookMode, CommitOptions,
    CommitPlan, CommitResult, FileModeOverride, MergeConflictKind, MergeConflictReport,
    MergeConflictSide, MergeConflictStageEntry, MergeConflictStagePlan, MergeOptions, MergePlan,
    MergeResult, RebaseContinueResult, RebaseCurrentPatch, RebaseSkipResult, RebaseStartResult,
    ResetPlan, SignatureIdentity, SignatureTime,
};
pub use write_plan::{
    PlannedHook, PlannedObjectWrite, PlannedPathAction, PlannedPathChange, PlannedPolicyCheck,
    PlannedRefAction, PlannedRefChange, WritePlan, WritePlanEffects, WritePlanKind,
};

/// Returns the crate version used by the CLI and tests.
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}
pub use attributes::{
    AttributeAssignment, AttributeMacro, AttributeRule, AttributeState, GitAttributes,
};
pub use auth::{
    AuthExplanation, AuthInteractionPolicy, AuthProtocol, Credential, CredentialKind,
    CredentialProvider, CredentialRequest, DEFAULT_TOKEN_ENV_VARS, EnvironmentToken,
    EnvironmentTokenProvider, GitCredentialHelper, GitCredentialHelperExecutor,
    GitCredentialHelperOperation, GitCredentialHelperProvider, GitCredentialMessage,
    KeychainProviderKind, ProcessGitCredentialHelperExecutor, SecretString, SshAgentClient,
    SshAgentConfig, SshAgentIdentity, SshAgentSignFlags, SshAgentSignature, SystemKeychainConfig,
    SystemKeychainProvider, explain_auth_location, explain_auth_location_with_env,
};
pub use commit::{Commit, Signature, parse_commit};
pub use config::{GitConfig, GitConfigEntry};
pub use diff::{DiffFileStat, DiffOptions, DiffPatch, DiffPatchFile, DiffSummary};
pub use doctor::{DoctorCheck, DoctorReport, DoctorSeverity};
