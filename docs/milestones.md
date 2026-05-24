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

- Date: 2026-05-22
- Reference Git: `git version 2.54.0.windows.1`
- Required recurring checks:
  - `git --version`
  - `git help -a`
  - `git <command> -h` for each command being implemented
  - `cargo fmt --all`
  - `cargo clippy --workspace --all-targets -- -D warnings`
  - `cargo test --workspace`

## Milestone Verification

Verified on 2026-05-13 before continuing implementation:

- Production crates do not execute `git`; `Command::new` usage is limited to
  test infrastructure, hook execution, credential helper subprocesses, and SSH
  process transport. No production command shells out to `git`.
- M1 reusable local write fixture builders are present in `rit-testkit`; the
  older verification note saying they were missing is stale and corrected here.
- M2 linked worktree/common-dir support is implemented through `.git` gitdir
  files and `commondir`; shared objects, refs, config, and packed refs use the
  common directory.
- M3/M4 pathspec-file support is implemented for `add`, `restore`, and `reset`,
  including stdin and NUL-separated input; older compatibility prose saying it
  was unsupported is stale.
- M7 SSH fetch/push workflow wiring exists for one refspec via process-backed
  SSH service sessions. Remaining gaps are advanced SSH option/auth parity,
  multi-round negotiation, thin-pack fixups, and broader push/fetch options.
- M8 had an uncommitted fast-forward-only merge core in `write.rs`; this pass
  completed CLI wiring, operation journaling, tests, and notes for that first
  merge slice.
- 2026-05-15 verification before continuing from M0 found M0-M2 still
  accurately checked. The earliest real implementation gap remains M3 rename
  detection beyond cached diff. This pass added the first worktree rename/copy
  slice for default `diff` when Git's index contains intent-to-add entries.
- 2026-05-20 verification refreshed the local Git baseline to
  `git version 2.54.0.windows.1`, re-ran `git help -a`, confirmed production
  `git` execution is still limited to allowed helper/transport surfaces rather
  than Git-wrapper command implementations, and found the earliest tracked
  implementation gaps remain the existing `[~]` items rather than a falsely
  completed M0-M2 item.
- 2026-05-20 GitHub Actions analysis found every run from #1 through #43 had
  the same shape: `test (windows-latest)` passed, Ubuntu/macOS test jobs failed
  in `cargo test --workspace`, and all release builds passed. CI now keeps the
  full compatibility suite on the checked Windows Git baseline while retaining
  Ubuntu/macOS release builds for portability until Unix compatibility output
  is normalized.
- 2026-05-20 CLI readability pass split stash and workspace command handling
  out of `rit-cli/src/main.rs`. The entrypoint dropped from roughly 6400 lines
  to roughly 4360 lines; further command-module splits remain a standing
  maintainability task.
- 2026-05-20 CI trigger pass opened the workflow `push` trigger to all
  branches so milestone branches run GitHub Actions before a pull request is
  opened. The full compatibility oracle remains pinned to `windows-latest`
  until Unix/macOS baselines are normalized.
- 2026-05-22 milestone verification found a few stale documentation-only
  mismatches rather than false implementation claims in core code. The M16
  operation-journal notes still said command-aware undo was unsupported even
  though `rit undo --preserve-changes` and its tests already existed, the
  README repository layout still mentioned a separate `rit-indexdb` crate that
  is not part of the current workspace, and `docs/compatibility.md` understated
  the checked merge surface as fast-forward-only. This pass corrected those
  notes and made the preserve-changes undo result output explicitly say that
  staged and working-tree content was kept.
- 2026-05-22 read-only pathspec verification also confirmed `ls-files`, first-
  parent `log`, and `show --no-patch` already supported simple bracket-class
  wildcard pathspecs through the shared matcher. The code was ahead of the
  milestone text, so compatibility coverage and documentation were updated
  instead of changing the implementation.
- 2026-05-22 follow-up verification also confirmed first-parent `log` and
  `show --no-patch` already supported root `.gitattributes` attr pathspec
  requirements through the same shared matcher. This pass added explicit
  compatibility coverage and corrected the stale implementation notes that
  still marked that surface unsupported.
- 2026-05-22 diff verification also confirmed that plain untracked worktree
  renames and copies stay outside default `git diff` rename/copy detection.
  Git still shows a delete for the rename case and ignores the ordinary copy
  case, and `rit` already matched that behavior. The older milestone wording
  overstated that slice as a missing implementation gap.
- 2026-05-22 write-command attr-pathspec verification confirmed that the
  already-checked M6 attr support also holds for `restore` and `reset`, not
  just `add`. This pass added explicit Git-vs-rit compatibility coverage and
  corrected the implementation notes that had underspecified those supported
  options.
- 2026-05-22 pathspec-file toggle verification also promoted an older
  option-order probe into explicit Git-vs-rit coverage: for `add`, `restore`,
  and `reset`, `--no-pathspec-from-file` does not clear an already selected
  `--pathspec-from-file=<file>` and instead leaves that file selection active.

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
- [x] `rit show` object display basics.
  - [x] Checked default patch output for root and single-parent commits.
  - [x] Checked pathspec filtering for default patch output, including special
    `:(glob)` forms and `:(attr:...)`.
  - [x] `--no-patch` commit display with path filtering.
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
  - [x] `-z` NUL-terminated output for `--name-only`, `--name-status`,
    and `--numstat`, including rename/copy field layout.
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
  - [x] Simple bracket-class wildcard pathspec filters for `ls-files`.
  - [x] Ordinary literal path lookup for `ls-tree`.
  - [x] Ordinary literal path filters for first-parent `log`.
  - [x] Ordinary literal path filters for `show --no-patch`.
  - [x] Simple `*` and `?` wildcard pathspec filters for first-parent `log`
    and `show --no-patch`.
  - [x] Simple bracket-class wildcard pathspec filters for first-parent `log`
    and `show --no-patch`.
  - [x] Exclude pathspec magic is verified against Git for `ls-files`,
    first-parent `log`, and `show --no-patch`.
  - [x] Special `:(glob)**/*.txt` and trailing `:(glob)nested/**` forms are
    verified against Git for `ls-files`, first-parent `log`, and
    `show --no-patch`.
  - [x] Component-local `:(glob)**base.txt` remains non-recursive like Git for
    `ls-files`, first-parent `log`, and `show --no-patch`.
  - [x] Relative pathspecs are resolved from the command invocation directory
    for `status`, `diff`, `ls-files`, `ls-tree`, `log`, and `show`, while top
    magic remains repository-root relative.
