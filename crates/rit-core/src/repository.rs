use crate::{GitAttributes, GitConfig, GitObject, LooseObjectDb, ObjectId, Result, RitError};
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

    /// Returns the working tree root, or `None` for bare repositories.
    pub fn worktree(&self) -> Option<&Path> {
        self.worktree.as_deref()
    }

    /// Returns whether the repository is bare.
    pub fn is_bare(&self) -> bool {
        self.bare
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
        match config.get("core", key) {
            Some(value) => parse_git_bool(value, &format!("core.{key}")),
            None => Ok(default),
        }
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

fn parse_git_bool(value: &str, name: &str) -> Result<bool> {
    match value.trim().to_ascii_lowercase().as_str() {
        "true" | "yes" | "on" | "1" => Ok(true),
        "false" | "no" | "off" | "0" => Ok(false),
        _ => Err(RitError::invalid_input(format!(
            "invalid boolean config value for {name}: {value}"
        ))),
    }
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
    use super::{InitOptions, Repository};
    use std::fs;
    use std::path::{Path, PathBuf};
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
