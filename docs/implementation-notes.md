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
- Added local write compatibility coverage that compares Git and rit porcelain
  state after directory pathspec `add`, `restore`, and `reset`.
- Added local write compatibility coverage for simple wildcard and
  bracket-class pathspecs in `add`, `restore`, and `reset`.
- Still unsupported: attr pathspec magic, pathspec files, and
  `show` path filtering for patch output.

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
  `:(icase)` pathspec magic.
- Unsupported options: commit/tree/blob arguments, attr pathspec magic,
  rename/copy detection, and many advanced patch formatting options.
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
- Supported options: ordinary literal file, directory, `.`, simple `*`, `?`,
  and bracket-class wildcard pathspecs, positive `:(literal)`, `:(glob)`,
  `:(top)`, `:/`, and `:(icase)` pathspec magic, plus `--chmod=+x`, `--chmod=-x`,
  `--chmod +x`, and `--chmod -x`.
- Unsupported options: attr pathspec magic, pathspec files, update/all modes,
  patch/interactive mode, sparse mode, ignored-file override.
- Git-compatible behavior: writes blob loose objects and Git index v2 entries
  for regular files; directory pathspecs recursively add regular files and
  stage deletions for matching tracked files that no longer exist.
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
- Intentional differences: ignored-file checks and
  pathspec-file inputs are not implemented yet. On Windows, worktree
  executable bits remain filemode-insensitive like Git's usual `core.filemode`
  behavior there.
- Repository mutation: yes, writes loose objects and `.git/index` using lock/rename.
- Risk: low for explicit files; missing paths remove matching index entries.

### `rit commit`

- Baseline command checked: `git commit -h`
- Supported options: `-m <message>`, `--message <message>`,
  `--message=<message>`, `--author=<author>`, `--author <author>`,
  `--date=<date>`, `--date <date>`, `-n`, `--no-verify`, `--verify`.
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
  `:(glob)`, `:(top)`, `:/`, and `:(icase)` pathspec magic.
- Unsupported options: source revisions, patch mode, merge conflict modes,
  sparse controls, attr magic, pathspec files.
- Git-compatible behavior: explicit tracked file restore for regular files,
  including executable worktree permissions for `100755` index entries on Unix.
- Git-compatible behavior: symlink index entries are restored as symlinks on
  Unix and as link-target text files on platforms without Unix symlink support.
- Git-compatible behavior: when `core.symlinks=false`, restore and checkout
  materialize `120000` entries as plain `100644` files containing the link
  target text, and status treats that plain file as clean.
- Intentional differences: pathspec files and conflict handling are not
  implemented.
- Repository mutation: worktree restore writes files; staged restore writes `.git/index`.
- Risk: moderate; worktree writes use temp file then replace destination.

### `rit reset`

- Baseline command checked: `git reset -h`
- Supported options: ordinary literal file, directory, `.`, simple `*`, `?`,
  and bracket-class wildcard pathspecs plus positive `:(literal)`, `:(glob)`,
  `:(top)`, `:/`, and `:(icase)` pathspec magic, equivalent to unstaging matching paths
  from `HEAD`.
- Unsupported options: commit-moving resets, soft/mixed/hard/merge/keep modes,
  patch mode, attr magic, pathspec files.
- Git-compatible behavior: unstages explicit paths and reports remaining unstaged modifications.
- Git-compatible behavior: clean tracked paths refresh cached index stat
  metadata during `status --porcelain=v1`.
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
  partial worktree; local fetch into an existing repository is still pending.
- Repository mutation: creates a new repository directory and copies local
  object/ref files without invoking external `git`.
- Risk: moderate; object/ref transfer is copy-based and does not mutate the
  source repository.

### `rit fetch`

- Baseline command checked: `git fetch -h`
- Supported options: `--quiet`/`-q` with one local repository path and either no
  refspec or one simple `<src>:<dst>` refspec.
- Unsupported options: named remotes, multiple refspecs, append/atomic/force
  semantics, tags, prune, shallow/partial fetch, submodules, protocol options,
  stdin, and maintenance hooks.
- Git-compatible behavior: `fetch <local-repository>` copies source objects
  into the current repository and overwrites `.git/FETCH_HEAD` with the source
  `HEAD` commit. Local refs and remote-tracking refs are not updated, matching
  Git's no-refspec local fetch shape.
- Git-compatible behavior: `fetch <local-repository> <src>:<dst>` resolves the
  source ref, copies objects, writes `FETCH_HEAD`, and updates the destination
  full ref.
- Intentional differences: default progress/status text is simplified; quiet
  mode is used for compatibility coverage.
- Repository mutation: writes object files and `.git/FETCH_HEAD`.
- Risk: moderate; fetch mutates only the destination repository.

### Transport model

- Baseline commands checked: `git clone -h`, `git fetch -h`
- Supported protocol classification: local filesystem paths, `http://`,
  `https://`, `ssh://`, and scp-like `user@host:path` locations.
- Supported HTTP model: smart HTTP reference-discovery request metadata for
  `git-upload-pack` and `git-receive-pack`, plus pkt-line advertised-ref
  response parsing.
- Supported negotiation model: smart HTTP `git-upload-pack` request bodies with
  at least one `want`, optional first-want capabilities, optional `have` lines,
  and a terminal `done`.
- Unsupported behavior: no HTTP, HTTPS, or SSH network transfer is implemented
  yet. Remote-looking locations are classified and rejected by commands whose
  current implementation only supports local paths.
- Repository mutation: no direct mutation; command implementations decide how
  to act on a classified location.
- Risk: low; this is routing metadata for future transports.

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