- [~] Rename detection.
  - [x] Exact staged rename detection for `diff --cached -M` summary and
    patch output.
  - [x] Staged similarity thresholds, copy detection, and non-exact rename
    scoring for `diff --cached -M/-C` summary and patch output.
  - [x] Staged `--find-copies-harder` copy detection from unchanged HEAD
    sources.
  - [x] Rename/copy candidate limit model and CLI parsing for `-l<n>`.
  - [x] Exact rename detection still runs when `-l<n>` is below the changed
    path count, matching Git's cheap exact-pass behavior.
  - [x] Rename/copy `-l<n>` counts source and destination candidate widths
    instead of the total changed-path count for one-source/one-destination
    similarity detection.
  - [x] `-l<n>` exhaustive rename-limit warnings are returned through stderr
    when similarity detection is skipped.
  - [x] `diff.renameLimit` config is honored as the rename/copy candidate
    limit when no `-l<n>` CLI override is provided.
  - [x] Invalid `diff.renameLimit` config values fail with Git-compatible
    fatal output and exit code.
  - [x] Worktree rename/copy detection for default `diff -M/-C` when added
    paths are represented by Git intent-to-add index entries.
  - [x] Default worktree diff already performs checked exact rename detection
    for Git intent-to-add paths, and `diff.renames=false` disables that
    default rename detection like Git.
  - [x] Worktree non-exact rename similarity thresholds for default `diff -M`
    when added paths are represented by Git intent-to-add index entries.
  - [x] Worktree `-l<n>` exhaustive rename-limit warnings for non-exact
    intent-to-add rename detection.
  - [x] Worktree `-l<n>` exhaustive rename-limit warnings for intent-to-add
    copy detection.
  - [x] Exact copy detection runs before `-l<n>` exhaustive limit warnings for
    cached and worktree intent-to-add copy detection.
  - [x] Worktree `--find-copies-harder` copy detection from unchanged index
    sources for Git intent-to-add entries.
  - [x] Percent-less `-M<n>` and `-C<n>` similarity thresholds use Git's
    fractional notation, so `-M5`/`-C5` mean 50% while `-M05`/`-C05` mean 5%.
  - [x] Similarity thresholds above 100% are accepted like Git and simply
    cannot match ordinary rename/copy scores.
  - [x] `--no-renames` disables checked rename detection, while later `-M`
    re-enables it like Git.
  - [x] Checked `--find-copies-harder` copy detection, with or without `-C`,
    remains active like Git even when `--no-renames` appears before or after
    it.
  - [x] Plain checked `-C` copy detection follows Git's option order: a later
    `--no-renames` disables it, while a later `-C` re-enables it.
  - [x] The same `--no-renames` / later `-M` option-order behavior is checked
    for default worktree exact rename detection.
  - [x] Cached diff enables rename detection by default, honors
    `diff.renames=false`, and matches Git's checked `diff.renames=copies`
    behavior for the covered rename/copy slices.
  - [x] Checked explicit `--find-copies-harder`, with or without `-C`,
    still overrides `diff.renames=false` like Git for the covered cached and
    default worktree hard-copy slices.
  - [x] Plain checked `-C` likewise overrides `diff.renames=false` like Git
    for the covered cached and default worktree copy slices.
  - [x] The same checked `diff.renames=copies` behavior is verified for the
    covered default worktree rename, copy, and hard-copy slices.
  - [x] Explicit `--no-renames` also overrides checked `diff.renames=copies`
    like Git for the covered cached and default worktree copy and hard-copy
    slices.
  - [x] Invalid `diff.renames` values fail with Git-compatible fatal output
    and exit code.
  - [x] The same invalid `diff.renames` fatal behavior is checked for default
    worktree diff output.
  - [x] `diff.renameLimit=0` is treated as unlimited like Git for both cached
    and default worktree rename/copy detection.
  - [ ] Full rename limits and advanced Git diffcore parity.
- [x] Binary diff accounting for summary modes.

Completion criteria:
- Common read-only commands have Git comparison tests for simple repositories.

## M4: Local Write Commands

- [x] `rit init`
  - [x] `--initial-branch=<branch>` equals-form parsing writes the same
    initial `HEAD` target as Git for a checked local init.
  - [x] `--no-bare`, `--no-initial-branch`, and `--no-quiet` reverse earlier
    init flags for the checked local init cases.
  - [x] Default repository format options `--object-format=sha1`,
    `--ref-format=files`, `--no-object-format`, and `--no-ref-format` are
    accepted for checked local init cases without claiming SHA-256/reftable
    support.
  - [x] `--no-template` resets prior template selection for checked local init
    cases without claiming template copy support.
- [x] `rit add` explicit regular files.
- [x] `rit commit -m`
- [x] `rit branch` local list/create/delete basics.
  - [x] `branch --list` without patterns follows the checked default local
    branch list output.
  - [x] `branch --list <pattern>...` filters local branch names with checked
    Git-style wildcard behavior for simple branch globs.
  - [x] `branch -l [<pattern>...]` follows the same checked list behavior as
    Git's short list option.
  - [x] `branch -d <branch-name>...` deletes multiple local branches in order
    with checked Git-compatible output.
  - [x] `branch -D <branch-name>...` force-deletes checked local unmerged
    branches without requiring reachability from `HEAD`.
