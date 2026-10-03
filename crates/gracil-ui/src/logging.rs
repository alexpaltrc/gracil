//! Local diagnostic logs (spec §70): plain text files in `%LOCALAPPDATA%\Gracil\logs`, one per
//! day, the last week kept. They never leave the computer.

use tracing::Level;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_appender::rolling::{RollingFileAppender, Rotation};

const KEPT_DAYS: usize = 7;

/// Starts logging. Keep the guard alive until exit so the last lines reach the file.
/// Logging is a convenience: if the folder can't be created, Gracil simply runs without it.
pub fn init() -> Option<WorkerGuard> {
    let dir = gracil_core::toolchain::default_private_dir()?.join("logs");
    let appender = RollingFileAppender::builder()
        .rotation(Rotation::DAILY)
        .filename_prefix("gracil")
        .filename_suffix("log")
        .max_log_files(KEPT_DAYS)
        .build(dir)
        .ok()?;
    let (writer, guard) = tracing_appender::non_blocking(appender);
    tracing_subscriber::fmt()
        .with_writer(writer)
        .with_ansi(false)
        .with_max_level(Level::INFO)
        .try_init()
        .ok()?;
    Some(guard)
}
