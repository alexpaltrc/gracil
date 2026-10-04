//! Background work: starting the engine, compressing and placing results, each on its own thread
//! so the window never waits on FFmpeg or the disk.

use std::ffi::OsString;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::thread;

use gracil_core::compress::{CompressError, Compressed, Engine, compress};
use gracil_core::encode::JobControl;
use gracil_core::images::{self, ImageError};
use gracil_core::leftovers;
use gracil_core::media::{self, Kind};
use gracil_core::output::{replace_original, save_as_new};
use gracil_core::plan::output_extension;
use gracil_core::toolchain;
use iced::Task;
use iced::futures::SinkExt;
use iced::futures::channel::mpsc::{self, Sender};
use iced::futures::executor::block_on;
use tracing::{error, info, warn};

/// Why a file could not be compressed or placed, in the terms the user sees (worded in
/// [`crate::text`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Failure {
    NoVideo,
    Unreadable,
    CouldNotCompress,
    NotSmaller,
    /// Already compressed, and not even compressing it again would make it clearly smaller (ADR
    /// 0029, ADR 0033).
    AsSmallAsItGets,
    CouldNotSave,
    NameTaken,
    /// An animated image (ADR 0034, item 1).
    Animated,
    /// GIF, camera RAW or JPEG XL (ADR 0034, item 1).
    NotAccepted,
    /// A photo stored as true HDR (ADR 0034, item 11).
    Hdr,
    /// jpegli is missing from this copy of Gracil.
    NoEncoder,
}

/// What the user chooses to do with a finished result (spec §37.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Choice {
    Replace,
    SaveCopy,
}

#[derive(Debug, Clone)]
pub enum Outcome {
    /// Compressed and validated, waiting in `temp` for the user to choose.
    Ready {
        temp: PathBuf,
        /// The extension the result is saved with: `mp4`, or `jpg` or `png` for an image.
        extension: OsString,
        smaller_by: f64,
        placing: bool,
        /// Why the last attempt to place it failed, if it did.
        problem: Option<Failure>,
    },
    Placed {
        choice: Choice,
        path: PathBuf,
        smaller_by: f64,
    },
    Failed(Failure),
    Cancelled,
}

impl Outcome {
    /// Compressed, and waiting for the user to choose replace or copy.
    pub fn is_waiting(&self) -> bool {
        matches!(self, Self::Ready { placing: false, .. })
    }
}

#[derive(Debug, Clone)]
pub enum Report {
    Progress(Option<f64>),
    Done(Outcome),
}

/// Finds or installs FFmpeg and detects the hardware.
pub fn start_engine() -> Task<Option<Arc<Engine>>> {
    on_thread(|mut sender: Sender<Option<Arc<Engine>>>| {
        let engine = toolchain::default_private_dir().and_then(|dir| {
            tidy_leftovers(&dir);
            Engine::start(&dir)
                .inspect_err(|e| error!(error = %e, "engine unavailable"))
                .ok()
        });
        let _ = block_on(sender.send(engine.map(Arc::new)));
    })
}

/// Starts noting this run's temporary files, and settles those an earlier run left behind
/// (ADR 0017). Runs before any compression can start.
fn tidy_leftovers(private_dir: &Path) {
    if let Err(e) = leftovers::track(private_dir) {
        warn!(error = %e, "temporary files will not be tracked");
    }
    let swept = leftovers::sweep(private_dir);
    if swept != leftovers::Swept::default() {
        info!(?swept, "files left by an earlier run");
    }
}

/// Compresses `source`, a video or an image, leaving the result beside it for the user to place.
pub fn compress_file(engine: Arc<Engine>, source: PathBuf, control: JobControl) -> Task<Report> {
    on_thread(move |mut sender: Sender<Report>| {
        let mut progress = sender.clone();
        let mut on_update = |update: gracil_core::compress::Update| {
            // Progress is best effort: when the window is busy, a skipped update costs nothing.
            let _ = progress.try_send(Report::Progress(update.fraction));
        };
        let outcome = match media::kind_of(&source) {
            Kind::Video => video_outcome(
                compress(&engine, &source, &control, &mut on_update),
                &source,
            ),
            Kind::Image => image_outcome(
                images::compress(&engine, &source, &control, &mut on_update),
                &source,
            ),
        };
        let _ = block_on(sender.send(Report::Done(outcome)));
    })
}