- [x] `rit tag` lightweight tag list/create/delete basics.
  - [x] `tag -l`/`tag --list` without patterns follows the checked default tag
    list output.
  - [x] `tag -l`/`tag --list <pattern>...` filters lightweight tag names
    with checked Git-style wildcard behavior for simple tag globs.
  - [x] `tag -d <tag-name>...` deletes multiple lightweight tags in order
    with checked Git-compatible output.
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
    - [x] POSIX bracket character classes such as `[[:digit:]]` in shared
      pathspec glob matching.
    - [x] POSIX bracket write-command coverage is verified against Git for
      `add`, `restore`, and `reset`.
    - [x] `:(icase)` write-command coverage is verified against Git for `add`,
      `restore`, and `reset`.
    - [x] Exclude write-command coverage is verified against Git for `add`,
      `restore`, and `reset`.
    - [x] `:(glob)` double-star matching crosses slashes only in Git's special
      `**/` and trailing `**` forms; other `**` pairs stay within one path
      component.
    - [x] Git-vs-rit write-command coverage explicitly includes the special
      `:(glob)**/*.txt` and trailing `:(glob)nested/**` forms for `add`,
      `restore`, and `reset`.
    - [x] Git-vs-rit write-command coverage also includes component-local
      non-recursive `:(glob)**base.txt` for `add`, `restore`, and `reset`.
    - [x] `--pathspec-from-file` and `--pathspec-file-nul` for `add`,
      `restore`, and `reset`.
    - [x] `--pathspec-from-file=-` stdin pathspecs for `add`, `restore`, and
      `reset`.
    - [x] `--pathspec-from-file=- --pathspec-file-nul` stdin NUL pathspecs
      for `add`, `restore`, and `reset`.
    - [x] CRLF line endings in text pathspec files are parsed like Git for
      `add`, `restore`, and `reset`, including quoted entries.
    - [x] Deprecated `git reset --stdin` and `git reset --stdin -z` aliases
      reuse the shared stdin pathspec parser with Git-compatible warning and
      status behavior.
    - [x] `--no-pathspec-file-nul` turns a preceding NUL pathspec-file mode
      back into text pathspec parsing for `add`, `restore`, and `reset`.
    - [x] C-style quoted pathspec-file entries for common escapes.
    - [x] Octal C-style quoted pathspec-file escapes decode as UTF-8 bytes
      for `add`, `restore`, and `reset`.
    - [x] Short or incomplete octal C-style quoted pathspec-file escapes are
      rejected with Git-compatible fatal output for `add`, `restore`, and
      `reset`.
    - [x] Alarm C-style pathspec-file escape `\a` and Git-compatible
      pathspec-not-found output/no-op behavior for `add`, `restore`, and
      `reset`.
    - [x] Empty pathspec files match Git behavior for `add`, `restore`, and
      `reset`.
    - [x] Empty `--pathspec-from-file=` values match Git behavior for `add`,
      `restore`, and `reset`.
    - [x] Empty NUL-mode pathspec files match Git behavior for `add`,
      `restore`, and `reset`.
    - [x] Repeated `--pathspec-from-file` uses the last file or stdin source,
      including NUL-delimited file and stdin sources, like Git for `add`,
      `restore`, and `reset`.
    - [x] `--no-pathspec-from-file` is accepted as a Git-compatible no-op
      when no pathspec file selection is active for `add`, `restore`, and
      `reset`.
    - [x] `--no-pathspec-from-file` keeps an already selected pathspec file
      or stdin pathspec selection, including NUL-delimited stdin, active
      like Git for `add`, `restore`, and `reset`.
    - [x] `--no-pathspec-from-file` also leaves a later
      `--pathspec-from-file=<file>` or `--pathspec-from-file=-` stdin
      selection, including NUL-delimited stdin, active like Git for `add`,
      `restore`, and `reset`.
    - [x] `--pathspec-from-file` mixed with pathspec arguments is rejected
      with Git-compatible fatal output for `add`, `restore`, and `reset`.
    - [x] Missing `--pathspec-from-file` files are rejected with
      Git-compatible fatal output for `add`, `restore`, and `reset`.
    - [x] Non-UTF-8 text pathspec-file bytes follow Git-compatible lossy
      pathspec matching and errors for `add`, `restore`, and `reset`.
    - [x] NUL bytes inside text pathspec-file lines truncate the line like Git
      for `add`, `restore`, and `reset`.
    - [x] Lone carriage-return bytes in text pathspec-file lines remain part
      of the pathspec like Git for `add`, `restore`, and `reset`.
    - [x] `--pathspec-file-nul` without `--pathspec-from-file` is rejected
      with Git-compatible fatal output for `add`, `restore`, and `reset`.
    - [x] `--pathspec-from-file` without a value is rejected with
      Git-compatible option error output for `add`, `restore`, and `reset`.
    - [x] Empty line pathspec-file entries are rejected with Git-compatible
      fatal output for `add`, `restore`, and `reset`.
    - [x] Quoted empty pathspec-file entries are rejected with Git-compatible
      fatal output for `add`, `restore`, and `reset`.
    - [x] Quoted pathspec-file entries ignore trailing bytes after the closing
      quote like Git for `add`, `restore`, and `reset`.
    - [x] Empty NUL-delimited pathspec-file entries are rejected with
      Git-compatible fatal output for `add`, `restore`, and `reset`, while a
      trailing NUL terminator remains allowed.
    - [x] `--pathspec-file-nul` remains active like Git even when it appears
      before `--pathspec-from-file`, including `--pathspec-from-file=-`
      stdin input, for `add`, `restore`, and `reset`.
    - [x] `--no-pathspec-file-nul` likewise remains active like Git when it
      appears before `--pathspec-from-file`, including
      `--pathspec-from-file=-` stdin input, after an earlier
      `--pathspec-file-nul` for `add`, `restore`, and `reset`.
    - [x] Badly quoted pathspec-file entries are rejected with Git-compatible
      fatal output for `add`, `restore`, and `reset`.
    - [x] Relative pathspecs are resolved from the command invocation
      directory for `add`, `restore`, and `reset`, while top magic remains
      repository-root relative.
    - [x] Pathspec-file entries from a subdirectory are likewise resolved
      relative to that invocation directory unless they use top magic for
      `add`, `restore`, and `reset`.
    - [x] Stdin-delivered pathspec entries from a subdirectory are likewise
      resolved relative to that invocation directory unless they use top
      magic for `add`, `restore`, and `reset`.
    - [x] NUL-delimited stdin pathspec entries from a subdirectory are
      likewise resolved relative to that invocation directory unless they use
      top magic for `add`, `restore`, and `reset`.
    - [x] NUL-delimited pathspec-file entries from a subdirectory are
      likewise resolved relative to that invocation directory unless they use
      top magic for `add`, `restore`, and `reset`.
    - [x] Incompatible `:(literal,glob)` pathspec magic is rejected with
      Git-compatible fatal output before `add`, `restore`, or `reset` mutate
      repository state.
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
  - [x] Incompatible `:(literal,glob)` magic combinations are rejected.
  - [x] Case-insensitive `:(icase)` pathspec magic.
  - [x] Exclude `:!`, `:^`, and `:(exclude)` pathspec magic.
  - [x] Attr pathspec magic for root `.gitattributes` set/unset/value/
    unspecified requirements.
- [~] Case-sensitivity behavior by platform/config.
  - [x] `git add` honors `core.ignorecase=true` for mismatched-case
    pathspecs that Git accepts as no-ops.
  - [x] `git add` rejects mismatched-case pathspecs like Git when
    `core.ignorecase=false`.
  - [x] `git reset <pathspec>` accepts `core.ignorecase=true`
    mismatched-case tracked pathspecs as Git-compatible no-ops.
  - [x] On the current Windows Git baseline, `git reset <pathspec>` also keeps
    the same mismatched-case tracked-path no-op behavior when
    `core.ignorecase=false`.
  - [x] `git restore <pathspec>` keeps Git-compatible rejection behavior for
    mismatched-case tracked pathspecs even when `core.ignorecase=true`.
  - [x] `git restore <pathspec>` also keeps the same rejection behavior when
    `core.ignorecase=false`.
  - [x] On the current Windows Git baseline, plain read-only tracked path
    lookup remains case-sensitive regardless of `core.ignorecase` for
    `status`, `diff`, `ls-files`, `ls-tree`, `log`, and `show`.
  - [ ] Broader platform/config parity for case-sensitive path lookup.

Completion criteria:
- Status/add/diff path selection matches Git for ordinary pathspec and ignore
  usage.

## M6.5: SQLite Auxiliary IndexDB

- [x] Add `indexdb` Cargo feature.
- [x] Add `rit-indexdb` crate or internal module behind the `indexdb` feature.
- [x] Define indexdb storage location under `.git/rit/`.
- [x] Define shared repository DB and worktree-specific cache layout.
- [x] Add schema versioning and migration model.
- [x] Add `cache_state` table.
- [x] Add `commits` table.
- [x] Add `commit_parents` table with parent order preservation.
- [x] Add `file_changes` table.
- [x] Add `refs_snapshot` table or snapshot hash model.
- [x] Store object IDs as hash-kind-aware binary values, not SHA-1-only strings.
- [x] Implement `rit indexdb` as the default ensure command.
- [x] Implement `rit indexdb status`.
- [x] Implement `rit indexdb build`.
- [x] Implement `rit indexdb update`.
- [x] Implement `rit indexdb repair`.
- [x] Implement `rit indexdb rebuild`.
- [x] Implement `rit indexdb drop`.
- [x] Implement `rit indexdb vacuum`.
- [x] Implement write-through updates after `rit` creates commits, refs, tags,
  or checkout state changes.
- [x] Implement lightweight reconciliation when external Git-compatible tools
  changed refs, index, or pack snapshots.
  - [x] Detect and refresh stale `HEAD`, local branch, and lightweight tag
    snapshots.
  - [x] Detect stale index checksum and mtime.
  - [x] Detect stale packfile list, size, and mtime.
- [x] Implement fallback to canonical Git object/index/refs data when indexdb
  is missing, stale, or corrupted.
- [x] Add indexed commit query API.
- [x] Add indexed file history API.
- [x] Add indexed refs snapshot API.
- [x] Add optional indexed path history command such as `rit file-history <path>`.
- [x] Add compatibility tests proving indexdb does not change Git-compatible
  command output.
- [x] Add corruption tests proving broken indexdb never corrupts the repository.
- [x] Add linked worktree tests proving worktree cache isolation.
- [x] Add benchmark tests for large commit history and file history queries.

Completion criteria:
- `rit indexdb` creates or updates `.git/rit/indexdb.sqlite` without changing
  Git repository semantics.
- Basic Git-compatible commands still work when indexdb is missing, stale,
  dropped, or corrupted.
- New commits made by `rit` are reflected in indexdb through write-through
  updates.
- Commits made by external `git` are detected and reconciled incrementally on
  the next relevant `rit` run.
