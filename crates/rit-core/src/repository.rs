use crate::object::parse_tree_entries;
use crate::{
    BlockingSmartHttpClient, ConfiguredProcessSshServiceExecutor, FetchRefSpec, GitAttributes,
    GitConfig, GitObject, LooseObjectDb, ObjectId, ObjectKind, PartialClonePolicy,
    ReceivePackCommand, ReceivePackCommandStatus, ReceivePackRequest, ReceivePackStatus, Result,
    RitConfig, RitError, SmartHttpAdvertisement, SmartHttpService, SparseCheckout,
    SshProcessConfig, SshReceivePackExecutor, SshUploadPackExecutor, TransportLocation,
    TransportProtocol, parse_commit,
};
use std::collections::HashSet;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

/// Options for creating a Git-compatible repository.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InitOptions {
    /// Directory that will contain the working tree, or the repository itself
    /// when `bare` is true.
    pub directory: PathBuf,
    /// Create a bare repository without a working tree.
    pub bare: bool,
    /// Suppress user-facing success output in the CLI.
    pub quiet: bool,
    /// Initial branch name written into `HEAD`.
    pub initial_branch: String,
}

/// Options for cloning from a local repository without checking files out.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalCloneOptions {
    /// Existing local repository to copy from.
    pub source: PathBuf,
    /// Directory to create as the destination working tree.
    pub directory: PathBuf,
}

/// Options for fetching objects from a local repository.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalFetchOptions {
    /// Existing local repository to copy objects from.
    pub source: PathBuf,
    /// Optional refspec to update after objects are copied.
    pub refspec: Option<FetchRefSpec>,
}

/// Options for fetching from a smart HTTP remote.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RemoteFetchOptions {
    /// Remote repository location.
    pub location: TransportLocation,
    /// Optional refspec to update after objects are downloaded.
    pub refspec: Option<FetchRefSpec>,
}

/// Options for pushing one ref to a smart HTTP remote.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RemotePushOptions {
    /// Remote repository location.
    pub location: TransportLocation,
    /// Source-to-destination refspec to push.
    pub refspec: FetchRefSpec,
}

impl LocalFetchOptions {
    /// Builds local fetch options for `source`.
    pub fn new(source: impl Into<PathBuf>) -> Self {
        Self {
            source: source.into(),
            refspec: None,
        }
    }

    /// Adds one source-to-destination refspec.
    pub fn with_refspec(mut self, refspec: FetchRefSpec) -> Self {
        self.refspec = Some(refspec);
        self
    }
}

impl RemoteFetchOptions {
    /// Builds remote fetch options for `location`.
    pub fn new(location: TransportLocation) -> Self {
        Self {
            location,
            refspec: None,
        }
    }

    /// Adds one source-to-destination refspec.
    pub fn with_refspec(mut self, refspec: FetchRefSpec) -> Self {
        self.refspec = Some(refspec);
        self
    }
}

impl RemotePushOptions {
    /// Builds remote push options for one source-to-destination refspec.
    pub fn new(location: TransportLocation, refspec: FetchRefSpec) -> Self {
        Self { location, refspec }
    }
}

/// Summary of a local fetch operation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalFetchResult {
    /// Commit recorded in `FETCH_HEAD`.
    pub fetch_head: ObjectId,
    /// Human-readable source path recorded in `FETCH_HEAD`.
    pub source: String,
}

/// Summary of a smart HTTP fetch operation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RemoteFetchResult {
    /// Object recorded in `FETCH_HEAD`.
    pub fetch_head: ObjectId,
    /// Human-readable remote location recorded in `FETCH_HEAD`.
    pub source: String,
    /// Number of objects unpacked from the received pack.
    pub object_count: usize,
}

/// Summary of a smart HTTP push operation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RemotePushResult {
    /// Ref updated on the remote side.
    pub destination: String,
    /// Object ID sent as the new ref target.
    pub new_id: ObjectId,
    /// Number of local objects included in the generated pack.
    pub object_count: usize,
    /// Parsed receive-pack status returned by the remote.
    pub status: ReceivePackStatus,
}

impl LocalCloneOptions {
    /// Builds local clone options for `source` and `directory`.
    pub fn new(source: impl Into<PathBuf>, directory: impl Into<PathBuf>) -> Self {
        Self {
            source: source.into(),
            directory: directory.into(),
        }
    }
}

impl InitOptions {
    /// Builds default init options for `directory`.
    pub fn new(directory: impl Into<PathBuf>) -> Self {
        Self {
            directory: directory.into(),
            bare: false,
            quiet: false,
            initial_branch: "master".to_owned(),
        }
    }
}

/// A discovered Git repository.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Repository {
    worktree: Option<PathBuf>,
    git_dir: PathBuf,
    common_dir: PathBuf,
    bare: bool,
}

