# Implementation Notes

## Baseline Git Check

- Checked Git version: `git version 2.52.0.windows.1`
- Checked command list: `git help -a`
- `git help <command>` opened the local manual pager in this environment and timed out, so command-specific checks used `git <command> -h`.
- 2026-05-09 baseline refresh checked: `git status -h`, `git add -h`, `git commit -h`, `git diff -h`, and `git log -h`.
- 2026-05-09 diff cached milestone checked: `git diff -h`.
- 2026-05-09 diff numstat milestone checked: `git diff -h`.
- 2026-05-09 pathspec filter milestone checked: `git status -h`,
  `git diff -h`, `git add -h`, `git restore -h`, and `git reset -h`.
- 2026-05-10 simple wildcard pathspec slice checked: `git status -h` and
  `git diff -h`, plus direct Git comparisons for `*.txt` and `nested/*.txt`.
- 2026-05-10 bracket wildcard pathspec slice checked: `git status -h` and
  `git diff -h`, plus direct Git comparisons for `[ab]`, `[a-c]`, and `[!a]`
  forms.
- 2026-05-12 POSIX bracket pathspec slice checked `git add -h` and direct Git
  comparisons for `[[:digit:]]` forms in write-command pathspec expansion.
- 2026-05-09 diff patch milestone checked: `git diff -h`.
- 2026-05-09 detached checkout milestone checked: `git checkout -h`.
- 2026-05-09 branch delete safety milestone checked: `git branch -h`.
- 2026-05-10 milestone verification checked: `git --version`,
  `git help -a`, `git worktree -h`, `git rev-parse --git-dir`, and
  `git rev-parse --git-common-dir`.
- 2026-05-10 executable-bit slice checked: `git add -h`, `git status -h`,
  `git checkout -h`, and `git restore -h`.
- 2026-05-10 index extension slice checked: `git update-index -h`.
- 2026-05-10 symlink slice checked: `git add -h`, `git status -h`,
  `git diff -h`, `git checkout -h`, and `git restore -h`.
- 2026-05-10 ignore glob slice checked: `git check-ignore -h` and direct
  Git comparisons for `*.log`, `?`, bracket classes, anchored patterns, `**`,
  negation, and `.git/info/exclude`.
- 2026-05-10 attributes parser slice checked: `git check-attr -h`.
- 2026-05-10 pathspec magic slice checked: `git status -h`, `git diff -h`,
  `git add -h`, and direct Git comparisons for `:(literal)`, `:(glob)`,
  `:(top)`, and `:/`.
- 2026-05-10 icase pathspec magic slice checked direct Git comparisons for
  `:(icase)` in `status`, `diff`, and `add`.
- 2026-05-10 core.ignorecase add slice checked `git config -h`, `git add -h`,
  and direct Git comparisons for mismatched-case `git add` pathspecs.
- 2026-05-10 exclude pathspec magic slice checked direct Git comparisons for
  `:!`, `:^`, and `:(exclude)` in `status`, `diff`, `ls-files`, and `add`.
- 2026-05-11 attr pathspec magic slice checked `git config -h`, `git status -h`,
  `git add -h`, `git diff -h`, and direct Git comparisons for
  `:(attr:name)`, `:(attr:-name)`, `:(attr:name=value)`, and `:(attr:!name)`
  in `status`, `diff`, `ls-files`, and `add`.
- 2026-05-16 glob double-star component slice checked `git add -h`,
  `git status -h`, `git diff -h`, and direct Git comparisons for `:(glob)`
  patterns where `**` appears inside one path component instead of in Git's
  special `**/` or trailing `**` forms.
- 2026-05-16 diff `-z` summary slice checked `git diff -h` and direct Git
  byte-shape comparisons for `--name-only`, `--name-status`, and `--numstat`,
  including rename output fields.
- 2026-05-16 indexdb layout slice checked existing linked worktree discovery
  behavior and added explicit shared repository DB plus worktree-local cache
  storage paths.
- 2026-05-16 indexdb commit query slice added feature-gated read-only APIs for
  exact commit lookup and recent commit listing from the auxiliary SQLite DB.
- 2026-05-16 indexdb refs snapshot query slice added a feature-gated read-only
  API for HEAD, local branch, and lightweight tag snapshot rows.
- 2026-05-16 indexdb file history slice started recording simple first-parent
  file changes (`A`, `M`, `D`) into `file_changes` and exposed a read-only
  `file_history(path)` query API.
- 2026-05-16 `rit file-history <path>` slice added a read-only CLI over
  indexdb file history rows. The command ensures reproducible index metadata
  exists, then prints newest-first first-parent path changes.
- 2026-05-16 indexdb output-neutrality slice added compatibility coverage that
  `rit indexdb` metadata creation does not change `status`, `diff`, or
  `ls-files` output and that those outputs still match Git afterward.
- 2026-05-16 indexdb corruption-safety validation now asserts ordinary status
  and a later rit commit still succeed when `.git/rit/indexdb.sqlite` contains
  invalid SQLite bytes.
- 2026-05-16 linked-worktree indexdb validation now asserts linked worktrees
  share the repository DB path while using distinct worktree cache and lock
  paths.
- 2026-05-16 indexdb canonical fallback slice changed read APIs to use
  `.git/objects` and refs when indexdb is missing, stale, corrupted, or missing
  a requested row.
- 2026-05-12 exact rename-detection slice checked `git diff -h` and direct Git
  comparisons for `diff --cached -M` exact rename output.
- 2026-05-12 similarity rename/copy slice checked `git diff -h` and direct Git
  comparisons for `diff --cached -M`, `-M79%`, `--find-renames=79`, `-C`,
  `-C79%`, and `--find-copies=79`.
- 2026-05-12 HTTPS/TLS transport slice checked `git fetch -h`, `git push -h`,
  and the existing smart HTTP tests; TLS uses platform certificate
  verification through `native-tls`.
- 2026-05-12 SSH session slice checked `git fetch -h`, `git push -h`, and the
  existing SSH command model; added process-backed upload-pack session I/O
  without invoking external `git`.
- 2026-05-12 SSH fetch wiring slice checked `git fetch -h`; wired
  single-round SSH upload-pack advertisement parsing, pack ingestion, and
  `FETCH_HEAD`/destination ref updates without invoking external `git`.
- 2026-05-12 SSH push wiring slice checked `git push -h`; wired
  single-ref SSH receive-pack advertisement parsing, request serialization,
  pack sending, and report-status validation without invoking external `git`.
- 2026-05-12 SSH port slice checked the existing SSH command model; added
  `ssh://host:port/path` parsing and `ssh -p <port>` process argument wiring.
- 2026-05-12 SSH environment command slice checked `git fetch -h` and
  `git push -h`; added `GIT_SSH_COMMAND` and `GIT_SSH` process selection for
  SSH fetch/push process execution.
- 2026-05-12 copy-detection-hard slice checked `git diff -h` and direct Git
  comparisons for `diff --cached --find-copies-harder`.
- 2026-05-12 rename-limit slice checked `git diff -h` and direct Git
  comparisons for `diff --cached -M -l0`.
- 2026-05-15 worktree intent-to-add rename/copy slice checked `git diff -h`
  and direct Git comparisons for default `diff -M`, `diff -C`,
  `--find-copies=79`, and summary/patch output when added worktree paths are
  represented by Git intent-to-add index entries.
- 2026-05-15 worktree find-copies-harder slice checked `git diff -h` and
  direct Git comparisons for default worktree `diff --find-copies-harder` and
  `diff -C --find-copies-harder` with Git intent-to-add entries.
- 2026-05-15 exact rename-limit slice checked `git diff -h` and direct Git
  comparisons for cached and worktree intent-to-add exact renames with
  `diff -M -l1`.
- 2026-05-15 worktree similarity rename slice checked `git diff -h` and direct
  Git comparisons for default `diff -M` non-exact rename detection with
  intent-to-add destinations.
- 2026-05-15 worktree rename-limit warning slice checked `git diff -h` and
  direct Git comparisons for default `diff -M -l1` stdout and stderr when
  exhaustive worktree intent-to-add rename detection is skipped.
- 2026-05-15 worktree copy-limit warning slice checked `git diff -h` and
  direct Git comparisons for default `diff -C -l1` stdout and stderr when
  exhaustive worktree intent-to-add copy detection is skipped.
- 2026-05-15 exact copy-limit slice checked `git diff -h` and direct Git
  comparisons for cached and worktree `diff -C -l1` exact copies that should
  be detected before exhaustive copy detection is limit-skipped.
- 2026-05-15 rename/copy limit width slice checked `git diff -h` and direct
  Git comparisons for cached non-exact rename/copy detection with `-M -l1`
  and `-C -l1`.
- 2026-05-15 rename-limit warning slice checked `git diff -h` and direct Git
  comparisons for cached `diff -M -l1` stdout and stderr when exhaustive
  rename detection is skipped.
- 2026-05-15 diff.renameLimit config slice checked `git diff -h` and direct
  Git comparisons for cached and worktree `diff -M` stdout/stderr when
  `diff.renameLimit=1` skips exhaustive rename detection.
- 2026-05-16 invalid diff.renameLimit config slice checked `git diff -h` and
  direct Git comparisons for cached `diff -M` fatal output and exit code when
  `diff.renameLimit` is not numeric.
- 2026-05-15 fractional rename/copy threshold slice checked `git diff -h` and
  direct Git comparisons for cached `-M5`, `-M05`, `--find-renames=5`,
  `--find-renames=05`, `-C5`, `-C05`, `--find-copies=5`,
  `--find-copies=05`, and thresholds above 100%.
- 2026-05-12 pathspec-file slice checked `git add -h`, `git restore -h`,
  `git reset -h`, and direct Git comparisons for `--pathspec-from-file` and
  `--pathspec-file-nul`, including a quoted pathspec entry.
- 2026-05-15 quoted octal pathspec-file slice checked `git add -h`,
  `git restore -h`, `git reset -h`, and direct Git comparisons for UTF-8
  pathspec bytes encoded as octal C-style escapes.
- 2026-05-16 short octal pathspec-file slice checked `git add -h`,
  `git restore -h`, `git reset -h`, and direct Git comparisons for short or
  incomplete octal C-style escapes in quoted `--pathspec-from-file` entries.
