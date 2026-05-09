# Implementation Notes

## Baseline Git Check

- Checked Git version: `git version 2.52.0.windows.1`
- Checked command list: `git help -a`
- `git help <command>` opened the local manual pager in this environment and timed out, so command-specific checks used `git <command> -h`.
- 2026-05-09 baseline refresh checked: `git status -h`, `git add -h`, `git commit -h`, `git diff -h`, and `git log -h`.
- 2026-05-09 diff cached milestone checked: `git diff -h`.
- 2026-05-09 diff numstat milestone checked: `git diff -h`.
- 2026-05-09 pathspec filter milestone checked: `git status -h`, `git diff -h`, `git add -h`, `git restore -h`, and `git reset -h`.
- 2026-05-09 diff patch milestone checked: `git diff -h`.
- 2026-05-09 detached checkout milestone checked: `git checkout -h`.
- 2026-05-09 branch delete safety milestone checked: `git branch -h`.

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
- Known gap exposed by state comparison: `git status` refreshes `.git/index` stat data, while `rit status` currently leaves the index unchanged.
- Added rit CLI integration compatibility fixtures for read-only `diff` output modes: `--name-only`, `--name-status`, `--numstat`, and `--stat`, including cached diff variants.
- Compatibility reports now include the first differing stdout/stderr line when command text differs.

### M2: Core repository model foundation

- Added `Repository::open(path)` as the application-facing alias for repository discovery.
- Repository discovery and init now construct repositories through a shared format guard.
- Repositories with `core.repositoryformatversion` other than `0`, or unknown `[extensions]` keys, fail with clear errors before use.

### M3: Local diff scope expansion

- Added staged diff support through `Repository::diff_index_to_head()`.
- `rit diff --cached --name-only`, `rit diff --cached --stat`, `rit diff --staged --name-only`, and `rit diff --staged --stat` now compare the index with `HEAD`.
- Unborn `HEAD` is treated like an empty tree for staged diff, matching the usual Git shape for newly added files.
- Added `--name-status` formatting for both default and cached diff scopes.
- Added `--numstat` formatting for both default and cached diff scopes.
- Added a conservative `PathspecSet` model for ordinary literal file and
  directory pathspecs.
- Added pathspec filtering for `status --porcelain=v1` and all supported
  `diff` summary modes, including `--cached`/`--staged`.
- Added Git comparison coverage for `status --porcelain=v1 -- <pathspec>` and
  `diff ... -- <pathspec>`.
- Added ordinary literal file, directory, and `.` pathspec expansion for
  `add`, `restore`, and `reset`.
- Added ordinary literal pathspec filtering for `ls-files`, including
  `--stage`.
- Added ordinary literal path lookup for `ls-tree <tree-ish> <path>`,
  including `--name-only` and `--object-only`.
- Added ordinary literal path filtering for first-parent `log`, including
  `--oneline`.
- Added patch output for small text files in default and cached diff scopes,
  with Git comparison coverage for default patch, `-p`, and `--cached`.
- Added local write compatibility coverage that compares Git and rit porcelain
  state after directory pathspec `add`, `restore`, and `reset`.
- Still unsupported: pathspec magic, glob pathspec matching, pathspec files,
  and pathspec filtering for `show`.

## Implemented Commands

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
- Supported options: `-t`, `-s`, `-p`, and `<type> <object>` for full 40-character loose object IDs.
- Unsupported options: abbreviated object IDs, revision syntax, batch modes, filters, textconv, mailmap, packed objects.
- Git-compatible behavior: loose object zlib decoding, header validation, object type and size output, tree pretty printing.
- Intentional differences: packed objects and revision resolution fail clearly until the object database grows those features.
- Repository mutation: no
- Risk: none

### `rit ls-tree`

- Baseline command checked: `git ls-tree -h`
- Supported options: default output, `--name-only`, `--object-only`, commit or
  tree revisions, and ordinary literal path lookup.
- Unsupported options: recursion, long output, custom format, abbreviation,
  pathspec magic/globs.
- Git-compatible behavior: tree entry parsing and default `<mode> <type> <object>\t<path>` output.
- Intentional differences: advanced pathspec forms are not implemented yet.
- Repository mutation: no
- Risk: none

### `rit status`

- Baseline command checked: `git status -h`
- Supported options: `--porcelain`, `--porcelain=v1`, `-s`, plus ordinary
  literal file and directory pathspecs after `--`.
- Unsupported options: long output, branch header, ignored display modes,
  pathspec magic/globs, rename detection, submodules, sparse checkout.
- Git-compatible behavior: porcelain v1 entries for staged add/modify/delete, working tree modify/delete, and untracked files.
- Intentional differences: ignore handling supports simple literal and directory patterns first; advanced gitignore glob semantics are not complete yet.
- Repository mutation: no
- Risk: no repository writes.

### `rit diff`

- Baseline command checked: `git diff -h`
- Supported options: default patch output for small text files, `-p`, `-u`,
  `--name-only`, `--name-status`, `--numstat`, `--stat`, plus
  `--cached`/`--staged` with those output modes, and ordinary literal file and
  directory pathspec filters.
- Unsupported options: commit/tree/blob arguments, pathspec magic/globs,
  rename/copy detection, binary stat details, multi-hunk context splitting, and
  no-newline patch markers.