impl Repository {
    /// Opens the repository containing `path`.
    ///
    /// This is a convenience alias for [`Repository::discover`] and is the
    /// preferred entry point for application code that already has a path from
    /// the user.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        Self::discover(path)
    }

    /// Walks upward from `start` until a `.git` directory or file is found.
    pub fn discover(start: impl AsRef<Path>) -> Result<Self> {
        let start = start.as_ref();
        let start_path = if start.exists() {
            canonicalize_existing_path(start)?
        } else {
            start.to_path_buf()
        };
        let mut current = if start_path.is_file() {
            start_path
                .parent()
                .map(Path::to_path_buf)
                .unwrap_or_else(|| PathBuf::from("."))
        } else {
            start_path.clone()
        };

        loop {
            let dot_git = current.join(".git");
            if dot_git.is_dir() {
                let git_dir = canonicalize_existing_path(&dot_git)?;
                return Self::from_paths(Some(current), git_dir, false);
            }

            if dot_git.is_file() {
                let git_dir = read_gitdir_file(&dot_git)?;
                let resolved_git_dir = if git_dir.is_absolute() {
                    git_dir
                } else {
                    current.join(git_dir)
                };
                let git_dir = canonicalize_existing_path(&resolved_git_dir)?;
                return Self::from_paths(Some(current), git_dir, false);
            }

            if looks_like_bare_repository(&current) {
                let git_dir = canonicalize_existing_path(&current)?;
                return Self::from_paths(None, git_dir, true);
            }

            if !current.pop() {
                return Err(RitError::RepositoryNotFound {
                    path: start_path.to_path_buf(),
                });
            }
        }
    }

    /// Creates a Git-compatible repository and returns its discovered paths.
    pub fn init(options: &InitOptions) -> Result<Self> {
        validate_branch_name(&options.initial_branch)?;

        let target = if options.directory.as_os_str().is_empty() {
            PathBuf::from(".")
        } else {
            options.directory.clone()
        };

        fs::create_dir_all(&target).map_err(|source| RitError::io(&target, source))?;
        let target = canonicalize_existing_path(&target)?;
        let git_dir = if options.bare {
            target.clone()
        } else {
            target.join(".git")
        };

        create_repository_directories(&git_dir)?;
        write_file_if_missing(
            &git_dir.join("HEAD"),
            format!("ref: refs/heads/{}\n", options.initial_branch).as_bytes(),
        )?;
        write_file_if_missing(
            &git_dir.join("description"),
            b"Unnamed repository; edit this file 'description' to name the repository.\n",
        )?;
        write_file_if_missing(
            &git_dir.join("config"),
            default_config(options.bare).as_bytes(),
        )?;

        Self::from_paths((!options.bare).then_some(target), git_dir, options.bare)
    }

    /// Clones a local repository by copying objects and local refs.
    ///
    /// This intentionally implements only the `--local --no-checkout` shape for
    /// now: no remote protocol, no working tree checkout, and no external `git`
    /// process.
    pub fn clone_local_no_checkout(options: &LocalCloneOptions) -> Result<Self> {
        ensure_clone_target_is_available(&options.directory)?;
        let source = Repository::open(&options.source)?;
        let branch_name = source.current_branch_name()?.ok_or_else(|| {
            RitError::invalid_input("local clone from detached HEAD is not implemented")
        })?;
        source.resolve_head()?.ok_or_else(|| {
            RitError::invalid_input("local clone from an unborn branch is not implemented")
        })?;

        let mut init_options = InitOptions::new(&options.directory);
        init_options.initial_branch = branch_name.clone();
        let target = Repository::init(&init_options)?;

        copy_directory_contents(
            &source.common_dir().join("objects"),
            &target.common_dir().join("objects"),
        )?;
        copy_ref_namespace(&source, &target, "heads")?;
        copy_ref_namespace(&source, &target, "tags")?;
        copy_file_if_exists(
            &source.common_dir().join("packed-refs"),
            &target.common_dir().join("packed-refs"),
        )?;
        write_file(
            &target.git_dir().join("HEAD"),
            format!("ref: refs/heads/{branch_name}\n").as_bytes(),
        )?;
        append_clone_remote_config(&target, &source, &branch_name)?;

        Ok(target)
    }

    /// Fetches the source repository's `HEAD` objects into this repository.
    ///
    /// The first supported shape mirrors `git fetch <local-repo>` without a
    /// refspec: objects are copied and `FETCH_HEAD` is overwritten, while local
    /// refs are left untouched.
    pub fn fetch_local(&self, options: &LocalFetchOptions) -> Result<LocalFetchResult> {
        let source = Repository::open(&options.source)?;
        let fetch_head = match &options.refspec {
            Some(refspec) => source.resolve_fetch_source(&refspec.source)?,
            None => source.resolve_head()?.ok_or_else(|| {
                RitError::invalid_input("local fetch from an unborn branch is not implemented")
            })?,
        };
        copy_directory_contents(
            &source.common_dir().join("objects"),
            &self.common_dir().join("objects"),
        )?;
        let source_name = options.source.display().to_string();
        let fetch_head_line = match &options.refspec {
            Some(refspec) => {
                validate_full_ref_name(&refspec.destination)?;
                write_full_ref(self, &refspec.destination, fetch_head)?;
                format!(
                    "{fetch_head}\t\t{} of {source_name}\n",
                    fetch_head_description(&refspec.source)
                )
            }
            None => format!("{fetch_head}\t\t{source_name}\n"),
        };
        write_file(
            &self.git_dir().join("FETCH_HEAD"),
            fetch_head_line.as_bytes(),
        )?;
        Ok(LocalFetchResult {
            fetch_head,
            source: source_name,
        })
    }

    /// Fetches one advertised ref from a smart HTTP or HTTPS remote.
    ///
    /// This first remote implementation uses one upload-pack negotiation round,
    /// ingests the returned pack into the object database, and updates
    /// `FETCH_HEAD`. SSH and multi-round negotiation are intentionally left to
    /// later transport milestones.
    pub fn fetch_remote_http(&self, options: &RemoteFetchOptions) -> Result<RemoteFetchResult> {
        if !matches!(
            options.location.protocol(),
            TransportProtocol::Http | TransportProtocol::Https
        ) {
            return Err(RitError::invalid_input(
                "remote fetch currently supports only http:// and https:// smart remotes",
            ));
        }

        let wanted_ref = options
            .refspec
            .as_ref()
            .map(|refspec| refspec.source.as_str())
            .unwrap_or("HEAD");
        let haves = self.fetch_negotiation_haves()?;
        let client = BlockingSmartHttpClient::default();
        let negotiation = client.negotiate_upload_pack(&options.location, wanted_ref, haves)?;
        let source_name = options.location.original().to_owned();
        self.finish_remote_fetch(
            options,
            source_name,
            negotiation.want_id,
            &negotiation.pack_bytes,
        )
    }

    /// Fetches one advertised ref from an SSH remote.
    pub fn fetch_remote_ssh(&self, options: &RemoteFetchOptions) -> Result<RemoteFetchResult> {
        let executor = self.configured_ssh_process_executor()?;
        self.fetch_remote_ssh_with_executor(options, &executor)
    }

    /// Fetches one advertised ref from an SSH remote using an explicit executor.
    pub fn fetch_remote_ssh_with_executor(
        &self,
        options: &RemoteFetchOptions,
        executor: &impl SshUploadPackExecutor,
    ) -> Result<RemoteFetchResult> {
        if options.location.protocol() != TransportProtocol::Ssh {
            return Err(RitError::invalid_input(
                "SSH fetch requires an ssh:// or scp-like remote",
            ));
        }

        let wanted_ref = options
            .refspec
            .as_ref()
            .map(|refspec| refspec.source.as_str())
            .unwrap_or("HEAD");
        let haves = self.fetch_negotiation_haves()?;
        let negotiation = executor.negotiate_upload_pack(&options.location, wanted_ref, haves)?;
        let source_name = options.location.original().to_owned();
        self.finish_remote_fetch(
            options,
            source_name,
            negotiation.want_id,
            &negotiation.pack_bytes,
        )
    }

    /// Pushes one source ref or revision to a smart HTTP or HTTPS remote.
    pub fn push_remote_http(&self, options: &RemotePushOptions) -> Result<RemotePushResult> {
        if !matches!(
            options.location.protocol(),
            TransportProtocol::Http | TransportProtocol::Https
        ) {
            return Err(RitError::invalid_input(
                "remote push currently supports only http:// and https:// smart remotes",
            ));
        }
        validate_full_ref_name(&options.refspec.destination)?;

        let client = BlockingSmartHttpClient::default();
        let advertisement =
            client.discover_refs(&options.location, SmartHttpService::ReceivePack)?;
        let old_id = advertised_ref_id(&advertisement, &options.refspec.destination)
            .unwrap_or_else(zero_object_id);
        let new_id = self.resolve_revision(&options.refspec.source)?;
        let object_ids = self.collect_reachable_object_ids(new_id)?;
        let pack_data = self.loose_objects().build_pack_from_objects(&object_ids)?;
        let command = ReceivePackCommand::new(old_id, new_id, options.refspec.destination.clone())?;
        let request = ReceivePackRequest::new(vec![command])?
            .with_capabilities(receive_pack_capabilities(&advertisement.capabilities))
            .with_pack_data(pack_data);
        let status = client.post_receive_pack(&options.location, &request)?;
        validate_receive_pack_status(&status, &options.refspec.destination)?;

        Ok(RemotePushResult {
            destination: options.refspec.destination.clone(),
            new_id,
            object_count: object_ids.len(),
            status,
        })
    }

    /// Pushes one source ref or revision to an SSH remote.
    pub fn push_remote_ssh(&self, options: &RemotePushOptions) -> Result<RemotePushResult> {
        let executor = self.configured_ssh_process_executor()?;
        self.push_remote_ssh_with_executor(options, &executor)
    }

    /// Pushes one source ref or revision to an SSH remote using an explicit executor.
    pub fn push_remote_ssh_with_executor(
        &self,
        options: &RemotePushOptions,
        executor: &impl SshReceivePackExecutor,
    ) -> Result<RemotePushResult> {
        if options.location.protocol() != TransportProtocol::Ssh {
            return Err(RitError::invalid_input(
                "SSH push requires an ssh:// or scp-like remote",
            ));
        }
        validate_full_ref_name(&options.refspec.destination)?;

        let new_id = self.resolve_revision(&options.refspec.source)?;
        let object_ids = self.collect_reachable_object_ids(new_id)?;
        let pack_data = self.loose_objects().build_pack_from_objects(&object_ids)?;
        let status = executor.send_receive_pack(
            &options.location,
            &options.refspec.destination,
            new_id,
            pack_data,
        )?;
        validate_receive_pack_status(&status, &options.refspec.destination)?;

        Ok(RemotePushResult {
            destination: options.refspec.destination.clone(),
            new_id,
            object_count: object_ids.len(),
            status,
        })
    }

    fn finish_remote_fetch(
        &self,
        options: &RemoteFetchOptions,
        source_name: String,
        fetch_head: ObjectId,
        pack_bytes: &[u8],
    ) -> Result<RemoteFetchResult> {
        let ingested = self.loose_objects().ingest_pack(pack_bytes)?;
        let fetch_head_line = match &options.refspec {
            Some(refspec) => {
                validate_full_ref_name(&refspec.destination)?;
                write_full_ref(self, &refspec.destination, fetch_head)?;
                format!(
                    "{fetch_head}\t\t{} of {source_name}\n",
                    fetch_head_description(&refspec.source)
                )
            }
            None => format!("{fetch_head}\t\t{source_name}\n"),
        };
        write_file(
            &self.git_dir().join("FETCH_HEAD"),
            fetch_head_line.as_bytes(),
        )?;

        Ok(RemoteFetchResult {
            fetch_head,
            source: source_name,
            object_count: ingested.object_ids.len(),
        })
    }

    fn fetch_negotiation_haves(&self) -> Result<Vec<ObjectId>> {
        let mut roots = Vec::new();
        if let Some(head) = self.resolve_head()? {
            roots.push(head);
        }
        for branch in self.list_branches()? {
            roots.push(branch.target);
        }
        for tag in self.list_tags()? {
            roots.push(tag.target);
        }
        self.collect_reachable_commit_ids(&roots)
    }

    fn collect_reachable_commit_ids(&self, roots: &[ObjectId]) -> Result<Vec<ObjectId>> {
        let mut seen = HashSet::new();
        let mut ordered = Vec::new();
        for root in roots {
            self.collect_reachable_commit_ids_inner(*root, &mut seen, &mut ordered)?;
        }
        Ok(ordered)
    }

    fn collect_reachable_commit_ids_inner(
        &self,
        object_id: ObjectId,
        seen: &mut HashSet<ObjectId>,
        ordered: &mut Vec<ObjectId>,
    ) -> Result<()> {
        if !seen.insert(object_id) {
            return Ok(());
        }
        let object = self.read_object(object_id)?;
        if object.kind != ObjectKind::Commit {
            return Ok(());
        }
        ordered.push(object_id);
        let commit = parse_commit(&object.data)?;
        for parent in commit.parents {
            self.collect_reachable_commit_ids_inner(parent, seen, ordered)?;
        }
        Ok(())
    }

    /// Collects objects reachable from one object ID in a simple deterministic order.
    pub fn collect_reachable_object_ids(&self, root: ObjectId) -> Result<Vec<ObjectId>> {
        let mut seen = HashSet::new();
        let mut ordered = Vec::new();
        self.collect_reachable_object_ids_inner(root, &mut seen, &mut ordered)?;
        Ok(ordered)
    }

    fn collect_reachable_object_ids_inner(
        &self,
        object_id: ObjectId,
        seen: &mut HashSet<ObjectId>,
        ordered: &mut Vec<ObjectId>,
    ) -> Result<()> {
        if !seen.insert(object_id) {
            return Ok(());
        }
        ordered.push(object_id);
        let object = self.read_object(object_id)?;
        match object.kind {
            ObjectKind::Commit => {
                let commit = parse_commit(&object.data)?;
                self.collect_reachable_object_ids_inner(commit.tree, seen, ordered)?;
                for parent in commit.parents {
                    self.collect_reachable_object_ids_inner(parent, seen, ordered)?;
                }
            }
            ObjectKind::Tree => {
                for entry in parse_tree_entries(&object.data)? {
                    self.collect_reachable_object_ids_inner(entry.object_id, seen, ordered)?;
                }
            }
            ObjectKind::Blob | ObjectKind::Tag => {}
        }
        Ok(())
    }

    /// Returns the path to the repository metadata directory.
    pub fn git_dir(&self) -> &Path {
        &self.git_dir
    }

    /// Returns the common metadata directory shared by linked worktrees.
    pub fn common_dir(&self) -> &Path {
        &self.common_dir
    }

    /// Returns whether symlink index entries should materialize as symlinks.
    pub(crate) fn core_symlinks_enabled(&self) -> Result<bool> {
        self.read_core_bool("symlinks", default_core_symlinks())
    }

    /// Returns whether Git should compare worktree path names case-insensitively.
    pub(crate) fn core_ignorecase_enabled(&self) -> Result<bool> {
        self.read_core_bool("ignorecase", default_core_ignorecase())
    }

    /// Reads repository-level `.gitattributes` rules from the working tree.
    pub fn root_attributes(&self) -> Result<GitAttributes> {
        let Some(worktree) = self.worktree() else {
            return Ok(GitAttributes::default());
        };
        let attributes_path = worktree.join(".gitattributes");
        if !attributes_path.exists() {
            return Ok(GitAttributes::default());
        }
        GitAttributes::read(&attributes_path)
    }

    /// Reads sparse-checkout config and pattern state for this worktree.
    pub fn sparse_checkout(&self) -> Result<SparseCheckout> {
        let config_path = self.common_dir.join("config");
        let config = if config_path.exists() {
            GitConfig::read(&config_path)?
        } else {
            GitConfig::default()
        };
        SparseCheckout::read_from_git_dir(&config, &self.git_dir)
    }

    /// Reads optional `rit.toml` workspace profile configuration.
    pub fn rit_config(&self) -> Result<RitConfig> {
        let Some(worktree) = self.worktree() else {
            return Ok(RitConfig::default());
        };
        RitConfig::read_from_worktree(worktree)
    }

    /// Reads partial-clone promisor remotes and pack markers.
    pub fn partial_clone_policy(&self) -> Result<PartialClonePolicy> {
        let config_path = self.common_dir.join("config");
        let config = if config_path.exists() {
            GitConfig::read(&config_path)?
        } else {
            GitConfig::default()
        };
        PartialClonePolicy::read(&config, &self.common_dir.join("objects"))
    }

    /// Returns a loose object database reader for this repository.
    pub fn loose_objects(&self) -> LooseObjectDb {
        LooseObjectDb::new(self.common_dir.join("objects"))
    }

    /// Reads an object by its full object ID.
    pub fn read_object(&self, object_id: ObjectId) -> Result<GitObject> {
        self.loose_objects().read_object(object_id)
    }

    /// Resolves `HEAD` to an object ID. Unborn branches return `None`.
    pub fn resolve_head(&self) -> Result<Option<ObjectId>> {
        let head_path = self.git_dir.join("HEAD");
        let contents =
            fs::read_to_string(&head_path).map_err(|source| RitError::io(&head_path, source))?;
        let trimmed = contents.trim();
        if let Some(reference_name) = trimmed.strip_prefix("ref: ") {
            let reference_path = self.common_dir.join(reference_name);
            if reference_path.exists() {
                let object_id = fs::read_to_string(&reference_path)
                    .map_err(|source| RitError::io(&reference_path, source))?;
                return Ok(Some(ObjectId::from_hex(object_id.trim())?));
            }
            return self.packed_ref(reference_name);
        }

        Ok(Some(ObjectId::from_hex(trimmed)?))
    }

    /// Resolves a small, explicit revision set: full object ID, `HEAD`, local
    /// branch name, or lightweight tag name.
    pub fn resolve_revision(&self, revision: &str) -> Result<ObjectId> {
        if revision == "HEAD" {
            return self
                .resolve_head()?
                .ok_or_else(|| RitError::invalid_input("HEAD does not point at a commit yet"));
        }
        if revision == "FETCH_HEAD" {
            let path = self.git_dir.join("FETCH_HEAD");
            let contents =
                fs::read_to_string(&path).map_err(|source| RitError::io(&path, source))?;
            let object_id = contents
                .split_whitespace()
                .next()
                .ok_or_else(|| RitError::invalid_input("FETCH_HEAD is empty"))?;
            return ObjectId::from_hex(object_id);
        }
        if revision.starts_with("refs/") {
            return self.resolve_full_ref(revision);
        }

        if revision.len() == 40
            && revision
                .chars()
                .all(|character| character.is_ascii_hexdigit())
        {
            return ObjectId::from_hex(revision);
        }

        if let Some(object_id) = self.loose_objects().find_object_id_by_prefix(revision)? {
            return Ok(object_id);
        }

        for namespace in ["heads", "tags"] {
            let path = self.common_dir.join("refs").join(namespace).join(revision);
            if path.exists() {
                let target =
                    fs::read_to_string(&path).map_err(|source| RitError::io(&path, source))?;
                return ObjectId::from_hex(target.trim());
            }
            if let Some(target) = self.packed_ref(&format!("refs/{namespace}/{revision}"))? {
                return Ok(target);
            }
        }

        Err(RitError::invalid_input(format!(
            "unknown revision or object: {revision}"
        )))
    }

    fn resolve_fetch_source(&self, source: &str) -> Result<ObjectId> {
        if source.starts_with("refs/") {
            return self.resolve_full_ref(source);
        }
        self.resolve_revision(source)
    }

    fn resolve_full_ref(&self, full_name: &str) -> Result<ObjectId> {
        validate_full_ref_name(full_name)?;
        let path = self.common_dir.join(full_name);
        if path.exists() {
            let target = fs::read_to_string(&path).map_err(|source| RitError::io(&path, source))?;
            return ObjectId::from_hex(target.trim());
        }
        self.packed_ref(full_name)?
            .ok_or_else(|| RitError::invalid_input(format!("ref not found: {full_name}")))
    }

    /// Returns the working tree root, or `None` for bare repositories.
    pub fn worktree(&self) -> Option<&Path> {
        self.worktree.as_deref()
    }

    /// Returns whether the repository is bare.
    pub fn is_bare(&self) -> bool {
        self.bare
    }

    /// Refreshes optional auxiliary metadata after a successful Git write.
    ///
    /// IndexDB is never the source of truth, so a refresh failure must not turn
    /// a successful Git repository update into a failed command.
    pub(crate) fn refresh_indexdb_after_git_write(&self) {
        #[cfg(feature = "indexdb")]
        {
            let indexdb = self.indexdb();
            if indexdb.storage().database_path.exists() {
                let _ = indexdb.update();
            }
        }
    }

    fn from_paths(worktree: Option<PathBuf>, git_dir: PathBuf, bare: bool) -> Result<Self> {
        let common_dir = resolve_common_dir(&git_dir)?;
        let repository = Self {
            worktree,
            git_dir,
            common_dir,
            bare,
        };
        repository.ensure_supported_format()?;
        Ok(repository)
    }

    fn ensure_supported_format(&self) -> Result<()> {
        let config_path = self.common_dir.join("config");
        if !config_path.exists() {
            return Ok(());
        }

        let config = GitConfig::read(&config_path)?;
        let format_version = config
            .get("core", "repositoryformatversion")
            .unwrap_or("0")
            .parse::<u32>()
            .map_err(|_| RitError::invalid_input("invalid repository format version in config"))?;
        if format_version != 0 {
            return Err(RitError::UnsupportedRepositoryFormat {
                version: format_version,
            });
        }
        if let Some(extension) = config.keys_in_section("extensions").first() {
            return Err(RitError::UnsupportedRepositoryExtension {
                name: (*extension).to_owned(),
            });
        }
        Ok(())
    }

    fn read_core_bool(&self, key: &str, default: bool) -> Result<bool> {
        let config_path = self.common_dir.join("config");
        if !config_path.exists() {
            return Ok(default);
        }
        let config = GitConfig::read(&config_path)?;
        config.get_bool("core", key, default)
    }

    fn configured_ssh_process_executor(&self) -> Result<ConfiguredProcessSshServiceExecutor> {
        Ok(ConfiguredProcessSshServiceExecutor::new(
            self.ssh_process_config()?,
        ))
    }

    fn ssh_process_config(&self) -> Result<SshProcessConfig> {
        let config_path = self.common_dir.join("config");
        if !config_path.exists() {
            return Ok(SshProcessConfig::default());
        }
        let config = GitConfig::read(&config_path)?;
        Ok(SshProcessConfig::from_git_config(&config))
    }
}