- 2026-05-15 quoted alarm pathspec-file slice checked `git add -h`,
  `git restore -h`, `git reset -h`, and direct Git comparisons for `\a`
  C-style escapes and pathspec-not-found output.
- 2026-05-15 empty pathspec-file slice checked `git add -h`,
  `git restore -h`, `git reset -h`, and direct Git comparisons for empty
  `--pathspec-from-file` inputs.
- 2026-05-15 empty pathspec-from-file value slice checked `git add -h`,
  `git restore -h`, `git reset -h`, and direct Git comparisons for
  `--pathspec-from-file=`.
- 2026-05-15 empty NUL pathspec-file slice checked `git add -h`,
  `git restore -h`, `git reset -h`, and direct Git comparisons for empty
  `--pathspec-from-file=<file> --pathspec-file-nul` inputs.
- 2026-05-15 repeated pathspec-from-file slice checked `git add -h`,
  `git restore -h`, `git reset -h`, and direct Git comparisons proving the
  last `--pathspec-from-file` value is used.
- 2026-05-15 pathspec-file argument mixing slice checked `git add -h`,
  `git restore -h`, `git reset -h`, and direct Git comparisons for
  `--pathspec-from-file` combined with ordinary pathspec arguments.
- 2026-05-15 missing pathspec-file slice checked `git add -h`,
  `git restore -h`, `git reset -h`, and direct Git comparisons for missing
  `--pathspec-from-file` input files.
- 2026-05-15 non-UTF-8 pathspec-file slice checked `git add -h`,
  `git restore -h`, `git reset -h`, and direct Git comparisons for non-UTF-8
  bytes in text `--pathspec-from-file` inputs.
- 2026-05-15 text pathspec-file NUL byte slice checked `git add -h`,
  `git restore -h`, `git reset -h`, and direct Git comparisons for NUL bytes
  embedded inside text `--pathspec-from-file` lines.
- 2026-05-15 lone carriage-return pathspec-file slice checked `git add -h`,
  `git restore -h`, `git reset -h`, and direct Git comparisons for text
  `--pathspec-from-file` lines ending with a lone `\r` byte.
- 2026-05-15 pathspec-file option validation slice checked `git add -h`,
  `git restore -h`, `git reset -h`, and direct Git comparisons for
  `--pathspec-file-nul` without `--pathspec-from-file`.
- 2026-05-15 pathspec-from-file missing value slice checked `git add -h`,
  `git restore -h`, `git reset -h`, and direct Git comparisons for
  `--pathspec-from-file` without a value.
- 2026-05-12 stdin pathspec-file slice checked `git add -h`, `git restore -h`,
  `git reset -h`, and direct Git comparisons for `--pathspec-from-file=-`.
- 2026-05-12 stdin NUL pathspec-file slice checked `git add -h`,
  `git restore -h`, `git reset -h`, and direct Git comparisons for
  `--pathspec-from-file=- --pathspec-file-nul`.
- 2026-05-15 empty pathspec-file entry slice checked `git add -h`,
  `git restore -h`, `git reset -h`, and direct Git comparisons for leading
  empty line entries in `--pathspec-from-file`.
- 2026-05-15 quoted empty pathspec-file slice checked `git add -h`,
  `git restore -h`, `git reset -h`, and direct Git comparisons for quoted
  empty entries in `--pathspec-from-file`.
- 2026-05-15 quoted pathspec trailing-bytes slice checked `git add -h`,
  `git restore -h`, `git reset -h`, and direct Git comparisons for bytes after
  the closing quote in quoted `--pathspec-from-file` entries.
- 2026-05-15 empty NUL pathspec-file entry slice checked `git add -h`,
  `git restore -h`, `git reset -h`, and direct Git comparisons for leading
  empty entries in `--pathspec-from-file --pathspec-file-nul`.
- 2026-05-15 badly quoted pathspec-file entry slice checked `git add -h`,
  `git restore -h`, `git reset -h`, and direct Git comparisons for
  unterminated quoted entries in `--pathspec-from-file`.
- 2026-05-12 merge-state model slice checked `git merge -h`,
  `git cherry-pick -h`, `git rebase -h`, and `git stash -h`.
- 2026-05-13 fast-forward merge slice checked `git merge -h`; implemented
  `rit merge [--ff-only] <target>` for clean fast-forward-only updates.
- 2026-05-13 operation journal slice checked `git --version` and
  `git help -a`; this feature is rit-specific metadata under `.git/rit/` and
  does not use Git command output as a compatibility target.
- 2026-05-13 transaction plan slices checked `git add -h`,
  `git commit -h`, `git reset -h`, and `git merge -h`; implemented
  `rit add --plan`, `rit commit --plan`, `rit reset --plan`, and
  `rit merge --plan` as rit-specific dry-run views over the same selection
  logic used by the applying commands.
- 2026-05-13 explainable pathspec slice used the existing Git-compatible
  pathspec parser; `rit pathspec explain <pathspec>` is rit-specific and
  prints normalized patterns, matching mode, exclusions, wildcard use,
  case-sensitivity, and attribute requirements without touching repository
  state.
- 2026-05-13 ignore explain slice checked `git check-ignore -h`; `rit ignore
  explain <path>` reuses rit's ignore parser and matcher to report matching
  `.gitignore` and `.git/info/exclude` rules without invoking external `git`.
- 2026-05-13 merge explain slice checked `git merge -h`; `rit merge explain
  <target>` reuses the fast-forward merge planner to explain already-up-to-date,
  fast-forward, and unsupported merge decisions without writing refs, index, or
  worktree files.
- 2026-05-13 status explain slice checked `git status -h`; `rit status
  --explain <path>` is a rit-specific read-only explanation layer over HEAD,
  index, worktree, and ignore-rule classification.
- 2026-05-13 IndexDB slice checked `git --version` and `git help -a`; IndexDB
  is rit-specific and has no Git command baseline. Added feature-gated SQLite
  support with `rit indexdb`, `status`, `build`, `update`, `repair`, `rebuild`,
  `drop`, and `vacuum`.
- IndexDB shared repository storage is `.git/rit/indexdb.sqlite` using the
  repository common directory. `.git/rit/indexdb.lock` is reserved in the
  public storage layout. Worktree-local caches use
  `.git/rit/worktree-cache.sqlite` for the primary worktree and
  `.git/rit/worktrees/<worktree-id>/worktree-cache.sqlite` for linked
  worktrees.
- IndexDB schema version 1 creates `cache_state`, `commits`,
  `commit_parents`, `file_changes`, and `refs_snapshot`. Object IDs are stored
  as `hash_kind` plus binary object-id bytes, not fixed SHA-1 text columns.
- `Repository::indexdb().commit_by_id(...)` and `recent_commits(...)` expose a
  small read-only commit query API. They use the auxiliary database when it is
  healthy and fresh, and fall back to canonical `.git/objects` traversal when
  IndexDB is missing, stale, corrupted, or incomplete.
- `Repository::indexdb().refs_snapshot()` exposes the indexed HEAD/local
  branch/lightweight tag snapshot as a read-only API without changing
  Git-compatible command behavior. It falls back to canonical refs when the
  auxiliary snapshot is stale or unavailable.
- `Repository::indexdb().file_history(path)` exposes indexed first-parent file
  changes for a repository-relative path. Rename/copy-aware history remains
  future work; the current API stores straightforward add/modify/delete rows and
  falls back to first-parent tree walking when indexdb is unavailable.
- `rit file-history <path>` is feature-gated behind `indexdb` and reports a
  clear missing-feature error in minimal builds. It creates or updates
  `.git/rit/indexdb.sqlite` as reproducible metadata but does not modify Git
  objects, refs, index entries, or working tree files.
- Compatibility coverage now captures representative Git-compatible read
  command output before and after `rit indexdb` creates metadata, then compares
  the post-indexdb output with Git. This guards the “indexdb is not source of
  truth” rule for user-visible command behavior.
- Corruption coverage keeps broken indexdb scoped to optional metadata: status
  reads canonical Git/index/worktree state, and write-through refresh failures
  do not prevent the Git commit from being created.
- Linked-worktree coverage checks both sides of the layout contract: shared
  `.git/rit/indexdb.sqlite` for reproducible repository metadata, and isolated
  `.git/rit/worktrees/<id>/worktree-cache.sqlite` paths for worktree-local
  cache data.
- Source of truth: IndexDB stores reproducible metadata only. `drop` removes
  the SQLite file without touching Git objects, refs, `.git/index`, or working
  tree files. Normal Git-compatible commands do not require IndexDB.
- Unsupported behavior: writing worktree-specific cache DB contents, corruption
  repair beyond rebuild, and migration beyond rebuild guidance.
- 2026-05-13 IndexDB write-through slice: when the `indexdb` feature is built
  and `.git/rit/indexdb.sqlite` already exists, successful rit-created commits,
  branch/tag ref changes, checkout state changes, and fast-forward merges
  refresh IndexDB on a best-effort basis. A failed or corrupted IndexDB refresh
  never rolls back the already-successful Git repository write.
- Ref snapshots now include `HEAD`, local branches, and lightweight tags.
- 2026-05-13 IndexDB reconciliation slice: `rit indexdb status` now separates
  schema health from freshness and reports stale ref snapshots. `rit indexdb`
  reconciles external Git-compatible HEAD/ref changes by refreshing the
  snapshot and indexing newly reachable commit objects from canonical
  `.git/objects` data.
- 2026-05-13 IndexDB index-state reconciliation slice: IndexDB records the
  canonical `.git/index` checksum, mtime, and size in `cache_state`.
  `rit indexdb status` reports stale index snapshots, and `rit indexdb`
  refreshes the stored index snapshot without making IndexDB a source of truth.
- 2026-05-13 IndexDB pack-state reconciliation slice: IndexDB records the
  `.git/objects/pack` `.pack`/`.idx` file list, size, and mtime as a compact
  cache snapshot. Stale pack snapshots are reported and refreshed by
  `rit indexdb` without parsing pack data during status.
- 2026-05-12 large-file backend trait slice checked the AGENTS large-object
  backend guidance; no external `git-lfs` binary is used.
- 2026-05-12 LFS pointer slice checked installed `git-lfs/3.7.1` and the
  official Git LFS specification for v1 pointer format.