- Git-compatible behavior: default diff scope compares working tree files against the index and ignores untracked files.
- Git-compatible behavior: cached diff scope compares the index against `HEAD`.
- Intentional differences: binary `--stat` reports a clear unsupported error until binary diff accounting is implemented.
- Repository mutation: no
- Risk: no repository writes.

### `rit log`

- Baseline command checked: `git log -h`
- Supported options: default output, `--oneline`, and ordinary literal file or
  directory path filters.
- Unsupported options: revision ranges, decoration, graph, advanced path
  history simplification, grep, ordering controls, diff output.
- Git-compatible behavior: reads commits from `HEAD`, follows the first parent,
  prints default author/date/message layout and 7-character oneline IDs.
- Intentional differences: merge traversal is first-parent only until revision
  walking is implemented; rename-aware history simplification is not
  implemented.
- Repository mutation: no
- Risk: no repository writes.

### `rit add`

- Baseline command checked: `git add -h`
- Supported options: ordinary literal file, directory, and `.` pathspecs.
- Unsupported options: advanced pathspec forms, update/all modes,
  patch/interactive mode, chmod, sparse mode, ignored-file override.
- Git-compatible behavior: writes blob loose objects and Git index v2 entries
  for regular files; directory pathspecs recursively add regular files and
  stage deletions for matching tracked files that no longer exist.
- Intentional differences: pathspec magic, glob pathspecs, ignored-file checks,
  and pathspec-file inputs are not implemented yet.
- Repository mutation: yes, writes loose objects and `.git/index` using lock/rename.
- Risk: low for explicit files; missing paths remove matching index entries.

### `rit commit`

- Baseline command checked: `git commit -h`
- Supported options: `-m <message>`, `--message <message>`, `--message=<message>`.
- Unsupported options: hooks, signing, amend, templates, cleanup modes, pathspec commits, author/date override.
- Git-compatible behavior: writes tree and commit loose objects, uses first parent from `HEAD`, advances symbolic `HEAD` ref.
- Intentional differences: commit timestamps use UTC `+0000`; hooks are not run yet.
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
  restore index from `HEAD`, with ordinary literal file, directory, and `.`
  pathspecs.
- Unsupported options: source revisions, patch mode, merge conflict modes, sparse controls, pathspec files.
- Git-compatible behavior: explicit tracked file restore for regular files.
- Intentional differences: advanced pathspec forms and conflict handling are not
  implemented.
- Repository mutation: worktree restore writes files; staged restore writes `.git/index`.
- Risk: moderate; worktree writes use temp file then replace destination.

### `rit reset`

- Baseline command checked: `git reset -h`
- Supported options: ordinary literal file, directory, and `.` pathspecs,
  equivalent to unstaging matching paths from `HEAD`.
- Unsupported options: commit-moving resets, soft/mixed/hard/merge/keep modes, patch mode, pathspec files.
- Git-compatible behavior: unstages explicit paths and reports remaining unstaged modifications.
- Intentional differences: no index refresh metadata beyond object ID/size/mode.
- Repository mutation: writes `.git/index`.
- Risk: low for explicit paths; index writes use lock/rename.

### `rit checkout`

- Baseline command checked: `git checkout -h`
- Supported options: checkout existing local branch, checkout a commit with
  detached `HEAD`, `-b <branch>` create and checkout.
- Unsupported options: path checkout, explicit `--detach`, force, orphan,
  tracking, merge/conflict modes, submodules.
- Git-compatible behavior: updates symbolic `HEAD`, writes index from target commit tree, materializes tracked worktree files.
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

### `rit rev-parse` revision support

- Baseline command checked: `git rev-parse -h`
- Added support: full object IDs, unambiguous abbreviated object IDs, `HEAD`, local branch names, lightweight tag names.
- Unsupported revision syntax: ancestry operators, ranges, path suffixes, reflog selectors.
- Repository mutation: no.

### `rit show`

- Baseline command checked: `git show -h`
- Supported options: default object display and `--no-patch`/`-s` for commits, with optional revision.
- Unsupported options: commit diffs, revision ranges, path filters, decorations, formatting controls.
- Git-compatible behavior: commit no-patch layout, tree pretty printing, blob contents.
- Intentional differences: commit diffs are not emitted yet.
- Repository mutation: no.

### `rit ls-files`

- Baseline command checked: `git ls-files -h`
- Supported options: default cached file listing, `--stage`/`-s`, and ordinary
  literal file or directory pathspec filters.
- Unsupported options: deleted/modified/others/ignored filters, advanced
  pathspec forms, EOL/debug/format output, sparse/submodule modes.
- Git-compatible behavior: lists index paths and stage records as `<mode> <object> 0<TAB><path>`.
- Repository mutation: no.

## Object Database

### Loose objects

- Supports reading and writing loose `blob`, `tree`, `commit`, and `tag` objects.
- Loose writes use zlib compression and temp-file/rename placement.

### Packed objects

- Baseline documents checked through `git help -a` developer-facing `format-pack` listing and compatibility tests against `git gc`.
- Supported index format: pack index v2.
- Supported pack object types: non-delta commit, tree, blob, tag.
- Unsupported object types: OFS_DELTA and REF_DELTA are detected and reported clearly.
- Git-compatible behavior: after `git gc --aggressive --prune=now`, `rit cat-file`, `rit ls-tree`, `rit log`, and `rit show --no-patch` can read packed non-delta objects.
- Repository mutation: no.
