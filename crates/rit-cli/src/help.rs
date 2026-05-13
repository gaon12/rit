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
  branch        List, create, or delete branches
  tag           List, create, or delete lightweight tags
  restore       Restore working tree or staged files
  reset         Reset staged file entries
  checkout      Switch branches
  switch        Switch branches
  merge         Fast-forward a branch or revision
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

Show a conservative porcelain v1 status.
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
rit merge [--plan] [--ff-only] <target>

Fast-forward the current branch to a local branch or revision, or print the planned fast-forward without writing. Merge commits and conflict handling are not implemented yet.
";

const OP_HELP: &str = "\
rit op log
rit op restore <id>

Inspect or restore the rit operation journal stored under .git/rit/ops.log.
";

const UNDO_HELP: &str = "\
rit undo

Restore HEAD and the working tree to the state captured before the last restorable rit operation.
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
rit workspace prefetch <profile>

Print the prefetch plan for a named workspace profile. Network prefetch execution is not implemented yet.
";

const DOCTOR_HELP: &str = "\
rit doctor

Run read-only repository health checks and print structured check results.
";

const REPAIR_HELP: &str = "\
rit repair [--dry-run|--apply]

Plan conservative repository repairs. Use --apply to create missing standard Git directories.
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
        "branch" => stdout.write_all(BRANCH_HELP.as_bytes())?,
        "tag" => stdout.write_all(TAG_HELP.as_bytes())?,
        "restore" => stdout.write_all(RESTORE_HELP.as_bytes())?,
        "reset" => stdout.write_all(RESET_HELP.as_bytes())?,
        "checkout" => stdout.write_all(CHECKOUT_HELP.as_bytes())?,
        "switch" => stdout.write_all(SWITCH_HELP.as_bytes())?,
        "merge" => stdout.write_all(MERGE_HELP.as_bytes())?,
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
