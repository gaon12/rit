use crate::{Branch, GitConfig, ObjectId, ObjectKind, Repository, Result, RitError, parse_commit};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

/// Read-only local repository graph summary.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalGraph {
    /// Current HEAD state.
    pub head: LocalGraphHead,
    /// Local branches in stable name order.
    pub branches: Vec<LocalGraphBranch>,
    /// Stash entries ordered as Git displays them.
    pub stashes: Vec<LocalGraphStash>,
    /// Known worktrees for this repository.
    pub worktrees: Vec<LocalGraphWorktree>,
}

/// Current HEAD graph state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalGraphHead {
    /// Current branch name when HEAD is symbolic.
    pub branch: Option<String>,
    /// Commit pointed to by HEAD when it is not unborn.
    pub target: Option<ObjectId>,
}

/// One local branch and its relationship to an upstream tracking ref.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalGraphBranch {
    /// Short local branch name.
    pub name: String,
    /// Commit pointed to by the local branch.
    pub target: ObjectId,
    /// Whether this branch is checked out in the current worktree.
    pub current: bool,
    /// Upstream tracking ref, when configured and resolvable.
    pub upstream: Option<LocalGraphUpstream>,
    /// Local commits not reachable from the upstream.
    pub ahead: usize,
    /// Upstream commits not reachable from the local branch.
    pub behind: usize,
    /// Whether this branch has local commits not known to its upstream.
    pub unpushed: bool,
    /// Whether both local and upstream have unique commits.
    pub diverged: bool,
}

/// Upstream tracking ref for a local branch.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalGraphUpstream {
    /// Full upstream ref name, such as `refs/remotes/origin/main`.
    pub name: String,
    /// Commit pointed to by the upstream ref.
    pub target: ObjectId,
}

/// One stash entry in the local graph.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalGraphStash {
    /// Display index, where newest is zero.
    pub index: usize,
    /// Stash message.
    pub message: String,
}

/// One known worktree.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalGraphWorktree {
    /// Stable local identifier for the worktree.
    pub id: String,
    /// Worktree path when known.
    pub path: Option<PathBuf>,
    /// Whether this entry describes the currently opened worktree.
    pub current: bool,
}

impl Repository {
    /// Builds a read-only graph of local repository state.
    pub fn local_graph(&self) -> Result<LocalGraph> {
        let config = read_common_config(self)?;
        let head = LocalGraphHead {
            branch: self.current_branch_name()?,
            target: self.resolve_head()?,
        };
        let branches = self
            .list_branches()?
            .into_iter()
            .map(|branch| self.local_graph_branch(branch, &config))
            .collect::<Result<Vec<_>>>()?;
        let stashes = self
            .stash_list()?
            .into_iter()
            .map(|stash| LocalGraphStash {
                index: stash.index,
                message: stash.message,
            })
            .collect();
        let worktrees = local_graph_worktrees(self)?;

        Ok(LocalGraph {
            head,
            branches,
            stashes,
            worktrees,
        })
    }

    fn local_graph_branch(&self, branch: Branch, config: &GitConfig) -> Result<LocalGraphBranch> {
        let upstream = self.local_graph_upstream(&branch.name, config)?;
        let (ahead, behind) = if let Some(upstream) = &upstream {
            self.ahead_behind(branch.target, upstream.target)?
        } else {
            (0, 0)
        };
        Ok(LocalGraphBranch {
            name: branch.name,
            target: branch.target,
            current: branch.current,
            upstream,
            ahead,
            behind,
            unpushed: ahead > 0,
            diverged: ahead > 0 && behind > 0,
        })
    }

    fn local_graph_upstream(
        &self,
        branch_name: &str,
        config: &GitConfig,
    ) -> Result<Option<LocalGraphUpstream>> {
        let Some(remote) = config.get_in_subsection("branch", Some(branch_name), "remote") else {
            return Ok(None);
        };
        let Some(merge) = config.get_in_subsection("branch", Some(branch_name), "merge") else {
            return Ok(None);
        };
        let upstream_name = upstream_ref_name(remote, merge);
        let target = match self.resolve_revision(&upstream_name) {
            Ok(target) => target,
            Err(RitError::InvalidInput { .. }) => return Ok(None),
            Err(error) => return Err(error),
        };
        Ok(Some(LocalGraphUpstream {
            name: upstream_name,
            target,
        }))
    }

    fn ahead_behind(&self, local: ObjectId, upstream: ObjectId) -> Result<(usize, usize)> {
        let local_reachable = self.reachable_commit_set(local)?;
        let upstream_reachable = self.reachable_commit_set(upstream)?;
        let ahead = local_reachable.difference(&upstream_reachable).count();
        let behind = upstream_reachable.difference(&local_reachable).count();
        Ok((ahead, behind))
    }