- 2026-05-11 local clone object-transfer slice checked `git clone -h` and a
  direct Git comparison for `clone --local --no-checkout`.
- 2026-05-11 local fetch object-transfer slice checked `git fetch -h` and a
  direct Git comparison for `fetch <local-repository>`.
- 2026-05-11 transport protocol model slice checked `git clone -h` and
  `git fetch -h`; implemented classification for local paths, HTTP(S), SSH
  URLs, and scp-like SSH locations.
- 2026-05-11 local fetch refspec slice checked `git fetch -h` and direct Git
  comparisons for `fetch <local-repository> <src>:<dst>`.
- 2026-05-11 smart HTTP request-model slice checked `git help protocol-http`;
  modeled `info/refs?service=git-upload-pack` and `git-receive-pack`
  discovery URLs and expected advertisement content types.
- 2026-05-11 smart HTTP advertisement parser slice checked
  `git help protocol-http`; implemented pkt-line service-header validation,
  first-ref capability parsing, and advertised ref parsing.
- 2026-05-11 upload-pack request-model slice checked `git help protocol-http`;
  modeled pkt-line `want`, `have`, and `done` request bodies.

## Milestone Notes

### M0: Baseline and rules

- Added `docs/compatibility.md` to record the active Git baseline, command-help checks, current implemented surface, and compatibility policy.
- Added `docs/roadmap.md` to keep the final-product milestones visible while implementation proceeds in small commits.
- Quality gates remain `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo test --workspace`.

### M1: Compatibility test harness

- Added the `rit-testkit` crate as a library and `rit-testkit` CLI.
- The harness copies a fixture repository into isolated Git and rit workspaces, runs the provided command specs, captures stdout/stderr/exit code, and optionally compares final repository snapshots.
- `rit-testkit` may execute `git` because it is test infrastructure. Production `rit` runtime code must not depend on it.
- First smoke test: `git status --porcelain=v1` and `rit status --porcelain=v1` match stdout, stderr, and exit code on a simple committed fixture.
- `status --porcelain=v1` now refreshes `.git/index` stat data for clean
  tracked files while preserving existing index extensions. The compatibility
  test compares command output and final repository state against Git.
- `Index::parsed_extensions()` now exposes optional extension records with
  classified signatures for `TREE`, `REUC`, `UNTR`, `FSMN`, `link`, and
  `sdir`, while preserving raw payload bytes.
- `IndexExtension::cache_tree()` parses `TREE` cache-tree payloads into
  depth-first nodes with path component, covered entry count, optional tree
  object ID, and child nodes. Invalidated cache-tree nodes use `entry_count =
  -1` and no object ID.
- `IndexExtension::resolve_undo()` parses `REUC` payloads into per-path stage
  records with octal mode and object ID values for stages 1 through 3.
- `IndexExtension::fs_monitor()` parses `FSMN` version 1 timestamps, version 2
  opaque tokens, bitmap size, and raw bitmap bytes. EWAH bitmap decoding is
  still deliberately left as a later payload-specific step.
- `IndexExtension::split_index_link()` parses the `link` shared-index object
  ID and preserves the following delete/replace bitmap bytes.
- `IndexExtension::sparse_directory()` exposes the `sdir` sparse-directory
  marker extension and preserves any raw marker payload.
- `IndexExtension::untracked_cache()` parses `UNTR` environment strings, stat
  blocks, dir flags, exclude-file hashes, per-directory exclude filename, and
  directory blocks with untracked name lists. It also parses the three EWAH
  bitmaps in the tail, then the stat/hash arrays selected by the valid-stat
  bitmap, and validates the trailing NUL.
- `IndexExtension::end_of_index_entry()` parses `EOIE` entry-end offsets and
  extension table hashes. `IndexExtension::index_entry_offset_table()` parses
  `IEOT` version 1 offset/count entries.
- Added rit CLI integration compatibility fixtures for read-only `diff` output modes: `--name-only`, `--name-status`, `--numstat`, and `--stat`, including cached diff variants.
- Compatibility reports now include the first differing stdout/stderr line when command text differs.
- Added reusable `rit-testkit` local write fixture builders for nested tracked
  files, detached checkout history, merged branch deletion, and unmerged branch
  deletion. The `rit-cli` write compatibility tests now share these builders
  instead of each test hand-writing repository setup.

### M2: Core repository model foundation

- Added `Repository::open(path)` as the application-facing alias for repository discovery.
- Repository discovery and init now construct repositories through a shared format guard.
- Repositories with `core.repositoryformatversion` other than `0`, or unknown `[extensions]` keys, fail with clear errors before use.
- Linked worktrees now read the per-worktree gitdir from a `.git` file and
  resolve `commondir` for shared objects, refs, packed refs, config, and
  info/exclude while keeping `HEAD` and `index` under the per-worktree gitdir.
- Added a shared `GitConfig` parser for scalar config reads. It handles
  section names case-insensitively, quoted subsections, comments, key-only
  booleans, quoted values, and last-one-wins lookup. It is now used for the
  repository format guard and commit identity config reads.
- Still unsupported: include/includeIf expansion, full escape parity, typed
  value coercion beyond current callers, and multi-valued config APIs.

### M3: Local diff scope expansion

- Added staged diff support through `Repository::diff_index_to_head()`.
- `rit diff --cached --name-only`, `rit diff --cached --stat`, `rit diff --staged --name-only`, and `rit diff --staged --stat` now compare the index with `HEAD`.
- Unborn `HEAD` is treated like an empty tree for staged diff, matching the usual Git shape for newly added files.
- Added `--name-status` formatting for both default and cached diff scopes.
- Added `--numstat` formatting for both default and cached diff scopes.
- Added a conservative `PathspecSet` model for ordinary literal file and
  directory pathspecs, plus simple `*`, `?`, and bracket-class wildcard
  pathspecs.
- Added pathspec filtering for `status --porcelain=v1` and all supported
  `diff` summary modes, including `--cached`/`--staged`.
- `status --porcelain=v1` now collapses fully untracked directories into
  `?? dir/` entries like Git, while exact file pathspecs keep the untracked
  file path expanded.
- `status --porcelain=v1` now quotes paths containing whitespace, quotes, or
  backslashes with Git-like C-style escaping.
- Added Git comparison coverage for `status --porcelain=v1 -- <pathspec>` and
  `diff ... -- <pathspec>`, including simple wildcard pathspecs.
- Added ordinary literal file, directory, and `.` pathspec expansion for
  `add`, `restore`, and `reset`.
- Added ordinary literal pathspec filtering for `ls-files`, including
  `--stage`.
- Added ordinary literal path lookup for `ls-tree <tree-ish> <path>`,
  including `--name-only` and `--object-only`.
- Added ordinary literal path filtering for first-parent `log`, including
  `--oneline`.
- Added ordinary literal path filtering for `show --no-patch`. Commits that do
  not touch the requested paths produce no output, matching Git's no-patch
  behavior for simple path filters.
- Added simple wildcard pathspec compatibility coverage for `ls-files`,
  first-parent `log`, and `show --no-patch`.
- Added ASCII POSIX bracket character class support, such as `[[:digit:]]`,
  to the shared pathspec wildcard matcher.
- Refined `:(glob)` double-star matching so `**/` and trailing `**` can cross
  directories, while component-local forms such as `**base.txt` behave like
  ordinary stars and do not cross `/`, matching Git.
- Added `git diff -z` compatible NUL-terminated output for `--name-only`,
  `--name-status`, and `--numstat`, including rename/copy field splitting.
- Added positive pathspec magic support for `:(literal)`, `:(glob)`,
  `:(top)`, and `:/` with Git comparison coverage for status, diff, ls-files,
  first-parent `log`, `show --no-patch`, and `add`.
- Added ASCII case-insensitive `:(icase)` pathspec matching with Git comparison
  coverage for status, diff, and add.
- Added exclude pathspec magic for `:!`, `:^`, and `:(exclude)`, including the
  Git behavior where exclude-only pathspecs filter from the full path set.
- Added patch output for small text files in default and cached diff scopes,
  with Git comparison coverage for default patch, `-p`, and `--cached`.
- Patch output now emits `\ No newline at end of file` markers for missing
  trailing newlines in default and cached text patches.
- Binary patch output now emits Git-like `Binary files ... differ`
  placeholders for default and cached diff scopes.
- Patch output now splits distant changes into multiple hunks with three lines
  of context and simple Git-like hunk header context.
- Added binary diff accounting for summary modes. `--numstat` reports
  `-\t-\t<path>` and `--stat` reports `Bin <old> -> <new> bytes` with zero
  insertion/deletion totals.
- Added exact staged rename detection for `diff --cached -M` summary and patch
  output.
- Added worktree rename/copy detection for default `diff -M/-C` when the added
  path is represented by Git's intent-to-add index state, including summary
  and patch output.
- Added compatibility coverage for default `diff -M` worktree non-exact
  rename similarity thresholds with Git intent-to-add destinations.
- Added compatibility coverage for worktree `diff -M -l<n>` rename-limit
  warnings when exhaustive intent-to-add similarity detection is skipped.
- Added compatibility coverage for worktree `diff -C -l<n>` copy-limit
  warnings when exhaustive intent-to-add copy detection is skipped.
- Exact copy detection now runs before the `-l<n>` exhaustive similarity limit
  for cached and worktree intent-to-add copy detection, matching Git's cheap
  exact-copy pass and avoiding spurious warnings for exact copies.
- Added `-l<n>` parsing and a conservative candidate limit model for
  rename/copy detection. A limit of `0` is treated as unlimited, matching
  Git's command shape.
- Added a Git-compatible exact-rename pass before the `-l<n>` exhaustive
  similarity limit check, so exact cached and worktree intent-to-add renames
  are still reported when the limit is below the changed path count.
- Refined the `-l<n>` limit model to count the larger side of source and
  destination candidate sets instead of the total changed-path count, matching
  Git for one-source/one-destination non-exact rename and copy detection.
- Diff summary and patch results now carry Git-shaped warnings; the CLI writes
  rename-limit warnings to stderr for supported `diff` output modes when
  exhaustive similarity detection is skipped.