- File history queries can use indexdb without walking every commit tree from
  scratch.
- All indexdb data is reproducible from `.git/objects`, refs, `.git/index`,
  and working tree state.

## M7: Transport Foundation

- [x] Local clone/fetch object transfer.
  - [x] `clone --local --no-checkout` copies objects and local refs without
    calling external `git` in production code.
  - [x] `clone --local --no-hardlinks --no-checkout` follows the same
    copy-based local clone path and matches checked no-checkout object/ref
    state.
  - [x] `clone --local --no-checkout -o <name>` / `--origin=<name>` records
    the checked remote name in clone config and branch merge config.
  - [x] `clone --local --no-checkout --tags` accepts the explicit default tag
    copying mode and matches checked no-checkout tag/config state.
  - [x] `clone --local --no-checkout --no-tags` skips checked loose tag refs
    and records `remote.<name>.tagOpt = --no-tags`.
  - [x] `clone --local --no-checkout -b <branch>` / `--branch=<branch>`
    checks out the selected local source branch in the cloned repository state.
  - [x] `fetch <local-repository>` copies objects into an existing repository
    and writes `FETCH_HEAD` without updating refs.
- [x] Protocol model for local, HTTP(S), and SSH location classification.
- [x] HTTP transport.
  - [x] Smart HTTP `info/refs?service=...` request model.
  - [x] Smart HTTP advertised refs response parser.
  - [x] HTTP client I/O.
    - [x] Blocking plain HTTP GET discovery and POST upload-pack requests.
    - [x] Chunked response decoding.
    - [x] Smart HTTP status, content-type, and advertisement prefix
      validation.
    - [x] HTTPS/TLS via platform certificate verification.
- [x] SSH transport.
  - [x] SSH/scp-like upload-pack and receive-pack command model.
  - [x] Process-based SSH upload-pack session I/O model.
  - [x] SSH fetch workflow wiring for a single advertised ref, including
    pack ingestion and `FETCH_HEAD` updates.
  - [x] SSH receive-pack session I/O and push workflow wiring for one
    source-to-destination refspec.
  - [x] `ssh://host:port/path` URL port parsing and `ssh -p` process
    argument wiring.
  - [x] `GIT_SSH_COMMAND` and `GIT_SSH` process selection for SSH transport.
  - [x] SSH auth option parity and broader advanced SSH transport
    configuration.
    - [x] Read `core.sshCommand` from `.git/config` for process-based SSH
      fetch and push, with Git-shaped precedence behind `GIT_SSH_COMMAND`.
    - [x] Add `ssh.variant` and `GIT_SSH_VARIANT` argument-shape parity for
      OpenSSH, plink, putty, tortoiseplink, and simple variants.
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
  - [x] Wire single-round SSH upload-pack ingestion into `rit fetch`.
  - [x] Send local reachable commit `have` lines from HEAD, branches, and tags
    during HTTP and SSH fetch negotiation.
  - [x] Accept packless already-have upload-pack responses and still update
    `FETCH_HEAD`/destination refs without ingesting an empty pack.
  - [x] Avoid requesting `thin-pack` until explicit thin-pack fixup storage is
    implemented.
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
- [~] `rit merge`
  - [x] Fast-forward-only merge core API.
  - [x] `rit merge [--ff-only] <target>` CLI for clean worktrees.
  - [x] Compatibility test for fast-forward final HEAD, status, and worktree
    contents.
  - [x] Non-fast-forward `--plan` reports `HEAD`, target, and merge-base
    without writing repository state.
  - [x] Non-fast-forward `--plan` reports head-side changes, target-side
    changes, and conflict candidates without writing repository state.
  - [x] Non-fast-forward `--plan` reports candidate base/head/target stage
    entries for conflicting paths without writing conflict index stages.
  - [x] Index parser/writer preserves Git index stage bits for future conflict
    entries.
  - [x] Automatic clean merge commits for non-fast-forward merges without
    conflicts.
  - [x] Clean merge commits run `pre-merge-commit` hooks and honor
    `--no-verify` for that hook.
  - [x] `merge -s ours`, `merge --strategy ours`, and compact `-sours`
    create a merge commit that keeps the current `HEAD` tree while recording
    the target as the second parent.
  - [x] `merge --no-ff` forces a merge commit for fast-forwardable targets,
    and `--ff`/`--no-ff` option order follows Git for the covered branch-target
    forms.
  - [x] `merge --no-commit` stops clean non-fast-forward merges before commit
    with Git-shaped merge state, honors `--commit`/`--no-commit` option order,
    and still fast-forwards when Git would.
  - [x] `merge -X ours/theirs` and `--strategy-option=ours/theirs` resolve
    supported same-path text conflicts by choosing the requested side while
    still creating a normal merge commit.
  - [x] `merge -m/--message` uses the selected merge message for clean merge
    commits and stopped merge state in covered branch-target forms.
  - [x] Covered successful fast-forward merges and automatic clean merge
    commits run `post-merge 0` like Git, relay hook output to stderr, ignore
    non-zero hook exits, and clean `--no-commit` merge stops do not run the
    hook.
  - [x] Covered resolved `merge --continue` runs commit hooks like Git:
    `pre-commit`, `prepare-commit-msg` with source `merge`, `commit-msg`, and
    `post-commit`; blocking `pre-commit` keeps merge state intact.
  - [x] Covered merge option parsing keeps `-n` as no-stat rather than
    no-verify for clean merge hooks, and rejects `merge --continue` with
    `--no-verify` or `-n` like Git.
  - [x] Covered merge state modes reject extra target revisions, recognized
    options, and mixed `--abort`/`--quit`/`--continue` mode flags with
    Git-shaped fatal messages before changing merge state.
  - [ ] Full conflict handling, merge hooks, and strategies.
- [~] `rit cherry-pick`
  - [x] Clean single-parent cherry-pick applies the picked commit onto `HEAD`,
    preserves the picked author/message, and creates a new one-parent commit.
  - [x] Clean `-n`/`--no-commit` cherry-pick applies the change to the
    index/worktree without advancing `HEAD` or writing sequencer state.
  - [x] Conflicting single-commit cherry-pick writes `CHERRY_PICK_HEAD`,
    `MERGE_MSG`, unmerged index stages, and conflict markers.
  - [x] `cherry-pick --abort` restores `ORIG_HEAD` for the first conflicted
    cherry-pick slice.
  - [x] `cherry-pick --continue` commits a resolved single-commit conflict
    using the picked commit's author/message and clears cherry-pick state.
  - [x] `cherry-pick --quit` clears cherry-pick state while preserving the
    conflicted index and worktree.
  - [x] Clean merge commit cherry-pick supports `-m`/`--mainline` parent
    selection.
  - [x] `cherry-pick --skip` restores `ORIG_HEAD` for the first conflicted
    cherry-pick slice and clears state.
  - [x] Clean multi-target cherry-pick applies commits in order and records
    the created commit ids.
  - [x] Clean `-x` cherry-pick appends the original commit id to the created
    commit message.
  - [x] Clean `--ff` cherry-pick fast-forwards when `HEAD` is the picked
    commit's parent.
  - [x] Clean `-s`/`--signoff` cherry-pick appends a committer signoff
    trailer.
  - [x] Clean multi-target `--no-commit` cherry-pick applies all picked
    changes to the index/worktree without advancing `HEAD`.
  - [x] Multi-target committing cherry-pick writes Git-shaped sequencer
    `head`, `abort-safety`, and `todo` metadata when a later target conflicts.
  - [x] `cherry-pick --continue` clears final multi-target sequencer state
    after committing the resolved stopped pick.
  - [x] `cherry-pick --continue` replays additional remaining clean sequencer
    todo entries after committing the resolved stopped pick.
  - [x] `cherry-pick --continue` stops with Git-compatible state when a later
    remaining todo commit conflicts after the resolved stopped pick is
    created.
  - [x] `cherry-pick --continue` prints conflict reports and `could not apply`
    output when a later remaining todo commit conflicts.
  - [x] `cherry-pick --skip` drops the stopped pick and replays additional
    remaining clean sequencer todo entries.
  - [x] `cherry-pick -X ours/theirs` and `--strategy-option=ours/theirs`
    resolve supported same-path text conflicts by choosing the requested side
    while still applying non-conflicting picked changes.
  - [x] Multi-target `cherry-pick -X ours/theirs` writes and rereads
    `.git/sequencer/opts`, so `--continue` and `--skip` reuse the selected
    side for remaining todo entries.
  - [x] Conflicting merge-commit cherry-pick with `-m`/`--mainline` can be
    resolved and continued, creating the same one-parent commit state as Git.
  - [x] Cherry-pick commit hooks now match the covered Git paths: clean picks
    run `prepare-commit-msg` and `post-commit` without `commit-msg`, while
    resolved `--continue` runs commit verification hooks with merge-source
    prepared messages.
  - [ ] Exact full later-conflict commit summary/hint parity, broader strategy
    options, and remaining editor/hook parity.
