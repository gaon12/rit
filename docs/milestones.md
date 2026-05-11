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

- Date: 2026-05-10
- Reference Git: `git version 2.52.0.windows.1`
- Required recurring checks:
  - `git --version`
  - `git help -a`
  - `git <command> -h` for each command being implemented
  - `cargo fmt --all`
  - `cargo clippy --workspace --all-targets -- -D warnings`
  - `cargo test --workspace`

## Milestone Verification

Verified on 2026-05-10 before continuing implementation:

- Production crates do not execute `git`; `Command::new` usage is limited to
  `rit-testkit` and compatibility tests.
- M1 local write compatibility coverage exists as generated Git-vs-rit
  scenarios, but reusable checked-in local write fixtures are still not present.
- M2 linked worktree/common-dir support was marked incomplete and was also
  incomplete in code: `Repository::common_dir` always pointed at `git_dir`.
- M2 config parsing was previously split across repository format and user
  identity reads; it now has a shared parser for scalar config reads.
- M3/M4 pathspec and diff gaps in this file match the implementation notes:
  ordinary literal, wildcard, positive magic, icase, and exclude pathspecs
  exist; attr pathspec magic and pathspec files remain open.

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
- [x] Add fixtures for local write commands.
  - [x] Generated Git-vs-rit compatibility scenarios for directory pathspec
    `add`, `restore`, and `reset`.
  - [x] Reusable checked-in local write fixture builders in `rit-testkit`.
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
- [x] Linked worktree/common-dir support.
- [x] More complete config parser.

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
  - [x] Git-like collapse of fully untracked directories in default porcelain
    output.
  - [x] Git-like porcelain path quoting for paths with whitespace or special
    characters.
  - [x] Git-like `--untracked-files=no|normal|all` and `-uno|-unormal|-uall`
    modes.
  - [x] Git-like `-z` NUL-terminated porcelain v1 output.
  - [x] Git-like `-b` / `--branch` porcelain v1 branch header for local and
    unborn/detached HEAD states.
  - [x] Git-like `--ignored` porcelain v1 entries for simple ignore rules.
- [x] `rit diff --name-only`
- [x] `rit diff --name-status`
- [x] `rit diff --stat`
- [x] `rit diff --numstat`
- [x] `rit diff` patch output.
  - [x] Small text patch output for default and cached diff scopes.
  - [x] No-newline markers for default and cached text patch output.
  - [x] Binary patch placeholders for default and cached diff scopes.
  - [x] Multi-hunk context splitting.
- [~] Pathspec support for read-only commands.
  - [x] Ordinary literal pathspec filters for `status --porcelain=v1` and
    `diff` summary modes.
  - [x] Simple `*` and `?` wildcard pathspec filters for
    `status --porcelain=v1` and `diff` summary modes.
  - [x] Simple bracket-class wildcard pathspec filters for
    `status --porcelain=v1` and `diff` summary modes.
  - [x] Ordinary literal pathspec filters for `ls-files`.
  - [x] Simple `*` and `?` wildcard pathspec filters for `ls-files`.
  - [x] Ordinary literal path lookup for `ls-tree`.
  - [x] Ordinary literal path filters for first-parent `log`.
  - [x] Ordinary literal path filters for `show --no-patch`.
  - [x] Simple `*` and `?` wildcard pathspec filters for first-parent `log`
    and `show --no-patch`.
- [ ] Rename detection.
- [x] Binary diff accounting for summary modes.

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
  - [~] Pathspec magic, pathspec files, and glob parity.
    - [x] Shared simple `*` and `?` wildcard matcher.
    - [x] Shared simple bracket-class wildcard matcher.
    - [x] Simple wildcard and bracket-class expansion for `add`, `restore`,
      and `reset`.
- [x] Hook execution for commit.
  - [x] `pre-commit`, `prepare-commit-msg`, and `commit-msg` can block the
    commit.
  - [x] `post-commit` runs after a successful commit without changing the
    commit result.
  - [x] `--no-verify` bypasses `pre-commit` and `commit-msg`.
- [x] Commit author/date override for `--author=<name <email>>` and raw
  `--date=<unix-seconds> <+/-HHMM>`.
- [x] Safer branch delete merge checks.
- [x] Detached HEAD checkout.

Completion criteria:
- Local write commands use lock files or atomic writes and have compatibility
  coverage for simple repositories.

## M5: Object Database And Index Depth

