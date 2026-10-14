//! Local diagnostic logs (spec §70): plain text files in `logs` in Gracil's data folder, one
//! per day, the last week kept. They never leave the computer.

use std::io::{self, Write};
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use tracing::{Level, Metadata};
use tracing_appender::non_blocking::{NonBlocking, WorkerGuard};
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::fmt::MakeWriter;

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
        .with_writer(Capped(writer))
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

/// About the most a day's log takes on the disk (ADR 0064). Past it, one line says so, and only
/// warnings and errors are written, each cut short.
const DAILY_LIMIT: u64 = 20 << 20;

/// How long a line written past the day's limit may be, so FFmpeg's last words, kept with a
/// failure, don't fill the disk.
const SHORT_LINE: usize = 400;

/// The line written once the day's limit is reached.
const LIMIT_REACHED: &[u8] = b"log limit reached for today\n";

/// What today's log took so far: the day, counted from 1970 like the log's own days, the bytes
/// written, and whether [`LIMIT_REACHED`] was written.
static TODAY: Mutex<(u64, u64, bool)> = Mutex::new((0, 0, false));

/// Hands each line to the log, while the day's limit allows.
struct Capped(NonBlocking);

impl<'a> MakeWriter<'a> for Capped {
    type Writer = Line;

    fn make_writer(&'a self) -> Self::Writer {
        Line {
            to: self.0.clone(),
            important: true,
        }
    }

    fn make_writer_for(&'a self, meta: &Metadata<'_>) -> Self::Writer {
        Line {
            to: self.0.clone(),
            important: *meta.level() <= Level::WARN,
        }
    }
}

/// One line on its way to the log.
struct Line {
    to: NonBlocking,
    /// A warning or an error, written even past the day's limit.
    important: bool,
}

impl Write for Line {
    fn write(&mut self, line: &[u8]) -> io::Result<usize> {
        match within_limit(line.len() as u64, self.important) {
            Allowed::Whole => self.to.write_all(line)?,
            Allowed::Short { first } => {
                if first {
                    self.to.write_all(LIMIT_REACHED)?;
                }
                if self.important {
                    self.to.write_all(&shortened(line))?;
                }
            }
        }
        Ok(line.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.to.flush()
    }
}

/// What may be written of one line.
#[derive(Debug, PartialEq, Eq)]
enum Allowed {
    Whole,
    /// Past the limit: only an important line, cut short; `first` the first time today.
    Short {
        first: bool,
    },
}

/// Counts `bytes` against today's limit.
fn within_limit(bytes: u64, important: bool) -> Allowed {
    let day = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_secs() / (24 * 60 * 60));
    let mut today = TODAY
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    count(&mut today, day, bytes, important)
}

fn count(today: &mut (u64, u64, bool), day: u64, bytes: u64, important: bool) -> Allowed {
    if today.0 != day {
        *today = (day, 0, false);
    }
    if !today.2 && today.1.saturating_add(bytes) <= DAILY_LIMIT {
        today.1 += bytes;
        return Allowed::Whole;
    }
    let first = !today.2;
    today.2 = true;
    if important {
        today.1 = today.1.saturating_add(bytes.min(SHORT_LINE as u64));
    }
    Allowed::Short { first }
}

/// `line` cut to [`SHORT_LINE`] bytes, never in the middle of a character, ending its line.
fn shortened(line: &[u8]) -> Vec<u8> {
    if line.len() <= SHORT_LINE {
        return line.to_vec();
    }
    let mut end = SHORT_LINE;
    // A byte that continues a character starts with the bits 10.
    while end > 0 && line[end] & 0xC0 == 0x80 {
        end -= 1;
    }
    let mut short = line[..end].to_vec();
    short.extend_from_slice("…\n".as_bytes());
    short
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn past_the_daily_limit_only_warnings_are_written_short() {
        let mut today = (7, DAILY_LIMIT - 10, false);
        assert_eq!(count(&mut today, 7, 10, false), Allowed::Whole);
        assert_eq!(
            count(&mut today, 7, 1, false),
            Allowed::Short { first: true }
        );
        assert_eq!(
            count(&mut today, 7, 1, true),
            Allowed::Short { first: false }
        );
        // A new day starts afresh.
        assert_eq!(count(&mut today, 8, 100, false), Allowed::Whole);
        assert_eq!(today, (8, 100, false));
    }

    #[test]
    fn a_long_line_is_cut_short_between_characters() {
        let line = "é".repeat(SHORT_LINE);
        let short = shortened(line.as_bytes());
        let short = std::str::from_utf8(&short).unwrap();
        assert!(short.ends_with("…\n"));
        assert!(short.len() <= SHORT_LINE + "…\n".len());
        assert_eq!(shortened(b"short\n"), b"short\n");
    }
}
