# Compatibility Baseline

This document records the Git baseline used while implementing `rit`.
`rit` must not call the `git` executable at runtime, but compatibility tests
and implementation notes may use Git as the reference implementation.

## Checked Baseline

- Date checked: 2026-05-22
- Git version: `git version 2.54.0.windows.1`
- Command list checked with: `git help -a`
- Command help checked with: `git <command> -h`

`git help <command>` may open a pager or local manual viewer on Windows, so
short help output is used for repeatable baseline capture in this workspace.

## CI Baseline Scope

The full `cargo test --workspace` compatibility suite currently runs against
the Windows Git baseline above. GitHub Actions runs that suite on
`windows-latest`, where the checked baseline and local validation match.

The CI workflow is triggered for pull requests and for all branch pushes. This
keeps milestone branches observable in GitHub Actions before a PR exists,
instead of waiting for a main/master push.

GitHub Actions release builds still compile `rit-min` and `rit-full` on
Ubuntu, macOS, and Windows. Earlier CI runs showed a stable pattern from run
1 through run 43: Windows tests passed, all release builds passed, and the
Ubuntu/macOS test jobs failed in `cargo test --workspace`. Until the test
fixtures normalize platform-specific Git output, file-mode, symlink, and
line-ending differences, Linux/macOS are treated as build-portability targets
rather than full compatibility-oracle targets.

## Current Implemented Surface

The current codebase implements an early local Git subset:

- Repository discovery, init, bare repository detection, loose object I/O, and
  packed object reading for whole and delta-compressed objects.
- Linked worktree discovery reads `.git` gitdir files plus `commondir` for
  shared metadata.
- Config reads use a shared scalar parser for repository format checks and
  user identity lookup.
- Transport locations are classified as local, HTTP(S), or SSH before command
  implementations choose the supported transfer path. Smart HTTP discovery
  request URLs, expected advertisement content types, and advertised-ref
  response parsing are modeled. Smart HTTP upload-pack and receive-pack
  requests can be sent over plain HTTP or platform-verified HTTPS.
- Index v2 read/write for regular files, including status stat refresh for
  clean tracked files, raw extension-byte preservation, and committed
  `100644`/`100755` executable-bit modes. Optional index extension records can
  be parsed and classified by signature while preserving raw payloads; `TREE`
  cache-tree payloads can be parsed into structured nodes and `REUC`
  resolve-undo payloads can be parsed into per-path stage records. `FSMN`
  file-system-monitor headers and `link` split-index shared-index IDs are
  parsed while preserving raw bitmap bytes. `sdir` sparse-directory marker
  extensions are recognized. `UNTR` untracked-cache headers and directory
  blocks are parsed, including EWAH bitmap/stat/hash tails. `EOIE` and `IEOT`
  extensions are parsed for fast extension/index-entry offset metadata.
- Local refs, packed refs lookup, lightweight tags, and simple revision
  resolution.
- CLI commands: `version`, `help`, `init`, `rev-parse`, `cat-file`, `ls-tree`,
  `ls-files`, `show`, `status --porcelain=v1`, `diff --name-only`,
  `diff --name-status`, `diff --numstat`, `diff --stat`,
  `diff --cached --name-only`, `diff --cached --name-status`,
  `diff --cached --numstat`, `diff --cached --stat`, `log`, `add`, `commit`,
  `branch`, `tag`, `restore`, `reset`, `checkout`, `switch`,
  fast-forward, clean merge-commit, and conflicted-state `merge` flows,
  `cherry-pick`, `rebase`, `stash`, and `clone --local --no-checkout`, plus
  local-path, smart HTTP(S), and one-refspec SSH `fetch` and `push`.
- Small text patch output is supported for default `diff`, `diff -p`, and
  `diff --cached`.