- `diff.renameLimit` from Git config is now used as the default rename/copy
  candidate limit when the CLI does not provide `-l<n>`; explicit `-l<n>`
  still wins.
- Invalid `diff.renameLimit` values now use Git-compatible fatal output and
  exit code before diff output is produced.
- Percent-less `-M<n>`/`-C<n>` and `--find-renames=<n>`/`--find-copies=<n>`
  values now use Git's fractional notation: `5` means 50%, `05` means 5%,
  and `400` means 40%. Percent-suffixed values above 100 are accepted and
  naturally match no ordinary similarity score, like Git.
- Added local write compatibility coverage that compares Git and rit porcelain
  state after directory pathspec `add`, `restore`, and `reset`.
- Added local write compatibility coverage for simple wildcard and
  bracket-class pathspecs in `add`, `restore`, and `reset`.
- Added `--pathspec-from-file` and `--pathspec-file-nul` CLI parsing for
  `add`, `restore`, and `reset`, backed by the existing pathspec expansion.
- Added Git comparison coverage for stdin-delivered NUL-separated pathspecs
  in `add`, `restore`, and `reset`.
- Added C-style quoted pathspec-file entry parsing for common escapes.
- Octal C-style pathspec-file escapes are decoded as bytes before UTF-8
  validation, matching Git for non-ASCII paths such as `caf\303\251.txt`.
- Octal C-style pathspec-file escapes must contain exactly three octal digits;
  shorter or incomplete octal escapes are rejected as badly quoted before any
  `add`, `restore`, or `reset` mutation.
- Added `\a` C-style pathspec-file decoding and Git-compatible
  pathspec-not-found output for `add` and `restore`; pathspec-only `reset`
  keeps Git's no-op behavior when no index or `HEAD` path matches.
- Empty pathspec files now match Git: `add` succeeds with Git's empty
  pathspec advice, `restore` fails with Git's fatal restore message, and
  `reset` treats the empty file as a full pathspec reset of the index.
- Empty `--pathspec-from-file=` values follow the same empty-input behavior
  as Git instead of attempting to open an empty file name.
- Empty NUL-mode pathspec files now follow Git's empty-input behavior while
  still rejecting actual empty NUL-delimited pathspec entries.
- Repeated `--pathspec-from-file` options use the last file, matching Git for
  `add`, `restore`, and `reset`.
- `--pathspec-from-file` cannot be mixed with ordinary pathspec arguments,
  matching Git's fatal pre-mutation validation for `add`, `restore`, and
  `reset`.
- Missing pathspec input files are rejected with Git's fatal
  `could not open ... for reading` message and exit code before any mutation.
- Non-UTF-8 bytes in text pathspec files are decoded lossily so unmatched
  pathspec behavior follows Git instead of failing as an input encoding error.
- NUL bytes inside text pathspec-file lines truncate that line before normal
  empty-entry and quoted-entry parsing, matching Git's non-NUL mode behavior.
- Lone carriage-return bytes are preserved as pathspec characters; CRLF line
  endings are still normalized by text line splitting, matching Git behavior.
- `--pathspec-file-nul` without `--pathspec-from-file` is rejected before any
  `add`, `restore`, or `reset` mutation with Git's fatal dependency message.
- `--pathspec-from-file` without a following value is rejected before any
  `add`, `restore`, or `reset` mutation with Git's option-value error.
- Added Git-compatible rejection for empty line-delimited pathspec-file
  entries before any `add`, `restore`, or `reset` mutation is applied.
- Added Git-compatible rejection for quoted empty pathspec-file entries before
  any `add`, `restore`, or `reset` mutation is applied.
- Quoted pathspec-file entries now stop at the first unescaped closing quote
  and ignore trailing bytes, matching Git's text pathspec-file parser.
- Added Git-compatible rejection for empty NUL-delimited pathspec-file entries
  before any `add`, `restore`, or `reset` mutation is applied, while keeping a
  final trailing NUL terminator valid like Git.
- Added Git-compatible rejection for badly quoted pathspec-file entries before
  any `add`, `restore`, or `reset` mutation is applied.
- Still unsupported: full Git pathspec-file edge cases, broader worktree
  rename/copy diffcore parity beyond intent-to-add entries, full rename
  limits/advanced diffcore parity, and `show` path filtering for patch output.

### M7: Remote transport foundation

- Added SSH Git protocol advertisement parsing for native upload-pack streams
  that do not include the smart HTTP service header.
- Added a process-backed interactive SSH upload-pack executor that reads the
  advertisement, selects the requested ref, writes one upload-pack request in
  the same session, and extracts the returned pack.
- `rit fetch` now routes SSH/scp-like remotes through the SSH upload-pack
  fetch path, ingests the received pack, and writes `FETCH_HEAD`.
- Added a process-backed interactive SSH receive-pack executor and routed
  `rit push` for SSH/scp-like remotes through one source-to-destination refspec
  update with report-status validation.
- Added SSH URL port parsing for `ssh://host:port/path` and process executor
  wiring for `ssh -p <port>`.
- Added `GIT_SSH_COMMAND` shell-word parsing and `GIT_SSH` program override
  support when building SSH process invocations.
- Still unsupported: auth option parity, broader SSH option/config support,
  multi-round negotiation, and thin-pack fixups.

### M8: Merge-state local workflows

- Added `Repository::merge_state()` and structured `MergeState`/
  `RebaseState` models.
- Reads `MERGE_HEAD`, `CHERRY_PICK_HEAD`, `REVERT_HEAD`, `MERGE_MSG`,
  `SQUASH_MSG`, `rebase-apply`, and `rebase-merge` from the repository
  operation state directory.
- Added `Repository::merge_ff_only(target)` and `rit merge [--ff-only]
  <target>` for the first merge command slice. It requires a clean worktree,
  resolves a local branch or revision, verifies the current `HEAD` is an
  ancestor of the target, checks out the target tree, and advances the current
  branch or detached `HEAD`.
- Added non-fast-forward merge planning for `rit merge --plan <target>` and
  `rit merge explain <target>`. This reports `HEAD`, target, and a simple
  graph-walk merge base without writing refs, index, or worktree files.
- Non-fast-forward planning now compares merge-base, `HEAD`, and target trees
  to report head-side changes, target-side changes, and conflict candidates.
  It also reports candidate base/head/target stage entries for conflicting
  paths. This is planning metadata only; no conflict index stages are written
  yet.
- The index parser/writer now preserves Git stage bits 0/1/2/3, sorts duplicate
  conflict paths by stage, rejects invalid stage values, and lets
  `ls-files --stage` print the stored stage instead of assuming stage 0.
- Conflicted non-fast-forward `rit merge <target>` now writes unmerged index
  stage entries, `MERGE_HEAD`, and `MERGE_MSG`, records the operation journal
  entry, and refuses to commit while the index contains unmerged entries.
- Regular text content conflicts now materialize simple `<<<<<<< HEAD`,
  `=======`, and `>>>>>>> <target>` markers in the working tree.
- `rit merge --abort` restores the `ORIG_HEAD` tree, clears merge state files,
  refreshes the index/worktree, and records the operation journal entry.
- `rit merge --quit` clears merge state files while leaving unmerged index
  stages and working tree conflict contents untouched, matching Git's
  state-forgetting behavior for the supported slice.
- `rit merge --continue` commits a resolved merge using `HEAD` plus
  `MERGE_HEAD` as parents, clears merge state files, and records the created
  merge commit in the operation journal.
- Clean non-fast-forward merges without conflict candidates now materialize the
  merged index/worktree and create a merge commit with `HEAD` and the target as
  parents.
- Delete/modify conflicts now leave the modified side in the working tree for
  both `HEAD`-deleted and target-deleted cases while preserving the relevant
  conflict stage entries. Merge results carry structured conflict reports so
  the CLI can print Git-shaped `modify/delete` messages instead of treating
  every conflict as content-only.
- Binary content conflicts now leave the `HEAD` version in the working tree,
  preserve conflict stages, and print a Git-shaped binary merge warning.
- Add/add conflicts now carry a structured conflict kind and print a
  Git-shaped `add/add` conflict result message.
- Clean non-fast-forward merges now combine mode-only changes on one side with
  content-only changes on the other side, producing a regular merge commit with
  the content-side blob and the mode-side file mode.
- Regular-file/symlink distinct-type conflicts now follow Git's split-path
  shape in both directions: the non-regular side stays at the original path,
  the regular side is written to a suffixed path such as `~HEAD` or
  `~<target>`, and the CLI prints a `CONFLICT (distinct types)` message.
- Content conflicts that also change file mode now preserve Git-shaped stage
  modes, for example stage 3 can keep `100755` while the conflict is reported
  as a regular `CONFLICT (content)`.
- Content, binary, and add/add conflicts now print Git-shaped `Auto-merging`
  lines, and conflicted merge output no longer includes rit-only pre-merge
  target debug text.
- Still unsupported: remaining full conflict result message parity, strategies,
  merge hooks, `cherry-pick`, `rebase`, and `stash`.

### M16: Operation journal and universal undo

- Added `Repository::operations()` as the public entry point for rit-specific
  operation metadata.
- Operation records are appended to `.git/rit/ops.log`; this file is never a
  source of truth for Git compatibility and can be deleted without changing the
  underlying repository.
- The first snapshot model records `HEAD`, the current branch, and a raw index
  checksum before and after a user operation.
- `rit commit`, `rit checkout`, `rit switch`, and fast-forward `rit merge`
  record successful operations from the CLI.
- Added `rit op log` to print operation records newest-first.
- Added `rit op restore <id>` and `rit undo` for records with a restorable
  previous `HEAD`; restore updates the previous branch or detached `HEAD`,
  checks out that commit tree, and rewrites the index from that tree.
- Operation records now include changed path lists computed by comparing the
  before/after commit trees. They also include known created object IDs; the
  first wired caller records the new commit object ID after `rit commit`.
- Malformed operation journal lines are skipped with diagnostics from
  `log_with_warnings`; `rit op log` reports warnings on stderr while preserving
  valid records.
- `rit op log --json` prints newest-first operation records as structured JSON,
  including before/after snapshots, changed paths, created object IDs, and
  malformed-line warnings in a `warnings` array.
- Successful `rit add`, `rit restore`, and pathspec `rit reset` operations now
  append operation records with command-provided changed path metadata, covering
  the first index-only/worktree-changing journal slice.
