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
- [x] Pathspec support for read-only commands.
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
- [~] Rename detection.
  - [x] Exact staged rename detection for `diff --cached -M` summary and
    patch output.
  - [ ] Similarity thresholds, copy detection, and non-exact rename scoring.
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
    - [x] `--pathspec-from-file` and `--pathspec-file-nul` for `add`,
      `restore`, and `reset`.
    - [x] C-style quoted pathspec-file entries for common escapes.
    - [ ] Full Git pathspec-file edge cases and advanced glob parity.
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
- [x] Pathspec magic.
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

- [x] Local clone/fetch object transfer.
  - [x] `clone --local --no-checkout` copies objects and local refs without
    calling external `git` in production code.
  - [x] `fetch <local-repository>` copies objects into an existing repository
    and writes `FETCH_HEAD` without updating refs.
- [x] Protocol model for local, HTTP(S), and SSH location classification.
- [~] HTTP transport.
  - [x] Smart HTTP `info/refs?service=...` request model.
  - [x] Smart HTTP advertised refs response parser.
  - [~] HTTP client I/O.
    - [x] Blocking plain HTTP GET discovery and POST upload-pack requests.
    - [x] Chunked response decoding.
    - [x] Smart HTTP status, content-type, and advertisement prefix
      validation.
    - [ ] HTTPS/TLS.
- [~] SSH transport.
  - [x] SSH/scp-like upload-pack and receive-pack command model.
  - [ ] SSH process/session I/O.
- [~] Fetch refs negotiation.
  - [x] Single local fetch refspec updates a destination ref after copying
    objects.
  - [x] Smart HTTP upload-pack `want`/`have`/`done` request model.
  - [x] Smart HTTP upload-pack ACK/NAK/ERR response parser and raw pack
    classifier.
  - [x] Upload-pack side-band pack/progress/error response parser.
  - [x] Upload-pack raw/side-band pack byte extraction.
  - [x] Received packfile checksum validation and `.pack` storage.
  - [x] Whole, offset-delta, and ref-delta received pack application to loose
    objects.
  - [x] Pack index v2 generation for received packs.
  - [x] Received pack ingest helper for store, index, and loose application.
  - [x] Remote advertised refs discovery through the smart HTTP client.
  - [x] Single-round remote pack negotiation for an advertised ref through the
    smart HTTP client.
  - [x] Wire negotiated plain HTTP pack ingestion into `rit fetch`.
  - [ ] Multi-round negotiation, thin-pack fixups, and advanced capability
    parity.
- [x] Push basics.
  - [x] receive-pack reference update request body model.
  - [x] receive-pack `report-status` parser.
  - [x] Smart HTTP receive-pack POST wiring.
  - [x] Push pack generation and ref update workflow.
    - [x] Whole-object pack generation from existing object IDs.
    - [x] Plain HTTP client push workflow to choose reachable objects, send
      receive-pack, and validate ref status.

Completion criteria:
- Transport code does not live in core command formatting and does not depend on
  external Git.

## M8: Merge-State Local Workflows

- [x] Merge state model.
  - [x] Read `MERGE_HEAD`, `CHERRY_PICK_HEAD`, `REVERT_HEAD`, `MERGE_MSG`,
    `SQUASH_MSG`, `rebase-apply`, and `rebase-merge` state.
- [ ] `rit merge`
- [ ] `rit cherry-pick`
- [ ] `rit rebase`
- [ ] `rit stash`
- [ ] Conflict index stages.

Completion criteria:
- Interrupted operations leave clear state and can be continued, aborted, or
  inspected.

## M9: Large File Backends

- [x] Large-file backend trait.
  - [x] Backend kind, track rule, pointer metadata, and object-safe backend
    interface.
- [x] LFS pointer parse/write.
  - [x] Git LFS v1 pointer parser and encoder for `version`, `oid sha256`,
    and `size`.
- [x] LFS local cache.
  - [x] Sharded `.git/lfs/objects/<aa>/<bb>/<sha256>` storage with SHA-256
    and size verification.
- [x] LFS batch API.
  - [x] Batch request/response models and JSON encoding/parsing for basic
    transfer actions.
- [x] Xet detection.
  - [x] Explicit `filter=xet` attribute rules and Xet pointer hash extension
    hints.
- [x] Xet chunk/cache model.
  - [x] Xet hash, xorb chunk range, reconstruction term/file models, and
    sharded local cache paths.
- [x] LFS/Xet Cargo feature gates.
  - [x] `rit-core` builds without LFS/Xet by default and exposes them through
    `lfs`, `xet`, and `large-files` features.

