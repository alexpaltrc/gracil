//! Background work: starting the engine, compressing and placing results, each on its own thread
//! so the window never waits on FFmpeg or the disk.

use std::ffi::OsString;
use std::fs;
use std::io;
use std::panic::{self, AssertUnwindSafe};
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex, OnceLock, PoisonError};
use std::thread;
use std::time::{Duration, SystemTime};

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
    /// A finished result couldn't be put in place; choosing again may work.
    CouldNotPlace,
    NameTaken,
    /// Another program holds the file open.
    InUse,
    /// The file is no longer where it was: deleted, moved, or on a drive that was unplugged.
    Gone,
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
    /// The original changed while it was being compressed, so the result was let go (ADR 0064).
    Changed,
    /// The original is marked read-only, so it can't be replaced; a copy still can be saved (ADR
    /// 0064).
    ReadOnly,
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

impl std::ops::Sub for Sizes {
    type Output = Self;

    /// Takes off sizes added before.
    fn sub(self, other: Self) -> Self {
        Self {
            original: self.original.saturating_sub(other.original),
            compressed: self.compressed.saturating_sub(other.compressed),
        }
    }
}

/// What the original was like when Gracil started on it, to tell whether it changed meanwhile
/// (ADR 0064).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Snapshot {
    len: u64,
    modified: Option<SystemTime>,
    /// Kept only online by a cloud drive before Gracil read it, so it is let go back there once
    /// Gracil is done with it.
    online_only: bool,
}

impl Snapshot {
    /// How `source` is now, if it can be told.
    fn of(source: &Path) -> Option<Self> {
        let metadata = fs::metadata(source).ok()?;
        Some(Self {
            len: metadata.len(),
            modified: metadata.modified().ok(),
            online_only: online_only(&metadata),
        })
    }

    /// Whether `source` is still the file this snapshot was taken of: same size, same time it
    /// was last changed.
    fn unchanged(self, source: &Path) -> bool {
        Self::of(source).is_some_and(|now| now.len == self.len && now.modified == self.modified)
    }
}

/// Whether a cloud drive keeps the file only online, downloading it when it is read.
fn online_only(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    const OFFLINE: u32 = 0x1000;
    const RECALL_ON_OPEN: u32 = 0x4_0000;
    const RECALL_ON_DATA_ACCESS: u32 = 0x40_0000;
    metadata.file_attributes() & (OFFLINE | RECALL_ON_OPEN | RECALL_ON_DATA_ACCESS) != 0
}

