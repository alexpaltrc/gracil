//! Local diagnostic logs (spec §70): plain text files in `logs` in Gracil's data folder, one
//! per day, the last week kept. They never leave the computer.

use std::sync::Mutex;
use std::time::{Duration, Instant};

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
    listen_to_libraries();
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

/// The same warning from the drawing library is written at most this often; the times in
/// between are counted instead.
const REPEAT_EVERY: Duration = Duration::from_secs(60);

/// Passes the warnings and errors of the libraries that draw the window, which report through
/// `log` rather than `tracing`, into Gracil's log. Without it, a window that stops drawing leaves
/// no trace of why.
struct Libraries {
    /// The last message written, when, and how many times it came again since.
    last: Mutex<Option<(String, Instant, u64)>>,
}

impl log::Log for Libraries {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        metadata.level() <= log::Level::Warn
    }

    fn log(&self, record: &log::Record) {
        if !self.enabled(record.metadata()) {
            return;
        }
        let message = record.args().to_string();
        let mut last = self
            .last
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let repeated = match last.as_mut() {
            Some((previous, since, times)) if *previous == message => {
                if since.elapsed() < REPEAT_EVERY {
                    *times += 1;
                    return;
                }
                *since = Instant::now();
                std::mem::take(times)
            }
            _ => {
                *last = Some((message.clone(), Instant::now(), 0));
                0
            }
        };
        let from = record.target();
        if record.level() == log::Level::Error {
            tracing::error!(from, repeated, "{message}");
        } else {
            tracing::warn!(from, repeated, "{message}");
        }
    }

    fn flush(&self) {}
}

static LIBRARIES: Libraries = Libraries {
    last: Mutex::new(None),
};

/// Starts passing the drawing libraries' warnings into the log.
fn listen_to_libraries() {
    if log::set_logger(&LIBRARIES).is_ok() {
        log::set_max_level(log::LevelFilter::Warn);
    }
}