- [~] `rit rebase`
  - [x] `rit rebase <upstream>` reports Git-compatible up-to-date status when
    the upstream is already an ancestor of `HEAD`.
  - [x] `rit rebase <upstream>` fast-forwards the current branch or detached
    `HEAD` when `HEAD` is already an ancestor of the upstream.
  - [x] `rit rebase <upstream>` replays one clean linear commit onto the
    upstream and updates the current branch.
  - [x] `rit rebase <upstream>` replays multiple clean linear commits onto the
    upstream and updates the current branch.
  - [x] `rit rebase <upstream>` stops on a final replay conflict with
    Git-compatible conflict output, `rebase-merge` metadata, `REBASE_HEAD`,
    `MERGE_MSG`, detached `HEAD`, unmerged index stages, and worktree conflict
    markers, including cases where earlier commits replayed cleanly first.
  - [x] `rit rebase <upstream>` stops on a replay conflict with remaining todo
    entries and records Git-compatible `git-rebase-todo`, `done`, `msgnum`,
    and `end` state.
  - [x] `rit rebase --abort` restores the original branch, index, and
    worktree from Git-compatible rebase state and removes rebase conflict
    metadata.
  - [x] `rit rebase --show-current-patch` prints the stopped rebase commit
    header and patch from `REBASE_HEAD`.
  - [x] `rit rebase --continue` commits a resolved final stopped rebase commit
    with the original author/message, updates the original branch, and clears
    rebase state.
  - [x] `rit rebase --continue` commits a resolved stopped commit, replays
    remaining clean linear todo entries, updates the original branch, and
    clears rebase state.
  - [x] `rit rebase --skip` completes a single-step stopped rebase by dropping
    the current commit and updating the original branch to the current `HEAD`.
  - [x] `rit rebase --skip` drops a stopped commit, replays remaining clean
    linear todo entries, updates the original branch, and clears rebase state.
  - [x] `rit rebase --quit` removes Git-compatible `rebase-apply` and
    `rebase-merge` state while preserving HEAD, index, and worktree.
  - [x] `rit rebase --continue` stops with Git-compatible state/output when a
    later remaining todo commit conflicts after the resolved commit is created.
  - [x] `rit rebase --skip` stops with Git-compatible state/output when a
    later remaining todo commit conflicts after the skipped commit is dropped.
  - [x] `rit rebase [--no-verify|--verify] <upstream>` handles the
    `pre-rebase` hook like Git for covered start paths: up-to-date rebases do
    not run it, mutating rebases run it before changing repository state, and
    `--no-verify` skips it. A failing `pre-rebase` hook prints Git-compatible
    stderr and leaves rebase state untouched.
  - [x] `rit rebase <upstream>`, covered successful `rit rebase --continue`
    completions, and covered successful `rit rebase --skip` completions run
    `post-rewrite rebase` like Git: rewritten commit pairs are passed on stdin,
    hook output is relayed to stderr before the success message, and non-zero
    hook exits do not fail the rebase.
  - [ ] Todo editing, autostash, remaining hooks, apply/merge backends, and
    strategy options.
