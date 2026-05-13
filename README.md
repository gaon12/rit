# rit

`rit` is a Rust implementation of Git focused on compatibility, readable
internals, safe repository writes, and a simple public API.

The name means **Rust + Git**. `rit` is not a wrapper around the `git`
executable: runtime command implementations must read and write Git repository
formats directly. The system `git` binary is used only as a reference
implementation for documentation, compatibility tests, and benchmarks.

## Goals

- Stay compatible with existing Git repositories wherever behavior is externally
  observable.
- Keep the internal code approachable for beginner-to-intermediate Rust
  developers.
- Expose a straightforward Rust API as well as a familiar Git-like CLI.
- Prefer safe, explicit repository writes using lock files and atomic replaces.
- Grow advanced features as modules: LFS, Xet, auth, sparse checkout, partial
  clone, semantic diff, policy checks, and VFS.
- Ship as a single binary by default, with feature-gated minimal and full builds.

## Current Status

`rit` is an early local Git engine. It currently includes:

- Workspace crates for CLI, core library, and compatibility testkit.
- Repository discovery, init, bare repository detection, linked worktree
  common-dir discovery, shared config parsing, and repository format guards.
- Loose object read/write for blobs, trees, commits, and tags.
- Pack index v2 lookup and non-delta packed object reads.
- Index v2 read/write for regular files.
- Local refs, packed refs lookup, lightweight tags, and simple revision
  resolution including unambiguous short object IDs.
- Basic local working tree operations and branch switching.

Implemented CLI surface:

```text
rit version
rit help
rit init
rit rev-parse
rit cat-file
rit ls-tree [--name-only|--object-only] <tree-ish> [--] [<pathspec>...]
rit ls-files [--stage] [--] [<pathspec>...]
rit show
rit show --no-patch -- <pathspec>
rit status --porcelain=v1
rit status --porcelain=v1 -- <pathspec>
rit diff --name-only
rit diff
rit diff -p
rit diff --name-status
rit diff --numstat
rit diff --stat
rit diff --cached --name-only
rit diff --cached --name-status
rit diff --cached --numstat
rit diff --cached --stat
rit diff [--cached] <summary-mode> -- <pathspec>
rit log [--oneline] [--] [<pathspec>...]
rit add [--plan] <pathspec>...
rit commit [--plan]
rit branch
rit tag
rit restore <pathspec>...
rit reset [--plan] <pathspec>...
rit checkout <branch-or-commit>
rit switch
rit merge [--plan] [--ff-only] <target>
rit op log
rit op restore <id>
rit undo
```

Many options are intentionally still unsupported. Unsupported behavior should
fail clearly rather than guessing and risking repository damage.

## Repository Layout

```text
crates/
  rit-cli/       CLI entry point and command formatting
  rit-core/      Git data models and repository operations
  rit-indexdb/   Planned optional SQLite auxiliary index
  rit-testkit/   Git-vs-rit compatibility harness

docs/
  compatibility.md        Current Git compatibility baseline
  implementation-notes.md Command notes, supported options, known gaps
  milestones.md           Day-to-day milestone tracker
  roadmap.md              Product-level roadmap
```

## Build And Test

Use stable Rust with the workspace commands below:

```bash
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Build the development binaries:

```bash
cargo build --workspace
```

Run the CLI from the workspace:

```bash
cargo run -p rit-cli -- status --porcelain=v1
cargo run -p rit-cli -- diff --cached --name-status
```

## Release Builds

`rit-min` is the default single-binary release profile:

```bash
cargo build -p rit-cli --release --locked
```

`rit-full` enables the optional model and adapter features currently intended
for full releases:

```bash
cargo build -p rit-cli --release --locked --features rit-core/large-files,rit-core/semantic-json,rit-core/semantic-rust,rit-core/semantic-typescript,rit-core/semantic-python,rit-core/vfs
```

Release archive names, contents, and target triples are defined in
`docs/release.md`.

## Compatibility Workflow

Before implementing or expanding a command, refresh the local Git baseline:

```bash
git --version
git help -a
git <command> -h
```

Record the result in `docs/implementation-notes.md`. For compatibility-sensitive
behavior, compare the observable outputs:

- stdout
- stderr
- exit code
- `.git` metadata
- index state
- refs
- working tree files
- object graph

`rit-testkit` exists for these comparisons and is allowed to execute `git`
because it is test infrastructure. Production `rit` command code must not call
`git`.

## Development Rules

- Keep CLI output formatting separate from core repository logic.
- Prefer clear data structures over clever abstractions.
- Use explicit error types in public library APIs.
- Write repository state through lock files or temporary files followed by
  atomic replacement.
- Treat unknown repository formats and extensions conservatively.
- Add tests for parser logic, repository state changes, and compatibility
  behavior as features grow.
- Update `docs/milestones.md` when a milestone starts, finishes, or changes
  scope.

## Roadmap

The active milestone checklist lives in `docs/milestones.md`. The high-level
product roadmap lives in `docs/roadmap.md`.

Near-term implementation focus:

1. Complete read-only local command coverage.
2. Add stronger compatibility fixtures for status, diff, log, and object
   inspection commands.
3. Expand pathspec and ignore handling.
4. Improve index/worktree fidelity, including stat refresh and file modes.
5. Move toward transport, large-file, sparse, semantic diff, policy, and release
   milestones once the local core is solid.

## License

This workspace is licensed under `MIT`.
