# Milestone Tracker

This file is the day-to-day implementation tracker for `rit`. The larger
product direction lives in `docs/roadmap.md`; this file records concrete
status, next actions, and completion criteria so implementation work does not
drift.

## Status Legend

- `[x]` Done and committed
- `[~]` In progress
- `[ ]` Not started
- `[!]` Blocked or deliberately deferred

## Current Baseline

- Date: 2026-05-09
- Reference Git: `git version 2.52.0.windows.1`
- Required recurring checks:
  - `git --version`
  - `git help -a`
  - `git <command> -h` for each command being implemented
  - `cargo fmt --all`
  - `cargo clippy --workspace --all-targets -- -D warnings`
  - `cargo test --workspace`

## M0: Baseline And Rules

- [x] Record active Git baseline in `docs/compatibility.md`.
- [x] Keep implementation notes for each command.
- [x] Keep no-wrapper policy explicit: production code must not execute `git`.
- [x] Keep quality gates documented.

Completion criteria:
- Baseline documents exist and name the checked Git version.
- Quality gates are documented and pass before each commit.

## M1: Compatibility Test Harness

- [x] Provide `rit-testkit` library.
- [x] Provide `rit-testkit` CLI.
- [x] Compare stdout, stderr, and exit code.
- [x] Compare repository snapshots.
- [x] Add reusable checked-in fixtures for common read-only commands.
- [~] Add fixtures for local write commands.
  - [x] Generated Git-vs-rit compatibility scenarios for directory pathspec
    `add`, `restore`, and `reset`.
  - [ ] Reusable checked-in local write fixtures.
- [x] Add focused reports for first differing stdout/stderr line.

Completion criteria:
- A contributor can add a Git-vs-rit comparison without hand-writing process
  setup code.

## M2: Core Repository Model

- [x] Repository discovery from nested paths.
- [x] `Repository::open(path)` public entry point.
- [x] Basic bare repository detection.
- [x] Repository format version guard.
- [x] Unknown repository extension guard.
- [ ] Linked worktree/common-dir support.
- [ ] More complete config parser.

Completion criteria:
- Read and write operations fail clearly before touching unsupported
  repository formats.

## M3: Read-Only Local Commands

- [x] `rit version`
- [x] `rit help`
- [x] `rit rev-parse` for repository facts and simple revisions.
- [x] `rit cat-file` for objects available in the object database.
- [x] `rit ls-tree`
- [x] `rit ls-files`
- [x] `rit show --no-patch` and object display basics.
- [x] `rit log` first-parent traversal.
- [x] `rit status --porcelain=v1` basic tracked/untracked state.
- [x] `rit diff --name-only`
- [x] `rit diff --name-status`
- [x] `rit diff --stat`
- [x] `rit diff --numstat`
- [ ] `rit diff` patch output.
- [~] Pathspec support for read-only commands.
  - [x] Ordinary literal pathspec filters for `status --porcelain=v1` and
    `diff` summary modes.
  - [ ] Pathspec filters for `log`, `show`, `ls-tree`, and `ls-files`.
- [ ] Rename detection.
- [ ] Binary diff accounting.

Completion criteria:
- Common read-only commands have Git comparison tests for simple repositories.

## M4: Local Write Commands

- [x] `rit init`
- [x] `rit add` explicit regular files.
- [x] `rit commit -m`
- [x] `rit branch` local list/create/delete basics.
- [x] `rit tag` lightweight tag list/create/delete basics.
- [x] `rit restore` explicit tracked files.
- [x] `rit reset` explicit path unstaging.
- [x] `rit checkout` local branch basics.
- [x] `rit switch` local branch basics.
- [~] Pathspec expansion for write commands.
  - [x] Ordinary literal file, directory, and `.` pathspec expansion for
    `add`, `restore`, and `reset`.
  - [ ] Pathspec magic, pathspec files, and glob parity.
- [ ] Hook execution for commit.
- [ ] Commit author/date override.
- [ ] Safer branch delete merge checks.
- [ ] Detached HEAD checkout.

Completion criteria:
- Local write commands use lock files or atomic writes and have compatibility
  coverage for simple repositories.

## M5: Object Database And Index Depth

