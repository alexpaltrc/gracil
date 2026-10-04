//! Gracil's compression engine: everything that does not depend on the user interface.

pub mod capabilities;
pub mod compress;
pub mod encode;
pub mod images;
pub mod job;
pub mod leftovers;
pub mod media;
pub mod naming;
pub mod output;
pub mod plan;
pub mod probe;
pub mod progress;
pub mod queue;
pub mod search;
pub mod share;
pub mod standard;
pub mod toolchain;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
