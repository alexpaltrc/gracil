//! Gracil's compression engine: everything that does not depend on the user interface.

pub mod capabilities;
pub mod job;
pub mod naming;
pub mod probe;
pub mod toolchain;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
