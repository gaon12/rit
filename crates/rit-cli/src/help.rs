use std::io::{self, Write};
use std::process::ExitCode;

pub const GENERAL_HELP: &str = "\
rit - a readable Rust implementation of Git

Usage:
  rit <command> [<args>]

Core commands:
  version       Display rit version information
  help          Display help for rit or a command
  init          Create an empty Git repository
  clone         Clone a local repository without checkout
  fetch         Fetch objects from a local or HTTP(S) repository
  push          Push one ref to an HTTP(S) repository
  rev-parse     Inspect the current repository paths
  cat-file      Inspect loose objects
  ls-tree       List entries in a tree object
  status        Show porcelain working tree status
  diff          Show working tree changes
  log           Show commit history
  add           Add file contents to the index
  commit        Record staged changes
  compat        Compare safe rit commands against Git
  branch        List, create, or delete branches
  tag           List, create, or delete lightweight tags
  restore       Restore working tree or staged files
  reset         Reset staged file entries
  checkout      Switch branches
  switch        Switch branches
  merge         Merge a branch or revision into the current branch
  rebase        Manage an in-progress rebase
  cherry-pick   Apply one clean commit onto HEAD
  stash         List saved working tree states
  auth          Explain remote authentication selection
  indexdb       Manage optional SQLite auxiliary index metadata
  large-files   Audit and plan large-file tracking
  file-history  Show indexed first-parent path history
  graph         Show the local branch, stash, and worktree graph
  impact        Summarize package and CI impact for a commit range
  schema        Print stable JSON schemas for machine-readable output
  op            Inspect or restore rit operation journal entries
  undo          Undo the last restorable rit operation
  show          Show one object
  ls-files      Show files in the index
  ignore        Explain ignore rule decisions
  pathspec      Explain pathspec parsing and matching
  workspace     Inspect workspace profile operations
  doctor        Check repository health without modifying it
  repair        Plan or apply conservative repository repairs

Run 'rit help <command>' for command-specific notes.
";

const VERSION_HELP: &str = "\
rit version

Display version information for this rit binary.
";

const HELP_HELP: &str = "\
rit help [<command>]

Display general help or command-specific help.
";

const INIT_HELP: &str = "\
rit init [-q|--quiet] [--bare] [-b <branch>|--initial-branch <branch>] [<directory>]

Create an empty Git-compatible repository.
";

const CLONE_HELP: &str = "\
rit clone [-q|--quiet] --local --no-checkout <source> [<directory>]

Clone a local repository by copying objects and refs. Checkout is not implemented yet.
";

const FETCH_HELP: &str = "\
rit fetch [-q|--quiet] <repository> [<src>:<dst>]

Fetch objects from a local, smart HTTP(S), or SSH repository and write FETCH_HEAD.
";

const PUSH_HELP: &str = "\
rit push [-q|--quiet] <repository> <src>:<dst>

Push one local source ref or revision to a smart HTTP(S) or SSH repository.
";

const REV_PARSE_HELP: &str = "\
rit rev-parse [--git-dir] [--show-toplevel] [--is-inside-work-tree] [<revision>...]

Print selected paths, repository facts, or resolved object IDs.
";

const CAT_FILE_HELP: &str = "\
rit cat-file (-t|-s|-p|<type>) <object>

Read a loose object and print its type, size, pretty contents, or raw contents.
";

const LS_TREE_HELP: &str = "\
rit ls-tree [--name-only|--object-only] <tree> [--] [<pathspec>...]

List entries in a loose tree object.
";

const STATUS_HELP: &str = "\
rit status --porcelain[=v1] [-b] [-z] [--ignored] [-uno|-unormal|-uall] [--] [<pathspec>...]
rit status --explain <path>

Show a conservative porcelain v1 status, or explain how one path is classified.
";

const DIFF_HELP: &str = "\
rit diff [--cached|--staged] [-M[<n>]|--find-renames[=<n>]] [-C[<n>]|--find-copies[=<n>]] [--find-copies-harder] [--name-only|--name-status|--numstat|--stat|-p] [--] [<pathspec>...]