- [~] `rit stash`
  - [x] `rit stash list` reads the Git-compatible `refs/stash` reflog and
    prints entries in newest-first order.
  - [x] `rit stash clear` removes loose `refs/stash` state and its reflog.
  - [x] `rit stash clear` also removes packed `refs/stash` entries.
  - [x] `rit stash drop` removes a loose reflog entry, relinks remaining
    reflog entries, and updates loose `refs/stash`.
  - [x] `rit stash drop -q` and basic empty/out-of-range drop errors match
    Git.
  - [x] `rit stash show` supports default stat output plus `--stat`,
    `--compact-summary`, `--no-compact-summary`, `--shortstat`,
    `--name-only`, `--name-status`, and `--numstat` for loose stash entries.
  - [x] `rit stash show -p/--patch` renders Git-compatible patch output for
    loose stash entries.
  - [x] `rit stash store` stores an existing commit in loose `refs/stash` with
    a Git-compatible reflog message.
  - [x] `rit stash store -q` and the default store message match Git for loose
    stash state.
  - [x] Basic `stash push` saves tracked index and working-tree changes into a
    two-parent stash commit, updates loose `refs/stash`, and restores `HEAD`.
  - [x] Basic `stash push -- <pathspec>` limits tracked stash snapshots and
    cleanup to selected tracked paths while leaving unselected tracked
    worktree changes in place.
  - [x] Basic `stash push --pathspec-from-file=<file>` and
    `--pathspec-file-nul` reuse the shared pathspec file parser for tracked
    stash push path filtering.
  - [x] `stash push --no-pathspec-file-nul` turns a preceding NUL
    pathspec-file mode back into text pathspec parsing for tracked stash push
    path filtering.
  - [x] `stash push --no-pathspec-from-file` is accepted as a Git-compatible
    no-op when no pathspec file selection is active.
  - [x] Basic `stash push --keep-index` records tracked changes while restoring
    selected paths to the pre-stash index state so staged changes remain
    staged.
  - [x] Basic `stash push --staged` records selected staged changes while
    leaving unrelated unstaged worktree changes in place for non-overlapping
    paths.
  - [x] `stash push/save --staged` with same-path staged and unstaged changes
    stores the stash, reports cleanup failure, and leaves index/worktree state.
  - [x] Basic `stash push --include-untracked` records untracked files in the
    third stash parent and removes those files from the working tree.
  - [x] Basic `stash push --all` also records ignored files in the third stash
    parent and removes them from the working tree.
  - [x] Basic `stash apply -q` restores untracked files from the stash's third
    parent without dropping the stash.
  - [x] Basic `stash pop -q` restores untracked files through the shared clean
    apply path and then drops the selected loose stash entry.
  - [x] `stash apply/pop -q` refuse to overwrite existing untracked files from
    a stash third parent and leave the selected stash entry intact on failure.
  - [x] `stash show --include-untracked` summary formats include third-parent
    untracked additions for shortstat/name-only/name-status/numstat output.
  - [x] `stash show --include-untracked --patch` includes third-parent
    untracked additions in Git-compatible patch output.
  - [x] `stash show --only-untracked` renders only third-parent untracked
    additions for stat/shortstat/name-only/name-status/numstat/patch output.
  - [x] `stash.showIncludeUntracked=true` makes default `stash show` include
    third-parent untracked entries unless an explicit untracked show option is
    provided.
  - [x] `stash.showStat` and `stash.showPatch` config values control default
    `stash show` stat/patch output, including Git's combined stat-plus-patch
    mode, while explicit show format options take precedence.
  - [x] `stash show --patch-with-stat`, `--stat --patch`, and
    `--patch --stat` render Git-compatible combined stat-plus-patch output.
  - [x] `stash show --no-include-untracked` and `stash show --no-patch`
    disable config-provided untracked and patch output like Git.
  - [x] `stash show --quiet` suppresses output and returns Git-compatible
    diff-exists exit codes for tracked and untracked show scopes.
  - [x] `stash show --exit-code` returns Git-compatible diff-exists exit
    codes while preserving explicit output formats, and defaults to patch
    output when no format is provided.
  - [x] `stash show --full-index` renders Git-compatible full object IDs in
    patch index headers, including combined stat-plus-patch and untracked
    patch scopes.
  - [x] `stash show --abbrev[=<n>]` renders Git-compatible abbreviated object
    IDs in patch index headers, including Git's four-character minimum and
    `--full-index` precedence.
  - [x] `stash show --raw`, `--patch-with-raw`, `--raw --patch`, and
    `--patch --raw` render Git-compatible raw records for checked tracked and
    untracked patch scopes.
  - [x] `stash show --summary`, `--summary --patch`, `--patch --summary`,
    `--summary --stat`, and `--stat --summary` render Git-compatible extended
    file summaries for checked tracked and untracked patch scopes.
  - [x] `stash show --compact-summary`, `--compact-summary --patch`,
    `--patch --compact-summary`, `--compact-summary --summary`, and
    `--summary --compact-summary` render Git-compatible compact stat and
    extended summary combinations for checked tracked show scopes.
  - [x] `stash show --diff-filter=<letters>` filters checked patch, stat,
    compact-summary, summary, and name-status output with Git-compatible
    uppercase include, lowercase exclude, invalid-class, and all-or-none
    behavior for checked added/deleted/modified stash paths.
  - [x] `stash show -U<n>` and `--unified=<n>` render Git-compatible patch
    context widths for checked tracked patch and stat-plus-patch scopes.
  - [x] `stash show --inter-hunk-context=<n>` merges nearby patch hunks for
    checked tracked patch scopes, including Git's `k` suffix form.
  - [x] `stash show --no-prefix` and `--default-prefix` render
    Git-compatible patch path prefixes for checked tracked patch and
    stat-plus-patch scopes.
  - [x] `stash show --src-prefix=<prefix>` and `--dst-prefix=<prefix>` render
    Git-compatible custom old-side and new-side patch path prefixes for
    checked single-prefix tracked patch scopes.
  - [x] `stash show --line-prefix=<prefix>` prefixes checked explicit
    stat, patch, name-only, and numstat text output like Git.
  - [x] `stash show --stat[=<width>[,<name-width>[,<count>]]]`,
    `--stat-width=<n>`, `--stat-name-width=<n>`,
    `--stat-graph-width=<n>`, and `--stat-count=<n>` are accepted for checked
    simple stat and stat-plus-patch output, including Git-compatible invalid
    numeric errors.
  - [x] `stash show --output-indicator-new=<char>`,
    `--output-indicator-old=<char>`, and
    `--output-indicator-context=<char>` render Git-compatible patch line
    prefixes and invalid multi-character errors for checked tracked patches.
  - [x] `stash show` accepts no-external-diff and no-color diff options
    (`--no-ext-diff`, `--ext-diff`, `--no-color`, `--color=never`,
    `--color=auto`) for Git-compatible no-color output in checked captures.
  - [x] `stash show --output=<file>` writes checked patch, stat,
    name-only, and default diff output to the target file with
    Git-compatible empty stdout.
  - [x] `stash show` accepts color-moved and root-relative passthrough diff
    options (`--color-moved[=<mode>]`, `--no-color-moved`,
    `--color-moved-ws=<modes>`, `--no-color-moved-ws`, `--relative`,
    `--no-relative`) for Git-compatible no-color simple text patch output in
    checked captures, including invalid color-moved mode errors.
  - [x] `stash show --relative=<path>` filters checked explicit stat, patch,
    name-only, name-status, and numstat output to matching paths and strips
    the selected directory prefix like Git.
  - [x] `stash show` accepts diff algorithm passthrough options
    (`--minimal`, `--patience`, `--histogram`,
    `--diff-algorithm=<algorithm>`, `--anchored=<text>`) for Git-compatible
    simple text patch output in checked captures, including invalid
    diff-algorithm errors.
  - [x] `stash show` accepts whitespace and indent heuristic passthrough diff
    options (`-w`, `--ignore-all-space`, `-b`, `--ignore-space-change`,
    `--ignore-space-at-eol`, `--ignore-cr-at-eol`, `--ignore-blank-lines`,
    `--indent-heuristic`, `--no-indent-heuristic`,
    `--ws-error-highlight=<kind>`) for Git-compatible simple text patch output
    in checked captures, including invalid ws-error-highlight errors.
  - [x] `stash show` accepts checked patch context/delete passthrough diff
    options (`--irreversible-delete`, `-D`, `--function-context`, `-W`) for
    Git-compatible simple text patch output in checked captures.
  - [x] `stash show --word-diff=none` is accepted for checked simple text
    patch output and invalid `--word-diff=<mode>` errors match Git.
  - [x] `stash show` accepts pickaxe and rewrite passthrough diff options
    (`--pickaxe-all`, `--pickaxe-regex`,
    `--break-rewrites[=<n>[/<m>]]`, `-B[<n>[/<m>]]`) for Git-compatible
    simple text patch output in checked captures, including invalid
    break-rewrites threshold errors.
  - [x] `stash show` accepts rename/copy/binary passthrough diff options
    (`--binary`, `--no-renames`, `--find-renames[=<n>]`, `-M[<n>]`,
    `--find-copies[=<n>]`, `-C[<n>]`, `--find-copies-harder`, `-l<n>`) for
    Git-compatible simple text patch output in checked captures.
  - [x] `stash show` accepts text and submodule/textconv passthrough diff
    options (`-a`, `--text`, `--textconv`, `--no-textconv`,
    `--ignore-submodules[=<when>]`, `--submodule[=<format>]`) for
    Git-compatible non-submodule text patch output in checked captures,
    including invalid `--submodule=<format>` errors.
  - [x] `stash show` accepts intent-to-add visibility passthrough diff
    options (`--ita-invisible-in-index`, `--ita-visible-in-index`) for
    Git-compatible simple text stash patch output in checked captures.
  - [x] Basic legacy `stash save [-q] [<message>]` uses the same tracked-change
    stash shape as push and matches Git's saved/no-change output.
  - [x] Legacy `stash save -u/--include-untracked` and `-a/--all` reuse the
    push untracked stash shapes while preserving legacy positional messages.
  - [x] Legacy `stash save -k/--keep-index` and `-S/--staged` reuse the push
    keep-index/staged stash shapes while preserving legacy positional messages.
  - [x] `rit stash create [<message>]` creates the same tracked-change stash
    commit shape without updating `refs/stash` or cleaning the worktree.
  - [x] Basic `stash apply -q [<stash>]` restores tracked worktree changes from
    a loose stash without dropping it when `HEAD` matches the stash base.
  - [x] Basic `stash pop -q [<stash>]` restores tracked worktree changes and
    drops the selected loose stash entry when `HEAD` matches the stash base.
  - [x] Basic `stash apply --index -q [<stash>]` restores the stash index
    parent for the same clean tracked apply scope.
  - [x] Basic `stash pop --index -q [<stash>]` restores the stash index parent
    before dropping the selected loose stash entry.
  - [x] Default `stash apply` and `stash pop` print Git-compatible human status
    summaries for the same clean tracked apply/pop scope.
  - [x] Basic `stash branch <branchname> [<stash>]` creates a branch at the
    stash base, checks it out, applies the clean tracked stash, and drops the
    selected loose stash entry on success.
  - [x] Default `stash apply`, `stash pop`, and `stash branch` match
    Git-compatible human status output for checked untracked-only stash
    restores.
  - [x] Basic `stash export --print`, `stash export --to-ref <ref>`, and
    `stash import <commit>` write/read Git-compatible stash-export chains for
    all loose stash entries and selected stash arguments.
  - [x] `stash show -z` renders NUL-terminated `--name-only`,
    `--name-status`, and `--numstat` summary output.
  - [ ] Broader untracked apply/pop restoration, broader show options, broader
    error parity, and conflict handling for apply/pop/branch.