- Successful `rit branch` create/delete and `rit tag` create/delete operations
  now append operation records for explicit local ref changes.
- Successful `rit fetch` operations and smart-remote `rit push` success paths
  now append operation records for explicit transport writes.
- Index-changing operation records now write a `.git/rit/ops/<id>/before.index`
  sidecar when the pre-operation index exists. `rit undo` and
  `rit op restore <id>` can use that sidecar to restore index-only operations
  without rewriting the working tree.
- Still unsupported: command-aware undo modes, reversible patches for
  worktree-only operations and complete object creation inventories.

### M9: Large-file backends

- Added backend-neutral `LargeFileBackendKind`, `LargeFileTrackRule`, and
  `LargeFilePointer` models.
- Added an object-safe `LargeFileBackend` trait for parsing and encoding
  backend pointer blobs without depending on external `git-lfs`.
- Added `GitLfsBackend`, `parse_lfs_pointer`, and `encode_lfs_pointer` for Git
  LFS v1 pointer blobs with `version`, `oid sha256`, and `size` fields.
- Added `LfsLocalCache` for sharded `.git/lfs/objects/<aa>/<bb>/<sha256>`
  storage with streaming writes and SHA-256/size verification.
- Added Git LFS Batch API request/response models and JSON codecs for the
  `basic` transfer adapter, including per-object actions and errors. Batch
  request ref context serializes with the Git LFS protocol key `ref`.
- Added conservative Xet detection from explicit `filter=xet` attributes and
  Xet pointer hash extension lines, while preserving LFS-compatible attribute
  rules as separate hints.
- Added Xet hash, xorb chunk range, reconstruction term/file, and local cache
  path models for future CAS integration.
- Added `rit-core` Cargo feature gates: `lfs`, `xet`, and `large-files`.
- Still unsupported: LFS Batch API HTTP client execution, Xet CAS API
  execution, content-defined chunking, and xorb/shard binary parsing.

### M10: Sparse, partial clone, workspace

- Baseline checked: `git version 2.52.0.windows.1`, `git help -a`, and
  `git sparse-checkout -h`.
- Added a read-only sparse-checkout state model for `core.sparseCheckout`,
  `core.sparseCheckoutCone`, and `.git/info/sparse-checkout` patterns.
- Added `Repository::sparse_checkout()` as the public reader entry point.
- Added optional `rit.toml` / `.rit.toml` workspace profile parsing for
  `[workspace.<name>] include = [...]` and `Repository::rit_config()`.
- Added read-only partial clone policy discovery for `remote.*.promisor`,
  `remote.*.partialCloneFilter`, and `objects/pack/*.promisor` markers.
- Added workspace-profile `partial_clone` / `lazy_files` flags and a derived
  lazy materialization policy model.
- Added a dry-run workspace prefetch plan model and
  `rit workspace prefetch <profile>` command shape.
- Still unsupported: sparse-checkout write commands, applying named workspace
  profiles, fetching missing partial-clone objects, lazy materialization file
  I/O, and prefetch command execution.

## Implemented Commands

### M11: Auth

- Baseline checked: `git credential -h` exposes `fill`, `approve`, and
  `reject` actions.
- Added transport-independent credential models: `Credential`,
  `CredentialRequest`, `CredentialProvider`, and redacted `SecretString`.
- Secrets are redacted through `Debug` and `Display`; callers must explicitly
  call `expose_secret()` to authenticate.
- Added `EnvironmentTokenProvider` for `RIT_TOKEN`, `GIT_TOKEN`,
  `GITHUB_TOKEN`, `GITLAB_TOKEN`, and `HF_TOKEN`.
- Added Git credential helper line-protocol encode/decode for request and
  response-shaped messages.
- Added `credential.helper` config lookup for helper command shape.
- Added ordered `credential.helper` chain parsing, including empty helper
  entries that reset earlier helpers.
- Added process-backed credential helper execution for `get`, `store`, and
  `erase`, with helper-chain merging, `quit` handling, and ignored stdout for
  store/erase operations.
- Intentional difference: named helpers execute as `git-credential-*` programs
  directly instead of routing through `git credential-*`, preserving rit's
  no-Git-wrapper production rule while remaining compatible with helpers
  installed on `PATH`.
- Added SSH agent availability modeling from `SSH_AUTH_SOCK`.
- Added SSH agent protocol identity lookup and signing client for blocking
  read/write streams, plus Unix-domain socket connection from `SSH_AUTH_SOCK`.
- Added platform default OS keychain adapter selection models for Windows
  Credential Manager, macOS Keychain, and freedesktop Secret Service/libsecret.
- Added `SystemKeychainProvider` with Windows Credential Manager read/write and
  erase support through the native Credential Manager API.
- Added CI/non-interactive prompt policy for `CI`, `GITHUB_ACTIONS`,
  `RIT_NONINTERACTIVE`, and `GIT_TERMINAL_PROMPT=0`.
- Still unsupported: Windows named-pipe SSH agent connections, macOS Keychain
  read/write, libsecret read/write, and using auth policies inside transport
  execution.

### M12: Semantic Diff

- Existing text diff foundation includes unified patch output, hunk splitting,
  missing-newline markers, binary placeholders/stat accounting, and exact
  staged rename detection.
- Added a standalone word-level diff model with stable `Equal`, `Delete`, and
  `Insert` operations for later semantic summaries.
- Added optional `semantic-tree-sitter` Cargo feature and parser wrapper for
  language-specific semantic adapters.
- Added optional `semantic-rust` feature with a tree-sitter Rust adapter that
  summarizes added, deleted, and changed functions.
- Added optional `semantic-typescript` feature with a tree-sitter TypeScript
  adapter that summarizes added, deleted, and changed function declarations.
- Added optional `semantic-python` feature with a tree-sitter Python adapter
  that summarizes added, deleted, and changed function definitions.
- Added `SemanticDiffReport` JSON output model behind `semantic-json`,
  including path classification for code, tests, docs, and other files.
- M12 still does not wire semantic summaries into the `rit diff` CLI.

### M13: Policy, Doctor, Repair

- Added `[policy]` config model in `rit.toml` / `.rit.toml` with optional
  `max_regular_blob_size`, `deny_secrets`, `protect_branches`, and explicit
  `enforcement = "warn" | "block"`.
- Added regular blob size policy findings with warning-by-default and explicit
  blocking severity.
- Added conservative secret-pattern policy findings for private key blocks and
  common token prefixes without including matched secret values in messages.
- Added protected branch policy findings that accept either short branch names
  or `refs/heads/*` names in `protect_branches`.
- Added read-only `Repository::doctor` and `rit doctor` to check repository
  directories, Git config readability, rit config readability, HEAD parsing,
  and HEAD object presence without invoking external Git.
- Added conservative `Repository::repair_plan`, `Repository::apply_repair_plan`,
  and `rit repair [--dry-run|--apply]`; the first repair action set only creates
  missing standard Git directories and refuses paths outside the repository.
- Policy defaults warn and do not block writes; blocking requires explicit
  `enforcement = "block"`.
- Still unsupported: full object graph fsck, automatic ref/object repair, and
  repair of unsupported repository formats.

### `rit doctor`

- Baseline command checked: `git version 2.52.0.windows.1`; `git help -a`
  does not list `doctor`, while related Git maintenance commands include
  `git fsck` and `git maintenance`.
- Supported options: `rit doctor`.
- Unsupported options: JSON output, repair actions, full object graph fsck.
- Intentional differences: `doctor` is a rit-specific read-only health summary,
  not a Git porcelain command.
- Repository mutation: no.
- Risk: low; reads repository files only.

### `rit repair`

- Baseline command checked: `git version 2.52.0.windows.1`; `git help -a`
  does not list `repair`, while related Git maintenance commands include
  `git fsck` and `git maintenance`.
- Supported options: `rit repair`, `rit repair --dry-run`, `rit repair --apply`.
- Unsupported options: object recovery, ref recovery, index repair, config
  rewrite, and JSON output.
- Intentional differences: `repair` is a rit-specific safety command; it plans
  by default and only creates missing standard Git directories with `--apply`.
- Repository mutation: only with `--apply`.
- Risk: medium-low; it creates directories but does not overwrite existing
  files or refs.

### M14: VFS

- Added optional Cargo feature `vfs`, disabled by default.
- Added common VFS model types: `VfsBackendPreference`, `VfsAvailability`,
  `VfsLazyMaterialization`, and `VfsPlan`.
- Added `FallbackMaterializedBackend` planning so builds without a platform VFS
  can keep the whole worktree or configured workspace include paths as ordinary
  materialized files.
- Added platform backend planning for Windows Projected File System, macOS FUSE,
  and Linux FUSE candidates without claiming that OS-specific drivers are
  implemented.
- Added `Repository::materialize_vfs_blob` for safe on-demand blob
  materialization into a worktree path; it rejects path traversal, writes via a
  temporary file, does not overwrite existing files, and verifies the object is
  a blob.
- Added `Repository::prefetch_vfs_objects` and `Repository::spawn_vfs_prefetch`
  for local object warmup in a background worker, reporting available and
  missing objects without invoking external Git.
- VFS availability now reports a clear message when a binary is built without
  the `vfs` feature.
- Still unsupported: platform backend execution and network/promisor object
  fetching for missing VFS objects.

### M15: Release Packaging

- Added `docs/release.md` with the first `rit-min` / `rit-full` feature matrix
  and the current `rit-core` feature flag inventory.
- Added `.github/workflows/ci.yml` with stable Rust format, clippy, test, and
  release build matrices across Ubuntu, macOS, and Windows for `rit-min` and
  `rit-full`.
- Documented release archive naming, target triples, and archive contents in
  `docs/release.md`.
- Added README release build instructions for `rit-min` and `rit-full`.
- Added root `LICENSE-MIT`, `LICENSE-APACHE`, and `THIRD-PARTY-NOTICES.md`;
  updated README/release docs to match the workspace `MIT OR Apache-2.0`
  license and full-feature dependency attribution audit.

### `rit version`