Show working tree changes compared with the index, or staged changes compared with HEAD.
";

const LOG_HELP: &str = "\
rit log [--oneline] [--] [<pathspec>...]

Show commits reachable from HEAD by following the first parent.
";

const ADD_HELP: &str = "\
rit add [--plan] [--chmod=(+|-)x] [--pathspec-from-file <file>] [--pathspec-file-nul] <file>...

Add regular files to the index, or print the planned index changes without writing.
";

const COMMIT_HELP: &str = "\
rit commit [-m <message>] [--plan]

Create a commit from the current index and advance HEAD, or print the planned commit without writing.
";

const COMPAT_HELP: &str = "\
rit compat check [--] <command> [args...]
rit compat report --since <rev>
rit compat fixture generate [path]

Compare read-only rit command output against the current Git executable, summarize a small compatibility report for the current repository, or create a basic Git fixture repository. This compatibility command is allowed to invoke Git as a reference implementation; normal rit commands must not shell out to Git.
";

const BRANCH_HELP: &str = "\
rit branch
rit branch --show-current
rit branch <branch-name>
rit branch -d <branch-name>

List, create, or delete local branches.
";

const TAG_HELP: &str = "\
rit tag
rit tag <tag-name>
rit tag -d <tag-name>

List, create, or delete lightweight tags.
";

const RESTORE_HELP: &str = "\
rit restore [--staged] [--pathspec-from-file <file>] [--pathspec-file-nul] <file>...

Restore working tree files from the index, or staged files from HEAD.
";

const RESET_HELP: &str = "\
rit reset [--plan] [--pathspec-from-file <file>] [--pathspec-file-nul] <file>...

Reset staged file entries from HEAD, or print the planned index changes without writing.
";

const CHECKOUT_HELP: &str = "\
rit checkout <branch>
rit checkout <commit>
rit checkout -b <branch>

Switch to an existing branch, detach at a commit, or create and switch to a new branch.
";

const SWITCH_HELP: &str = "\
rit switch <branch>
rit switch -c <branch>

Switch to an existing branch, or create and switch to a new branch.
";

const MERGE_HELP: &str = "\
rit merge [--plan] [--ff-only] [--no-verify] <target>
rit merge --abort
rit merge --quit
rit merge --continue [--no-verify]
rit merge explain <target>

Fast-forward the current branch to a local branch or revision, create a clean merge commit, start a conflicted text merge, abort, quit, or continue an in-progress merge, print a dry-run plan, or explain the merge decision without writing.
";

const CHERRY_PICK_HELP: &str = "\
rit cherry-pick [-n|--no-commit] [--ff] [-s] [-x] [-m <parent-number>] <commit>...
rit cherry-pick --abort
rit cherry-pick --continue
rit cherry-pick --skip
rit cherry-pick --quit

Apply one non-merge commit onto the current HEAD, or manage an in-progress conflicted cherry-pick.
";

const REBASE_HELP: &str = "\
rit rebase --abort
rit rebase --continue
rit rebase --show-current-patch
rit rebase --skip
rit rebase --quit
rit rebase <upstream>

Abort an in-progress rebase by restoring the original branch, index, and working tree, continue a resolved final stopped commit, show the current stopped patch, skip a final stopped commit, clear Git-compatible rebase state while leaving HEAD, the index, and working tree unchanged, or report an already up-to-date branch for `<upstream>`.
";

const STASH_HELP: &str = "\
rit stash list
rit stash [push [(-m|--message) <message>] [-S|--staged] [-k|--keep-index] [-u|--include-untracked] [-a|--all] [-q|--quiet] [--pathspec-from-file <file>] [--pathspec-file-nul] [--] [<pathspec>...]]
rit stash save [-q|--quiet] [-k|--keep-index] [-S|--staged] [-u|--include-untracked] [-a|--all] [<message>]
rit stash show [-u|--include-untracked|--no-include-untracked|--only-untracked] [-p|--patch|--patch-with-stat|--patch-with-raw|--no-patch|--quiet|--exit-code|--stat|--compact-summary|--shortstat|--raw|--summary|--name-only|--name-status|--numstat] [--full-index|--abbrev[=<n>]|-U<n>|--unified=<n>|--diff-filter=<letters>|--no-prefix|--default-prefix|--no-ext-diff|--ext-diff|--no-color|--color=never|--color=auto] [<stash>]
rit stash drop [-q|--quiet] [<stash>]
rit stash apply [--index] [-q|--quiet] [<stash>]
rit stash pop [--index] [-q|--quiet] [<stash>]
rit stash branch <branchname> [<stash>]
rit stash create [<message>]
rit stash store [(-m|--message) <message>] [-q|--quiet] <commit>
rit stash clear