- [~] Conflict index stages.
  - [x] Preserve stage 0/1/2/3 in index read, write, sorting, and
    `ls-files --stage` output.
  - [x] Write actual stage entries during conflicted merge application.
  - [x] Write `MERGE_HEAD` and `MERGE_MSG` for conflicted merge starts.
  - [x] Write simple worktree conflict markers for regular text content
    conflicts.
  - [x] Abort conflicted merge state with `rit merge --abort`.
  - [x] Quit conflicted merge state with `rit merge --quit` while leaving
    unmerged index stages and working tree conflict contents untouched.
  - [x] Continue resolved conflicted merge state with `rit merge --continue`.
  - [~] Write binary/delete/mode conflict worktree states and full merge result
    contents.
    - [x] Delete/modify conflicts leave the modified side in the working tree
      for both `HEAD`-deleted and target-deleted cases.
    - [x] Delete/modify conflicts print clear rit-specific conflict guidance
      for both directions.
    - [x] Binary content conflicts leave the `HEAD` version in the working tree
      and print clear rit-specific guidance.
    - [x] Add/add conflicts print clear rit-specific guidance.
    - [x] Mode-only changes combine cleanly with content-only changes instead
      of creating false conflicts.
    - [x] Regular-file/symlink distinct-type conflicts split both directions
      into Git-shaped index/worktree paths and explain that both versions were
      kept.
    - [x] Content conflicts with mode changes preserve Git-shaped stage modes.
    - [x] Content, binary, and add/add conflicts print rit-specific guidance
      and omit pre-merge debug output.
    - [x] Strategy-option `ours`/`theirs` removes supported same-path content
      conflicts from the unmerged index by selecting the requested side.
    - [x] Supported conflict result behavior has Git-vs-rit coverage for
      exit-code, status, index stages, and rit-specific explanatory output;
      new prose should remain clear unless exact text is required by a stable
      machine-readable interface.
    - [ ] Remaining conflict result coverage for unsupported merge strategies
      and conflict variants.

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
- [x] Lazy file materialization policy.
  - [x] Derive a lazy materialization policy from workspace profile
    `partial_clone` and `lazy_files` settings.
- [x] Prefetch command shape.
  - [x] Add a dry-run workspace prefetch plan model and
    `rit workspace prefetch <profile>` command shape.

Completion criteria:
- Users can read named workspace profiles and inspect sparse, lazy, partial,
  and prefetch plans without needing to understand Git sparse internals.

## M11: Auth

- [x] Credential abstraction.
  - [x] Add redacted credential, credential request, and provider trait models
    independent from transport execution.
- [x] Environment token provider.
  - [x] Read default token environment variables into redacted credential
    providers without mutating process environment in tests.
- [x] Git credential helper compatibility.
  - [x] Encode and parse Git credential helper line-protocol messages.
  - [x] Read `credential.helper` command shape from Git config.
  - [x] Read ordered helper chains and honor empty helper reset entries.
  - [x] Helper subprocess execution for `get`, `store`, and `erase`.
- [x] SSH agent integration.
  - [x] Model `SSH_AUTH_SOCK` based agent availability.
  - [x] Agent protocol identity lookup and signing.
- [x] OS keychain adapters.
  - [x] Model platform default keychain adapter selection.
  - [x] Windows Credential Manager read/write implementation.
  - [x] macOS Keychain read/write implementation.
  - [x] freedesktop Secret Service/libsecret read/write implementation.
- [x] CI non-interactive mode.
  - [x] Disable auth prompts for CI-like environments and
    `GIT_TERMINAL_PROMPT=0`.

Completion criteria:
- Secrets are never printed and auth is separated from transport.

## M12: Semantic Diff

- [x] Text diff foundation complete.
- [x] Word diff.
- [x] Tree-sitter feature gate.
- [x] Rust semantic adapter.
- [x] TypeScript semantic adapter.
- [x] Python semantic adapter.
- [x] JSON output model.
- [x] Use indexdb as an optional acceleration layer for semantic impact queries.

Completion criteria:
- Semantic output is structured and can distinguish code-only changes from
  tests/docs changes for supported languages.

## M13: Policy, Doctor, Repair

- [x] Policy config model.
- [x] Blob size warning/check.
- [x] Secret pattern warning/check.
- [x] Protected branch policy.
- [x] `rit doctor`
- [x] `rit repair`
- [x] `rit doctor` checks indexdb state, schema version, staleness, and
  corruption.
- [x] `rit repair` can rebuild or drop corrupted indexdb without touching Git
  objects.

Completion criteria:
- Policy defaults warn conservatively and blocking behavior requires explicit
  config.

## M14: VFS

- [x] Common VFS model.
- [x] Fallback materialized backend.
- [x] Platform backend plan.
- [x] Lazy materialization.
- [x] Background prefetch.

Completion criteria:
- Builds without VFS still work normally and VFS-specific errors are clear.

## M15: Release Packaging

- [x] Feature matrix for `rit-min` and `rit-full`.
- [x] CI build matrix.
- [x] Release archive layout.
- [x] README release instructions.
- [x] License and attribution audit.
- [x] Document which release builds include the `indexdb` feature.
- [x] Document SQLite dependency and bundled/non-bundled build choices.

Completion criteria:
- A release can be built as a single binary with documented feature choices.

## M16: Operation Journal And Universal Undo

- [x] Define `.git/rit/ops.log` as separate rit metadata that does not replace
  Git refs, objects, index, or working tree state.
- [x] Add `Repository::operations()` API.
- [x] Capture before/after HEAD, current branch, and index checksum snapshots.
- [x] Record successful `rit commit`, `rit checkout`, `rit switch`,
  fast-forward/conflicted `rit merge`, `rit merge --abort`, and
  `rit merge --continue`, and clean `rit cherry-pick` operations from the CLI.
- [x] Implement `rit op log`.
- [x] Implement `rit op restore <id>` for restorable HEAD snapshots.
- [x] Implement `rit undo` as restore-last-operation.
- [x] Add tests proving commit undo restores HEAD, index, and tracked worktree
  content for the first supported slice.
- [x] Record changed path lists.
- [x] Record created object IDs.
- [x] Store reversible patches for index-only and worktree-changing
  operations.
  - [x] Store before-index sidecars for index-only operations and let
    `undo`/`op restore` restore the index without rewriting the worktree.
  - [x] Store reversible worktree sidecars for selected worktree-changing
    operations and let `undo` restore pre-operation file contents or missing
    paths.