- Baseline command checked: `git version`
- Help reference used: `git help -a` lists `version` as "Display version information about Git".
- Supported options: `rit version`, `rit --version`
- Unsupported options: Git-specific build option details are not emitted yet.
- Intentional differences: output begins with `rit version`, not `git version`.
- Repository mutation: no
- Risk: none

### `rit help`

- Baseline command checked: `git help -a`
- Supported options: `rit help`, `rit help version`, `rit help help`, `rit --help`, `rit -h`
- Unsupported options: external man, web, info, config-driven help format.
- Intentional differences: help text documents only implemented `rit` commands.
- Repository mutation: no
- Risk: none

### `rit init`

- Baseline command checked: `git init -h`
- Supported options: `-q`, `--quiet`, `--bare`, `-b <branch>`, `--initial-branch <branch>`, optional directory.
- Unsupported options: templates, separate git dir, object format, ref format, shared repositories.
- Git-compatible behavior: creates `HEAD`, `config`, `objects`, `refs`, `info`, `hooks`, `branches`.
- Intentional differences: default branch is currently `master` unless explicitly set; template hooks are not copied.
- Repository mutation: yes
- Safety notes: files are first written through exclusive `.lock` files and then renamed into place.
- Risk: low for empty or reinitialized repositories; unsupported advanced init modes fail instead of guessing.

### `rit rev-parse`

- Baseline command checked: `git rev-parse -h`
- Supported options: `--git-dir`, `--show-toplevel`, `--is-inside-work-tree`.
- Unsupported options: revision parsing, path formatting, quoting, parseopt, abbreviation, symbolic refs.
- Git-compatible behavior: discovers `.git` by walking upward from the current directory.
- Intentional differences: only the supported path/fact options are accepted.
- Repository mutation: no
- Risk: none

### `rit cat-file`

- Baseline command checked: `git cat-file -h`
- Supported options: `-t`, `-s`, `-p`, and `<type> <object>` for full
  40-character object IDs.
- Unsupported options: abbreviated object IDs, revision syntax, batch modes,
  filters, textconv, mailmap.
- Git-compatible behavior: loose and packed object decoding, header
  validation, object type and size output, tree pretty printing.
- Intentional differences: revision resolution is intentionally narrow.
- Repository mutation: no
- Risk: none

### `rit ls-tree`

- Baseline command checked: `git ls-tree -h`
- Supported options: default output, `--name-only`, `--object-only`, commit or
  tree revisions, and ordinary literal path lookup including positive
  `:(literal)`, `:(top)`, and `:/` magic forms.
- Unsupported options: recursion, long output, custom format, abbreviation,
  glob/exclude/attr/icase pathspec magic.
- Git-compatible behavior: tree entry parsing and default `<mode> <type> <object>\t<path>` output.
- Intentional differences: advanced pathspec forms are not implemented yet.
- Repository mutation: no
- Risk: none

### `rit status`

- Baseline command checked: `git status -h`
- Supported options: `--porcelain`, `--porcelain=v1`, `-s`, plus ordinary
  literal file/directory pathspecs and simple `*`, `?`, and bracket-class
  wildcard pathspecs after `--`, positive `:(literal)`, `:(glob)`, `:(top)`,
  `:/`, and `:(icase)` pathspec magic, and
  `--untracked-files=no|normal|all` / `-uno|-unormal|-uall`, including
  default-all `-u`, Git 2.52's normal-mode `--no-untracked-files`, and `-z`
  NUL-terminated output, plus `-b` / `--branch` branch headers and
  `--ignored` / `--ignored=traditional|matching` for `.gitignore` and
  `.git/info/exclude` rules.
- Unsupported options: long output, attr pathspec magic, rename
  detection, submodules, sparse checkout.
- Git-compatible behavior: porcelain v1 entries for staged add/modify/delete, working tree modify/delete, and untracked files.
- Git-compatible behavior: fully untracked directories are collapsed in the
  default porcelain output, with direct file pathspecs preserving file output.
- Git-compatible behavior: `--untracked-files=no` hides untracked paths,
  `normal` collapses directories, and `all` lists each untracked file.
- Git-compatible behavior: porcelain paths with whitespace or special
  characters are quoted.
- Git-compatible behavior: `-z` writes raw paths and terminates each entry with
  NUL instead of newline.
- Git-compatible behavior: `-b` writes the porcelain branch header for local,
  unborn, and detached HEAD states. Upstream ahead/behind details are not
  implemented yet.
- Git-compatible behavior: `--ignored` writes `!!` entries for ignored files
  and collapsed ignored directories. Ignore matching supports literal,
  directory-only, anchored, `*`, `?`, bracket-class, `**`, last-match-wins
  negation, and `.git/info/exclude` rules. `-uno` hides ignored entries,
  matching Git 2.52 behavior.
- Intentional differences: ignore matching is still rooted at repository-level
  ignore files; nested per-directory `.gitignore` files are not loaded yet.
- Repository mutation: no
- Risk: no repository writes.

### `rit diff`

- Baseline command checked: `git diff -h`
- Supported options: default patch output for small text files, `-p`, `-u`,
  `--name-only`, `--name-status`, `--numstat`, `--stat`, plus
  `--cached`/`--staged` with those output modes, and ordinary literal
  file/directory plus simple `*`, `?`, and bracket-class wildcard pathspec
  filters and positive `:(literal)`, `:(glob)`, `:(top)`, `:/`, and
  `:(icase)` pathspec magic. `-M[<n>]`/`--find-renames[=<n>]` supports staged
  exact and non-exact rename detection in cached diff output. `-C[<n>]` and
  `--find-copies[=<n>]` support staged copy detection from modified source
  files. `--find-copies-harder` also considers unchanged HEAD files as staged
  copy sources. Default worktree diff supports `-M[<n>]` and `-C[<n>]` when
  the added worktree path is represented by Git's intent-to-add index state;
  `--find-copies-harder` can also use unchanged index files as worktree copy
  sources for that intent-to-add slice.
- Unsupported options: commit/tree/blob arguments, pathspec files,
  broader worktree rename/copy diffcore parity beyond intent-to-add entries,
  full rename limits, and many advanced patch formatting options.
- Git-compatible behavior: default diff scope compares working tree files against the index and ignores untracked files.
- Git-compatible behavior: cached diff scope compares the index against `HEAD`.
- Intentional differences: advanced patch formatting and custom diff drivers
  are not implemented yet.
- Repository mutation: no
- Risk: no repository writes.

### `rit log`

- Baseline command checked: `git log -h`
- Supported options: default output, `--oneline`, and ordinary literal plus
  simple `*`/`?` wildcard file or directory path filters and positive
  `:(literal)`, `:(glob)`, `:(top)`, `:/`, and `:(icase)` pathspec magic.
- Unsupported options: revision ranges, decoration, graph, pathspec attr magic,
  advanced path history simplification, grep, ordering
  controls, diff output.
- Git-compatible behavior: reads commits from `HEAD`, follows the first parent,
  prints default author/date/message layout and 7-character oneline IDs.
- Intentional differences: merge traversal is first-parent only until revision
  walking is implemented; rename-aware history simplification is not
  implemented.
- Repository mutation: no
- Risk: no repository writes.

### `rit add`

- Baseline command checked: `git add -h`
- Supported options: `--plan`, ordinary literal file, directory, `.`, simple `*`, `?`,
  and bracket-class wildcard pathspecs, positive `:(literal)`, `:(glob)`,
  `:(top)`, `:/`, and `:(icase)` pathspec magic, plus `--chmod=+x`, `--chmod=-x`,
  `--chmod +x`, `--chmod -x`, `--pathspec-from-file`, `--pathspec-from-file=-`, and
  `--pathspec-file-nul`.
- Unsupported options: full Git pathspec-file edge cases, update/all modes,
  patch/interactive mode, sparse mode, ignored-file override.
- Git-compatible behavior: writes blob loose objects and Git index v2 entries
  for regular files; directory pathspecs recursively add regular files and
  stage deletions for matching tracked files that no longer exist.
- Git-compatible behavior: empty pathspec input succeeds with Git's
  `Nothing specified, nothing added.` advice and does not write the index.
- Git-compatible behavior: when `core.ignorecase=true`, a mismatched-case
  non-wildcard `add` pathspec that corresponds to an existing worktree or
  indexed path is accepted as Git-compatible no-op instead of creating a
  wrongly cased index entry.
- Git-compatible behavior: `--chmod=+x|-x` updates the index mode for regular
  files and committed trees preserve `100644`/`100755` modes.
- Git-compatible behavior: existing index modes are preserved when content is
  refreshed without an explicit `--chmod` override.
- Git-compatible behavior: symlinks are indexed as `120000` blobs containing
  the link target text.
- Git-compatible behavior: when `core.symlinks=false`, `rit add` records a
  worktree symlink as a regular `100644` blob containing the link target text.
- rit-specific behavior: `--plan` prints paths that would be added or removed
  from the index without writing objects or `.git/index`.
- Intentional differences: ignored-file checks are not implemented yet. On
  Windows, worktree executable bits remain filemode-insensitive like Git's
  usual `core.filemode` behavior there.
- Repository mutation: yes, writes loose objects and `.git/index` using lock/rename.
- Risk: low for explicit files; missing paths remove matching index entries.

### `rit commit`

- Baseline command checked: `git help commit` and `git commit -h`; this
  Windows environment produced no standard output for `git help commit`, so
  option details were confirmed with `git commit -h`.
- Supported options: `-m <message>`, `--message <message>`,
  `--message=<message>`, `--author=<author>`, `--author <author>`,
  `--date=<date>`, `--date <date>`, `-n`, `--no-verify`, `--verify`,
  `--plan`.
- Supported environment: `GIT_AUTHOR_NAME`, `GIT_AUTHOR_EMAIL`,
  `GIT_AUTHOR_DATE`, `GIT_COMMITTER_NAME`, `GIT_COMMITTER_EMAIL`, and
  `GIT_COMMITTER_DATE`.
- Unsupported options: signing, amend, templates, cleanup modes,
  pathspec commits, natural-language date parsing, `core.hooksPath`.
- Git-compatible behavior: writes tree and commit loose objects, uses first parent from `HEAD`, advances symbolic `HEAD` ref.
- Git-compatible behavior: `--author` accepts `Name <email>` and `--date`
  accepts the raw `<unix-seconds> <+/-HHMM>` date form used by commit objects.