- Cached diff supports staged rename/copy detection with `-M[<n>]`,
  `--find-renames[=<n>]`, `-C[<n>]`, and `--find-copies[=<n>]` for summary
  modes and patch output. `--find-copies-harder` can use unchanged HEAD files
  as copy sources in cached diff. Exact renames are detected before the
  exhaustive `-l<n>` similarity limit is applied, and the supported limit
  model counts source/destination candidate width rather than total changed
  paths for one-source/one-destination similarity detection. When the
  supported exhaustive similarity pass is skipped by `-l<n>`, rit emits the
  Git-shaped warning on stderr. `--no-renames` disables the checked rename
  path, but checked `--find-copies-harder` copy detection, with or without
  `-C`, stays active like Git even when `--no-renames` appears before or
  after it. Plain checked `-C` copy detection follows Git's option order there: a later
  `--no-renames` disables it, while a later `-C` re-enables it. Checked
  `diff.renames=copies` config also matches Git on the covered cached
  rename, copy, and hard-copy slices rather than forcing extra copy
  promotion, and explicit `--no-renames` still overrides that checked
  `diff.renames=copies` default on the covered cached copy and hard-copy
  slices.
  Checked explicit `--find-copies-harder`, with or without `-C`,
  also overrides `diff.renames=false` like Git on the covered cached
  hard-copy slices, and plain checked `-C` does the same on the covered
  cached copy slices.
- Default worktree diff already performs checked exact rename detection for
  the Git-compatible intent-to-add slice, where added worktree paths are
  already represented in the index by `git add -N`. `diff.renames=false`
  disables that default rename detection, and `--no-renames` / later `-M`
  option order matches Git there. Invalid `diff.renames` values also fail the
  checked default worktree diff forms with the same fatal path. `-M[<n>]` and
  `-C[<n>]` cover the checked non-exact rename/copy cases for that same
  intent-to-add slice. Checked `--find-copies-harder` copy detection, with or
  without `-C`, also stays active like Git even when `--no-renames` appears
  before or after it.
  Plain checked `-C` copy detection likewise follows Git's option order
  there: a later `--no-renames` disables it, while a later `-C`
  re-enables it. Checked `diff.renames=copies` config likewise matches Git on
  the covered default worktree rename, copy, and hard-copy slices instead of
  forcing extra copy promotion, and explicit `--no-renames` still overrides
  that checked `diff.renames=copies` default on the covered default worktree
  copy and hard-copy slices. Checked explicit `--find-copies-harder`, with
  or without `-C`, also overrides `diff.renames=false` like Git on the
  covered default worktree hard-copy slices, and plain checked `-C` does the
  same on the covered default worktree copy slices.
  `--find-copies-harder` can use unchanged index entries as copy sources in
  that same worktree intent-to-add slice. Plain untracked worktree renames and
  copies stay outside default diff scope like Git, so they remain a delete or
  stay ignored rather than being promoted into rename/copy output.
- `diff.renameLimit=0` now has explicit compatibility coverage for cached and
  default worktree rename/copy detection, matching Git's unlimited behavior.
- Ordinary literal file and directory pathspec filtering is supported for
  `status --porcelain=v1` and the supported `diff` summary modes.
- Ordinary literal file and directory pathspec filtering is supported for
  `ls-files`, including `--stage`.
- Ordinary literal path lookup is supported for `ls-tree`.
- Ordinary literal path filtering is supported for first-parent `log`.
- Ordinary literal file, directory, and `.` pathspec expansion is supported for
  `add`, `restore`, and `reset`.
- `.gitattributes` parser support exists for ordinary rule lines, macro
  definitions, and set/unset/value/unspecified assignment states. Root
  `.gitattributes` rules are applied for `:(attr:...)` pathspecs in the shared
  path filtering used by supported `status`, `diff`, `ls-files`, and write
  paths. Nested attributes files, macro expansion, quoted patterns, and full
  Git wildcard syntax are still not implemented.
- `add --chmod=+x|-x` records executable-bit mode overrides in the index and
  committed trees.