#[cfg(unix)]
fn default_core_symlinks() -> bool {
    true
}

#[cfg(not(unix))]
fn default_core_symlinks() -> bool {
    false
}

#[cfg(windows)]
fn default_core_ignorecase() -> bool {
    true
}

#[cfg(not(windows))]
fn default_core_ignorecase() -> bool {
    false
}

fn create_repository_directories(git_dir: &Path) -> Result<()> {
    for directory in [
        git_dir.to_path_buf(),
        git_dir.join("objects"),
        git_dir.join("objects").join("info"),
        git_dir.join("objects").join("pack"),
        git_dir.join("refs"),
        git_dir.join("refs").join("heads"),
        git_dir.join("refs").join("tags"),
        git_dir.join("branches"),
        git_dir.join("hooks"),
        git_dir.join("info"),
    ] {
        fs::create_dir_all(&directory).map_err(|source| RitError::io(&directory, source))?;
    }

    write_file_if_missing(
        &git_dir.join("info").join("exclude"),
        b"# git ls-files --others --exclude-from=.git/info/exclude\n",
    )?;

    Ok(())
}

fn write_file_if_missing(path: &Path, contents: &[u8]) -> Result<()> {
    if path.exists() {
        return Ok(());
    }

    write_file(path, contents)
}

fn write_file(path: &Path, contents: &[u8]) -> Result<()> {
    let lock_path = path.with_extension("lock");
    {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&lock_path)
            .map_err(|source| RitError::io(&lock_path, source))?;
        file.write_all(contents)
            .map_err(|source| RitError::io(&lock_path, source))?;
        file.sync_all()
            .map_err(|source| RitError::io(&lock_path, source))?;
    }
    fs::rename(&lock_path, path).map_err(|source| RitError::io(path, source))?;
    Ok(())
}