Completion criteria:
- LFS/Xet features are feature-gated and never require external `git-lfs` in
  production code.

## M10: Sparse, Partial Clone, Workspace

- [x] Sparse checkout reader.
  - [x] Read `core.sparseCheckout`, `core.sparseCheckoutCone`, and
    `.git/info/sparse-checkout` pattern state without mutating the repository.
- [x] Workspace profile config.
  - [x] Read optional `rit.toml` / `.rit.toml` workspace profile includes from
    `[workspace.<name>]` tables.
- [x] Partial clone object policy.
  - [x] Read promisor remotes, partial clone filters, and `.promisor` pack
    markers without fetching missing objects.
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
3. Keep large implementation files moving toward focused modules before adding
   more transport or command surface area.

## Implementation Notes

- 2026-05-11, M7 upload-pack response model:
  - Reference Git: `git version 2.52.0.windows.1`.
  - Reference docs checked: local `gitprotocol-http(5)` smart HTTP
    upload-pack POST/result content types and local `gitprotocol-pack(5)`
    ACK/NAK negotiation plus packfile data sections.
  - Implemented: pure Rust parsing for upload-pack `NAK`, `ACK <object>`,
    `ACK <object> continue|common|ready`, `ERR <message>`, and detection of
    non-sideband raw `PACK` data.
  - Implemented: pure Rust parsing for side-band records 1 (pack data), 2
    (progress), and 3 (server error).
  - Later slices completed plain HTTP pack storage/application and one-round
    fetch negotiation. Still open: HTTPS/TLS and full multi-round negotiation.
- 2026-05-11, M7 smart HTTP client I/O:
  - Reference Git: `git version 2.52.0.windows.1`.
  - Reference docs checked: local `gitprotocol-http(5)` smart HTTP
    `info/refs?service=git-upload-pack`, upload-pack POST content type, and
    upload-pack result content type.
  - Implemented: `BlockingSmartHttpClient` for plain `http://` GET discovery
    and POST upload-pack requests using Rust `TcpStream`, plus raw HTTP
    response parsing and transport I/O errors.
  - Implemented: chunked transfer decoding for smart HTTP responses.
  - Implemented: status code validation, smart HTTP content-type validation,
    and `info/refs` advertisement prefix validation.
  - Implemented: smart HTTP advertised ref discovery using the blocking client
    and advertisement parser.
  - Later slices completed plain HTTP pack negotiation/application and
    `rit fetch` wiring. Still open: TLS for `https://`.
- 2026-05-11, M7 SSH command model:
  - Reference Git: `git version 2.52.0.windows.1`.
  - Reference docs checked: local `gitprotocol-pack(5)` SSH transport examples
    for `git-upload-pack` and repository path quoting.
  - Implemented: pure Rust parsing for `ssh://user@host/path` and
    `user@host:path` locations plus remote `git-upload-pack` /
    `git-receive-pack` command construction.
  - Still open: starting an SSH session, pkt-line I/O over that session,
    authentication, and pack negotiation.
- 2026-05-11, M7 receive-pack request model:
  - Reference Git: `git version 2.52.0.windows.1`.
  - Reference docs checked: local `gitprotocol-pack(5)` reference update
    request and packfile transfer grammar, plus `git push -h`.
  - Implemented: pure Rust receive-pack command/request serialization with
    first-command capabilities, command-list flush, and trailing raw pack data.
  - Implemented: pure Rust receive-pack `report-status` parsing for unpack
    results and per-ref `ok` / `ng` statuses.
  - Implemented: smart HTTP `git-receive-pack` POST request wiring and
    response parsing through the blocking HTTP client.
  - Still open: pack generation, server-side status handling beyond
    `report-status`, and CLI `rit push`.
- 2026-05-11, M7 upload-pack pack extraction:
  - Reference Git: `git version 2.52.0.windows.1`.
  - Reference docs checked: local `gitprotocol-pack(5)` side-band packfile
    data section.
  - Implemented: API to extract raw pack bytes from non-sideband responses or
    concatenate side-band band 1 pack data while surfacing band 3 errors.
  - Implemented: packfile `PACK` header/version/trailer checksum validation
    and atomic `.git/objects/pack/pack-<checksum>.pack` storage.
  - Implemented: whole, offset-delta, and ref-delta pack object application to
    loose objects.
  - Implemented: pack index v2 generation with fanout, sorted object names,
    CRC32 table, offsets, pack checksum, and index checksum.
  - Implemented: pack ingest helper that stores the pack, writes the index, and
    applies supported objects as loose objects.
  - Later slices completed plain HTTP `rit fetch` ingestion. Still open:
    HTTPS/TLS, thin-pack fixups, and deeper negotiation parity.