Save tracked changes, create a stash commit, list, show, drop, or clear entries from the Git-compatible refs/stash reflog.
";

const AUTH_HELP: &str = "\
rit auth explain <url>

Explain remote auth protocol selection, credential request fields, and available redacted token sources without reading or printing secrets.
";

const INDEXDB_HELP: &str = "\
rit indexdb [status|build|update|repair|rebuild|drop|vacuum]

Manage the optional SQLite auxiliary index under .git/rit/indexdb.sqlite. The database is reproducible metadata and is never the source of truth for Git objects, refs, index, or working tree state.
";

const LARGE_FILES_HELP: &str = "\
rit large-files audit [--threshold <bytes>]

Audit blobs reachable from HEAD history, recommend LFS/Xet tracking patterns, and print a safe migration plan. This command does not rewrite history or change tracking rules.
";

const FILE_HISTORY_HELP: &str = "\
rit file-history <path>

Show indexed first-parent add, modify, and delete history for one repository-relative path. This command uses the optional SQLite indexdb feature and creates or updates reproducible index metadata when needed.
";

const GRAPH_HELP: &str = "\
rit graph [--json]

Show a read-only local graph for HEAD, local branches, upstreams, stashes, and worktrees. JSON output uses the same typed graph model exposed by rit-core.
";

const IMPACT_HELP: &str = "\
rit impact <range>

Summarize changed packages, affected tests, public API path hints, docs-only status, large-file changes, reviewer hints, semantic path categories, and optional indexdb acceleration availability for `<old>..<new>`.
";

const SCHEMA_HELP: &str = "\
rit schema <status|diff|doctor|operations|impact|indexdb>

Print a stable JSON Schema document for rit machine-readable output. The same schema documents are exposed by the rit-core JSON schema API.
";

const OP_HELP: &str = "\
rit op log [--json]
rit op restore <id>

Inspect or restore the rit operation journal stored under .git/rit/ops.log.
";

const UNDO_HELP: &str = "\
rit undo [--preserve-changes]

Restore HEAD and the working tree to the state captured before the last restorable rit operation.
Use --preserve-changes with commit undo to move HEAD back while keeping the commit contents staged and present in the working tree.
";

const SHOW_HELP: &str = "\
rit show [--no-patch] [<revision>] [--] [<pathspec>...]

Show one commit, tree, or blob object. Commit diffs are not emitted yet.
";

const LS_FILES_HELP: &str = "\
rit ls-files [--stage] [--] [<pathspec>...]

Show files tracked in the index.
";

const IGNORE_HELP: &str = "\
rit ignore explain <path>

Explain which .gitignore or info/exclude rules decide whether a path is ignored.
";

const PATHSPEC_HELP: &str = "\
rit pathspec explain <pathspec>...

Explain how rit parses pathspec magic, matching mode, exclusions, wildcards, case handling, and attributes.
";

const WORKSPACE_HELP: &str = "\
rit workspace suggest
rit workspace from-change
rit workspace from-package <path>
rit workspace prefetch <profile>
rit workspace explain <profile>

Suggest workspace profiles from current changes, infer a workspace from one package path, print the prefetch plan, or explain workspace, partial-clone, LFS, and Xet decisions for a named profile. Network prefetch execution is not implemented yet.
";

const DOCTOR_HELP: &str = "\
rit doctor [--json|--explain|--fix-plan]

Run read-only repository health checks and print text, JSON, explained check results, or a safe fix plan.
";

