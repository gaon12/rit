//! Core library for `rit`.
//!
//! This crate owns Git-compatible data models and repository operations. The
//! CLI crate should format these structured results instead of embedding Git
//! behavior directly in argument handling.

pub mod error;
pub mod object;
pub mod odb;
pub mod repository;

pub use error::{Result, RitError};
pub use object::{GitObject, ObjectId, ObjectKind, TreeEntry, hash_object};
pub use odb::LooseObjectDb;
pub use repository::{InitOptions, Repository};

/// Returns the crate version used by the CLI and tests.
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}