- [x] Loose object read/write.
- [x] Pack index v2 lookup.
- [x] Non-delta packed object read.
- [x] Index v2 read/write for regular files.
- [ ] Delta packed object resolution.
- [ ] Index extensions.
- [ ] Index stat refresh compatible with Git status.
- [ ] File mode and executable bit handling.
- [ ] Symlink support.

Completion criteria:
- `rit` can read normal repositories after `git gc` and can safely update the
  index for common file types.

## M6: Ignore, Attributes, And Pathspecs

- [x] Simple `.gitignore` literal and directory patterns.
- [ ] Git-compatible ignore glob rules.
- [ ] `.git/info/exclude` parity beyond simple patterns.
- [ ] Attributes parser.
- [ ] Pathspec magic.
- [ ] Case-sensitivity behavior by platform/config.

Completion criteria:
- Status/add/diff path selection matches Git for ordinary pathspec and ignore
  usage.

## M7: Transport Foundation

- [ ] Local clone/fetch object transfer.
- [ ] Protocol model.
- [ ] HTTP transport.
- [ ] SSH transport.
- [ ] Fetch refs negotiation.
- [ ] Push basics.

Completion criteria:
- Transport code does not live in core command formatting and does not depend on
  external Git.

## M8: Merge-State Local Workflows

- [ ] Merge state model.
- [ ] `rit merge`
- [ ] `rit cherry-pick`
- [ ] `rit rebase`
- [ ] `rit stash`
- [ ] Conflict index stages.

Completion criteria:
- Interrupted operations leave clear state and can be continued, aborted, or
  inspected.

## M9: Large File Backends

- [ ] Large-file backend trait.
- [ ] LFS pointer parse/write.
- [ ] LFS local cache.
- [ ] LFS batch API.
- [ ] Xet detection.
- [ ] Xet chunk/cache model.

Completion criteria:
- LFS/Xet features are feature-gated and never require external `git-lfs` in
  production code.

## M10: Sparse, Partial Clone, Workspace

- [ ] Sparse checkout reader.
- [ ] Workspace profile config.
- [ ] Partial clone object policy.
- [ ] Lazy file materialization policy.
- [ ] Prefetch command shape.

Completion criteria:
- Users can work with a named workspace without needing to understand Git sparse
  internals.

## M11: Auth

- [ ] Credential abstraction.
- [ ] Environment token provider.
- [ ] Git credential helper compatibility.
- [ ] SSH agent integration.
- [ ] OS keychain adapters.
- [ ] CI non-interactive mode.

Completion criteria:
- Secrets are never printed and auth is separated from transport.

## M12: Semantic Diff

- [ ] Text diff foundation complete.
- [ ] Word diff.
- [ ] Tree-sitter feature gate.
- [ ] Rust semantic adapter.
- [ ] TypeScript semantic adapter.
- [ ] Python semantic adapter.
- [ ] JSON output model.

Completion criteria:
- Semantic output is structured and can distinguish code-only changes from
  tests/docs changes for supported languages.

## M13: Policy, Doctor, Repair

- [ ] Policy config model.
- [ ] Blob size warning/check.
- [ ] Secret pattern warning/check.
- [ ] Protected branch policy.
- [ ] `rit doctor`
- [ ] `rit repair`

Completion criteria:
- Policy defaults warn conservatively and blocking behavior requires explicit
  config.

## M14: VFS

- [ ] Common VFS model.
- [ ] Fallback materialized backend.
- [ ] Platform backend plan.
- [ ] Lazy materialization.
- [ ] Background prefetch.

Completion criteria:
- Builds without VFS still work normally and VFS-specific errors are clear.

## M15: Release Packaging

- [ ] Feature matrix for `rit-min` and `rit-full`.
- [ ] CI build matrix.
- [ ] Release archive layout.
- [ ] README release instructions.
- [ ] License and attribution audit.

Completion criteria:
- A release can be built as a single binary with documented feature choices.

## Active Queue

1. Add reusable checked-in fixtures for local write commands.
2. Expand `status --porcelain=v1` with stronger Git compatibility.
3. Continue pathspec support for remaining read-only commands and advanced
   pathspec forms.
4. Add index stat refresh or document and test the remaining difference.
5. Start patch output for `rit diff`.
