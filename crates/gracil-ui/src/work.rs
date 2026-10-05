//! Background work: starting the engine, compressing and placing results, each on its own thread
//! so the window never waits on FFmpeg or the disk.

use std::ffi::OsString;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::thread;

use gracil_core::compress::{self, CompressError, Compressed, Engine, PreparedVideo};
use gracil_core::encode::JobControl;
use gracil_core::images::{self, ImageError, PreparedImage};
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
    /// The drive is too full to write the compressed file and still leave room (ADR 0049).
    NoRoom,
    /// An image too large for this computer's memory (ADR 0049).
    TooLarge,
}

/// What the user chooses to do with a finished result (spec §37.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Choice {
    Replace,
    SaveCopy,
}

/// How big a file was, and how big its compressed version is, in bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Sizes {
    pub original: u64,
    pub compressed: u64,
}

impl Sizes {
    /// How much smaller, as a share of the original from 0 to 1.
    pub fn smaller_by(self) -> f64 {
        if self.original == 0 {
            return 0.0;
        }
        1.0 - self.compressed as f64 / self.original as f64
    }
}

impl std::ops::Add for Sizes {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        Self {
            original: self.original + other.original,
            compressed: self.compressed + other.compressed,
        }
    }
}

#[derive(Debug, Clone)]
pub enum Outcome {
    /// Compressed and validated, waiting in `temp` for the user to choose.
    Ready {
        temp: PathBuf,
        /// The extension the result is saved with: `mp4`, or `jpg` or `png` for an image.
        extension: OsString,
        sizes: Sizes,
        placing: bool,
        /// Why the last attempt to place it failed, if it did.
        problem: Option<Failure>,
    },
    Placed {
        choice: Choice,
        path: PathBuf,
        sizes: Sizes,
    },
    Failed(Failure),
    Cancelled,
}

impl Outcome {
    /// Compressed, and waiting for the user to choose replace or copy.
    pub fn is_waiting(&self) -> bool {
        matches!(self, Self::Ready { placing: false, .. })
    }

    /// The sizes before and after, for a file that came out smaller.
    pub fn sizes(&self) -> Option<Sizes> {
        match self {
            Self::Ready { sizes, .. } | Self::Placed { sizes, .. } => Some(*sizes),
            Self::Failed(_) | Self::Cancelled => None,
        }
    }
}

#[derive(Debug, Clone)]
pub enum Report {
    /// How far compressing has gone.
    Progress(Option<f64>),
    /// Getting ready is done; the file waits its turn to compress (ADR 0040).
    Prepared(Prepared),
    Done(Outcome),
}

/// A file got ready, handed from the thread that got it ready to the one that compresses it.
#[derive(Debug, Clone)]
pub struct Prepared(Arc<Mutex<Option<Stage>>>);

#[derive(Debug)]
enum Stage {
    Video(PreparedVideo),
    Image(PreparedImage),
}

impl Prepared {
    fn new(stage: Stage) -> Self {
        Self(Arc::new(Mutex::new(Some(stage))))
    }

    /// For an image, how many pixels it has, when its header told.
    pub fn pixels(&self) -> Option<u64> {
        match self.0.lock().ok()?.as_ref()? {
            Stage::Image(image) => image.pixels(),
            Stage::Video(_) => None,
        }
    }

    fn take(&self) -> Option<Stage> {
        self.0.lock().ok()?.take()
    }
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

/// What looking through File Explorer's files and folders turns up (ADR 0037).
#[derive(Debug, Clone)]
pub enum Found {
    /// The files of one folder, or the files sent by themselves.
    Files(Vec<PathBuf>),
    /// Everything has been looked through.
    Done,
}

/// Looks through `paths` away from the window, which a slow folder (on a network or cloud drive)
/// would otherwise freeze, handing over each folder's files as soon as they are known.
pub fn gather(paths: Vec<PathBuf>, only: Option<Kind>) -> Task<Found> {
    on_thread(move |mut sender: Sender<Found>| {
        media::gather_each(paths, only, &mut |files| {
            let _ = block_on(sender.send(Found::Files(files)));
        });
        let _ = block_on(sender.send(Found::Done));
    })
}

/// Gets `source` ready, the first stage of compressing it (ADR 0040): for a video, the short test
/// samples that choose its compression; for an image, reading it. Reports
/// [`Report::Prepared`], or [`Report::Done`] when it can go no further.
pub fn prepare_file(engine: Arc<Engine>, source: PathBuf, control: JobControl) -> Task<Report> {
    on_thread(move |mut sender: Sender<Report>| {
        let report = match media::kind_of(&source) {
            Kind::Video => match compress::prepare(&engine, &source, &control, &mut |_| {}) {
                Ok(video) => Report::Prepared(Prepared::new(Stage::Video(video))),
                Err(error) => Report::Done(video_outcome(Err(error), &source)),
            },
            Kind::Image => match images::prepare(&source) {
                Ok(image) => Report::Prepared(Prepared::new(Stage::Image(image))),
                Err(error) => Report::Done(image_outcome(Err(error), &source)),
            },
        };
        let _ = block_on(sender.send(report));
    })
}

/// Compresses a file [`prepare_file`] got ready, leaving the result beside it for the user to
/// place.
pub fn compress_file(
    engine: Arc<Engine>,
    source: PathBuf,
    prepared: Prepared,
    control: JobControl,
) -> Task<Report> {
    on_thread(move |mut sender: Sender<Report>| {
        let mut progress = sender.clone();
        let mut on_update = |update: gracil_core::compress::Update| {
            // Progress is best effort: when the window is busy, a skipped update costs nothing.
            let _ = progress.try_send(Report::Progress(update.fraction));
        };
        let stage = prepared.take();
        let outcome = match stage {
            _ if !room_on_drive(&source) => Outcome::Failed(Failure::NoRoom),
            Some(Stage::Image(image)) if too_large(&image) => Outcome::Failed(Failure::TooLarge),
            Some(Stage::Video(video)) => video_outcome(
                compress::compress_prepared(&engine, &source, video, &control, &mut on_update),
                &source,
            ),
            Some(Stage::Image(image)) => image_outcome(
                images::compress_prepared(&engine, &source, image, &control, &mut on_update),
                &source,
            ),
            None => Outcome::Failed(Failure::CouldNotCompress),
        };
        let _ = block_on(sender.send(Report::Done(outcome)));
    })
}

/// Room left free on a drive after the compressed file is written, so Windows and other programs
/// never find it full (ADR 0049).
const ROOM_TO_SPARE: u64 = 2 << 30;

/// Whether the drive holding `source` has room for its compressed version, which is never larger
/// than the original, and [`ROOM_TO_SPARE`] besides. A drive that doesn't say has room.
fn room_on_drive(source: &Path) -> bool {
    let (Some(folder), Ok(metadata)) = (source.parent(), fs::metadata(source)) else {
        return true;
    };
    let room = gracil_windows::free_space(folder)
        .is_none_or(|free| free >= metadata.len().saturating_add(ROOM_TO_SPARE));
    if !room {
        warn!("the drive is too full to compress onto");
    }
    room
}

/// Whether `image` needs more than half of all the computer's memory to compress, which would
/// leave too little for everything else (ADR 0049).
fn too_large(image: &PreparedImage) -> bool {
    let need = crate::pace::image_memory(image.pixels());
    let large = gracil_windows::total_memory().is_some_and(|total| need > total / 2);
    if large {
        warn!(need, "the image is too large for this computer's memory");
    }
    large
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
        sizes: Sizes {
            original,
            compressed: size,
        },
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
