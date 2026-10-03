//! Gracil's compression engine: everything that does not depend on the user interface.

pub mod capabilities;
pub mod compress;
pub mod encode;
pub mod job;
pub mod naming;
pub mod plan;
pub mod probe;
pub mod progress;
pub mod toolchain;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