fn ensure_clone_target_is_available(path: &Path) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }
    if !path.is_dir() {
        return Err(RitError::invalid_input(format!(
            "destination path exists and is not a directory: {}",
            path.display()
        )));
    }
    let mut entries = fs::read_dir(path).map_err(|source| RitError::io(path, source))?;
    if entries.next().is_some() {
        return Err(RitError::invalid_input(format!(
            "destination path already exists and is not empty: {}",
            path.display()
        )));
    }
    Ok(())
}

fn copy_ref_namespace(source: &Repository, target: &Repository, namespace: &str) -> Result<()> {
    copy_directory_contents(
        &source.common_dir().join("refs").join(namespace),
        &target.common_dir().join("refs").join(namespace),
    )
}

fn copy_directory_contents(source: &Path, target: &Path) -> Result<()> {
    if !source.exists() {
        return Ok(());
    }
    fs::create_dir_all(target).map_err(|source| RitError::io(target, source))?;
    for entry in fs::read_dir(source).map_err(|error| RitError::io(source, error))? {
        let entry = entry.map_err(|error| RitError::io(source, error))?;
        let source_path = entry.path();
        let target_path = target.join(entry.file_name());
        let file_type = entry
            .file_type()
            .map_err(|error| RitError::io(&source_path, error))?;
        if file_type.is_dir() {
            copy_directory_contents(&source_path, &target_path)?;
        } else if file_type.is_file() {
            if let Some(parent) = target_path.parent() {
                fs::create_dir_all(parent).map_err(|error| RitError::io(parent, error))?;
            }
            fs::copy(&source_path, &target_path)
                .map_err(|error| RitError::io(&target_path, error))?;
        }
    }
    Ok(())
}

