# rit

`rit` is a Rust implementation of Git focused on compatibility, readable
internals, safe repository writes, and a simple public API.

The name means **Rust + Git**. `rit` is not a wrapper around the `git`
executable: runtime command implementations must read and write Git repository
formats directly. The system `git` binary is used only as a reference
implementation for documentation, compatibility tests, and benchmarks.

## Goals

- **Compatibility:** Stay compatible with existing Git repositories wherever behavior is externally observable.
- **Approachability:** Keep the internal code approachable for beginner-to-intermediate Rust developers.
- **Simplicity & Safety:** Expose a straightforward Rust API, a familiar CLI, and prefer safe, explicit repository writes using lock files and atomic replaces.
- **Performance:** Fast execution powered by Rust and optional acceleration layers like `indexdb`.
- **Advanced Features:** Grow advanced features natively: LFS, Xet, auth, sparse checkout, partial clone, semantic diff, policy checks, VFS, and universal undo (Operation Journal).
- **Single Binary:** Ship as a single binary by default, with feature-gated minimal and full builds.

## Current Status & Features

`rit` is rapidly evolving from a local Git engine into a next-generation VCS. Current capabilities include:

- **Core Git Engine:** Loose/packed object read/write, index v2 read/write, refs, tags, and revision resolution.
- **Local Operations:** `add`, `commit`, `branch`, `checkout`, `switch`, `restore`, `reset`, `merge`, `cherry-pick`, `rebase`, `stash`.
- **Information & Diff:** `status`, `diff` (with rename/copy detection), `log`, `show`, `ls-files`, `ls-tree`.
- **Advanced Capabilities:**
  - **IndexDB:** Optional SQLite auxiliary index for accelerating history queries and operations.
  - **Operation Journal & Undo:** Universal `undo` for local state changes using `.git/rit/ops.log`.
  - **Explainable Git:** `status --explain`, `ignore explain`, `pathspec explain`, `merge explain`.
  - **Semantic Diff:** AST-aware diffing for Rust, TypeScript, and Python.
  - **Large Files:** Native LFS and Xet pointer parsing and batch APIs.
  - **Auth:** Extensible credential helpers, SSH agent integration, and OS keychain adapters.
  - **Maintenance:** `doctor` and `repair` for repository health checks.

*(For the full list of implemented CLI commands, refer to the source or run `rit help`)*

## Future Vision (Killer Features)

- **Time Machine:** Visual, timeline-based exploration and recovery of repository state.
- **Semantic Rebase & Merge:** AST-aware conflict resolution and logical change tracking.
- **Zero-Config Large Repo (Smart VFS):** Instant cloning and on-demand file materialization for 100GB+ repositories.
- **Impact Analysis CI Helper:** Intelligent change-impact detection to optimize CI pipelines.
- **Policy-as-Code:** Built-in compliance checks (secret scanning, file size limits, style enforcement).

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
ANALIST.md                In-depth architecture analysis and feature proposals
```

## Build And Test

Use stable Rust with the workspace commands below:

```bash
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

## Continuous Integration

The full compatibility test suite is currently pinned to the checked Windows
Git baseline recorded in `docs/compatibility.md`, so CI runs formatting,
clippy, and `cargo test --workspace` on `windows-latest`.

CI runs for pull requests and for pushes to feature branches as well as the
default branch, so branch pushes can be used to verify milestone work before a
PR is opened.

Linux and macOS are still guarded in CI through the release-build matrix for
both `rit-min` and `rit-full`. Expanding the full test matrix to Linux and
macOS is tracked as a compatibility-baseline task once platform-specific Git
output and file-mode differences are normalized.

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

## License

This workspace is licensed under `MIT`.
