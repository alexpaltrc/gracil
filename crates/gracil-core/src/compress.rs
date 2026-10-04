//! Compressing one video into a temporary file next to it.
//!
//! The source is never touched. The result is a validated temporary file; deciding whether it
//! replaces the original or is saved alongside it belongs to the caller (spec §28).

use std::fs::{self, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

use tracing::{info, warn};

use crate::capabilities::Capabilities;
use crate::encode::{self, EncodeOutcome, JobControl};
use crate::leftovers;
use crate::plan::{self, Attempt, EncodePlan};
use crate::probe::{self, MediaInfo, ProbeError, StreamKind};
use crate::progress::fraction;
use crate::search::{self, Found, Lighter};
use crate::standard::{Resize, Shape};
use crate::toolchain::{self, Resolved, Toolchain, ToolchainError};

/// A ready FFmpeg and what it can do on this computer. Built once per process.
#[derive(Debug, Clone)]
pub struct Engine {
    pub toolchain: Toolchain,
    pub capabilities: Capabilities,
}

impl Engine {
    /// Finds or installs FFmpeg and detects the hardware. Slow on first run; call off the UI thread.
    pub fn start(private_dir: &Path) -> Result<Self, ToolchainError> {
        toolchain::resolve(private_dir).map(Self::with)
    }

    /// An engine for an FFmpeg that has already been found.
    pub fn with(resolved: Resolved) -> Self {
        let capabilities = Capabilities::detect(&resolved.toolchain, &resolved.encoders);
        Self {
            toolchain: resolved.toolchain,
            capabilities,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Update {
    /// `0.0..=1.0`, or `None` when the source length is unknown.
    pub fraction: Option<f64>,
    /// Encoding speed relative to real time.
    pub speed: Option<f64>,
}

#[derive(Debug)]
pub struct Compressed {
    /// The finished, validated file. The caller moves it into place or deletes it.
    pub temp: PathBuf,
    pub size: u64,
    pub attempt: Attempt,
}

#[derive(Debug, thiserror::Error)]
pub enum CompressError {
    #[error(transparent)]
    Probe(#[from] ProbeError),
    #[error("the file has no video")]
    NoVideo,
    #[error("could not create the temporary file")]
    Io(#[from] io::Error),
    #[error("cancelled")]
    Cancelled,
    #[error("FFmpeg could not compress the file: {0}")]
    Failed(String),
    /// Compressing it to look the same would not make it clearly smaller (ADR 0029). Nothing was
    /// encoded; these lighter choices can be offered instead, and may be none.
    #[error("the video is already compressed")]
    AlreadyCompressed(Vec<Lighter>),
}

/// Compresses `source`, trying each [`plan::attempts`] step until one produces a valid file.
pub fn compress(
    engine: &Engine,
    source: &Path,
    control: &JobControl,
    on_update: &mut dyn FnMut(Update),
) -> Result<Compressed, CompressError> {
    compress_with(engine, source, None, Resize::default(), control, on_update)
}

/// Like [`compress`], with the size the "Always" resolution pill asks for (ADR 0032), and a choice
/// the user made among those offered: its size and level are used without searching again.
pub fn compress_with(
    engine: &Engine,
    source: &Path,
    choice: Option<Lighter>,
    resize: Resize,
    control: &JobControl,
    on_update: &mut dyn FnMut(Update),
) -> Result<Compressed, CompressError> {
    let info = probe::probe(&engine.toolchain, source)?;
    if !info.has_video() {
        return Err(CompressError::NoVideo);
    }
    let temp = reserve_temp(source)?;

    let result = run_attempts(
        engine,
        source,
        &temp,
        &info,
        Wanted { choice, resize },
        control,
        on_update,
    );
    if result.is_err()
        && let Err(error) = fs::remove_file(&temp)
    {
        warn!(%error, path = %temp.display(), "could not remove temporary file");
    }
    result
}

/// What the user asked for: a choice among those offered, and the "Always" resolution pill.
struct Wanted {
    choice: Option<Lighter>,
    resize: Resize,
}

fn run_attempts(
    engine: &Engine,
    source: &Path,
    temp: &Path,
    info: &MediaInfo,
    wanted: Wanted,
    control: &JobControl,
    on_update: &mut dyn FnMut(Update),
) -> Result<Compressed, CompressError> {
    let capabilities = engine.capabilities;
    let Wanted { choice, resize } = wanted;
    // "Reduce" makes every video one size smaller, never below 480p.
    let reduced = (resize == Resize::Reduce)
        .then(|| {
            let first = plan::attempts(capabilities).into_iter().next()?;
            let (index, _) =
                EncodePlan::new(source, temp, info, first, capabilities, 0).main_video()?;
            let stream = info.streams.iter().find(|s| s.index == index)?;
            Shape::reduced(stream)
        })
        .flatten();
    let plan_for = |attempt: Attempt, level| {
        let plan = EncodePlan::new(source, temp, info, attempt, capabilities, level);
        match (choice, reduced) {
            (Some(choice), _) => plan.with_shape(choice.shape),
            (None, Some(shape)) => plan.with_shape(shape),
            (None, None) => plan,
        }
    };
    // Resizing frames left on the GPU needs `scale_cuda`; without it those attempts are skipped.
    let attempts: Vec<Attempt> = plan::attempts(capabilities)
        .into_iter()
        .filter(|&attempt| {
            let resizes = || {
                plan_for(attempt, 0)
                    .main_video()
                    .is_some_and(|(_, shape)| shape.size.is_some())
            };
            !attempt.hw_decode || capabilities.scale_cuda || !resizes()
        })
        .collect();

    // Shown as "getting ready" until the first attempt reports progress; Stop works throughout.
    on_update(Update {
        fraction: fraction(Duration::ZERO, info.duration),
        speed: None,
    });
    let found = match (choice, attempts.first()) {
        // The choice was measured when it was offered.
        (Some(_), _) | (None, None) => Found::Fallback,
        (None, Some(&first)) => {
            let fallback = Found::Fallback.level_for(first.encoder);
            search::find_level(
                &engine.toolchain,
                &plan_for(first, fallback),
                info,
                resize,
                control,
            )
        }
    };
    match found {
        Found::Cancelled => return Err(CompressError::Cancelled),
        Found::Lighter(choices) => return Err(CompressError::AlreadyCompressed(choices)),
        Found::Level { .. } | Found::Fallback => {}
    }
    let level_for = |encoder| match choice {
        Some(choice) => choice.level_for(encoder),
        None => found.level_for(encoder),
    };

    let mut last_error = String::new();
    for attempt in attempts {
        if control.is_cancelled() {
            return Err(CompressError::Cancelled);
        }
        let plan = plan_for(attempt, level_for(attempt.encoder));
        info!(?attempt, level = plan.level, dropped = ?plan.dropped, "encoding");
        on_update(Update {
            fraction: fraction(Duration::ZERO, info.duration),
            speed: None,
        });

        let outcome = encode::run(&engine.toolchain, &plan, control, &mut |progress| {
            on_update(Update {
                fraction: fraction(progress.position, info.duration),
                speed: progress.speed,
            });
        });

        let (error, made_progress) = match outcome {
            EncodeOutcome::Cancelled => return Err(CompressError::Cancelled),
            EncodeOutcome::Finished => match validate(&engine.toolchain, temp, info) {
                Ok(size) => {
                    return Ok(Compressed {
                        temp: temp.to_owned(),
                        size,
                        attempt,
                    });
                }
                Err(error) => (error, true),
            },
            EncodeOutcome::Failed {
                error,
                made_progress,
            } => (error, made_progress),
        };
        warn!(?attempt, %error, made_progress, "attempt failed");
        last_error = error;
        // A failure partway through a software decode means the source itself is damaged there;
        // the next attempt would stop at the same place after just as long.
        if made_progress && !attempt.hw_decode {
            break;
        }
    }
    Err(CompressError::Failed(last_error))
}

/// Checks the output really is the whole video, re-encoded to HEVC.
fn validate(toolchain: &Toolchain, output: &Path, source: &MediaInfo) -> Result<u64, String> {
    let size = fs::metadata(output).map_err(|e| e.to_string())?.len();
    if size == 0 {
        return Err("output is empty".into());
    }
    let result = probe::probe(toolchain, output).map_err(|e| format!("output unreadable: {e}"))?;
    let has_hevc = result
        .streams
        .iter()
        .any(|s| s.kind == StreamKind::Video && !s.attached_pic && s.codec == "hevc");
    if !has_hevc {
        return Err("output has no HEVC video".into());
    }
    if let (Some(expected), Some(actual)) = (source.duration, result.duration)
        && !durations_match(expected, actual)
    {
        return Err(format!(
            "output lasts {actual:?}, source lasts {expected:?}"
        ));
    }
    Ok(size)
}

/// Lengths agree within 1% or one second, whichever is larger.
fn durations_match(expected: Duration, actual: Duration) -> bool {
    let tolerance = (expected / 100).max(Duration::from_secs(1));
    expected.abs_diff(actual) <= tolerance
}

/// Creates an empty, uniquely named file next to `source` (same volume, so the final move is a
/// rename) and returns its path. Named `<file name>.gracil-<id>.part`.
fn reserve_temp(source: &Path) -> io::Result<PathBuf> {
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let file_name = source
        .file_name()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "source has no file name"))?;
    loop {
        let id = format!(
            "{:x}{:x}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        );
        let mut name = file_name.to_owned();
        name.push(format!(".gracil-{id}.part"));
        let path = source.with_file_name(name);
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(_) => {
                leftovers::record_temp(&path);
                return Ok(path);
            }
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn duration_tolerance() {
        let secs = Duration::from_secs;
        assert!(durations_match(secs(10), Duration::from_millis(10_900)));
        assert!(!durations_match(secs(10), Duration::from_millis(11_100)));
        assert!(durations_match(secs(1000), secs(1009)));
        assert!(!durations_match(secs(1000), secs(1011)));
    }

    #[test]
    fn temp_files_are_unique_siblings() {
        let dir = std::env::temp_dir().join(format!("gracil-temp-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let source = dir.join("Mi vídeo 🎂.mp4");
        let first = reserve_temp(&source).unwrap();
        let second = reserve_temp(&source).unwrap();
        let names = [&first, &second].map(|p| p.file_name().unwrap().to_str().unwrap().to_owned());
        fs::remove_dir_all(&dir).unwrap();

        assert_ne!(first, second);
        assert_eq!(first.parent(), Some(dir.as_path()));
        for name in names {
            assert!(name.starts_with("Mi vídeo 🎂.mp4.gracil-"), "{name}");
            assert!(name.ends_with(".part"), "{name}");
        }
    }
}