fn copy_file_if_exists(source: &Path, target: &Path) -> Result<()> {
    if !source.exists() {
        return Ok(());
    }
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent).map_err(|error| RitError::io(parent, error))?;
    }
    fs::copy(source, target).map_err(|error| RitError::io(target, error))?;
    Ok(())
}

fn write_full_ref(repository: &Repository, full_name: &str, target: ObjectId) -> Result<()> {
    validate_full_ref_name(full_name)?;
    let path = repository.common_dir().join(full_name);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| RitError::io(parent, source))?;
    }
    write_file(&path, format!("{target}\n").as_bytes())
}

fn validate_full_ref_name(full_name: &str) -> Result<()> {
    if !full_name.starts_with("refs/")
        || full_name.ends_with('/')
        || full_name.contains('\\')
        || full_name.contains("..")
        || full_name.contains("//")
        || full_name.ends_with(".lock")
        || full_name
            .chars()
            .any(|character| character.is_control() || character.is_whitespace())
    {
        return Err(RitError::invalid_input(format!(
            "invalid ref name: {full_name}"
        )));
    }
    Ok(())
}

fn fetch_head_description(source_ref: &str) -> String {
    if let Some(name) = source_ref.strip_prefix("refs/heads/") {
        format!("branch '{name}'")
    } else if let Some(name) = source_ref.strip_prefix("refs/tags/") {
        format!("tag '{name}'")
    } else {
        format!("ref '{source_ref}'")
    }
}

fn advertised_ref_id(advertisement: &SmartHttpAdvertisement, ref_name: &str) -> Option<ObjectId> {
    advertisement
        .refs
        .iter()
        .find(|advertised_ref| advertised_ref.name == ref_name)
        .map(|advertised_ref| advertised_ref.object_id)
}

fn zero_object_id() -> ObjectId {
    ObjectId::from_bytes([0; 20])
}

fn receive_pack_capabilities(advertised: &[String]) -> Vec<String> {
    if advertised
        .iter()
        .any(|capability| capability == "report-status")
    {
        vec!["report-status".to_owned()]
    } else {
        Vec::new()
    }
}

fn validate_receive_pack_status(status: &ReceivePackStatus, ref_name: &str) -> Result<()> {
    if let Some(error) = &status.unpack_error {
        return Err(RitError::invalid_input(format!(
            "receive-pack unpack failed: {error}"
        )));
    }
    for command in &status.commands {
        match command {
            ReceivePackCommandStatus::Ok { ref_name: ok_ref } if ok_ref == ref_name => {
                return Ok(());
            }
            ReceivePackCommandStatus::Rejected {
                ref_name: rejected_ref,
                message,
            } if rejected_ref == ref_name => {
                return Err(RitError::invalid_input(format!(
                    "receive-pack rejected {ref_name}: {message}"
                )));
            }
            _ => {}
        }
    }
    Err(RitError::invalid_input(format!(
        "receive-pack did not report status for {ref_name}"
    )))
}

fn append_clone_remote_config(
    target: &Repository,
    source: &Repository,
    branch_name: &str,
) -> Result<()> {
    let source_path = source
        .worktree()
        .unwrap_or_else(|| source.git_dir())
        .to_string_lossy()
        .replace('\\', "/");
    let config_path = target.common_dir().join("config");
    let mut config =
        fs::read_to_string(&config_path).map_err(|source| RitError::io(&config_path, source))?;
    config.push_str(&format!(
        "[remote \"origin\"]\n\turl = {source_path}\n\tfetch = +refs/heads/*:refs/remotes/origin/*\n[branch \"{branch_name}\"]\n\tremote = origin\n\tmerge = refs/heads/{branch_name}\n"
    ));
    write_file(&config_path, config.as_bytes())
}

fn default_config(bare: bool) -> String {
    format!(
        "[core]\n\trepositoryformatversion = 0\n\tfilemode = false\n\tbare = {}\n",
        if bare { "true" } else { "false" }
    )
}

fn read_gitdir_file(path: &Path) -> Result<PathBuf> {
    let contents = fs::read_to_string(path).map_err(|source| RitError::io(path, source))?;
    let Some(rest) = contents.strip_prefix("gitdir:") else {
        return Err(RitError::invalid_input(format!(
            "invalid .git file at {}",
            path.display()
        )));
    };

    Ok(PathBuf::from(rest.trim()))
}

fn resolve_common_dir(git_dir: &Path) -> Result<PathBuf> {
    let common_dir_file = git_dir.join("commondir");
    if !common_dir_file.exists() {
        return Ok(git_dir.to_path_buf());
    }

    let contents = fs::read_to_string(&common_dir_file)
        .map_err(|source| RitError::io(&common_dir_file, source))?;
    let raw_common_dir = PathBuf::from(contents.trim());
    let resolved = if raw_common_dir.is_absolute() {
        raw_common_dir
    } else {
        git_dir.join(raw_common_dir)
    };
    canonicalize_existing_path(&resolved)
}

fn looks_like_bare_repository(path: &Path) -> bool {
    path.join("HEAD").is_file() && path.join("objects").is_dir() && path.join("refs").is_dir()
}

fn canonicalize_existing_path(path: &Path) -> Result<PathBuf> {
    fs::canonicalize(path).map_err(|source| RitError::io(path, source))
}