    fn reachable_commit_set(&self, start: ObjectId) -> Result<HashSet<ObjectId>> {
        let mut seen = HashSet::new();
        self.collect_reachable_commits(start, &mut seen)?;
        Ok(seen)
    }

    fn collect_reachable_commits(
        &self,
        object_id: ObjectId,
        seen: &mut HashSet<ObjectId>,
    ) -> Result<()> {
        if !seen.insert(object_id) {
            return Ok(());
        }
        let object = self.read_object(object_id)?;
        if object.kind != ObjectKind::Commit {
            return Ok(());
        }
        let commit = parse_commit(&object.data)?;
        for parent in commit.parents {
            self.collect_reachable_commits(parent, seen)?;
        }
        Ok(())
    }
}

fn read_common_config(repository: &Repository) -> Result<GitConfig> {
    let path = repository.common_dir().join("config");
    if path.exists() {
        GitConfig::read(&path)
    } else {
        Ok(GitConfig::default())
    }
}

fn upstream_ref_name(remote: &str, merge: &str) -> String {
    if remote == "." {
        return merge.to_owned();
    }
    let branch = merge.strip_prefix("refs/heads/").unwrap_or(merge);
    format!("refs/remotes/{remote}/{branch}")
}

fn local_graph_worktrees(repository: &Repository) -> Result<Vec<LocalGraphWorktree>> {
    let mut worktrees = Vec::new();
    let linked_root = repository.common_dir().join("worktrees");
    let current_is_linked = repository.git_dir().starts_with(&linked_root);
    worktrees.push(LocalGraphWorktree {
        id: "main".to_owned(),
        path: main_worktree_path(repository),
        current: !current_is_linked,
    });

    if !linked_root.is_dir() {
        return Ok(worktrees);
    }
    for entry in fs::read_dir(&linked_root).map_err(|source| RitError::io(&linked_root, source))? {
        let entry = entry.map_err(|source| RitError::io(&linked_root, source))?;
        let path = entry.path();
        if !entry
            .file_type()
            .map_err(|source| RitError::io(&path, source))?
            .is_dir()
        {
            continue;
        }
        let id = entry.file_name().to_string_lossy().into_owned();
        let worktree_path = read_linked_worktree_path(&path)?;
        let current = repository.git_dir() == path;
        worktrees.push(LocalGraphWorktree {
            id,
            path: worktree_path,
            current,
        });
    }
    Ok(worktrees)
}

fn main_worktree_path(repository: &Repository) -> Option<PathBuf> {
    if repository.is_bare() {
        return None;
    }
    repository.common_dir().parent().map(Path::to_path_buf)
}