- On Unix, `status --porcelain=v1` detects worktree executable-bit changes and
  `restore`/`checkout` materialize `100755` paths as executable. Windows keeps
  filemode-insensitive behavior.
- Symlink entries are stored as `120000` blobs containing the link target and
  are preserved through add, commit tree writing, status/diff hashing, and
  restore/checkout core paths. `core.symlinks=false` is honored by adding
  worktree symlinks as regular link-target blobs and by materializing committed
  symlink entries as plain link-target files.
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
- User-facing prose should be clear, educational, and `rit`-specific when the
  exact Git wording is not required for a stable machine-readable contract.
  Compatibility tests should prefer repository state, exit code, and structured
  output checks over copying Git's human help or advice text verbatim.

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
default and cached output modes. Worktree diff comparisons that do not mutate
state still skip repository-state comparison, while status refresh coverage
compares final `.git/index` state against Git.

Pathspec compatibility tests currently cover ordinary literal file/directory
filters and simple `*`, `?`, and bracket-class wildcard filters for
`status --porcelain=v1`, supported `diff` summary outputs, `ls-files`
cached/staged output, and first-parent `log` plus `show --no-patch`.
Positive `:(literal)`, `:(glob)`, `:(top)`, and `:/` pathspec magic is
covered for status, diff, ls-files, log, show, add, restore, and reset.
`:(icase)` is covered for status, diff, add, restore, reset, log, and show.
Exclude `:!`, `:^`, and `:(exclude)` is covered for status, diff, ls-files,
add, restore, reset, log, and show. Attr magic is
covered for root `.gitattributes` set/unset/value/unspecified requirements in
status, diff, ls-files, first-parent `log`, and `show --no-patch`.
Pathspec-file input is covered for `add`, `restore`, and `reset`, including
stdin and NUL-separated input, including the Git-compatible option order where
`--pathspec-file-nul` appears before `--pathspec-from-file` and where a later
`--no-pathspec-file-nul` still restores text mode before `--pathspec-from-file`.
Line-delimited pathspec-file input also covers Git-compatible rejection of
empty and badly quoted pathspec entries before repository mutation.
Local write compatibility tests cover `core.ignorecase=true` for a
mismatched-case `add` pathspec that Git accepts as a no-op. They also cover
Git-matching rejection of mismatched-case tracked pathspecs for `add` and
`restore` when `core.ignorecase=false`, plus the checked split where
`reset` accepts and `restore` rejects the same tracked path when
`core.ignorecase=true`. On the current Windows Git baseline, `reset` also
keeps that tracked-path no-op behavior when `core.ignorecase=false`.

Status compatibility tests cover Git-like collapsed output for fully untracked
directories, including directory and exact-file pathspec behavior.
Status compatibility tests also cover porcelain quoting for paths containing
spaces, and `--untracked-files=no|normal|all` / `-uno|-unormal|-uall`
untracked display modes, including Git's default-all `-u` form and
Git 2.52's normal-mode `--no-untracked-files` behavior.
Status compatibility tests also cover `-z` NUL-terminated porcelain output,
including raw paths with spaces.
Status compatibility tests cover `-b` / `--branch` branch headers for simple
local-branch, unborn-branch, and detached-HEAD repositories, including the
NUL-terminated form.
Status compatibility tests cover `--ignored` porcelain entries for simple
literal and directory ignore rules, glob ignore rules, negation, and
`.git/info/exclude`, including pathspec and NUL-terminated forms.
Pathspec compatibility tests cover positive `:(literal)`, `:(glob)`,
`:(top)`, and `:/` magic for status, diff, ls-files, log, show, add, restore,
and reset. They also cover `:(icase)` for status, diff, add, restore, reset,
log, and show, and exclude magic for status, diff, ls-files, add, restore,
reset, log, and show.
One status compatibility test covers index stat refresh: stdout/stderr/exit
code and final `.git/index` state must match Git after a clean tracked file's
mtime changes.
Additional read-only compatibility coverage checks that plain tracked
pathspec lookup stays case-sensitive for `status`, `diff`, `ls-files`,
`ls-tree`, `log`, and `show` on the current Windows Git baseline, regardless of
`core.ignorecase`.