- Git-compatible behavior: runs executable `pre-commit`,
  `prepare-commit-msg`, `commit-msg`, and `post-commit` hooks from
  `.git/hooks`; `--no-verify` bypasses `pre-commit` and `commit-msg`.
- Git-compatible behavior: committed `100644`/`100755` blob modes are written
  into tree objects from the index.
- Git-compatible behavior: committed symlink entries are written as `120000`
  blob tree entries.
- rit-specific behavior: `--plan` compares `.git/index` with `HEAD` and
  prints the parent, message summary, hook mode, indexed file count, and staged
  paths that would be committed without writing tree objects, commit objects,
  refs, operation metadata, or running hooks.
- Intentional differences: default commit timestamps use UTC `+0000`;
  Windows hook execution looks for common Git for Windows `sh.exe` locations
  when running shebang hook scripts.
- Repository mutation: yes, writes objects and updates the current branch ref using lock/rename.
- Risk: moderate; implemented only for simple indexed regular files.

### `rit branch`

- Baseline command checked: `git branch -h`
- Supported options: list local branches, `--show-current`, create branch at `HEAD`, `-d`/`--delete`.
- Unsupported options: remote branches, rename/copy, upstream config,
  `--merged`/`--no-merged` listing filters, formatting, sorting controls,
  force.
- Git-compatible behavior: local branches are refs under `refs/heads`; current branch is detected from symbolic `HEAD`.
- Git-compatible behavior: `-d` refuses the current branch and refuses local
  branches whose target commit is not reachable from `HEAD`.
- Intentional differences: packed branch deletion and force deletion with `-D`
  are not implemented yet.
- Repository mutation: branch create/delete writes or removes refs.
- Risk: low for explicit local refs; create uses lock/rename.

### `rit tag`

- Baseline command checked: `git tag -h`
- Supported options: list tags, `-l`/`--list`, create lightweight tag at `HEAD`, `-d`/`--delete`.
- Unsupported options: annotated/signed tags, messages, object arguments, patterns, verification, sort/format controls.
- Git-compatible behavior: lightweight tags are refs under `refs/tags`.
- Intentional differences: only `HEAD` can be tagged for now.
- Repository mutation: tag create/delete writes or removes refs.
- Risk: low for explicit lightweight refs; create uses lock/rename.

### `rit restore`

- Baseline command checked: `git restore -h`
- Supported options: default worktree restore from index, `--staged`/`-S`
  restore index from `HEAD`, with ordinary literal file, directory, `.`, simple
  `*`, `?`, and bracket-class wildcard pathspecs plus positive `:(literal)`,
  `:(glob)`, `:(top)`, `:/`, and `:(icase)` pathspec magic, plus
  `--pathspec-from-file`, `--pathspec-from-file=-`, and `--pathspec-file-nul`.
- Unsupported options: source revisions, patch mode, merge conflict modes,
  sparse controls, full Git pathspec-file edge cases.
- Git-compatible behavior: explicit tracked file restore for regular files,
  including executable worktree permissions for `100755` index entries on Unix.
- Git-compatible behavior: unmatched pathspec-file entries report
  `pathspec ... did not match any file(s) known to git`.
- Git-compatible behavior: symlink index entries are restored as symlinks on
  Unix and as link-target text files on platforms without Unix symlink support.
- Git-compatible behavior: when `core.symlinks=false`, restore and checkout
  materialize `120000` entries as plain `100644` files containing the link
  target text, and status treats that plain file as clean.
- Intentional differences: conflict handling is not implemented.
- Repository mutation: worktree restore writes files; staged restore writes `.git/index`.
- Risk: moderate; worktree writes use temp file then replace destination.

### `rit reset`

- Baseline command checked: `git reset -h`
- Supported options: ordinary literal file, directory, `.`, simple `*`, `?`,
  and bracket-class wildcard pathspecs plus positive `:(literal)`, `:(glob)`,
  `:(top)`, `:/`, and `:(icase)` pathspec magic, equivalent to unstaging matching paths
  from `HEAD`, plus `--pathspec-from-file`, `--pathspec-from-file=-`, and
  `--pathspec-file-nul`, and `--plan`.
- Unsupported options: commit-moving resets, soft/mixed/hard/merge/keep modes,
  patch mode, full Git pathspec-file edge cases.
- Git-compatible behavior: unstages explicit paths and reports remaining unstaged modifications.
- Git-compatible behavior: pathspec-only resets are successful no-ops when no
  index or `HEAD` path matches, including pathspecs read from a file.
- Git-compatible behavior: an empty pathspec file resets all index entries
  from `HEAD`, matching Git's pathspec-file reset behavior.
- Git-compatible behavior: clean tracked paths refresh cached index stat
  metadata during `status --porcelain=v1`.
- rit-specific behavior: `--plan` prints which index entries would be restored
  from `HEAD` or removed because they do not exist in `HEAD` without writing
  `.git/index`.
- Intentional differences: full index extension parsing remains limited, but
  raw extension bytes are preserved when status only refreshes stat fields.
- Repository mutation: writes `.git/index`.
- Risk: low for explicit paths; index writes use lock/rename.

### `rit checkout`

- Baseline command checked: `git checkout -h`
- Supported options: checkout existing local branch, checkout a commit with
  detached `HEAD`, `-b <branch>` create and checkout.
- Unsupported options: path checkout, explicit `--detach`, force, orphan,
  tracking, merge/conflict modes, submodules.
- Git-compatible behavior: updates symbolic `HEAD`, writes index from target
  commit tree, materializes tracked worktree files, and applies executable
  permissions for `100755` entries on Unix. Symlink entries are materialized
  through the same restore path.
- Git-compatible behavior: detached checkout writes the target commit ID
  directly to `.git/HEAD`.
- Intentional differences: checkout requires a clean index and working tree
  instead of attempting merges; detached checkout emits a short message instead
  of Git's full advisory text.
- Repository mutation: writes `HEAD`, `.git/index`, and tracked worktree files.
- Risk: moderate; file writes use temp files and branch refs use lock/rename.

### `rit switch`

- Baseline command checked: `git switch -h`
- Supported options: switch existing local branch, `-c`/`--create <branch>`.
- Unsupported options: force create/reset, detach, discard changes, guess, tracking, merge/conflict modes, submodules.
- Git-compatible behavior: same local branch switching machinery as `checkout`.
- Intentional differences: switch requires a clean index and working tree.
- Repository mutation: writes `HEAD`, `.git/index`, and tracked worktree files.
- Risk: moderate; file writes use temp files and branch refs use lock/rename.

### `rit clone`

- Baseline command checked: `git clone -h`
- Supported options: `--local`/`-l`, `--no-checkout`/`-n`, and `--quiet`/`-q`.
- Unsupported options: checkout, bare/mirror clones, remote protocols,
  hardlink/shared/reference modes, branch selection, shallow/partial clone,
  submodules, sparse checkout, templates, and config overrides.
- Git-compatible behavior: local no-checkout clone copies the source object
  store, local heads/tags, optional `packed-refs`, writes a symbolic `HEAD`,
  and records `remote.origin` plus current branch merge config.
- Intentional differences: checkout is rejected instead of silently producing a
  partial worktree; remote clone protocols are still pending.
- Repository mutation: creates a new repository directory and copies local
  object/ref files without invoking external `git`.
- Risk: moderate; object/ref transfer is copy-based and does not mutate the
  source repository.

### `rit fetch`

- Baseline command checked: `git fetch -h`
- Supported options: `--quiet`/`-q` with one local repository path or
  `http://`/`https://` smart HTTP repository and either no refspec or one simple
  `<src>:<dst>` refspec.
- Unsupported options: named remotes, multiple refspecs, append/atomic/force
  semantics, SSH sessions, tags, prune, shallow/partial fetch, submodules,
  protocol options, stdin, and maintenance hooks.
- Git-compatible behavior: `fetch <local-repository>` copies source objects
  into the current repository and overwrites `.git/FETCH_HEAD` with the source
  `HEAD` commit. Local refs and remote-tracking refs are not updated, matching
  Git's no-refspec local fetch shape.
- Git-compatible behavior: `fetch <local-repository> <src>:<dst>` resolves the
  source ref, copies objects, writes `FETCH_HEAD`, and updates the destination
  full ref.
- Implemented smart HTTP behavior: `fetch http://... [<src>:<dst>]` discovers
  advertised refs, performs one upload-pack negotiation round, ingests the
  received pack, writes `FETCH_HEAD`, and updates the destination full ref when
  a refspec is supplied.
- Intentional differences: default progress/status text is simplified; quiet
  mode is used for compatibility coverage. Plain HTTP fetch supports only one
  negotiation round and does not yet implement thin-pack fixups.
- Repository mutation: writes object files and `.git/FETCH_HEAD`.
- Risk: moderate; fetch mutates only the destination repository.

### `rit push`

- Baseline command checked: `git push -h`
- Supported options: `--quiet`/`-q` with one `http://`, `https://`, `ssh://`,
  or scp-like SSH repository and one simple `<src>:<dst>` refspec.
- Unsupported options: named remotes, multiple refspecs, delete/mirror/all
  /tags, dry-run, force/lease semantics, upstream config, hooks,
  signed/atomic pushes, push options, and submodule behavior.
- Implemented smart HTTP behavior: discovers receive-pack refs, resolves the
  local source revision, walks reachable commit/tree/blob objects, sends a
  whole-object pack through receive-pack, and validates `report-status` for the
  destination ref.
- Intentional differences: default progress/status text is simplified; the
  object set is conservative and may send more objects than Git because
  thin-pack/delta generation and remote-history minimization are not implemented
  yet.
- Repository mutation: no local mutation; remote mutation is requested through
  receive-pack.
- Risk: moderate; the first push path is protocol-limited and avoids external
  `git`.

### `rit merge`

- Baseline command checked: `git merge -h`
- Supported options: default fast-forward shape and explicit `--ff-only` with
  one target branch or revision, conflicted non-fast-forward index-stage
  starts, `--abort`, `--quit`, `--continue`, plus rit-specific `--plan` and
  `merge explain <target>`.
- Unsupported options: remaining advanced mode/symlink conflict edge cases,
  remaining full conflict message parity, strategies, stat output, hooks,
  squash, autostash, signing, and verification options.