fn read_linked_worktree_path(git_dir: &Path) -> Result<Option<PathBuf>> {
    let gitdir_path = git_dir.join("gitdir");
    if !gitdir_path.exists() {
        return Ok(None);
    }
    let text =
        fs::read_to_string(&gitdir_path).map_err(|source| RitError::io(&gitdir_path, source))?;
    Ok(PathBuf::from(text.trim()).parent().map(Path::to_path_buf))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AddOptions, CommitOptions, InitOptions};
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn local_graph_reports_head_branches_stashes_and_worktrees() {
        let root = temp_path("local-graph-basic");
        let repository = Repository::init(&InitOptions::new(&root)).expect("repo should init");
        write_user_config(&repository, "");
        write_commit(&repository, &root, "base", "base\n");
        repository
            .create_branch("topic")
            .expect("branch should create");
        let stash_id = repository
            .resolve_head()
            .expect("head should resolve")
            .unwrap();
        repository
            .stash_store(stash_id, Some("WIP on master: base"))
            .expect("stash should store");

        let graph = repository.local_graph().expect("graph should build");

        assert_eq!(graph.head.branch.as_deref(), Some("master"));
        assert!(graph.head.target.is_some());
        assert!(graph.branches.iter().any(|branch| {
            branch.name == "master" && branch.current && !branch.unpushed && !branch.diverged
        }));
        assert!(graph.branches.iter().any(|branch| branch.name == "topic"));
        assert_eq!(graph.stashes.len(), 1);
        assert_eq!(graph.stashes[0].message, "WIP on master: base");
        assert!(
            graph
                .worktrees
                .iter()
                .any(|worktree| worktree.id == "main" && worktree.current)
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn local_graph_reports_upstream_ahead_behind_and_diverged() {
        let root = temp_path("local-graph-upstream");
        let repository = Repository::init(&InitOptions::new(&root)).expect("repo should init");
        write_user_config(&repository, "");
        let base = write_commit(&repository, &root, "base", "base\n");
        repository
            .create_branch("main")
            .expect("branch should create");
        let remote_ref = repository.common_dir().join("refs/remotes/origin");
        fs::create_dir_all(&remote_ref).expect("remote refs should create");

        write_user_config(
            &repository,
            "[branch \"master\"]\nremote = origin\nmerge = refs/heads/main\n",
        );
        let upstream = write_commit(&repository, &root, "remote", "remote\n");
        fs::write(remote_ref.join("main"), format!("{upstream}\n"))
            .expect("remote ref should write");
        fs::write(
            repository.common_dir().join("refs/heads/master"),
            format!("{base}\n"),
        )
        .expect("branch should rewind");
        let local = write_commit(&repository, &root, "local", "local\n");
        fs::write(
            repository.common_dir().join("refs/heads/master"),
            format!("{local}\n"),
        )
        .expect("branch should point at local");

        let graph = repository.local_graph().expect("graph should build");
        let master = graph
            .branches
            .iter()
            .find(|branch| branch.name == "master")
            .expect("master branch should exist");

        assert_eq!(
            master
                .upstream
                .as_ref()
                .map(|upstream| upstream.name.as_str()),
            Some("refs/remotes/origin/main")
        );
        assert_eq!(master.ahead, 1);
        assert_eq!(master.behind, 1);
        assert!(master.unpushed);
        assert!(master.diverged);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn local_graph_marks_current_linked_worktree() {
        let root = temp_path("local-graph-linked-worktree");
        let main_worktree = root.join("main");
        let linked_worktree = root.join("linked");
        let repository =
            Repository::init(&InitOptions::new(&main_worktree)).expect("repo should init");
        write_user_config(&repository, "");
        write_commit(&repository, &main_worktree, "base", "base\n");

        let linked_git_dir = repository.common_dir().join("worktrees").join("linked");
        fs::create_dir_all(&linked_git_dir).expect("linked git dir should create");
        fs::create_dir_all(&linked_worktree).expect("linked worktree should create");
        let linked_dot_git = linked_worktree.join(".git");
        fs::write(
            &linked_dot_git,
            format!("gitdir: {}\n", linked_git_dir.display()),
        )
        .expect("linked .git file should write");
        fs::write(linked_git_dir.join("commondir"), "../..").expect("commondir should write");
        fs::write(linked_git_dir.join("HEAD"), "ref: refs/heads/master\n")
            .expect("linked HEAD should write");
        fs::write(
            linked_git_dir.join("gitdir"),
            format!(
                "{}\n",
                fs::canonicalize(&linked_dot_git)
                    .expect("linked .git should canonicalize")
                    .display()
            ),
        )
        .expect("linked gitdir should write");

        let linked_repository =
            Repository::open(&linked_worktree).expect("linked repo should open");
        let graph = linked_repository.local_graph().expect("graph should build");
        let main_path = fs::canonicalize(&main_worktree).expect("main should canonicalize");
        let linked_path = fs::canonicalize(&linked_worktree).expect("linked should canonicalize");

        assert!(graph.worktrees.iter().any(|worktree| {
            worktree.id == "main"
                && worktree.path.as_deref() == Some(main_path.as_path())
                && !worktree.current
        }));
        assert!(graph.worktrees.iter().any(|worktree| {
            worktree.id == "linked"
                && worktree.path.as_deref() == Some(linked_path.as_path())
                && worktree.current
        }));
        let _ = fs::remove_dir_all(root);
    }

    fn write_commit(
        repository: &Repository,
        worktree: &Path,
        message: &str,
        contents: &str,
    ) -> ObjectId {
        fs::write(worktree.join("tracked.txt"), contents).expect("file should write");
        repository
            .add_paths_with_options(&["tracked.txt".to_owned()], &AddOptions::default())
            .expect("add should work");
        let mut options = CommitOptions::new();
        options.verify = false;
        repository
            .commit_index_with_options(message, &options)
            .expect("commit should work")
            .commit_id
    }

    fn write_user_config(repository: &Repository, extra_config: &str) {
        fs::write(
            repository.common_dir().join("config"),
            format!("[user]\nname = Rit Test\nemail = rit@example.test\n{extra_config}"),
        )
        .expect("config should write");
    }

    fn temp_path(name: &str) -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time should be available")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("rit-{name}-{suffix}"));
        let _ = fs::remove_dir_all(&path);
        path
    }
}
