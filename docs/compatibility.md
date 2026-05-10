# Compatibility Baseline

This document records the Git baseline used while implementing `rit`.
`rit` must not call the `git` executable at runtime, but compatibility tests
and implementation notes may use Git as the reference implementation.

## Checked Baseline

- Date checked: 2026-05-10
- Git version: `git version 2.52.0.windows.1`
- Command list checked with: `git help -a`
- Command help checked with: `git <command> -h`

`git help <command>` may open a pager or local manual viewer on Windows, so
short help output is used for repeatable baseline capture in this workspace.

## Current Implemented Surface

The current codebase implements an early local Git subset:

- Repository discovery, init, bare repository detection, loose object I/O, and
  packed object reading for non-delta objects.
- Linked worktree discovery reads `.git` gitdir files plus `commondir` for
  shared metadata.
- Config reads use a shared scalar parser for repository format checks and
  user identity lookup.
- Index v2 read/write for regular files.
- Local refs, packed refs lookup, lightweight tags, and simple revision
  resolution.
- CLI commands: `version`, `help`, `init`, `rev-parse`, `cat-file`, `ls-tree`,
  `ls-files`, `show`, `status --porcelain=v1`, `diff --name-only`,
  `diff --name-status`, `diff --numstat`, `diff --stat`,
  `diff --cached --name-only`, `diff --cached --name-status`,
  `diff --cached --numstat`, `diff --cached --stat`, `log`, `add`, `commit`,
  `branch`, `tag`, `restore`, `reset`, `checkout`, and `switch`.
- Small text patch output is supported for default `diff`, `diff -p`, and
  `diff --cached`.
- Ordinary literal file and directory pathspec filtering is supported for
  `status --porcelain=v1` and the supported `diff` summary modes.
- Ordinary literal file and directory pathspec filtering is supported for
  `ls-files`, including `--stage`.
- Ordinary literal path lookup is supported for `ls-tree`.
- Ordinary literal path filtering is supported for first-parent `log`.
- Ordinary literal file, directory, and `.` pathspec expansion is supported for
  `add`, `restore`, and `reset`.
- Detached `checkout <commit>` is supported for clean worktrees.
- `branch -d` refuses unmerged local branches.

## Compatibility Policy

- Runtime command implementations must not shell out to `git`.
- Compatibility fixtures may run both `git` and `rit` against copied
  repositories and compare outputs plus repository state.
- Any intentional difference from Git must be listed in
  `docs/implementation-notes.md`.
- Unknown repository formats must be read-only or rejected with a clear error
  before any write operation.

## Priority Command Help Checked

- `git status -h`: porcelain, short, branch, ignored, untracked, rename, and
  pathspec options are the primary compatibility surface for status.
- `git add -h`: pathspecs, `--all`, `--update`, `--patch`, chmod, sparse, and
  pathspec-file options define the eventual add scope.
- `git commit -h`: message, author/date, hooks, amend, dry-run, signing, and
  pathspec commit modes define the eventual commit scope.
- `git diff -h`: patch, cached/staged, stat, numstat, name-only, name-status,
  rename/copy, and no-index modes define the eventual diff scope.
- `git log -h`: revision ranges, decoration, mailmap, path filtering, and diff
  output define the eventual log/show scope.

## Test Harness Direction

`rit-testkit` is the compatibility test crate. It creates isolated copies of a
fixture repository, runs the reference Git command in one copy and the `rit`
command in the other copy, then compares:

- stdout
- stderr
- exit code
- selected `.git` metadata
- working tree file snapshot

The `rit-cli` integration tests include reusable read-only diff fixtures for
default and cached output modes. Some worktree diff comparisons intentionally
skip repository-state comparison until Git-compatible index stat refresh is
implemented.

Pathspec compatibility tests currently cover ordinary literal file and
directory filters for `status --porcelain=v1` and supported `diff` summary
outputs, plus `ls-files` cached and staged output. Git pathspec magic, glob
matching, and pathspec-file input remain unsupported.

Status compatibility tests cover Git-like collapsed output for fully untracked
directories, including directory and exact-file pathspec behavior.
Status compatibility tests also cover porcelain quoting for paths containing
spaces.
One status compatibility test intentionally documents the remaining index stat
refresh difference: stdout/stderr/exit code match Git, but `.git/index` differs
because Git refreshes cached stat metadata and rit currently does not.

`ls-tree` compatibility tests cover literal directory and file path lookup with
default, `--name-only`, and `--object-only` output.

`log` compatibility tests cover `--oneline -- <pathspec>` on simple
first-parent histories.

`show` compatibility tests cover `--no-patch -- <pathspec>` for commits that
do and do not touch the requested path.

Patch compatibility tests cover default and cached text patches for small files.
Patch compatibility tests also cover missing trailing newline markers for
default and cached text patches.
Binary patch compatibility tests cover default and cached `Binary files ...
differ` placeholders.
Binary diff compatibility tests cover `--name-only`, `--name-status`,
`--numstat`, and `--stat` summary output.

Local write compatibility tests currently cover directory pathspec behavior for
`add`, `restore`, and `reset` by comparing the resulting porcelain status and
restored file contents.
Reusable local write fixture builders live in `rit-testkit` so new write
comparisons can share repository setup.

Detached checkout compatibility tests compare detached `.git/HEAD`, branch
state, porcelain status, and materialized file contents.

Branch deletion compatibility tests cover merged branch deletion and refusal to
delete an unmerged branch.
