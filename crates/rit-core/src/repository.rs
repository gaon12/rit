use crate::{GitObject, LooseObjectDb, ObjectId, Result, RitError};
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
                return Ok(Self {
                    worktree: Some(current),
                    common_dir: git_dir.clone(),
                    git_dir,
                    bare: false,
                });
            }

            if dot_git.is_file() {
                let git_dir = read_gitdir_file(&dot_git)?;
                let resolved_git_dir = if git_dir.is_absolute() {
                    git_dir
                } else {
                    current.join(git_dir)
                };
                let git_dir = canonicalize_existing_path(&resolved_git_dir)?;
                return Ok(Self {
                    worktree: Some(current),
                    common_dir: git_dir.clone(),
                    git_dir,
                    bare: false,
                });
            }

            if looks_like_bare_repository(&current) {
                let git_dir = canonicalize_existing_path(&current)?;
                return Ok(Self {
                    worktree: None,
                    common_dir: git_dir.clone(),
                    git_dir,
                    bare: true,
                });
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

        Ok(Self {
            worktree: (!options.bare).then_some(target),
            common_dir: git_dir.clone(),
            git_dir,
            bare: options.bare,
        })
    }

    /// Returns the path to the repository metadata directory.
    pub fn git_dir(&self) -> &Path {
        &self.git_dir
    }

    /// Returns the common metadata directory. This is currently the same as
    /// `git_dir`, with a separate accessor reserved for linked worktrees.
    pub fn common_dir(&self) -> &Path {
        &self.common_dir
    }

    /// Returns a loose object database reader for this repository.
    pub fn loose_objects(&self) -> LooseObjectDb {
        LooseObjectDb::new(self.common_dir.join("objects"))
    }

    /// Reads a loose object by its full object ID.
    pub fn read_object(&self, object_id: ObjectId) -> Result<GitObject> {
        self.loose_objects().read_object(object_id)
    }

    /// Returns the working tree root, or `None` for bare repositories.
    pub fn worktree(&self) -> Option<&Path> {
        self.worktree.as_deref()
    }

    /// Returns whether the repository is bare.
    pub fn is_bare(&self) -> bool {
        self.bare
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
    fn init_rejects_invalid_initial_branch() {
        let temp = temp_path("invalid-branch");
        let mut options = InitOptions::new(&temp);
        options.initial_branch = "bad branch".to_owned();

        let error = Repository::init(&options).expect_err("branch validation should fail");

        assert!(error.to_string().contains("invalid initial branch name"));
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
