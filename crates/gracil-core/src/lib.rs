//! Gracil compression engine.
//!
//! This crate must never depend on a UI framework: background compression started from File
//! Explorer runs it without creating any window. See `docs/decisions/0001-core-independent-from-ui.md`.

pub mod job;
pub mod naming;

/// The Gracil version, taken from the workspace manifest.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