`ls-tree` compatibility tests cover literal directory and file path lookup with
default, `--name-only`, and `--object-only` output.

`log` compatibility tests cover `--oneline -- <pathspec>` on simple
first-parent histories, including simple wildcard, bracket-class, exclude,
special `:(glob)` double-star forms, component-local non-recursive
`:(glob)**base.txt`, and attr pathspecs.

`show` compatibility tests cover default patch output for checked root and
single-parent commits, including checked literal, simple wildcard,
bracket-class, top-magic, exclude, and `:(icase)` path filters, special
`:(glob)` double-star forms, component-local non-recursive
`:(glob)**base.txt`, and attr pathspecs,
`--patch`/`--no-patch` option-order toggles, plus `--no-patch -- <pathspec>`
for commits that do and do not touch the requested path, including simple
wildcard, bracket-class, exclude, special
`:(glob)` double-star forms, component-local non-recursive
`:(glob)**base.txt`, and attr pathspecs. The checked `show` path forms also
include subdirectory invocation with relative and top-magic pathspecs.

Patch compatibility tests cover default and cached text patches for small files.
Patch compatibility tests also cover missing trailing newline markers for
default and cached text patches.
Binary patch compatibility tests cover default and cached `Binary files ...
differ` placeholders.
Patch compatibility tests cover splitting distant changes into multiple hunks.
Worktree rename/copy compatibility tests cover default `diff -M/-C` summary
and patch output when Git intent-to-add entries make added worktree paths part
of the index. Separate checks also pin Git's default treatment of ordinary
untracked worktree renames and copies, which stay outside diff scope.
Binary diff compatibility tests cover `--name-only`, `--name-status`,
`--numstat`, and `--stat` summary output.
Packed object compatibility tests cover reading a delta-compressed packed blob
created by `git gc --aggressive --prune=now` through `rit cat-file -p`.
Local clone compatibility tests cover `clone --local --no-checkout` by
comparing the cloned `HEAD` object and ensuring no checkout file is
materialized.
Local fetch compatibility tests cover `fetch <local-repository>` and one
`fetch <local-repository> <src>:<dst>` refspec in quiet mode by comparing
`FETCH_HEAD`, updated refs, and fetched commit contents.

Fast-forward merge compatibility coverage compares final `HEAD`, porcelain
status, and worktree contents for `merge --ff-only`.

Operation journal tests cover rit-specific `.git/rit/ops.log` metadata and
`rit undo`; this metadata is intentionally outside Git compatibility snapshots
and must be safe to delete.

Local write compatibility tests currently cover directory pathspec behavior and
simple wildcard/bracket-class pathspec behavior for `add`, `restore`, and
`reset` by comparing the resulting porcelain status and restored file contents.
Reusable local write fixture builders live in `rit-testkit` so new write
comparisons can share repository setup.
Local write compatibility tests also cover `add --chmod=+x` by comparing
porcelain status after staging and `git ls-tree` output after committing.
Unix-only unit coverage verifies executable-mode restore and worktree status
refresh behavior.
Unix-only unit coverage also verifies symlink add/restore, status target
hashing behavior, and `core.symlinks=false` add/restore parity.

Commit compatibility tests cover `--author` and raw `--date` overrides with
fixed committer environment variables by comparing the resulting `HEAD` object
ID and raw commit object contents.
Commit hook compatibility tests cover `commit-msg` message edits,
`pre-commit` failure blocking, `--no-verify` bypass behavior, and
`post-commit` side effects.

Detached checkout compatibility tests compare detached `.git/HEAD`, branch
state, porcelain status, and materialized file contents.

Branch deletion compatibility tests cover merged branch deletion and refusal to
delete an unmerged branch.
