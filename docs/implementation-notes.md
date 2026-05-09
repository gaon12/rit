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
