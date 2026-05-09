# Implementation Notes

## Baseline Git Check

- Checked Git version: `git version 2.52.0.windows.1`
- Checked command list: `git help -a`
- `git help <command>` opened the local manual pager in this environment and timed out, so command-specific checks used `git <command> -h`.

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
- Supported options: default output, `--name-only`, `--object-only` for full 40-character loose tree IDs.
- Unsupported options: recursion, path filtering, long output, custom format, abbreviation, revision syntax.
- Git-compatible behavior: tree entry parsing and default `<mode> <type> <object>\t<path>` output.
- Intentional differences: only loose tree object IDs are accepted.
- Repository mutation: no
- Risk: none

### `rit status`

- Baseline command checked: `git status -h`
- Supported options: `--porcelain`, `--porcelain=v1`, `-s`.
- Unsupported options: long output, branch header, ignored display modes, pathspecs, rename detection, submodules, sparse checkout.
- Git-compatible behavior: porcelain v1 entries for staged add/modify/delete, working tree modify/delete, and untracked files.
- Intentional differences: ignore handling supports simple literal and directory patterns first; advanced gitignore glob semantics are not complete yet.
- Repository mutation: no
- Risk: no repository writes.

### `rit diff`

- Baseline command checked: `git diff -h`
- Supported options: `--name-only`, `--stat`.
- Unsupported options: patch output, cached diff, commit/tree/blob arguments, pathspecs, rename/copy detection, binary stat details.
- Git-compatible behavior: default diff scope compares working tree files against the index and ignores untracked files.
- Intentional differences: binary `--stat` reports a clear unsupported error until binary diff accounting is implemented.
- Repository mutation: no
- Risk: no repository writes.

### `rit log`

- Baseline command checked: `git log -h`
- Supported options: default output, `--oneline`.
- Unsupported options: revision ranges, decoration, graph, path filtering, grep, ordering controls, diff output.
- Git-compatible behavior: reads commits from `HEAD`, follows the first parent, prints default author/date/message layout and 7-character oneline IDs.
- Intentional differences: merge traversal is first-parent only until revision walking is implemented.
- Repository mutation: no
- Risk: no repository writes.