const REPAIR_HELP: &str = "\
rit repair [--dry-run|--apply] [--drop-indexdb]

Plan conservative repository repairs. Use --apply to perform them. Corrupted optional indexdb metadata is rebuilt by default; --drop-indexdb leaves it absent instead.
";

pub fn print_command_help(
    topic: &str,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> io::Result<ExitCode> {
    match topic {
        "version" => stdout.write_all(VERSION_HELP.as_bytes())?,
        "help" => stdout.write_all(HELP_HELP.as_bytes())?,
        "init" => stdout.write_all(INIT_HELP.as_bytes())?,
        "clone" => stdout.write_all(CLONE_HELP.as_bytes())?,
        "fetch" => stdout.write_all(FETCH_HELP.as_bytes())?,
        "push" => stdout.write_all(PUSH_HELP.as_bytes())?,
        "rev-parse" => stdout.write_all(REV_PARSE_HELP.as_bytes())?,
        "cat-file" => stdout.write_all(CAT_FILE_HELP.as_bytes())?,
        "ls-tree" => stdout.write_all(LS_TREE_HELP.as_bytes())?,
        "status" => stdout.write_all(STATUS_HELP.as_bytes())?,
        "diff" => stdout.write_all(DIFF_HELP.as_bytes())?,
        "log" => stdout.write_all(LOG_HELP.as_bytes())?,
        "add" => stdout.write_all(ADD_HELP.as_bytes())?,
        "commit" => stdout.write_all(COMMIT_HELP.as_bytes())?,
        "compat" => stdout.write_all(COMPAT_HELP.as_bytes())?,
        "branch" => stdout.write_all(BRANCH_HELP.as_bytes())?,
        "tag" => stdout.write_all(TAG_HELP.as_bytes())?,
        "restore" => stdout.write_all(RESTORE_HELP.as_bytes())?,
        "reset" => stdout.write_all(RESET_HELP.as_bytes())?,
        "checkout" => stdout.write_all(CHECKOUT_HELP.as_bytes())?,
        "switch" => stdout.write_all(SWITCH_HELP.as_bytes())?,
        "merge" => stdout.write_all(MERGE_HELP.as_bytes())?,
        "rebase" => stdout.write_all(REBASE_HELP.as_bytes())?,
        "cherry-pick" => stdout.write_all(CHERRY_PICK_HELP.as_bytes())?,
        "stash" => stdout.write_all(STASH_HELP.as_bytes())?,
        "auth" => stdout.write_all(AUTH_HELP.as_bytes())?,
        "indexdb" => stdout.write_all(INDEXDB_HELP.as_bytes())?,
        "large-files" => stdout.write_all(LARGE_FILES_HELP.as_bytes())?,
        "file-history" => stdout.write_all(FILE_HISTORY_HELP.as_bytes())?,
        "graph" => stdout.write_all(GRAPH_HELP.as_bytes())?,
        "impact" => stdout.write_all(IMPACT_HELP.as_bytes())?,
        "schema" => stdout.write_all(SCHEMA_HELP.as_bytes())?,
        "op" => stdout.write_all(OP_HELP.as_bytes())?,
        "undo" => stdout.write_all(UNDO_HELP.as_bytes())?,
        "show" => stdout.write_all(SHOW_HELP.as_bytes())?,
        "ls-files" => stdout.write_all(LS_FILES_HELP.as_bytes())?,
        "ignore" => stdout.write_all(IGNORE_HELP.as_bytes())?,
        "pathspec" => stdout.write_all(PATHSPEC_HELP.as_bytes())?,
        "workspace" => stdout.write_all(WORKSPACE_HELP.as_bytes())?,
        "doctor" => stdout.write_all(DOCTOR_HELP.as_bytes())?,
        "repair" => stdout.write_all(REPAIR_HELP.as_bytes())?,
        unknown => {
            writeln!(stderr, "rit: no help for unknown command '{unknown}'")?;
            return Ok(ExitCode::from(129));
        }
    }

    Ok(ExitCode::SUCCESS)
}
