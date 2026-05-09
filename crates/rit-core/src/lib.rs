//! Core library for `rit`.
//!
//! This crate owns Git-compatible data models and repository operations. The
//! CLI crate should format these structured results instead of embedding Git
//! behavior directly in argument handling.

pub mod error;

pub use error::{Result, RitError};

/// Returns the crate version used by the CLI and tests.
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}