- [x] Add journal records for `add`, `restore`, `reset`, `branch`, `tag`,
  `fetch`, and `push` where applicable.
  - [x] Record successful `add`, `restore`, and `reset` operations.
  - [x] Record successful `branch` and `tag` ref operations.
  - [x] Record successful `fetch` operations and smart-remote `push` success
    paths where applicable.
- [x] Make undo semantics command-aware, such as commit undo that can preserve
  changes when requested.
- [x] Add corruption handling for malformed operation journal lines.
- [x] Add linked-worktree journal isolation tests.
- [x] Add JSON output for operation records.

Completion criteria:
- Operation metadata lives only under `.git/rit/` and can be deleted without
  breaking Git compatibility.
- `repo.operations().undo_last()?` provides a simple API for supported
  restorable operations.
- Broken or missing operation metadata never corrupts Git repository state.

## M17: Transaction Plan And Dry-Run API

- [x] Add structured plan type for all write operations.
- [x] Add `repo.add(...).plan()?` style API or equivalent builder.
- [x] Add structured plan type for `rit add`.
- [x] Add `rit add --plan`.
- [x] Add structured plan type for `rit commit`.
- [x] Add `rit commit --plan`.
- [x] Add structured plan type for `rit reset`.
- [x] Add `rit reset --plan`.
- [x] Add structured plan type for `rit merge`.
- [x] Add `rit merge --plan`.
- [x] Ensure plans describe refs, index paths, worktree paths, object writes,
  hooks, and policy checks before applying changes.

## M18: Explainable Git Expansion

- [x] Extend `status --explain` model beyond the current roadmap sketch.
- [x] Add `rit ignore explain <path>`.
- [x] Add `rit pathspec explain <pathspec>`.
- [x] Add `rit merge explain <target>`.
- [x] Add `rit auth explain <url>`.
- [x] Add explain output for LFS/Xet/workspace decisions.

## M19: Smartlog And Local Work Graph

- [x] Add local graph model for HEAD, local branches, upstreams, stashes,
  worktrees, unpushed commits, and diverged branches.
- [x] Add `rit smartlog` or `rit graph`.
- [x] Add JSON output for local graph consumers.

## M20: Doctor Fix Plans

- [x] Add `rit doctor --explain`.
- [x] Add `rit doctor --json`.
- [x] Add `rit doctor --fix-plan`.
- [x] Explain performance and maintenance findings such as loose objects,
  pack/index state, commit graph, and stale rit metadata.

## M21: Workspace Recommendation

- [x] Add `rit workspace suggest`.
- [x] Add `rit workspace from-change`.
- [x] Add `rit workspace from-package <path>`.
- [x] Use changed files, package manifests, CODEOWNERS, and import/build graph
  hints where available.

## M22: Impact Analysis And CI Helper

- [x] Add `rit impact <range>`.
- [x] Return changed packages, affected tests, public API changes, docs-only
  status, large-file changes, and reviewer hints.
- [x] Reuse semantic diff and optional indexdb acceleration.

## M23: Stable JSON Schema And Typed API

- [x] Define stable JSON schemas for status, diff, doctor, operations, impact,
  and indexdb.
- [x] Add `rit schema status`.
- [x] Add `rit schema diff`.
- [x] Add `rit schema doctor`.
- [x] Expose the same typed models from Rust APIs.

## M24: Compatibility Oracle

- [x] Add `rit compat check <command>`.
- [x] Add `rit compat report --since <rev>`.
- [x] Add `rit compat fixture generate`.
- [x] Let users validate Git compatibility against their own repositories.

## M25: Large File Audit And Migration Plan

- [x] Add `rit large-files audit`.
- [x] Report large blobs in current history.
- [x] Recommend LFS/Xet tracking patterns.
- [x] Produce a safe migration plan before any rewrite or tracking change.

## Active Queue

1. Normalize Unix/macOS compatibility-test baselines so the full
   `cargo test --workspace` oracle can safely expand beyond Windows.
2. Continue M8 merge with binary/delete/mode conflict handling,
   abort/continue, and merge-commit workflow planning.
3. Keep M6 case-sensitivity parity under verification as new path lookup
   surfaces are added.

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
  - Later slices completed HTTP(S) pack storage/application and one-round
    fetch negotiation. Still open: full multi-round negotiation.
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
  - Later slices completed HTTP(S) pack negotiation/application and `rit fetch`
    wiring.
- 2026-05-11, M7 SSH command model:
  - Reference Git: `git version 2.52.0.windows.1`.
  - Reference docs checked: local `gitprotocol-pack(5)` SSH transport examples
    for `git-upload-pack` and repository path quoting.
  - Implemented: pure Rust parsing for `ssh://user@host/path` and
    `user@host:path` locations plus remote `git-upload-pack` /
    `git-receive-pack` command construction.
  - Later slice added process-backed upload-pack pkt-line I/O over an `ssh`
    session executor.
  - Still open: SSH fetch/push workflow wiring, receive-pack session I/O,
    authentication options, and pack negotiation.
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
  - Later slices completed HTTP(S) `rit fetch` ingestion. Still open:
    thin-pack fixups and deeper negotiation parity.
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
  - Later slice completed HTTP(S) `rit fetch` ingestion. Still open:
    multi-round negotiation and thin-pack fixups.
- 2026-05-11, M7 plain HTTP fetch ingestion:
  - Reference Git: `git version 2.52.0.windows.1`.
  - Reference docs checked: `git fetch -h`, local `gitprotocol-http(5)`, and
    local `gitprotocol-pack(5)`.
  - Implemented: `Repository::fetch_remote_http` runs the smart HTTP
    negotiation API, ingests returned pack bytes into `.git/objects`, writes
    `.git/FETCH_HEAD`, and updates a destination ref for one simple refspec.
  - Implemented: `rit fetch http://... [<src>:<dst>]` dispatches to the plain
    HTTP path. A later TLS slice extended the same path to `https://`.
  - Still open: SSH sessions, named remote config, multiple refspecs,
    multi-round negotiation, and thin-pack fixups.
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
  - Implemented: `rit push http://... <src>:<dst>` CLI dispatch for this smart
    HTTP subset. A later TLS slice extended the same path to `https://`.
  - Still open: SSH sessions, named remotes, multiple refspecs, force/lease
    semantics, hooks, thin-pack/delta generation, and full object minimization
    against remote history.
- 2026-05-12, M7 HTTPS/TLS transport:
  - Reference Git: `git version 2.52.0.windows.1`.
  - Reference docs checked: `git fetch -h` and `git push -h`.
  - Implemented: `BlockingSmartHttpClient` parses `https://` URLs, uses
    `native-tls` for platform certificate verification, and reuses the existing
    smart HTTP GET/POST validation path.
  - Implemented: `rit fetch https://...` and `rit push https://...` now
    dispatch to the smart HTTP transport instead of being rejected at argument
    parsing time.
  - Still open: SSH sessions, multi-round negotiation, thin-pack fixups, and
    advanced fetch/push options.
- 2026-05-12, M7 SSH upload-pack session I/O:
  - Reference Git: `git version 2.52.0.windows.1`.
  - Reference docs checked: `git fetch -h` and `git push -h`.
  - Implemented: `SshServiceExecutor` trait, `ProcessSshServiceExecutor`
    backed by the system `ssh` program, `SshServiceCommand::target`, and
    `run_ssh_upload_pack` for one pkt-line upload-pack request/response cycle.
  - Safety note: the executor starts `ssh`, not `git`; remote Git service
    commands are passed as quoted SSH remote commands.
  - Still open: wiring SSH fetch/push workflows, receive-pack sessions,
    authentication options, and multi-round negotiation.
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