- 2026-05-11, M7 transport module hygiene:
  - Verified large-file state before continuing: `transport.rs` had grown to
    roughly 1955 lines after the pack ingest work.
  - Implemented: moved upload-pack request/response parsing and receive-pack
    request/status parsing into focused `transport/upload_pack.rs` and
    `transport/receive_pack.rs` modules while keeping the public transport API
    names re-exported from `transport`.
  - Result: `transport.rs` is now roughly 1546 lines, with upload-pack and
    receive-pack protocol logic isolated for easier review and future M7 work.
- 2026-05-11, M7 remote pack negotiation:
  - Reference Git: `git version 2.52.0.windows.1`.
  - Reference docs checked: local `gitprotocol-http(5)` smart HTTP discovery
    and upload-pack POST/result flow, plus local `gitprotocol-pack(5)`
    upload-pack negotiation and side-band data rules.
  - Implemented: `BlockingSmartHttpClient::negotiate_upload_pack` discovers
    upload-pack refs, finds a caller-selected advertised ref, sends one
    `want`/`have`/`done` request with supported advertised capabilities, parses
    the upload-pack result, rejects protocol `ERR`, and returns extracted raw
    pack bytes.
  - Later slice completed plain HTTP `rit fetch` ingestion. Still open:
    multi-round negotiation, thin-pack fixups, and HTTPS/TLS.
- 2026-05-11, M7 plain HTTP fetch ingestion:
  - Reference Git: `git version 2.52.0.windows.1`.
  - Reference docs checked: `git fetch -h`, local `gitprotocol-http(5)`, and
    local `gitprotocol-pack(5)`.
  - Implemented: `Repository::fetch_remote_http` runs the smart HTTP
    negotiation API, ingests returned pack bytes into `.git/objects`, writes
    `.git/FETCH_HEAD`, and updates a destination ref for one simple refspec.
  - Implemented: `rit fetch http://... [<src>:<dst>]` dispatches to the plain
    HTTP path while keeping HTTPS and SSH rejected until those transports exist.
  - Still open: HTTPS/TLS, SSH sessions, named remote config, multiple
    refspecs, multi-round negotiation, and thin-pack fixups.
- 2026-05-11, M7 push pack generation:
  - Reference Git: `git version 2.52.0.windows.1`.
  - Reference docs checked: local `gitformat-pack(5)` and
    `gitprotocol-pack(5)`.
  - Implemented: pure Rust whole-object packfile generation from existing
    object IDs in `LooseObjectDb`, with pack v2 header, object type/size
    headers, zlib-compressed payloads, and trailing pack checksum.
  - Still open: deciding the object set for push, thin-pack/delta generation,
    sending the generated pack through receive-pack, and interpreting remote
    ref update results as a full push workflow.
- 2026-05-12, M7 plain HTTP push workflow:
  - Reference Git: `git version 2.52.0.windows.1`.
  - Reference docs checked: `git push -h`, local `gitprotocol-http(5)`, and
    local `gitprotocol-pack(5)`.
  - Implemented: `Repository::push_remote_http` discovers receive-pack refs,
    resolves one local source revision, walks reachable commit/tree/blob
    objects, builds a whole-object pack, sends a receive-pack update request,
    and validates `report-status` for the destination ref.
  - Implemented: `rit push http://... <src>:<dst>` CLI dispatch for this plain
    HTTP subset.
  - Still open: HTTPS/TLS, SSH sessions, named remotes, multiple refspecs,
    force/lease semantics, hooks, thin-pack/delta generation, and full object
    minimization against remote history.
- 2026-05-11, CLI module hygiene:
  - Verified large-file state before continuing: `rit-cli/src/main.rs` had
    grown past 2100 lines.
  - Implemented: moved static help text and command help routing into
    `rit-cli/src/help.rs`.
  - Result: `rit-cli/src/main.rs` is now roughly 1909 lines, with help output
    covered by the existing CLI tests.
- 2026-05-12, CLI remote module hygiene:
  - Verified large-file state after plain HTTP push: `rit-cli/src/main.rs`
    had grown back to roughly 2000 lines.
  - Implemented: moved `clone`, `fetch`, and `push` command handling into
    `rit-cli/src/remote.rs`.
  - Result: `rit-cli/src/main.rs` is now roughly 1755 lines, with remote command
    behavior covered by the existing CLI and compatibility tests.
