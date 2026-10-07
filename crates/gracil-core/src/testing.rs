//! Helpers shared by the tests of every Gracil crate. Only built for tests: here, and in the other
//! crates through the `test-support` feature, which they turn on for their tests alone.

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};

/// A new, empty folder for one test, in the system's temporary folder. `name` shows in the
/// folder's name, so one a failed test left behind is easy to place; every call gets its own
/// folder, even with the same name.
#[allow(clippy::unwrap_used)]
pub fn fresh_dir(name: &str) -> PathBuf {
    static MADE: AtomicU32 = AtomicU32::new(0);
    let dir = std::env::temp_dir().join(format!(
        "gracil-test-{name}-{}-{}",
        std::process::id(),
        MADE.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}