- [x] Loose object read/write.
- [x] Pack index v2 lookup.
- [x] Non-delta packed object read.
- [x] Index v2 read/write for regular files.
- [x] Index stat refresh compatible with Git status for clean regular files.
- [x] Raw optional index extension preservation during status refresh.
- [x] Delta packed object resolution.
- [x] Semantic index extension parsing.
  - [x] Parse extension records and classify known signatures (`TREE`,
    `REUC`, `UNTR`, `FSMN`, `link`, `sdir`, `EOIE`, `IEOT`).
  - [x] Parse `TREE` cache-tree payloads into depth-first node models.
  - [x] Parse `REUC` resolve-undo payloads into per-path stage models.
  - [x] Parse `FSMN` file-system-monitor headers and raw bitmap payloads.
  - [x] Parse `link` split-index shared-index IDs and raw bitmap payloads.
  - [x] Parse `sdir` sparse-directory marker extensions.
  - [x] Parse `UNTR` untracked-cache headers and directory blocks.
  - [x] Parse EWAH bitmap, stat, hash, and null terminator tails used by
    `UNTR`.
  - [x] Parse `EOIE` end-of-entry offsets and extension table hashes.
  - [x] Parse `IEOT` offset-table entries.
- [x] File mode and executable bit handling.
  - [x] Preserve `100644`/`100755` modes when writing trees from the index.
  - [x] `rit add --chmod=+x|-x` records executable-bit overrides in the index.
  - [x] `status --porcelain=v1` and cached diff summaries detect staged
    mode-only changes.
  - [x] Unix worktree executable-bit refresh and checkout/restore
    materialization; Windows keeps Git-like filemode-insensitive behavior.
- [x] Symlink support.
  - [x] Add symlinks as `120000` blob entries containing the link target.
  - [x] Preserve `120000` tree/index modes through commit, status, diff, and
    restore/checkout core paths.
  - [x] Cross-platform Git config parity for `core.symlinks=false`.

Completion criteria:
- `rit` can read normal repositories after `git gc` and can safely update the
  index for common file types.

## M6: Ignore, Attributes, And Pathspecs

- [x] Simple `.gitignore` literal and directory patterns.
- [x] Git-compatible ignore glob rules.
- [x] `.git/info/exclude` parity beyond simple patterns.
- [x] Attributes parser.
- [~] Pathspec magic.
  - [x] Positive `:(literal)`, `:(glob)`, `:(top)`, and `:/` forms.
  - [x] Case-insensitive `:(icase)` pathspec magic.
  - [x] Exclude `:!`, `:^`, and `:(exclude)` pathspec magic.
  - [x] Attr pathspec magic for root `.gitattributes` set/unset/value/
    unspecified requirements.
- [~] Case-sensitivity behavior by platform/config.
  - [x] `git add` honors `core.ignorecase=true` for mismatched-case
    pathspecs that Git accepts as no-ops.
  - [ ] Broader platform/config parity for case-sensitive path lookup.

Completion criteria:
- Status/add/diff path selection matches Git for ordinary pathspec and ignore
  usage.

## M7: Transport Foundation

- [~] Local clone/fetch object transfer.
  - [x] `clone --local --no-checkout` copies objects and local refs without
    calling external `git` in production code.
  - [x] `fetch <local-repository>` copies objects into an existing repository
    and writes `FETCH_HEAD` without updating refs.
- [x] Protocol model for local, HTTP(S), and SSH location classification.
- [~] HTTP transport.
  - [x] Smart HTTP `info/refs?service=...` request model.
  - [x] Smart HTTP advertised refs response parser.
  - [ ] HTTP client I/O.
- [ ] SSH transport.
- [~] Fetch refs negotiation.
  - [x] Single local fetch refspec updates a destination ref after copying
    objects.
  - [x] Smart HTTP upload-pack `want`/`have`/`done` request model.
  - [x] Smart HTTP upload-pack ACK/NAK/ERR response parser and raw pack
    classifier.
  - [ ] Remote advertised refs and pack negotiation.
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

1. Keep M6 case-sensitivity parity under verification as new path lookup
   surfaces are added.
2. Continue M7 with HTTP transport planning and fetch negotiation boundaries.

## Implementation Notes

- 2026-05-11, M7 upload-pack response model:
  - Reference Git: `git version 2.52.0.windows.1`.
  - Reference docs checked: local `gitprotocol-http(5)` smart HTTP
    upload-pack POST/result content types and local `gitprotocol-pack(5)`
    ACK/NAK negotiation plus packfile data sections.
  - Implemented: pure Rust parsing for upload-pack `NAK`, `ACK <object>`,
    `ACK <object> continue|common|ready`, `ERR <message>`, and detection of
    non-sideband raw `PACK` data.
  - Still open: HTTP client I/O, side-band/side-band-64k unpacking, applying
    received packfiles into the object database, and full remote negotiation.