- Git-compatible behavior: fast-forward final `HEAD`, index, and worktree state
  match Git for simple clean repositories.
- rit-specific behavior: `--plan` prints whether the merge would be already
  up-to-date, fast-forward, or non-fast-forward. Fast-forward plans include
  paths that would be updated or removed; non-fast-forward plans include
  `HEAD`, target, merge-base, head-side changes, target-side changes, conflict
  candidates, candidate stage entries, and the merge-commit requirement without
  changing `HEAD`, refs, index, or worktree.
- rit-specific behavior: `merge explain` prints the fast-forward or
  non-fast-forward reason without changing repository state.
- rit-specific behavior: conflicted non-fast-forward merges leave stage 1/2/3
  entries in the index, merge state files, and simple worktree conflict markers
  for regular text content conflicts. Delete/modify conflicts leave the
  modified side in the working tree and print a `modify/delete` conflict
  message. Binary content conflicts leave the `HEAD` version in the working
  tree and print a binary merge warning. Add/add conflicts print an `add/add`
  conflict message.
- rit-specific behavior: `--abort` restores the `ORIG_HEAD` tree and removes
  merge state files, but does not yet handle autostash.
- Git-compatible behavior: `--quit` removes merge state files without changing
  unmerged index stages or working tree conflict contents, and succeeds with no
  output when no merge is active.
- rit-specific behavior: `--continue` creates a merge commit from the current
  resolved index and existing merge message without launching an editor.
- rit-specific behavior: clean non-fast-forward merges create a merge commit
  immediately when the simple tree merge has no conflict candidates.
- Intentional differences: output is simplified and no editor is launched for
  generated merge messages.
- Repository mutation: yes, updates `HEAD` or the current branch ref for
  fast-forward and continued merges, or writes unmerged index entries and merge
  state files for conflicted non-fast-forward merges.
- Risk: moderate; requires a clean worktree before writing.

### `rit op` and `rit undo`

- Baseline command checked: rit-specific command, no Git equivalent.
- Supported options: `rit op log`, `rit op log --json`,
  `rit op restore <id>`, and `rit undo`.
- Supported metadata: before/after HEAD snapshots, current branch snapshots,
  index checksums, changed paths, and known created object IDs.
- Recorded commands: commit, checkout, switch, fast-forward and conflicted
  merge, merge abort, add, restore, pathspec reset, branch create/delete, and
  tag create/delete, fetch, and smart-remote push success paths.
- Malformed operation journal lines are skipped with warnings and do not block
  reading later valid records.
- Unsupported options: filtering, complete object creation inventories, and
  command-aware undo policies.
- Git-compatible behavior: metadata is stored under `.git/rit/` and does not
  replace Git refs, objects, index, or working tree state.
- Intentional differences: this is a rit differentiator, not a Git-compatible
  porcelain command.
- Repository mutation: `op log` is read-only; `op restore` and `undo` restore
  a previous `HEAD` snapshot and check out its tree.
- Risk: moderate; current restore is HEAD/worktree-oriented and does not yet
  reconstruct index-only operations.

### Transport model

- Baseline commands checked: `git clone -h`, `git fetch -h`, `git push -h`
- Supported protocol classification: local filesystem paths, `http://`,
  `https://`, `ssh://`, and scp-like `user@host:path` locations.
- Supported HTTP model: smart HTTP reference-discovery request metadata for
  `git-upload-pack` and `git-receive-pack`, plus pkt-line advertised-ref
  response parsing. A small blocking HTTP/HTTPS client can perform GET
  discovery and POST `git-upload-pack` / `git-receive-pack` requests using
  plain TCP for `http://` and platform-verified TLS for `https://`; it validates
  smart status codes, content types, discovery prefixes, and decodes chunked
  responses.
- Supported negotiation model: smart HTTP `git-upload-pack` request bodies with
  at least one `want`, optional first-want capabilities, optional `have` lines,
  and a terminal `done`; upload-pack ACK/NAK/ERR, raw pack, and side-band
  response parsing; single-round smart HTTP negotiation for a caller-selected
  advertised ref that returns extracted pack bytes; receive-pack
  command/request bodies and `report-status` parsing.
- Supported SSH model: parse `ssh://user@host/path` and `user@host:path`
  locations, build quoted `git-upload-pack` / `git-receive-pack` remote
  commands, run one upload-pack or receive-pack request through a
  process-backed `ssh` session executor, and wire one-refspec SSH fetch/push
  through the CLI.
- Unsupported behavior: SSH auth option parity, broad SSH config support,
  multi-round negotiation, thin-pack fixups, push object minimization, and
  advanced push options are not implemented yet.
- Repository mutation: HTTP/HTTPS fetch ingests received packs and writes
  `FETCH_HEAD`; other transport APIs remain request/response models.
- Risk: moderate for fetch ingestion, low for request/response-only transport
  models.

### `rit rev-parse` revision support

- Baseline command checked: `git rev-parse -h`
- Added support: full object IDs, unambiguous abbreviated object IDs, `HEAD`,
  `FETCH_HEAD`, local branch names, lightweight tag names.
- Unsupported revision syntax: ancestry operators, ranges, path suffixes, reflog selectors.
- Repository mutation: no.

### `rit show`

- Baseline command checked: `git show -h`
- Supported options: default object display and `--no-patch`/`-s` for commits,
  optional revision, and ordinary literal plus simple `*`/`?` wildcard path
  filters plus positive `:(literal)`, `:(glob)`, `:(top)`, and `:/` pathspec
  magic plus `:(icase)` for no-patch commit display.
- Unsupported options: commit diffs, revision ranges, decorations, formatting
  controls, attr pathspec magic and bracket globs.
- Git-compatible behavior: commit no-patch layout, tree pretty printing, blob contents.
- Intentional differences: commit diffs are not emitted yet.
- Repository mutation: no.

### `rit ls-files`

- Baseline command checked: `git ls-files -h`
- Supported options: default cached file listing, `--stage`/`-s`, and ordinary
  literal file or directory pathspec filters plus simple `*`/`?` wildcard
  pathspec filters plus positive `:(literal)`, `:(glob)`, `:(top)`, and `:/`
  pathspec magic plus `:(icase)` and exclude pathspec magic.
- Unsupported options: deleted/modified/others/ignored filters, pathspec
  attr magic, bracket globs, EOL/debug/format output,
  sparse/submodule modes.
- Git-compatible behavior: lists index paths and stage records as `<mode> <object> 0<TAB><path>`.
- Repository mutation: no.

## Object Database

### Loose objects

- Supports reading and writing loose `blob`, `tree`, `commit`, and `tag` objects.
- Loose writes use zlib compression and temp-file/rename placement.

### Packed objects

- Baseline documents checked through `git help -a` developer-facing `format-pack` listing and compatibility tests against `git gc`.
- Supported index format: pack index v2.
- Supported pack object types: whole commit, tree, blob, tag; OFS_DELTA;
  REF_DELTA.
- Supported pack writing: whole-object pack v2 generation from existing object
  IDs, using Git object type codes, variable-length object sizes, zlib payloads,
  and a trailing SHA-1 pack checksum.
- Git-compatible behavior: after `git gc --aggressive --prune=now`,
  `rit cat-file`, `rit ls-tree`, `rit log`, and `rit show --no-patch` can
  read packed whole objects. `rit cat-file` compatibility coverage also reads
  a delta-compressed packed blob.
- Repository mutation: no.

### Git index

- Supported index format: v2/v3 entries for regular files.
- Supported stat behavior: status refreshes clean tracked file mtime/size
  metadata and preserves raw optional extension bytes such as `TREE`.
- Unsupported index behavior: conflict stages.

### Git attributes

- Baseline command checked: `git check-attr -h`
- Supported parser surface: repository-level `.gitattributes` style lines with
  ordinary path patterns, `[attr]name` macro definitions, and `name`, `-name`,
  `name=value`, and `!name` assignment tokens.
- Supported path application: root worktree `.gitattributes` rules can be
  applied to repository-relative paths for `:(attr:...)` pathspec matching.
  Supported requirements are set, unset, exact value, and unspecified states.
- Unsupported behavior: nested attributes files, full Git wildcard syntax in
  attributes patterns, quoted pattern parsing, macro expansion, and CLI
  `check-attr` output.
- Repository mutation: no.

## Code Organization

### Transport protocol modules

- 2026-05-11 hygiene pass: `crates/rit-core/src/transport.rs` had grown past
  1900 lines during M7 pack ingestion work.
- Split focused upload-pack request/response parsing into
  `crates/rit-core/src/transport/upload_pack.rs`.
- Split focused receive-pack request/status parsing into
  `crates/rit-core/src/transport/receive_pack.rs`.
- Public API compatibility: `transport` still re-exports the same
  `UploadPack*` and `ReceivePack*` types.
- Behavior change: none intended; this is a readability/module-boundary change
  before more remote fetch and push workflow work.

### CLI help module

- 2026-05-11 hygiene pass: moved long static help text and command-help routing
  from `crates/rit-cli/src/main.rs` into `crates/rit-cli/src/help.rs`.
- Behavior change: none intended; this keeps the CLI entrypoint focused on
  argument dispatch and command execution while preserving the existing help
  output tested by the CLI suite.

### CLI remote module

- 2026-05-12 hygiene pass: moved `clone`, `fetch`, and `push` command handling
  from `crates/rit-cli/src/main.rs` into `crates/rit-cli/src/remote.rs`.
- Behavior change: none intended; this keeps remote command parsing and
  dispatch together while reducing the CLI entrypoint after the plain HTTP push
  workflow landed.

### Auth explain

- Baseline command checked: `git credential -h` on Git 2.52.0.windows.1.
- Supported command: `rit auth explain <url>`.
- Supported behavior: classifies local, HTTP, HTTPS, SSH URL, and scp-like SSH
  locations; prints the credential request protocol, host, path, and username
  when applicable; reports which default token environment variables are set by
  name only.
- Secret handling: environment token values are never stored in the explanation
  and are never printed.
- Unsupported behavior: resolving full Git credential-helper precedence, SSH
  config expansion, interactive prompts, keychain lookups, OAuth/device flows,
  and host-specific auth provider selection.
- Repository mutation: no.