/// Lets the cloud drive free up `path` again if it was kept only online, best effort.
fn give_back(path: &Path, original: Snapshot) {
    if original.online_only
        && let Err(error) = gracil_windows::unpin(path)
    {
        warn!(%error, "could not let the cloud drive free up a file again");
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
        /// The original as it was when Gracil started on it.
        original: Snapshot,
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
    /// The file is kept only online and is being downloaded to be got ready (`true`, ADR 0066),
    /// or it is downloaded now (`false`); otherwise it counts as downloading until it is ready.
    Downloading(bool),
    /// Getting ready is done; the file waits its turn to compress (ADR 0040).
    Prepared(Prepared),
    Done(Outcome),
}

/// What background work reports when it stops unexpectedly.
fn report_failure() -> Report {
    Report::Done(Outcome::Failed(Failure::CouldNotCompress))
}

/// A file got ready, handed from the thread that got it ready to the one that compresses it.
#[derive(Debug, Clone)]
pub struct Prepared {
    held: Arc<Held>,
    original: Snapshot,
}

/// What a file got ready holds, shared by every copy of its [`Prepared`].
#[derive(Debug)]
struct Held {
    stage: Mutex<Option<Stage>>,
    source: PathBuf,
    original: Snapshot,
}

impl Drop for Held {
    /// A file let go before it compressed (cleared, removed or stopped) is given back to its cloud
    /// drive, if it was kept only online, as one done with any other way is (ADR 0064). Away
    /// from the window, which a slow drive would otherwise freeze.
    fn drop(&mut self) {
        let never_compressed = self
            .stage
            .get_mut()
            .unwrap_or_else(PoisonError::into_inner)
            .is_some();
        if never_compressed && self.original.online_only {
            let (source, original) = (std::mem::take(&mut self.source), self.original);
            let errand: Errand = Box::new(move || give_back(&source, original));
            if let Err(errand) = CLEANER.run(errand) {
                errand();
            }
        }
    }
}

#[derive(Debug)]
enum Stage {
    Video(PreparedVideo),
    Image(PreparedImage),
}

impl Prepared {
    fn new(stage: Stage, source: &Path, original: Snapshot) -> Self {
        Self {
            held: Arc::new(Held {
                stage: Mutex::new(Some(stage)),
                source: source.to_path_buf(),
                original,
            }),
            original,
        }
    }

    /// For an image, how many pixels it has, when its header told.
    pub fn pixels(&self) -> Option<u64> {
        match self.held.stage.lock().ok()?.as_ref()? {
            Stage::Image(image) => image.pixels(),
            Stage::Video(_) => None,
        }
    }

    /// For an image, how many bytes of it are held in memory, waiting their turn.
    pub fn bytes(&self) -> u64 {
        let Ok(stage) = self.held.stage.lock() else {
            return 0;
        };
        match stage.as_ref() {
            Some(Stage::Image(image)) => image.bytes(),
            Some(Stage::Video(_)) | None => 0,
        }
    }

    fn take(&self) -> Option<Stage> {
        self.held.stage.lock().ok()?.take()
    }
}

/// Finds FFmpeg and detects the hardware.
pub fn start_engine() -> Task<Option<Arc<Engine>>> {
    on_thread(
        || None,
        |mut sender: Sender<Option<Arc<Engine>>>| {
            match toolchain::default_private_dir() {
                Some(dir) => tidy_leftovers(dir),
                None => swept(),
            }
            let engine = Engine::start();
            if engine.is_none() {
                error!("no usable FFmpeg; Gracil's ffmpeg folder is missing or broken");
            }
            let _ = block_on(sender.send(engine.map(Arc::new)));
        },
    )
}

/// Starts noting this run's temporary files before any compression can start, then settles
/// those an earlier run left behind on a thread of its own, so a slow or unplugged drive never
/// holds up the start (ADR 0017, ADR 0064).
fn tidy_leftovers(private_dir: PathBuf) {
    if let Err(e) = leftovers::track(&private_dir) {
        warn!(error = %e, "temporary files will not be tracked");
    }
    let sweeping = thread::Builder::new()
        .name("gracil-sweep".into())
        .spawn(move || {
            let result = leftovers::sweep(&private_dir);
            if result != leftovers::Swept::default() {
                info!(swept = ?result, "files left by an earlier run");
            }
            swept();
        });
    if let Err(error) = sweeping {
        warn!(%error, "files left by an earlier run stay until the next start");
        swept();
    }
}

/// Whether the sweep of what an earlier run left is over, or won't happen.
static SWEPT: (Mutex<bool>, Condvar) = (Mutex::new(false), Condvar::new());

/// The longest looking through files waits for the sweep.
const SWEEP_PATIENCE: Duration = Duration::from_secs(10);

/// Notes that the sweep is over.
fn swept() {
    let (done, news) = &SWEPT;
    *done.lock().unwrap_or_else(PoisonError::into_inner) = true;
    news.notify_all();
}

/// Waits for the sweep, at most [`SWEEP_PATIENCE`], when one of `paths` isn't there: the sweep
/// may be about to put an original back where it was, as for a file left unfinished while it was
/// being replaced (ADR 0017). Files that are there never wait, nor does anything once the sweep
/// is over.
fn wait_for_sweep_if_missing(paths: &[PathBuf]) {
    let (done, news) = &SWEPT;
    let is_done = || *done.lock().unwrap_or_else(PoisonError::into_inner);
    if is_done() || paths.iter().all(|path| path.exists()) {
        return;
    }
    let done = done.lock().unwrap_or_else(PoisonError::into_inner);
    let _ = news
        .wait_timeout_while(done, SWEEP_PATIENCE, |done| !*done)
        .map_err(PoisonError::into_inner);
}

/// What looking through File Explorer's files and folders turns up (ADR 0037).
#[derive(Debug, Clone)]
pub enum Found {
    /// The files of one folder, or the files sent by themselves, each with its size in bytes
    /// (ADR 0069).
    Files(Vec<(PathBuf, u64)>),
    /// Everything has been looked through.
    Done,
}

/// Looks through `paths` away from the window, which a slow folder (on a network or cloud drive)
/// would otherwise freeze, handing over each folder's files as soon as they are known.
pub fn gather(paths: Vec<PathBuf>, only: Option<Kind>) -> Task<Found> {
    on_thread(
        || Found::Done,
        move |mut sender: Sender<Found>| {
            wait_for_sweep_if_missing(&paths);
            media::gather_each(paths, only, &mut |files| {
                let _ = block_on(sender.send(Found::Files(files)));
            });
            let _ = block_on(sender.send(Found::Done));
        },
    )
}

/// Gets `source` ready, the first stage of compressing it (ADR 0040): for a video, the short test
/// samples that choose its compression; for an image, reading it. Reports
/// [`Report::Prepared`], or [`Report::Done`] when it can go no further.
///
/// First it makes sure the folder takes new files, once per folder, so nothing is compressed only
/// to find it can't be saved; and a file kept only online needs room to be downloaded as well as
/// room for its result (ADR 0064).
pub fn prepare_file(engine: Arc<Engine>, source: PathBuf, control: JobControl) -> Task<Report> {
    on_thread(report_failure, move |mut sender: Sender<Report>| {
        // Said first, so the window says why getting it ready waits on the connection.
        if Snapshot::of(&source).is_some_and(|original| original.online_only) {
            let _ = block_on(sender.send(Report::Downloading(true)));
        }
        let mut downloaded = sender.clone();
        let report = prepare(&engine, &source, &control, &mut || {
            let _ = block_on(downloaded.send(Report::Downloading(false)));
        });
        let _ = block_on(sender.send(report));
    })
}

/// `downloaded` is called once a file kept only online is on the computer.
fn prepare(
    engine: &Engine,
    source: &Path,
    control: &JobControl,
    downloaded: &mut dyn FnMut(),
) -> Report {
    let Some(original) = Snapshot::of(source) else {
        warn!("the file is no longer there");
        return Report::Done(Outcome::Failed(Failure::Gone));
    };
    if !folder_takes_files(source) {
        return Report::Done(Outcome::Failed(Failure::CouldNotSave));
    }
    // Held while it downloads, so files downloading at once onto the same drive each count the
    // room the others take.
    let download = if original.online_only {
        match reserve_room(source, original.len, original.len) {
            Some(room) => Some(room),
            None => return Report::Done(Outcome::Failed(Failure::NoRoom)),
        }
    } else {
        None
    };
    let report = match media::kind_of(source) {
        Kind::Video if original.online_only && !download_now(source, control) => {
            Report::Done(Outcome::Cancelled)
        }
        Kind::Video if original.online_only => {
            downloaded();
            video_now(engine, source, control, original)
        }
        Kind::Video => video_now(engine, source, control, original),
        Kind::Image => {
            // Images are read in the background, below everything else (ADR 0063).
            gracil_windows::work_in_background();
            match images::prepare(source) {
                Ok(image) => Report::Prepared(Prepared::new(Stage::Image(image), source, original)),
                Err(error) => Report::Done(image_outcome(Err(error), source, original)),
            }
        }
    };
    drop(download);
    if matches!(report, Report::Done(_)) {
        give_back(source, original);
    }
    report
}

/// Gets a video ready: its test samples choose its compression.
fn video_now(engine: &Engine, source: &Path, control: &JobControl, original: Snapshot) -> Report {
    match compress::prepare(engine, source, control, &mut |_| {}) {
        Ok(video) => Report::Prepared(Prepared::new(Stage::Video(video), source, original)),
        Err(error) => Report::Done(video_outcome(Err(error), source, original)),
    }
}

/// How much of a file kept only online is read at a time to bring it onto the computer.
const DOWNLOAD_PIECE: usize = 1 << 20;

/// Brings a file kept only online onto the computer by reading it through, a piece at a time,
/// before any tool opens it (ADR 0064). A tool reading it itself would wait, silent, for the whole
/// download, and a large one would be taken for stuck. Between pieces, a pause or rest waits and
/// a stop stops: `false` when stopped. A file that can't be read is left for the tools to find
/// out, as any other.
fn download_now(source: &Path, control: &JobControl) -> bool {
    let Ok(mut file) = fs::File::open(source) else {
        return true;
    };
    let mut piece = vec![0; DOWNLOAD_PIECE];
    loop {
        while control.is_paused() && !control.is_cancelled() {
            thread::sleep(Duration::from_millis(100));
        }
        if control.is_cancelled() {
            return false;
        }
        match io::Read::read(&mut file, &mut piece) {
            Ok(0) | Err(_) => return true,
            Ok(_) => {}
        }
    }
}

/// Folders already found to take new files this run.
static WRITABLE: Mutex<Vec<PathBuf>> = Mutex::new(Vec::new());

/// Whether files can be saved beside `source`, tried once per folder (ADR 0064). Only a folder
/// that took a file is remembered, so one that refused is tried again next time.
fn folder_takes_files(source: &Path) -> bool {
    let Some(folder) = source.parent() else {
        return true;
    };
    let known = |writable: &Vec<PathBuf>| writable.iter().any(|known| known == folder);
    if known(&WRITABLE.lock().unwrap_or_else(|p| p.into_inner())) {
        return true;
    }
    match compress::can_write_beside(source) {
        Ok(()) => {
            let mut writable = WRITABLE.lock().unwrap_or_else(|p| p.into_inner());
            if !known(&writable) {
                writable.push(folder.to_path_buf());
            }
            true
        }
        Err(error) => {
            warn!(%error, "the folder doesn't take new files");
            false
        }
    }
}

/// Compresses a file [`prepare_file`] got ready, leaving the result beside it for the user to
/// place.
pub fn compress_file(
    engine: Arc<Engine>,
    source: PathBuf,
    prepared: Prepared,
    control: JobControl,
) -> Task<Report> {
    on_thread(report_failure, move |mut sender: Sender<Report>| {
        let mut progress = sender.clone();
        let mut on_update = |update: gracil_core::compress::Update| {
            // Progress is best effort: when the window is busy, a skipped update costs nothing.
            let _ = progress.try_send(Report::Progress(update.fraction));
        };
        let original = prepared.original;
        let stage = prepared.take();
        // Held until the result is written, so files compressing at once onto the same drive
        // each count the room the others will take (ADR 0063).
        let room = reserve_room(&source, original.len, 0);
        let outcome = match stage {
            _ if room.is_none() => Outcome::Failed(Failure::NoRoom),
            Some(Stage::Image(image)) if too_large(&image) => Outcome::Failed(Failure::TooLarge),
            Some(Stage::Video(video)) => video_outcome(
                compress::compress_prepared(&engine, &source, video, &control, &mut on_update),
                &source,
                original,
            ),
            Some(Stage::Image(image)) => {
                // Images compress in the background, below everything else (ADR 0063).
                gracil_windows::work_in_background();
                image_outcome(
                    images::compress_prepared(&engine, &source, image, &control, &mut on_update),
                    &source,
                    original,
                )
            }
            None => Outcome::Failed(Failure::CouldNotCompress),
        };
        drop(room);
        if !matches!(outcome, Outcome::Ready { .. }) {
            give_back(&source, original);
        }
        let _ = block_on(sender.send(Report::Done(outcome)));
    })
}

/// Room left free on a drive after the compressed file is written, so Windows and other programs
/// never find it full (ADR 0049).
const ROOM_TO_SPARE: u64 = 2 << 30;

/// Room on each drive that files being compressed will take, at most, once written.
static RESERVED: Mutex<Vec<(PathBuf, u64)>> = Mutex::new(Vec::new());

/// Room on a drive kept for one file's compressed version until it is written.
struct Reservation(Option<(PathBuf, u64)>);

impl Drop for Reservation {
    fn drop(&mut self) {
        if let Some(kept) = self.0.take() {
            let mut reserved = RESERVED
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            if let Some(at) = reserved.iter().position(|other| *other == kept) {
                reserved.swap_remove(at);
            }
        }
    }
}

/// Keeps `need` bytes on the drive holding `source`, if the drive has that room, `later` bytes
/// more for what comes after, and [`ROOM_TO_SPARE`] besides, after what the other files on it
/// will take. A compressed version is never larger than the original, so the original's size is
/// what it needs. A drive that doesn't say has room.
fn reserve_room(source: &Path, need: u64, later: u64) -> Option<Reservation> {
    let Some(folder) = source.parent() else {
        return Some(Reservation(None));
    };
    let Some(free) = gracil_windows::free_space(folder) else {
        return Some(Reservation(None));
    };
    let drive = gracil_windows::volume_of(folder).unwrap_or_else(|| folder.to_path_buf());
    let mut reserved = RESERVED
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let taken: u64 = reserved
        .iter()
        .filter(|(other, _)| *other == drive)
        .map(|(_, bytes)| bytes)
        .sum();
    let wanted = need.saturating_add(later).saturating_add(ROOM_TO_SPARE);
    if free.saturating_sub(taken) < wanted {
        warn!("the drive is too full to compress onto");
        return None;
    }
    reserved.push((drive.clone(), need));
    Some(Reservation(Some((drive, need))))
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

fn video_outcome(
    result: Result<Compressed, CompressError>,
    source: &Path,
    original: Snapshot,
) -> Outcome {
    match result {
        Ok(compressed) => {
            info!(?compressed.attempt, "compressed");
            ready(
                compressed.temp,
                compressed.size,
                output_extension(source),
                source,
                original,
            )
        }
        Err(CompressError::Cancelled) => Outcome::Cancelled,
        Err(CompressError::AsSmallAsItGets) => {
            info!("already as small as it gets");
            Outcome::Failed(Failure::AsSmallAsItGets)
        }
        Err(_) if !source.exists() => {
            warn!("the file is no longer there");
            Outcome::Failed(Failure::Gone)
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

fn image_outcome(
    result: Result<images::CompressedImage, ImageError>,
    source: &Path,
    original: Snapshot,
) -> Outcome {
    match result {
        Ok(compressed) => ready(
            compressed.temp,
            compressed.size,
            compressed.extension,
            source,
            original,
        ),
        Err(ImageError::Cancelled) => Outcome::Cancelled,
        Err(ImageError::AsSmallAsItGets) => {
            info!("image already as small as it gets");
            Outcome::Failed(Failure::AsSmallAsItGets)
        }
        Err(_) if !source.exists() => {
            warn!("the image is no longer there");
            Outcome::Failed(Failure::Gone)
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
                ImageError::TooLarge => Failure::TooLarge,
                ImageError::Failed(_) | ImageError::Cancelled | ImageError::AsSmallAsItGets => {
                    Failure::CouldNotCompress
                }
            })
        }
    }
}

/// Keeps the result for the user, unless it is not smaller (spec §40.1), or the original is gone
/// or changed while it was compressed (ADR 0064).
fn ready(
    temp: PathBuf,
    size: u64,
    extension: OsString,
    source: &Path,
    original: Snapshot,
) -> Outcome {
    let Ok(metadata) = fs::metadata(source) else {
        warn!("the file is no longer there");
        discard(&temp);
        return Outcome::Failed(Failure::Gone);
    };
    if !original.unchanged(source) {
        warn!("the file changed while it was compressed");
        discard(&temp);
        return Outcome::Failed(Failure::Changed);
    }
    let original_size = metadata.len();
    info!(original = original_size, compressed = size, "ready");
    if size >= original_size {
        discard(&temp);
        return Outcome::Failed(Failure::NotSmaller);
    }
    Outcome::Ready {
        temp,
        extension,
        sizes: Sizes {
            original: original_size,
            compressed: size,
        },
        original,
        placing: false,
        problem: None,
    }
}

/// Puts a result in place of the original or beside it, saved with `extension`. Returns where it
/// ended up.
///
/// Results on one drive are placed one after another, in the order chosen, by one worker for that
/// drive that stays for the whole run (ADR 0064), so a drive that stalls (a network share) holds
/// up only its own. An original that changed since it was compressed is left as it is and its
/// result let go; a read-only original is never replaced, and its result stays for Save a copy.
/// The answer always comes, even if placing stops unexpectedly.
pub fn place(
    temp: PathBuf,
    source: PathBuf,
    extension: OsString,
    original: Snapshot,
    choice: Choice,
) -> Task<Result<PathBuf, Failure>> {
    let (mut sender, receiver) = mpsc::channel(1);
    let drive = drive_of(&source);
    let errand = Box::new(move || {
        let placed = panic::catch_unwind(AssertUnwindSafe(|| {
            place_now(&temp, &source, &extension, original, choice)
        }))
        .unwrap_or_else(|_| {
            error!("putting a result in place stopped unexpectedly");
            Err(Failure::CouldNotPlace)
        });
        let _ = block_on(sender.send(placed));
    });
    match placer_for(drive).run(errand) {
        Ok(()) => Task::stream(receiver),
        Err(_) => {
            error!("could not start putting results in place");
            Task::done(Err(Failure::CouldNotPlace))
        }
    }
}

/// The drive `source` is on, by its name alone (`C:` or `\\server\share`), so telling it asks
/// nothing of a drive that may not answer. A folder mounted inside another drive counts as that
/// drive, which only shares its worker.
fn drive_of(source: &Path) -> OsString {
    match source.components().next() {
        Some(Component::Prefix(prefix)) => prefix.as_os_str().to_ascii_lowercase(),
        _ => OsString::new(),
    }
}

/// The workers that put results in place, one for each drive, made as first needed.
static PLACERS: Mutex<Vec<(OsString, &'static Errands)>> = Mutex::new(Vec::new());

/// The worker that puts results in place on `drive`.
fn placer_for(drive: OsString) -> &'static Errands {
    let mut placers = PLACERS
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if let Some((_, placer)) = placers.iter().find(|(known, _)| *known == drive) {
        return placer;
    }
    // Kept for the whole run, like the worker itself; there is one for each drive used.
    let placer: &'static Errands = Box::leak(Box::new(Errands::new("gracil-placer")));
    placers.push((drive, placer));
    placer
}

fn place_now(
    temp: &Path,
    source: &Path,
    extension: &OsString,
    original: Snapshot,
    choice: Choice,
) -> Result<PathBuf, Failure> {
    if !source.exists() {
        warn!("the file is no longer there");
        return Err(Failure::Gone);
    }
    if !original.unchanged(source) {
        warn!("the file changed after it was compressed");
        discard(temp);
        give_back(source, original);
        return Err(Failure::Changed);
    }
    if choice == Choice::Replace && read_only(source) {
        warn!("the file is read-only, so it isn't replaced");
        return Err(Failure::ReadOnly);
    }
    let placed = match choice {
        Choice::Replace => replace_original(temp, source, extension, gracil_windows::replace_file),
        Choice::SaveCopy => save_as_new(temp, source, extension),
    }
    .map_err(|e| {
        warn!(error = %e, ?choice, "could not place result");
        placing_failure(&e, source)
    });
    if let Ok(path) = &placed {
        // The original, or the file that took its place.
        give_back(
            if choice == Choice::Replace {
                path
            } else {
                source
            },
            original,
        );
    }
    placed
}

/// Whether `source` is marked read-only. Gracil never clears the mark.
fn read_only(source: &Path) -> bool {
    fs::metadata(source).is_ok_and(|metadata| metadata.permissions().readonly())
}

/// Why a result couldn't be put in place, in the user's terms.
fn placing_failure(error: &io::Error, source: &Path) -> Failure {
    // Windows' "being used by another process", "locked by another process", and ReplaceFile's
    // "couldn't remove the original", which is nearly always the same thing.
    const IN_USE: [i32; 3] = [32, 33, 1175];
    const ACCESS_DENIED: i32 = 5;
    if error.kind() == io::ErrorKind::AlreadyExists {
        Failure::NameTaken
    } else if !source.exists() {
        Failure::Gone
    } else if error
        .raw_os_error()
        .is_some_and(|code| IN_USE.contains(&code))
    {
        Failure::InUse
    } else if error.raw_os_error() == Some(ACCESS_DENIED) && read_only(source) {
        Failure::ReadOnly
    } else {
        Failure::CouldNotPlace
    }
}

/// Deletes a result the user did not keep, here and now. The original is never involved.
pub fn discard(temp: &Path) {
    if let Err(error) = fs::remove_file(temp) {
        warn!(%error, ?temp, "could not remove an unused result");
    }
}

/// Deletes a result nobody chose for away from the window, which a slow drive would otherwise
/// freeze (ADR 0064), and lets the cloud drive free up its original again if it was kept only
/// online. Deleting waits its turn behind the others; Gracil waits for them all on leaving.
pub fn discard_later(temp: PathBuf, source: Option<PathBuf>, original: Option<Snapshot>) {
    let errand: Errand = Box::new(move || {
        discard(&temp);
        if let (Some(source), Some(original)) = (source, original) {
            give_back(&source, original);
        }
    });
    if let Err(errand) = CLEANER.run(errand) {
        errand();
    }
}

/// Waits, at most `within`, for every result [`discard_later`] was given to be deleted.
pub fn finish_discarding(within: Duration) {
    let (done, finished) = std::sync::mpsc::channel();
    let errand: Errand = Box::new(move || {
        let _ = done.send(());
    });
    if CLEANER.run(errand).is_ok() {
        let _ = finished.recv_timeout(within);
    }
}

/// One piece of work for an [`Errands`] worker.
type Errand = Box<dyn FnOnce() + Send>;

/// One worker that stays for the whole run, doing its errands one after another, in the order
/// they came.
struct Errands {
    name: &'static str,
    line: OnceLock<Option<std::sync::mpsc::Sender<Errand>>>,
}

/// Deletes results nobody chose for.
static CLEANER: Errands = Errands::new("gracil-cleaner");

impl Errands {
    const fn new(name: &'static str) -> Self {
        Self {
            name,
            line: OnceLock::new(),
        }
    }

    /// Hands `errand` to the worker, starting it the first time. Gives the errand back when there
    /// is no worker to do it. An errand that fails unexpectedly is logged, and the next one goes
    /// ahead.
    fn run(&self, errand: Errand) -> Result<(), Errand> {
        let name = self.name;
        let line = self.line.get_or_init(|| {
            let (sender, receiver) = std::sync::mpsc::channel::<Errand>();
            let started = thread::Builder::new().name(name.into()).spawn(move || {
                for errand in receiver {
                    if panic::catch_unwind(AssertUnwindSafe(errand)).is_err() {
                        error!(worker = name, "an errand stopped unexpectedly");
                    }
                }
            });
            match started {
                Ok(_) => Some(sender),
                Err(error) => {
                    error!(%error, worker = name, "could not start a worker");
                    None
                }
            }
        });
        match line {
            Some(sender) => sender.send(errand).map_err(|unsent| unsent.0),
            None => Err(errand),
        }
    }
}

/// Runs `work` on its own thread; whatever it sends becomes the task's output. If the thread
/// can't start, or `work` stops unexpectedly, `fallback` is sent instead, so nothing is left
/// waiting forever (ADR 0064).
fn on_thread<T: Send + 'static>(
    fallback: fn() -> T,
    work: impl FnOnce(Sender<T>) + Send + 'static,
) -> Task<T> {
    let (sender, receiver) = mpsc::channel(1);
    let mut last_word = sender.clone();
    let started = thread::Builder::new()
        .name("gracil-work".into())
        .spawn(move || {
            if panic::catch_unwind(AssertUnwindSafe(|| work(sender))).is_err() {
                error!("background work stopped unexpectedly");
                let _ = block_on(last_word.send(fallback()));
            }
        });
    match started {
        Ok(_) => Task::stream(receiver),
        Err(error) => {
            error!(%error, "could not start background work");
            Task::done(fallback())
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    #[test]
    fn each_drive_gets_its_own_placer() {
        let drive = |path: &str| drive_of(Path::new(path));
        assert_eq!(drive(r"C:\Videos\a.mp4"), drive(r"c:\Other\b.mp4"));
        assert_ne!(drive(r"C:\Videos\a.mp4"), drive(r"D:\Videos\a.mp4"));
        assert_ne!(drive(r"\\nas\videos\a.mp4"), drive(r"\\nas\photos\a.jpg"));
        assert_eq!(drive(r"\\NAS\Videos\a.mp4"), drive(r"\\nas\videos\b.mp4"));
        assert!(std::ptr::eq(
            placer_for(drive(r"E:\a.mp4")),
            placer_for(drive(r"e:\b.mp4"))
        ));
        assert!(!std::ptr::eq(
            placer_for(drive(r"E:\a.mp4")),
            placer_for(drive(r"F:\a.mp4"))
        ));
    }

    #[test]
    fn a_download_stops_when_asked() {
        let dir = gracil_core::testing::fresh_dir("download");
        let file = dir.join("online.mp4");
        fs::write(&file, vec![7; 3 * DOWNLOAD_PIECE + 5]).unwrap();
        assert!(download_now(&file, &JobControl::default()));
        let stopped = JobControl::default();
        stopped.cancel();
        assert!(!download_now(&file, &stopped));
        // A file that can't be read is left for the tools.
        assert!(download_now(
            &dir.join("missing.mp4"),
            &JobControl::default()
        ));
        fs::remove_dir_all(dir).unwrap();
    }
}