fn validate_branch_name(branch_name: &str) -> Result<()> {
    if branch_name.is_empty()
        || branch_name.starts_with('-')
        || branch_name.contains('\\')
        || branch_name.contains("..")
        || branch_name.contains('@')
        || branch_name
            .chars()
            .any(|character| character.is_control() || character.is_whitespace())
    {
        return Err(RitError::invalid_input(format!(
            "invalid initial branch name: {branch_name}"
        )));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{InitOptions, RemoteFetchOptions, RemotePushOptions, Repository};
    use crate::{
        FetchRefSpec, ObjectKind, ReceivePackCommandStatus, ReceivePackStatus,
        RemotePackNegotiation, SmartHttpAdvertisement, SmartHttpService, SshReceivePackExecutor,
        SshUploadPackExecutor, TransportLocation, hash_object, object::sha1_bytes,
    };
    use flate2::{Compression, write::ZlibEncoder};
    use std::fs;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::path::{Path, PathBuf};
    use std::thread;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn init_creates_git_directory_layout() {
        let temp = temp_path("init-layout");
        let options = InitOptions::new(&temp);

        let repository = Repository::init(&options).expect("init should create repository");

        assert!(repository.git_dir().join("objects").join("pack").is_dir());
        assert!(repository.git_dir().join("refs").join("heads").is_dir());
        assert_eq!(
            fs::read_to_string(repository.git_dir().join("HEAD")).expect("HEAD should exist"),
            "ref: refs/heads/master\n"
        );

        remove_dir_all(&temp);
    }

    #[test]
    fn discover_walks_up_to_worktree_root() {
        let temp = temp_path("discover");
        let nested = temp.join("a").join("b");
        fs::create_dir_all(&nested).expect("nested directory should be created");
        Repository::init(&InitOptions::new(&temp)).expect("init should create repository");

        let repository = Repository::discover(&nested).expect("repository should be discovered");

        assert_eq!(
            repository.worktree(),
            Some(
                fs::canonicalize(&temp)
                    .expect("temp path should canonicalize")
                    .as_path()
            )
        );
        assert!(!repository.is_bare());

        remove_dir_all(&temp);
    }

    #[test]
    fn open_discovers_repository_from_path() {
        let temp = temp_path("open");
        Repository::init(&InitOptions::new(&temp)).expect("init should create repository");

        let repository = Repository::open(&temp).expect("repository should open");

        assert_eq!(
            repository.worktree(),
            Some(
                fs::canonicalize(&temp)
                    .expect("temp path should canonicalize")
                    .as_path()
            )
        );
        remove_dir_all(&temp);
    }

    #[test]
    fn open_rejects_unsupported_repository_format() {
        let temp = temp_path("unsupported-format");
        let repository =
            Repository::init(&InitOptions::new(&temp)).expect("init should create repository");
        fs::write(
            repository.git_dir().join("config"),
            "[core]\n\trepositoryformatversion = 9\n\tbare = false\n",
        )
        .expect("config should be rewritten");

        let error = Repository::open(&temp).expect_err("unsupported format should fail");

        assert!(
            error
                .to_string()
                .contains("unsupported repository format version: 9")
        );
        remove_dir_all(&temp);
    }

    #[test]
    fn open_rejects_unknown_repository_extension() {
        let temp = temp_path("unsupported-extension");
        let repository =
            Repository::init(&InitOptions::new(&temp)).expect("init should create repository");
        fs::write(
            repository.git_dir().join("config"),
            "[core]\n\trepositoryformatversion = 0\n\tbare = false\n[extensions]\n\tunknown = true\n",
        )
        .expect("config should be rewritten");

        let error = Repository::open(&temp).expect_err("unsupported extension should fail");

        assert!(
            error
                .to_string()
                .contains("unsupported repository extension: unknown")
        );
        remove_dir_all(&temp);
    }

    #[test]
    fn init_rejects_invalid_initial_branch() {
        let temp = temp_path("invalid-branch");
        let mut options = InitOptions::new(&temp);
        options.initial_branch = "bad branch".to_owned();

        let error = Repository::init(&options).expect_err("branch validation should fail");

        assert!(error.to_string().contains("invalid initial branch name"));
        remove_dir_all(&temp);
    }

    #[test]
    fn open_reads_common_dir_for_linked_worktree() {
        let temp = temp_path("linked-worktree");
        let main_worktree = temp.join("main");
        let linked_worktree = temp.join("linked");
        let repository =
            Repository::init(&InitOptions::new(&main_worktree)).expect("init should work");
        fs::write(
            repository.git_dir().join("config"),
            "[core]\n\trepositoryformatversion = 0\n\tbare = false\n[user]\n\tname = Rit Test\n\temail = rit@example.test\n",
        )
        .expect("config should be written");
        fs::write(main_worktree.join("tracked.txt"), "base\n").expect("file should be written");
        repository
            .add_paths(&["tracked.txt".to_owned()])
            .expect("file should be added");
        let commit = repository
            .commit_index("base")
            .expect("commit should be created")
            .commit_id;

        let linked_git_dir = repository.git_dir().join("worktrees").join("linked");
        fs::create_dir_all(&linked_git_dir).expect("linked git dir should be written");
        fs::create_dir_all(&linked_worktree).expect("linked worktree should be written");
        fs::write(
            linked_worktree.join(".git"),
            format!("gitdir: {}\n", linked_git_dir.display()),
        )
        .expect(".git file should be written");
        fs::write(linked_git_dir.join("commondir"), "../..").expect("commondir should be written");
        fs::write(linked_git_dir.join("HEAD"), "ref: refs/heads/master\n")
            .expect("linked HEAD should be written");

        let linked_repository =
            Repository::open(&linked_worktree).expect("linked worktree should open");

        assert_eq!(
            linked_repository.common_dir(),
            fs::canonicalize(repository.git_dir())
                .expect("main git dir should canonicalize")
                .as_path()
        );
        assert_eq!(
            linked_repository
                .resolve_head()
                .expect("HEAD should resolve"),
            Some(commit)
        );
        assert!(
            linked_repository.read_object(commit).is_ok(),
            "linked worktree should read objects from the common directory"
        );
        remove_dir_all(&temp);
    }

    #[test]
    fn reads_sparse_checkout_state_from_worktree_git_dir() {
        let root = temp_path("repository-sparse");
        let repository = Repository::init(&InitOptions::new(&root)).expect("repo should init");
        let config_path = repository.common_dir().join("config");
        let mut config = fs::read_to_string(&config_path).expect("config should be readable");
        config.push_str("\n[core]\n\tsparseCheckout = true\n\tsparseCheckoutCone = true\n");
        fs::write(&config_path, config).expect("config should be updated");
        let info_dir = repository.git_dir().join("info");
        fs::create_dir_all(&info_dir).expect("info dir should exist");
        fs::write(info_dir.join("sparse-checkout"), "/*\n!/*/\n/src/\n")
            .expect("sparse file should be written");

        let sparse = repository
            .sparse_checkout()
            .expect("sparse checkout should read");

        assert!(sparse.enabled);
        assert_eq!(sparse.mode, crate::SparseCheckoutMode::Cone);
        assert_eq!(sparse.cone_directories(), vec!["src"]);

        remove_dir_all(&root);
    }

    #[test]
    fn reads_rit_workspace_profile_config_from_worktree() {
        let root = temp_path("repository-rit-config");
        let repository = Repository::init(&InitOptions::new(&root)).expect("repo should init");
        fs::write(
            root.join("rit.toml"),
            "[workspace.mobile]\ninclude = [\"apps/mobile\", \"packages/ui\"]\n",
        )
        .expect("rit config should be written");

        let config = repository.rit_config().expect("rit config should read");
        let mobile = config
            .workspace_profile("mobile")
            .expect("mobile profile should exist");

        assert_eq!(mobile.include, vec!["apps/mobile", "packages/ui"]);

        remove_dir_all(&root);
    }

    #[test]
    fn reads_partial_clone_policy_from_config_and_promisor_markers() {
        let root = temp_path("repository-partial-clone");
        let repository = Repository::init(&InitOptions::new(&root)).expect("repo should init");
        let config_path = repository.common_dir().join("config");
        let mut config = fs::read_to_string(&config_path).expect("config should be readable");
        config.push_str(
            "\n[remote \"origin\"]\n\tpromisor = true\n\tpartialCloneFilter = blob:none\n",
        );
        fs::write(&config_path, config).expect("config should be updated");
        let pack_dir = repository.common_dir().join("objects").join("pack");
        fs::create_dir_all(&pack_dir).expect("pack dir should exist");
        fs::write(pack_dir.join("pack-test.promisor"), "").expect("marker should be written");

        let policy = repository
            .partial_clone_policy()
            .expect("partial clone policy should read");

        assert!(policy.is_enabled());
        assert_eq!(policy.promisor_remotes[0].name, "origin");
        assert_eq!(
            policy.promisor_remotes[0].partial_clone_filter.as_deref(),
            Some("blob:none")
        );
        assert_eq!(policy.promisor_pack_markers.len(), 1);

        remove_dir_all(&root);
    }

    #[test]
    fn reads_ssh_process_config_from_git_config() {
        let root = temp_path("ssh-process-config");
        let repository = Repository::init(&InitOptions::new(&root)).expect("repo should init");
        let config_path = repository.common_dir().join("config");
        let mut config = fs::read_to_string(&config_path).expect("config should be readable");
        config.push_str("\n[core]\n\tsshCommand = ssh -i config-key\n[ssh]\n\tvariant = simple\n");
        fs::write(&config_path, config).expect("config should be updated");

        let process_config = repository
            .ssh_process_config()
            .expect("ssh process config should read");

        assert_eq!(
            process_config.core_ssh_command.as_deref(),
            Some("ssh -i config-key")
        );
        assert_eq!(process_config.ssh_variant, Some(crate::SshVariant::Simple));
        remove_dir_all(&root);
    }

    #[test]
    fn fetch_remote_http_ingests_pack_and_writes_fetch_head() {
        let temp = temp_path("remote-http-fetch");
        let repository = Repository::init(&InitOptions::new(&temp)).expect("init should work");
        let local_commit = write_local_commit(&repository, "local commit", None);
        let object_data = b"hello";
        let object_id = hash_object(ObjectKind::Blob, object_data);
        let pack = pack_with_one_blob(object_data);
        let advertisement = upload_pack_advertisement(object_id);
        let upload_pack = upload_pack_response_with_pack(&pack);
        let (base_url, request_handle) = serve_http_requests(vec![
            http_response(
                "application/x-git-upload-pack-advertisement",
                &advertisement,
            ),
            http_response("application/x-git-upload-pack-result", &upload_pack),
        ]);
        let refspec =
            FetchRefSpec::parse("refs/heads/main:refs/remotes/origin/main").expect("refspec");
        let options =
            RemoteFetchOptions::new(TransportLocation::parse(&format!("{base_url}/repo.git")))
                .with_refspec(refspec);

        let result = repository
            .fetch_remote_http(&options)
            .expect("remote fetch should ingest pack");
        let requests = request_handle.join().expect("server thread");
        let post_request = String::from_utf8_lossy(&requests[1]);

        assert_eq!(result.fetch_head, object_id);
        assert_eq!(result.object_count, 1);
        assert_eq!(
            repository
                .read_object(object_id)
                .expect("fetched object should be readable")
                .data,
            object_data
        );
        assert_eq!(
            repository
                .resolve_revision("refs/remotes/origin/main")
                .expect("destination ref should resolve"),
            object_id
        );
        assert_eq!(
            fs::read_to_string(repository.git_dir().join("FETCH_HEAD"))
                .expect("FETCH_HEAD should be written"),
            format!("{object_id}\t\tbranch 'main' of {base_url}/repo.git\n")
        );
        assert_eq!(requests.len(), 2);
        assert!(post_request.contains(&format!("have {local_commit}\n")));

        remove_dir_all(&temp);
    }

    #[test]
    fn fetch_remote_ssh_ingests_pack_and_writes_fetch_head() {
        let temp = temp_path("remote-ssh-fetch");
        let repository = Repository::init(&InitOptions::new(&temp)).expect("init should work");
        let local_commit = write_local_commit(&repository, "local commit", None);
        let object_data = b"hello";
        let object_id = hash_object(ObjectKind::Blob, object_data);
        let pack = pack_with_one_blob(object_data);
        let advertisement = upload_pack_git_protocol_advertisement(object_id);
        let advertisement = SmartHttpAdvertisement::parse_git_protocol(
            SmartHttpService::UploadPack,
            &advertisement,
        )
        .expect("advertisement should parse");
        let refspec =
            FetchRefSpec::parse("refs/heads/main:refs/remotes/origin/main").expect("refspec");
        let location = TransportLocation::parse("git@example.test:org/repo.git");
        let executor = FakeSshUploadPackExecutor {
            negotiation: RemotePackNegotiation {
                advertisement,
                wanted_ref: "refs/heads/main".to_owned(),
                want_id: object_id,
                response: crate::UploadPackResponse::parse(&upload_pack_response_with_pack(&pack))
                    .expect("response should parse"),
                pack_bytes: pack,
            },
            expected_haves: vec![local_commit],
        };
        let options = RemoteFetchOptions::new(location).with_refspec(refspec);

        let result = repository
            .fetch_remote_ssh_with_executor(&options, &executor)
            .expect("remote fetch should ingest pack");

        assert_eq!(result.fetch_head, object_id);
        assert_eq!(result.object_count, 1);
        assert_eq!(
            repository
                .read_object(object_id)
                .expect("fetched object should be readable")
                .data,
            object_data
        );
        assert_eq!(
            repository
                .resolve_revision("refs/remotes/origin/main")
                .expect("destination ref should resolve"),
            object_id
        );
        assert_eq!(
            fs::read_to_string(repository.git_dir().join("FETCH_HEAD"))
                .expect("FETCH_HEAD should be written"),
            format!("{object_id}\t\tbranch 'main' of git@example.test:org/repo.git\n")
        );

        remove_dir_all(&temp);
    }

    #[test]
    fn push_remote_http_sends_pack_and_checks_status() {
        let temp = temp_path("remote-http-push");
        let repository = Repository::init(&InitOptions::new(&temp)).expect("init should work");
        let object_id = repository
            .loose_objects()
            .write_object(ObjectKind::Blob, b"hello")
            .expect("source object");
        let advertisement = receive_pack_advertisement();
        let status = receive_pack_status("refs/heads/main");
        let (base_url, request_handle) = serve_http_requests(vec![
            http_response(
                "application/x-git-receive-pack-advertisement",
                &advertisement,
            ),
            http_response("application/x-git-receive-pack-result", &status),
        ]);
        let refspec =
            FetchRefSpec::parse(&format!("{object_id}:refs/heads/main")).expect("refspec");
        let options = RemotePushOptions::new(
            TransportLocation::parse(&format!("{base_url}/repo.git")),
            refspec,
        );

        let result = repository
            .push_remote_http(&options)
            .expect("remote push should succeed");
        let requests = request_handle.join().expect("server thread");
        let post_request = String::from_utf8_lossy(&requests[1]);

        assert_eq!(result.destination, "refs/heads/main");
        assert_eq!(result.new_id, object_id);
        assert_eq!(result.object_count, 1);
        assert!(post_request.starts_with("POST /repo.git/git-receive-pack HTTP/1.1\r\n"));
        assert!(post_request.contains(&format!(
            "0000000000000000000000000000000000000000 {object_id} refs/heads/main"
        )));
        assert!(requests[1].windows(4).any(|window| window == b"PACK"));

        remove_dir_all(&temp);
    }

    #[test]
    fn push_remote_ssh_sends_pack_and_checks_status() {
        let temp = temp_path("remote-ssh-push");
        let repository = Repository::init(&InitOptions::new(&temp)).expect("init should work");
        let object_id = repository
            .loose_objects()
            .write_object(ObjectKind::Blob, b"hello")
            .expect("source object");
        let refspec =
            FetchRefSpec::parse(&format!("{object_id}:refs/heads/main")).expect("refspec");
        let location = TransportLocation::parse("git@example.test:org/repo.git");
        let executor = FakeSshReceivePackExecutor {
            ref_name: "refs/heads/main".to_owned(),
            new_id: object_id,
            status: ReceivePackStatus {
                unpack_error: None,
                commands: vec![ReceivePackCommandStatus::Ok {
                    ref_name: "refs/heads/main".to_owned(),
                }],
            },
        };
        let options = RemotePushOptions::new(location, refspec);

        let result = repository
            .push_remote_ssh_with_executor(&options, &executor)
            .expect("remote push should succeed");

        assert_eq!(result.destination, "refs/heads/main");
        assert_eq!(result.new_id, object_id);
        assert_eq!(result.object_count, 1);

        remove_dir_all(&temp);
    }

    fn upload_pack_advertisement(object_id: crate::ObjectId) -> Vec<u8> {
        let mut body = Vec::new();
        test_pkt_line(&mut body, b"# service=git-upload-pack\n");
        body.extend_from_slice(b"0000");
        test_pkt_line(&mut body, format!("{object_id} HEAD\n").as_bytes());
        test_pkt_line(
            &mut body,
            format!("{object_id} refs/heads/main\n").as_bytes(),
        );
        body.extend_from_slice(b"0000");
        body
    }

    fn upload_pack_git_protocol_advertisement(object_id: crate::ObjectId) -> Vec<u8> {
        let mut body = Vec::new();
        test_pkt_line(
            &mut body,
            format!("{object_id} HEAD\0multi_ack side-band-64k\n").as_bytes(),
        );
        test_pkt_line(
            &mut body,
            format!("{object_id} refs/heads/main\n").as_bytes(),
        );
        body.extend_from_slice(b"0000");
        body
    }

    fn upload_pack_response_with_pack(pack: &[u8]) -> Vec<u8> {
        let mut body = Vec::new();
        test_pkt_line(&mut body, b"NAK\n");
        body.extend_from_slice(pack);
        body
    }

    fn receive_pack_advertisement() -> Vec<u8> {
        let mut body = Vec::new();
        test_pkt_line(&mut body, b"# service=git-receive-pack\n");
        body.extend_from_slice(b"0000");
        body.extend_from_slice(b"0000");
        body
    }

    fn receive_pack_status(ref_name: &str) -> Vec<u8> {
        let mut body = Vec::new();
        test_pkt_line(&mut body, b"unpack ok\n");
        test_pkt_line(&mut body, format!("ok {ref_name}\n").as_bytes());
        body.extend_from_slice(b"0000");
        body
    }

    fn write_local_commit(
        repository: &Repository,
        message: &str,
        parent: Option<crate::ObjectId>,
    ) -> crate::ObjectId {
        let tree_id = repository
            .loose_objects()
            .write_object(ObjectKind::Tree, b"")
            .expect("empty tree should write");
        let parent_line = parent
            .map(|parent| format!("parent {parent}\n"))
            .unwrap_or_default();
        let commit = format!(
            "tree {tree_id}\n{parent_line}author Test User <test@example.test> 1700000000 +0000\ncommitter Test User <test@example.test> 1700000000 +0000\n\n{message}\n"
        );
        let commit_id = repository
            .loose_objects()
            .write_object(ObjectKind::Commit, commit.as_bytes())
            .expect("commit should write");
        let branch_path = repository
            .common_dir()
            .join("refs")
            .join("heads")
            .join("master");
        fs::write(branch_path, format!("{commit_id}\n")).expect("branch ref should write");
        commit_id
    }

    fn pack_with_one_blob(data: &[u8]) -> Vec<u8> {
        let mut object = Vec::new();
        object.push(0x30 | data.len() as u8);
        object.extend_from_slice(&zlib(data));

        let mut pack = Vec::new();
        pack.extend_from_slice(b"PACK");
        pack.extend_from_slice(&2_u32.to_be_bytes());
        pack.extend_from_slice(&1_u32.to_be_bytes());
        pack.extend_from_slice(&object);
        let checksum = sha1_bytes(&pack);
        pack.extend_from_slice(&checksum);
        pack
    }

    struct FakeSshUploadPackExecutor {
        negotiation: RemotePackNegotiation,
        expected_haves: Vec<crate::ObjectId>,
    }

    impl SshUploadPackExecutor for FakeSshUploadPackExecutor {
        fn negotiate_upload_pack(
            &self,
            location: &TransportLocation,
            wanted_ref: &str,
            haves: Vec<crate::ObjectId>,
        ) -> crate::Result<RemotePackNegotiation> {
            assert_eq!(location.protocol(), crate::TransportProtocol::Ssh);
            assert_eq!(wanted_ref, self.negotiation.wanted_ref);
            assert_eq!(haves, self.expected_haves);
            Ok(self.negotiation.clone())
        }
    }

    struct FakeSshReceivePackExecutor {
        ref_name: String,
        new_id: crate::ObjectId,
        status: ReceivePackStatus,
    }

    impl SshReceivePackExecutor for FakeSshReceivePackExecutor {
        fn send_receive_pack(
            &self,
            location: &TransportLocation,
            ref_name: &str,
            new_id: crate::ObjectId,
            pack_data: Vec<u8>,
        ) -> crate::Result<ReceivePackStatus> {
            assert_eq!(location.protocol(), crate::TransportProtocol::Ssh);
            assert_eq!(ref_name, self.ref_name);
            assert_eq!(new_id, self.new_id);
            assert!(pack_data.starts_with(b"PACK"));
            Ok(self.status.clone())
        }
    }

    fn zlib(data: &[u8]) -> Vec<u8> {
        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(data).expect("compress data");
        encoder.finish().expect("finish zlib")
    }

    fn serve_http_requests(responses: Vec<Vec<u8>>) -> (String, thread::JoinHandle<Vec<Vec<u8>>>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind test server");
        let address = listener.local_addr().expect("local address");
        let handle = thread::spawn(move || {
            let mut requests = Vec::new();
            for response in responses {
                let (mut stream, _) = listener.accept().expect("accept connection");
                let mut request = Vec::new();
                let mut buffer = [0_u8; 1024];
                loop {
                    let bytes_read = stream.read(&mut buffer).expect("read request");
                    if bytes_read == 0 {
                        break;
                    }
                    request.extend_from_slice(&buffer[..bytes_read]);
                    if request_is_complete(&request) {
                        break;
                    }
                }
                stream.write_all(&response).expect("write response");
                requests.push(request);
            }
            requests
        });
        (format!("http://127.0.0.1:{}", address.port()), handle)
    }

    fn http_response(content_type: &str, body: &[u8]) -> Vec<u8> {
        let mut response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        )
        .into_bytes();
        response.extend_from_slice(body);
        response
    }

    fn request_is_complete(request: &[u8]) -> bool {
        let Some(header_end) = request
            .windows(4)
            .position(|window| window == b"\r\n\r\n")
            .map(|index| index + 4)
        else {
            return false;
        };
        let header_text = String::from_utf8_lossy(&request[..header_end]);
        let Some(content_length) = header_text.lines().find_map(|line| {
            let (name, value) = line.split_once(':')?;
            name.eq_ignore_ascii_case("Content-Length")
                .then(|| value.trim().parse::<usize>().ok())
                .flatten()
        }) else {
            return true;
        };
        request.len() >= header_end + content_length
    }

    fn test_pkt_line(output: &mut Vec<u8>, payload: &[u8]) {
        let length = payload.len() + 4;
        output.extend_from_slice(format!("{length:04x}").as_bytes());
        output.extend_from_slice(payload);
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
