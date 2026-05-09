# rit

`rit` is a Rust implementation of Git focused on compatibility, readable internals, and a simple public API.

The project is intentionally not a wrapper around the `git` executable. Existing Git is used only as a reference implementation for documentation and compatibility tests.

## Current Scope

This repository currently implements the initial workspace skeleton, the `rit` CLI entry point, repository discovery, and basic `help` / `version` / `init` / `rev-parse` commands.

## Development

```bash
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```
