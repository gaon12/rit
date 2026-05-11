# Third-Party Notices

This file summarizes third-party Rust crates that may be included in `rit-full`
builds according to `cargo metadata --locked --features
rit-core/large-files,rit-core/semantic-json,rit-core/semantic-rust,rit-core/semantic-typescript,rit-core/semantic-python,rit-core/vfs`.

`rit` itself is licensed under `MIT OR Apache-2.0`.

| Crate | Version | License |
| --- | --- | --- |
| adler2 | 2.0.1 | 0BSD OR MIT OR Apache-2.0 |
| aho-corasick | 1.1.4 | Unlicense OR MIT |
| block-buffer | 0.10.4 | MIT OR Apache-2.0 |
| cc | 1.2.62 | MIT OR Apache-2.0 |
| cfg-if | 1.0.4 | MIT OR Apache-2.0 |
| cpufeatures | 0.2.17 | MIT OR Apache-2.0 |
| crc32fast | 1.5.0 | MIT OR Apache-2.0 |
| crypto-common | 0.1.7 | MIT OR Apache-2.0 |
| digest | 0.10.7 | MIT OR Apache-2.0 |
| equivalent | 1.0.2 | Apache-2.0 OR MIT |
| find-msvc-tools | 0.1.9 | MIT OR Apache-2.0 |
| flate2 | 1.1.9 | MIT OR Apache-2.0 |
| generic-array | 0.14.7 | MIT |
| hashbrown | 0.17.1 | MIT OR Apache-2.0 |
| indexmap | 2.14.0 | Apache-2.0 OR MIT |
| itoa | 1.0.18 | MIT OR Apache-2.0 |
| libc | 0.2.186 | MIT OR Apache-2.0 |
| memchr | 2.8.0 | Unlicense OR MIT |
| miniz_oxide | 0.8.9 | MIT OR Zlib OR Apache-2.0 |
| proc-macro2 | 1.0.106 | MIT OR Apache-2.0 |
| quote | 1.0.45 | MIT OR Apache-2.0 |
| regex | 1.12.3 | MIT OR Apache-2.0 |
| regex-automata | 0.4.14 | MIT OR Apache-2.0 |
| regex-syntax | 0.8.10 | MIT OR Apache-2.0 |
| serde | 1.0.228 | MIT OR Apache-2.0 |
| serde_core | 1.0.228 | MIT OR Apache-2.0 |
| serde_derive | 1.0.228 | MIT OR Apache-2.0 |
| serde_json | 1.0.149 | MIT OR Apache-2.0 |
| serde_spanned | 1.1.1 | MIT OR Apache-2.0 |
| sha2 | 0.10.9 | MIT OR Apache-2.0 |
| shlex | 1.3.0 | MIT OR Apache-2.0 |
| simd-adler32 | 0.3.9 | MIT |
| streaming-iterator | 0.1.9 | MIT OR Apache-2.0 |
| syn | 2.0.117 | MIT OR Apache-2.0 |
| toml | 1.1.2+spec-1.1.0 | MIT OR Apache-2.0 |
| toml_datetime | 1.1.1+spec-1.1.0 | MIT OR Apache-2.0 |
| toml_parser | 1.1.2+spec-1.1.0 | MIT OR Apache-2.0 |
| toml_writer | 1.1.1+spec-1.1.0 | MIT OR Apache-2.0 |
| tree-sitter | 0.26.8 | MIT |
| tree-sitter-language | 0.1.7 | MIT |
| tree-sitter-python | 0.25.0 | MIT |
| tree-sitter-rust | 0.24.2 | MIT |
| tree-sitter-typescript | 0.23.2 | MIT |
| typenum | 1.20.0 | MIT OR Apache-2.0 |
| unicode-ident | 1.0.24 | (MIT OR Apache-2.0) AND Unicode-3.0 |
| version_check | 0.9.5 | MIT/Apache-2.0 |
| winnow | 1.0.2 | MIT |
| zmij | 1.0.21 | MIT |

Before publishing a release, regenerate this table from `Cargo.lock` and review
new licenses against the release policy in `docs/release.md`.
