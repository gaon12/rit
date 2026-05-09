# Compatibility Baseline

This document records the Git baseline used while implementing `rit`.
`rit` must not call the `git` executable at runtime, but compatibility tests
and implementation notes may use Git as the reference implementation.

## Checked Baseline

- Date checked: 2026-05-09
- Git version: `git version 2.52.0.windows.1`
- Command list checked with: `git help -a`
- Command help checked with: `git <command> -h`

`git help <command>` may open a pager or local manual viewer on Windows, so
short help output is used for repeatable baseline capture in this workspace.

## Current Implemented Surface

The current codebase implements an early local Git subset:

- Repository discovery, init, bare repository detection, loose object I/O, and
  packed object reading for non-delta objects.
- Index v2 read/write for regular files.
- Local refs, packed refs lookup, lightweight tags, and simple revision
  resolution.
- CLI commands: `version`, `help`, `init`, `rev-parse`, `cat-file`, `ls-tree`,
  `ls-files`, `show`, `status --porcelain=v1`, `diff --name-only`,
  `diff --stat`, `log`, `add`, `commit`, `branch`, `tag`, `restore`, `reset`,
  `checkout`, and `switch`.

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
- `git diff -h`: patch, stat, numstat, name-only, name-status, rename/copy, and
  no-index modes define the eventual diff scope.
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

