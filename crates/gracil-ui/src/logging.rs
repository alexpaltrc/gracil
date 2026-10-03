//! Local diagnostic logs (spec §70): plain text files in `%LOCALAPPDATA%\Gracil\logs`, one per
//! day, the last week kept. They never leave the computer.

use std::sync::Mutex;

use tracing::Level;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_appender::rolling::{RollingFileAppender, Rotation};

const KEPT_DAYS: usize = 7;

/// Holds the log writer open; [`finish`] releases it so the last lines reach the file.
static WRITER: Mutex<Option<WorkerGuard>> = Mutex::new(None);

/// Starts logging. Logging is a convenience: if the folder can't be created, Gracil simply runs
/// without it.
pub fn init() {
    let Some(dir) = gracil_core::toolchain::default_private_dir() else {
        return;
    };
    let Ok(appender) = RollingFileAppender::builder()
        .rotation(Rotation::DAILY)
        .filename_prefix("gracil")
        .filename_suffix("log")
        .max_log_files(KEPT_DAYS)
        .build(dir.join("logs"))
    else {
        return;
    };
    let (writer, guard) = tracing_appender::non_blocking(appender);
    let started = tracing_subscriber::fmt()
        .with_writer(writer)
        .with_ansi(false)
        .with_max_level(Level::INFO)
        .try_init();
    if started.is_err() {
        return;
    }
    *WRITER
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(guard);
    // There is no console to print a crash to, so it goes to the log.
    std::panic::set_hook(Box::new(
        |info| tracing::error!(%info, "Gracil stopped unexpectedly"),
    ));
}

/// Writes out what is still waiting. Call it just before the process ends.
pub fn finish() {
    let guard = WRITER
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .take();
    drop(guard);
}