fn video_outcome(result: Result<Compressed, CompressError>, source: &Path) -> Outcome {
    match result {
        Ok(compressed) => {
            info!(?compressed.attempt, "compressed");
            ready(
                compressed.temp,
                compressed.size,
                output_extension(source),
                source,
            )
        }
        Err(CompressError::Cancelled) => Outcome::Cancelled,
        Err(CompressError::AsSmallAsItGets) => {
            info!("already as small as it gets");
            Outcome::Failed(Failure::AsSmallAsItGets)
        }
        Err(e) => {
            warn!(error = %e, "compression failed");
            Outcome::Failed(match e {
                CompressError::NoVideo => Failure::NoVideo,
                CompressError::Probe(_) => Failure::Unreadable,
                CompressError::Io(_) => Failure::CouldNotSave,
                CompressError::Failed(_)
                | CompressError::Cancelled
                | CompressError::AsSmallAsItGets => Failure::CouldNotCompress,
            })
        }
    }
}

fn image_outcome(result: Result<images::CompressedImage, ImageError>, source: &Path) -> Outcome {
    match result {
        Ok(compressed) => ready(
            compressed.temp,
            compressed.size,
            compressed.extension,
            source,
        ),
        Err(ImageError::Cancelled) => Outcome::Cancelled,
        Err(ImageError::AsSmallAsItGets) => {
            info!("image already as small as it gets");
            Outcome::Failed(Failure::AsSmallAsItGets)
        }
        Err(e) => {
            warn!(error = %e, "image compression failed");
            Outcome::Failed(match e {
                ImageError::Unreadable(_) => Failure::Unreadable,
                ImageError::Animated => Failure::Animated,
                ImageError::NotAccepted => Failure::NotAccepted,
                ImageError::Hdr => Failure::Hdr,
                ImageError::NoEncoder => Failure::NoEncoder,
                ImageError::Io(_) => Failure::CouldNotSave,
                ImageError::Failed(_) | ImageError::Cancelled | ImageError::AsSmallAsItGets => {
                    Failure::CouldNotCompress
                }
            })
        }
    }
}

/// Keeps the result for the user, unless it is not smaller (spec §40.1).
fn ready(temp: PathBuf, size: u64, extension: OsString, source: &Path) -> Outcome {
    let original = fs::metadata(source).map_or(0, |metadata| metadata.len());
    info!(original, compressed = size, "ready");
    if size >= original {
        discard(&temp);
        return Outcome::Failed(Failure::NotSmaller);
    }
    Outcome::Ready {
        temp,
        extension,
        smaller_by: 1.0 - size as f64 / original as f64,
        placing: false,
        problem: None,
    }
}

/// Puts a result in place of the original or beside it, saved with `extension`. Returns where it
/// ended up.
pub fn place(
    temp: PathBuf,
    source: PathBuf,
    extension: OsString,
    choice: Choice,
) -> Task<Result<PathBuf, Failure>> {
    on_thread(move |mut sender: Sender<Result<PathBuf, Failure>>| {
        let placed = match choice {
            Choice::Replace => {
                replace_original(&temp, &source, &extension, gracil_windows::replace_file)
            }
            Choice::SaveCopy => save_as_new(&temp, &source, &extension),
        }
        .map_err(|e| {
            warn!(error = %e, ?choice, "could not place result");
            match e.kind() {
                io::ErrorKind::AlreadyExists => Failure::NameTaken,
                _ => Failure::CouldNotSave,
            }
        });
        let _ = block_on(sender.send(placed));
    })
}

/// Deletes a result the user did not keep. The original is never involved.
pub fn discard(temp: &Path) {
    if let Err(error) = fs::remove_file(temp) {
        warn!(%error, ?temp, "could not remove an unused result");
    }
}

/// Runs `work` on its own thread; whatever it sends becomes the task's output.
fn on_thread<T: Send + 'static>(work: impl FnOnce(Sender<T>) + Send + 'static) -> Task<T> {
    let (sender, receiver) = mpsc::channel(1);
    thread::spawn(move || work(sender));
    Task::stream(receiver)
}
