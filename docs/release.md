# Release Packaging

This document defines the release packaging shape for `rit`.

`rit` ships as a single `rit` binary. The release names below describe build
profiles, not separate source trees.

## Feature Matrix

| Build | Cargo command | Included feature intent | Excluded by default |
| --- | --- | --- | --- |
| `rit-min` | `cargo build -p rit-cli --release --locked` | Core CLI, local repository operations, HTTP(S) transport, policy/doctor/repair models, fallback VFS model | LFS, Xet, semantic tree-sitter adapters, semantic JSON, platform VFS |
| `rit-full` | `cargo build -p rit-cli --release --locked --features rit-core/large-files,rit-core/semantic-json,rit-core/semantic-rust,rit-core/semantic-typescript,rit-core/semantic-python,rit-core/vfs` | Everything in `rit-min` plus LFS/Xet models, semantic JSON, Rust/TypeScript/Python semantic adapters, VFS planning APIs | Platform VFS drivers, network/promisor VFS fetching, and SQLite indexdb feature selection remain separate |
| `rit-indexed` | `cargo build -p rit-cli --release --locked --features indexdb` | Everything in `rit-min` plus the optional SQLite auxiliary index command surface, schema, fallback query APIs, and `rit file-history <path>` | Must remain safe when `.git/rit/indexdb.sqlite` is missing, stale, dropped, or corrupted |

Current `rit-core` feature flags:

| Feature | Purpose |
| --- | --- |
| `large-files` | Convenience feature for `lfs` and `xet`. |
| `lfs` | Git LFS pointer/cache/API models. |
| `xet` | Xet pointer/reconstruction models. |
| `semantic-json` | JSON serialization for semantic diff reports. |
| `semantic-tree-sitter` | Common tree-sitter integration. |
| `semantic-rust` | Rust function summary adapter. |
| `semantic-typescript` | TypeScript/TSX function summary adapter. |
| `semantic-python` | Python function summary adapter. |
| `vfs` | Enables VFS availability for planning APIs. |
| `indexdb` | Optional SQLite auxiliary index for commit metadata, parent graph, refs snapshots, file history, and cache state. Query APIs fall back to canonical Git data when the database is missing, stale, corrupted, or incomplete. |

Release builds must keep working with no feature flags. Feature-gated code must
return clear unsupported-feature messages instead of silently changing behavior.
The `indexdb` feature currently uses bundled SQLite through `rusqlite` and must
be documented as reproducible metadata, never the source of truth for repository
state.

## Archive Layout

Release archives use this name format:

```text
rit-<version>-<profile>-<target>.<ext>
```

Examples:

```text
rit-0.1.0-rit-min-x86_64-unknown-linux-gnu.tar.gz
rit-0.1.0-rit-full-x86_64-pc-windows-msvc.zip
```

Archive contents:

```text
rit-<version>-<profile>-<target>/
  rit or rit.exe
  README.md
  LICENSE-MIT
  LICENSE-APACHE
  THIRD-PARTY-NOTICES.md
  docs/
    compatibility.md
    implementation-notes.md
    release.md
```

The binary must live at the archive root after extracting the top-level
directory. Release archives should not include `target/`, `.git/`, test
fixtures, or local configuration files.

Recommended target triples for the first release matrix:

| Target | Archive extension |
| --- | --- |
| `x86_64-unknown-linux-gnu` | `.tar.gz` |
| `x86_64-apple-darwin` | `.tar.gz` |
| `aarch64-apple-darwin` | `.tar.gz` |
| `x86_64-pc-windows-msvc` | `.zip` |

## License And Attribution

The workspace license is `MIT`.

Release archives must include:

- `LICENSE-MIT`
- `THIRD-PARTY-NOTICES.md`

The third-party notice table is generated from `cargo metadata --locked` using
the `rit-full` feature set. A release must not ship with unknown or missing
license metadata in dependency packages without an explicit review note.
