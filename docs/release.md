# Release Packaging

This document defines the release packaging shape for `rit`.

`rit` ships as a single `rit` binary. The release names below describe build
profiles, not separate source trees.

## Feature Matrix

| Build | Cargo command | Included feature intent | Excluded by default |
| --- | --- | --- | --- |
| `rit-min` | `cargo build -p rit-cli --release --locked` | Core CLI, local repository operations, plain HTTP transport, policy/doctor/repair models, fallback VFS model | LFS, Xet, semantic tree-sitter adapters, semantic JSON, platform VFS |
| `rit-full` | `cargo build -p rit-cli --release --locked --features rit-core/large-files,rit-core/semantic-json,rit-core/semantic-rust,rit-core/semantic-typescript,rit-core/semantic-python,rit-core/vfs` | Everything in `rit-min` plus LFS/Xet models, semantic JSON, Rust/TypeScript/Python semantic adapters, VFS planning APIs | Platform VFS drivers and network/promisor VFS fetching remain planned, not implemented |

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

Release builds must keep working with no feature flags. Feature-gated code must
return clear unsupported-feature messages instead of silently changing behavior.
