//! Finding each video's own quality level (ADR 0028), and whether compressing it is worth it
//! (ADR 0029).
//!
//! Short samples spread through the video are encoded at candidate levels and compared with the
//! source using VMAF, both with the same frame-rate change. The highest level (the
//! smallest file) whose average score reaches [`standard::TARGET_SCORE`] wins. Each sample is one
//! FFmpeg process that encodes and measures at once, through a loopback decoder, so nothing is
//! written to disk. The same process reports the size of every encoded frame, from which the size
//! of the whole result is predicted.
//!
//! When that level would not make the video clearly smaller, the video is already compressed, and
//! the highest level reaching [`standard::AGAIN_SCORE`] is used instead (ADR 0033): compressing
//! again loses a little, which is what the user asks for. If even that would not be clearly
//! smaller, nothing is encoded.

use std::collections::{BTreeMap, HashMap};
use std::ffi::OsString;
use std::fs;
use std::ops::RangeInclusive;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread;
use std::time::Duration;

use tracing::{info, warn};

use crate::capabilities::{Capabilities, VideoEncoder};
use crate::encode::{self, EncodeOutcome, JobControl};
use crate::plan::{self, Attempt, EncodePlan, StreamAction};
use crate::probe::{MediaInfo, Rate, file_url};
use crate::share;
use crate::standard;
use crate::toolchain::Toolchain;

/// Length of one sample.
const SAMPLE: Duration = Duration::from_secs(4);
/// One sample for every this much video, up to [`MAX_SAMPLES`].
const SAMPLE_EVERY: Duration = Duration::from_secs(15);
const MAX_SAMPLES: u32 = 4;
/// Shorter videos use the fallback level: there is too little to measure.
const SHORTEST: Duration = Duration::from_secs(6);
/// x265 samples are slow to encode; shorter videos use the fallback level with it.
const SHORTEST_X265: Duration = Duration::from_secs(60);
/// Frames between key frames: the default of both encoders.
const KEY_FRAME_SPACING: f64 = 250.0;

/// The level a video will be compressed at.
#[derive(Debug, Clone, PartialEq)]
pub enum Found {
    /// Measured for this video with `encoder`.
    Level {
        encoder: VideoEncoder,
        level: u8,
    },
    /// The video is already compressed, and not even compressing it again (ADR 0033) would make
    /// it clearly smaller. Nothing is encoded.
    AsSmallAsItGets,
    /// Nothing could be measured; each encoder uses its fallback level.
    Fallback,
    Cancelled,
}

impl Found {
    /// The level to use with `encoder`.
    pub fn level_for(&self, encoder: VideoEncoder) -> u8 {
        match *self {
            Self::Level { encoder: e, level } if e == encoder => level,
            _ => standard::fallback_level(encoder),
        }
    }
}

/// The levels the last videos were found to need, by encoder, size and source codec, each the
/// first level tried for the next video of the same kind (ADR 0065). Kept in memory for as long
/// as Gracil runs, never written down. A hint only decides what is measured first: the level
/// chosen is always one measured to reach the target.
#[derive(Debug, Clone, Default)]
pub struct Hints(Arc<Mutex<HashMap<HintKey, u8>>>);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct HintKey {
    encoder: VideoEncoder,
    /// The picture's short side, in steps (see [`size_step`]).
    size: u8,
    codec: String,
    /// The level for compressing again (ADR 0033) rather than for looking the same.
    again: bool,
}

impl Hints {
    fn get(&self, key: &HintKey) -> Option<u8> {
        self.lock().get(key).copied()
    }

    fn remember(&self, key: HintKey, level: u8) {
        self.lock().insert(key, level);
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<HintKey, u8>> {
        // A plain map of numbers: a panic elsewhere leaves nothing half done in it.
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// Which of a few common sizes a picture of `size` is closest to, by its short side: up to 480,
/// 720, 1080, 1440, 2160, and above.
fn size_step(size: Option<(u32, u32)>) -> u8 {
    let short = size.map_or(0, |(width, height)| width.min(height));
    [480, 720, 1080, 1440, 2160]
        .iter()
        .position(|&step| short <= step)
        .unwrap_or(5) as u8
}

/// Searches the level for `plan`'s main video, encoded the way `plan.attempt` would, starting
/// from what `hints` remember of videos like it.
pub fn find_level(
    toolchain: &Toolchain,
    plan: &EncodePlan,
    info: &MediaInfo,
    control: &JobControl,
    hints: &Hints,
) -> Found {
    let encoder = plan.attempt.encoder;
    let Some((index, rate)) = plan.main_video() else {
        return Found::Fallback;
    };
    let shortest = match encoder {
        VideoEncoder::Nvenc | VideoEncoder::Qsv | VideoEncoder::Amf => SHORTEST,
        VideoEncoder::X265 => SHORTEST_X265,
    };
    let Some(duration) = info.duration.filter(|d| *d >= shortest) else {
        return Found::Fallback;
    };
    if !plan.capabilities.vmaf {
        return Found::Fallback;
    }

    let source = info.streams.iter().find(|stream| stream.index == index);
    let pixel_format = source.and_then(|stream| stream.pixel_format.as_deref());
    let key = HintKey {
        encoder,
        size: size_step(plan.picture),
        codec: source
            .map(|stream| stream.codec.clone())
            .unwrap_or_default(),
        again: false,
    };
    let mut search = Search {
        toolchain,
        input: &plan.input,
        stream: index,
        rate,
        attempt: plan.attempt,
        capabilities: &plan.capabilities,
        picture: plan.picture,
        card_frames: pixel_format
            .and_then(|format| card_download(format, plan.capabilities.ten_bit(encoder))),
        starts: sample_starts(duration),
        duration,
        control,
        measured: BTreeMap::new(),
    };
    let sizes = fs::metadata(&plan.input).ok().map(|metadata| {
        let original = metadata.len();
        Sizes {
            original,
            kept: kept_bytes(info, plan, original, duration),
        }
    });
    // What the source spends on each pixel, from its own bitrate or else the whole file's.
    let bit_rate = source
        .and_then(|stream| stream.bit_rate)
        .map(|bits| bits as f64)
        .or_else(|| sizes.map(|sizes| sizes.original as f64 * 8.0 / duration.as_secs_f64()));
    let fps = source
        .and_then(|stream| stream.frame_rate)
        .map(Rate::per_second);
    let bits = bit_rate.and_then(|bit_rate| bits_per_pixel(bit_rate, plan.picture, fps));
    let start = Start {
        guess: bits.map(|bits| first_guess(encoder, bits)),
        compressed: bits.is_some_and(looks_compressed),
    };
    info!(bits_per_pixel = bits, ?start, "searching the quality level");
    match choose(&mut search, encoder, sizes, hints, key, start) {
        Ok(found) => found,
        Err(Halt::Cancelled) => Found::Cancelled,
        Err(Halt::Failed(error)) => {
            warn!(%error, "could not measure quality; using the usual level");
            Found::Fallback
        }
        Err(Halt::GaveUp | Halt::AsSmallAsItGets) => {
            unreachable!("both are answers, given by `choose`")
        }
    }
}

/// The bits a video of `bit_rate` spends on each pixel of each frame, for a picture of `picture`
/// at `fps` frames a second.
fn bits_per_pixel(bit_rate: f64, picture: Option<(u32, u32)>, fps: Option<f64>) -> Option<f64> {
    let (width, height) = picture?;
    let pixels = f64::from(width) * f64::from(height) * fps.filter(|fps| *fps > 0.0)?;
    (pixels > 0.0 && bit_rate > 0.0).then(|| bit_rate / pixels)
}

/// The level measured first for a video unlike any searched before, from the `bits` its source
/// spends on each pixel (ADR 0067): a source that spends more holds finer detail, which needs a
/// lower level to look the same. Calibrated on the reference computer; it only decides what is
/// measured first.
fn first_guess(encoder: VideoEncoder, bits: f64) -> u8 {
    let levels = standard::levels(encoder);
    let (start, end) = (*levels.start(), *levels.end());
    // From the end of the levels for the leanest sources, a level lower for each doubling.
    let doublings = (bits / LEANEST_BITS).log2().max(0.0);
    let below_end = (doublings * LEVELS_PER_DOUBLING).round() as u8;
    end.saturating_sub(below_end + GUESS_BELOW_END).max(start)
}

/// Bits per pixel below which a source is taken to be compressed already (ADR 0067): whether any
/// level can be small enough is told before searching. Only what is measured first depends on it.
const COMPRESSED_BITS: f64 = 0.02;

/// Bits per pixel of the leanest sources [`first_guess`] tells apart.
const LEANEST_BITS: f64 = 0.03;
/// Levels lower for each doubling of the bits per pixel.
const LEVELS_PER_DOUBLING: f64 = 1.0;
/// Levels below the end of the range for the leanest sources.
const GUESS_BELOW_END: u8 = 2;

/// Whether a source spending `bits` on each pixel looks compressed already.
fn looks_compressed(bits: f64) -> bool {
    bits < COMPRESSED_BITS
}

/// What choosing a level needs measured: a level's score and predicted size, or a level's size
/// alone. Measured with FFmpeg by [`Search`]; read off score and size curves in the tests.
trait Meter {
    /// `level`, measured on every sample. Each level is measured once; asked again, it is what
    /// was measured.
    fn measure(&mut self, level: u8) -> Result<Measured, Halt>;
    /// Every level measured so far.
    fn measured(&self) -> &BTreeMap<u8, Measured>;
    /// The predicted bytes of the whole main video at `level`, from its samples encoded without
    /// measuring their quality (ADR 0067); `None` when that couldn't be told.
    fn size_alone(&mut self, level: u8) -> Result<Option<f64>, Halt>;
}

/// Where the first search starts for a video unlike any searched before (ADR 0067).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
struct Start {
    /// The level guessed from the bits the source spends on each pixel (see [`first_guess`]).
    guess: Option<u8>,
    /// The source looks compressed already (see [`looks_compressed`]): whether anything can be
    /// small enough is told first.
    compressed: bool,
}

/// Picks the level that looks the same. If that is not worth it, the video was already compressed,
/// and it gets the level that loses a little (ADR 0033), when that one is worth it. Each search
/// starts from the level `hints` remember for `key`, or from `start`'s guess, and leaves the level
/// it found there.
fn choose(
    meter: &mut impl Meter,
    encoder: VideoEncoder,
    sizes: Option<Sizes>,
    hints: &Hints,
    key: HintKey,
    start: Start,
) -> Result<Found, Halt> {
    let mut chooser = Chooser {
        meter,
        encoder,
        sizes,
        top_known: sizes.is_none(),
    };
    match chooser.choose(hints, key, start) {
        Err(Halt::AsSmallAsItGets) => {
            info!(
                original = sizes.map(|sizes| sizes.original),
                "already as small as it gets"
            );
            Ok(Found::AsSmallAsItGets)
        }
        result => result,
    }
}

/// One video's choice of level, from what its [`Meter`] measures.
struct Chooser<'m, M: Meter> {
    meter: &'m mut M,
    encoder: VideoEncoder,
    sizes: Option<Sizes>,
    /// Whether it is known that something can be small enough (see [`Chooser::check_top`]).
    top_known: bool,
}

impl<M: Meter> Chooser<'_, M> {
    fn choose(&mut self, hints: &Hints, key: HintKey, start: Start) -> Result<Found, Halt> {
        let encoder = self.encoder;
        let levels = standard::levels(encoder);
        if start.compressed {
            self.check_top()?;
        }
        let hint = hints.get(&key).or(start.guess);
        let chosen = match self.highest(levels.clone(), standard::TARGET_SCORE, hint) {
            // Content that never reaches the target gets the best quality searched.
            Ok(best) => {
                if let Some(level) = best {
                    hints.remember(key.clone(), level);
                }
                Some(best.unwrap_or(*levels.start()))
            }
            Err(Halt::GaveUp) => None,
            Err(halt) => return Err(halt),
        };
        let Some(sizes) = self.sizes else {
            let level = chosen.unwrap_or(*levels.start());
            info!(level, "chose a quality level");
            return Ok(Found::Level { encoder, level });
        };
        if let Some(found) = self.if_worth_it(sizes, chosen, "chose a quality level")? {
            return Ok(found);
        }
        info!(
            original = sizes.original,
            "already compressed; measuring the level for compressing again"
        );

        // The level that looked the same is where losing a little starts, unless a video like
        // this one was compressed again before.
        let key = HintKey { again: true, ..key };
        let hint = hints.get(&key).or(chosen);
        let again = match self.highest(standard::again_levels(encoder), standard::AGAIN_SCORE, hint)
        {
            Ok(level) => level,
            Err(Halt::GaveUp) => return Err(Halt::AsSmallAsItGets),
            Err(halt) => return Err(halt),
        };
        if let Some(level) = again {
            hints.remember(key, level);
        }
        match self.if_worth_it(sizes, again, "chose a level for compressing again")? {
            Some(found) => Ok(found),
            None => Err(Halt::AsSmallAsItGets),
        }
    }

    /// The highest level in `levels` whose score reaches `target`, assuming scores fall as
    /// levels rise. Starts from what is already measured, then tries `hint` first if given.
    /// Gives up ([`Halt::GaveUp`]) once a level that misses the target is still too big: every
    /// level that reaches it is bigger still. The first level found too big tells whether
    /// anything can be small enough ([`Chooser::check_top`]).
    fn highest(
        &mut self,
        levels: RangeInclusive<u8>,
        target: f64,
        hint: Option<u8>,
    ) -> Result<Option<u8>, Halt> {
        let misses_too_big = |chooser: &Self, level: &u8, measured: &Measured| {
            levels.contains(level) && measured.score < target && chooser.too_big(measured)
        };
        if self
            .meter
            .measured()
            .iter()
            .any(|(level, measured)| misses_too_big(self, level, measured))
        {
            return Err(Halt::GaveUp);
        }
        loop {
            let scores: BTreeMap<u8, f64> = self
                .meter
                .measured()
                .iter()
                .filter(|(level, _)| levels.contains(level))
                .map(|(&level, measured)| (level, measured.score))
                .collect();
            let heavy = self.heavy(&levels, reached(&scores, target));
            let Some(level) = next_level(&scores, &levels, target, hint, heavy) else {
                return Ok(reached(&scores, target));
            };
            let measured = self.meter.measure(level)?;
            if self.too_big(&measured) {
                if self.top_in_reach(level, &measured) {
                    self.check_top()?;
                }
                if measured.score < target {
                    return Err(Halt::GaveUp);
                }
            }
        }
    }

    /// Whether the top level for compressing again may be too big too, as estimated on a log
    /// scale from `level`, `measured` too big: then it is worth encoding to tell
    /// ([`Chooser::check_top`]). Otherwise that encode would most likely only say what the
    /// estimate already does.
    fn top_in_reach(&self, level: u8, measured: &Measured) -> bool {
        let (Some(sizes), Some(video)) = (self.sizes, measured.video_bytes) else {
            return false;
        };
        let top = *standard::again_levels(self.encoder).end();
        let per_level = size_slope(self.meter.measured()).unwrap_or(SIZE_PER_LEVEL);
        let estimate = video * (-per_level * f64::from(top.saturating_sub(level))).exp();
        estimate + sizes.kept >= sizes.original as f64 * standard::WORTH_IT * TOP_IN_REACH
    }

    /// Whether `measured` is predicted too big to be worth it.
    fn too_big(&self, measured: &Measured) -> bool {
        self.sizes
            .is_some_and(|sizes| !sizes.worth_it(sizes.predicted(measured.video_bytes)))
    }

    /// Tells, once, whether anything can be small enough (ADR 0067): the samples are encoded at
    /// the top level for compressing again, the smallest file there is, without measuring their
    /// quality. When even that is too big, nothing is ([`Halt::AsSmallAsItGets`]); otherwise,
    /// or when it can't be told, the search goes on as before.
    fn check_top(&mut self) -> Result<(), Halt> {
        if std::mem::replace(&mut self.top_known, true) {
            return Ok(());
        }
        let Some(sizes) = self.sizes else {
            return Ok(());
        };
        let top = *standard::again_levels(self.encoder).end();
        let video = match self.meter.measured().get(&top) {
            Some(measured) => measured.video_bytes,
            None => self.meter.size_alone(top)?,
        };
        let predicted = sizes.predicted(video);
        if sizes.worth_it(predicted) {
            return Ok(());
        }
        info!(
            level = top,
            ?predicted,
            original = sizes.original,
            "even the smallest level is too big"
        );
        Err(Halt::AsSmallAsItGets)
    }

    /// When the highest level reaching the target, `reached`, is too big: the highest level in
    /// `levels` whose size, estimated on a log scale from the sizes measured, is still too big
    /// (ADR 0067). Measured next, it either reaches the target too, and the search goes on above
    /// it, or misses it while too big, and nothing reaching the target is small enough. Only the
    /// order of measuring depends on it.
    fn heavy(&self, levels: &RangeInclusive<u8>, reached: Option<u8>) -> Option<u8> {
        let sizes = self.sizes?;
        let measured = self.meter.measured();
        let reached = reached?;
        let anchor = measured.get(&reached)?;
        let video = anchor.video_bytes.filter(|_| self.too_big(anchor))?;
        let limit = sizes.original as f64 * standard::WORTH_IT - sizes.kept;
        if limit <= 0.0 {
            return Some(*levels.end());
        }
        let per_level = size_slope(measured).unwrap_or(SIZE_PER_LEVEL);
        let steps = (video / limit).ln() / per_level;
        let heavy = f64::from(reached) + steps.ceil() - 1.0;
        Some(heavy.clamp(f64::from(reached), f64::from(*levels.end())) as u8)
    }

    /// `level`, if its predicted size is worth it.
    fn if_worth_it(
        &mut self,
        sizes: Sizes,
        level: Option<u8>,
        chose: &str,
    ) -> Result<Option<Found>, Halt> {
        let Some(level) = level else {
            return Ok(None);
        };
        let predicted = sizes.predicted(self.meter.measure(level)?.video_bytes);
        if !sizes.worth_it(predicted) {
            return Ok(None);
        }
        info!(level, ?predicted, original = sizes.original, "{chose}");
        let encoder = self.encoder;
        Ok(Some(Found::Level { encoder, level }))
    }
}

/// How much smaller, on a log scale, each level up usually makes the video when the sizes
/// measured don't tell: about a ninth, 0.105 to 0.13 with NVENC on the reference computer
/// (ADR 0067).
const SIZE_PER_LEVEL: f64 = 0.11;

/// The top level for compressing again is encoded to tell whether anything can be small enough
/// once it is estimated at more than this share of the most that is worth it.
const TOP_IN_REACH: f64 = 0.85;

/// How much smaller, on a log scale, each level up made the video, between the lowest and the
/// highest level `measured` with a size; `None` with fewer than two, or when it didn't shrink.
fn size_slope(measured: &BTreeMap<u8, Measured>) -> Option<f64> {
    let mut sized = measured
        .iter()
        .filter_map(|(&level, measured)| Some((level, measured.video_bytes?)))
        .filter(|&(_, bytes)| bytes > 0.0);
    let (low, low_bytes) = sized.next()?;
    let (high, high_bytes) = sized.next_back()?;
    let slope = (low_bytes / high_bytes).ln() / f64::from(high - low);
    (slope > 0.01).then_some(slope)
}

/// The original's size, and what the result's size is judged against.
#[derive(Debug, Clone, Copy)]
struct Sizes {
    original: u64,
    /// Predicted bytes of everything in the result except the main video.
    kept: f64,
}

impl Sizes {
    /// The result's predicted size, for a main video predicted at `video` bytes.
    fn predicted(self, video: Option<f64>) -> Option<u64> {
        video.map(|video| (video + self.kept).max(0.0).round() as u64)
    }

    /// Whether a result of `predicted` bytes is clearly smaller than the original. An unknown
    /// prediction gets the benefit of the doubt; the result is checked when it is done.
    fn worth_it(self, predicted: Option<u64>) -> bool {
        predicted.is_none_or(|bytes| bytes as f64 <= self.original as f64 * standard::WORTH_IT)
    }
}

/// Bytes of the result that are not the main video: the rest of the source as it is, except
/// audio converted to AAC. Streams that record no bitrate are counted generously.
fn kept_bytes(info: &MediaInfo, plan: &EncodePlan, original: u64, duration: Duration) -> f64 {
    let bytes = |bit_rate: u64| bit_rate as f64 * duration.as_secs_f64() / 8.0;
    let source_bytes = |index: u32| {
        info.streams
            .iter()
            .find(|s| s.index == index)
            .and_then(|s| s.bit_rate)
            .map(bytes)
    };
    let main = plan.main_video().map(|(index, _)| index);
    let mut kept = match main.and_then(source_bytes) {
        Some(video) => original as f64 - video,
        None => info
            .streams
            .iter()
            .filter(|s| Some(s.index) != main)
            .filter_map(|s| s.bit_rate.map(bytes))
            .sum(),
    };
    for stream in &plan.streams {
        if let StreamAction::ToAac { kbps } = stream.action {
            kept +=
                bytes(u64::from(kbps) * 1000) - source_bytes(stream.source_index).unwrap_or(0.0);
        }
    }
    kept.max(0.0)
}

/// Why a search stopped before finding its level.
enum Halt {
    Cancelled,
    Failed(String),
    /// A level that misses the target is already too big: no level reaching it is small enough.
    GaveUp,
    /// Nothing can be small enough (ADR 0029, ADR 0033).
    AsSmallAsItGets,
}

/// One level, measured on every sample.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Measured {
    /// Average VMAF score.
    score: f64,
    /// Predicted bytes of the whole main video, when FFmpeg reported frame sizes.
    video_bytes: Option<f64>,
}

impl Measured {
    /// Averages `samples` and predicts the whole video's size from their frames. A sample's first
    /// frame is a key frame, much larger than the rest; counted as it comes, it would dominate
    /// four seconds. So: the samples' typical frame times the video's frames, plus one typical
    /// key frame every [`KEY_FRAME_SPACING`] frames.
    fn of(samples: &[Sample], duration: Duration) -> Self {
        let count = samples.len().max(1) as f64;
        let score = samples.iter().map(|s| s.score).sum::<f64>() / count;
        let (mut keys, mut rest, mut rest_frames, mut frames) = (0.0, 0.0, 0usize, 0usize);
        for sample in samples {
            if let Some((first, others)) = sample.frame_sizes.split_first() {
                keys += *first as f64;
                rest += others.iter().sum::<u64>() as f64;
                rest_frames += others.len();
                frames += sample.frame_sizes.len();
            }
        }
        let video_bytes = (rest_frames > 0).then(|| {
            let per_second = frames as f64 / (count * SAMPLE.as_secs_f64());
            let total_frames = per_second * duration.as_secs_f64();
            let frame = rest / rest_frames as f64;
            let key = keys / count;
            frame * total_frames + key * total_frames / KEY_FRAME_SPACING
        });
        Self { score, video_bytes }
    }
}

/// One sample at one level.
struct Sample {
    score: f64,
    /// Bytes of each encoded frame, in the order the encoder produced them.
    frame_sizes: Vec<u64>,
}

struct Search<'a> {
    toolchain: &'a Toolchain,
    input: &'a Path,
    stream: u32,
    /// The frame rate the video leaves with; `None` keeps the source's.
    rate: Option<Rate>,
    attempt: Attempt,
    capabilities: &'a Capabilities,
    /// The video's width and height as stored, when the source says.
    picture: Option<(u32, u32)>,
    /// How decoded frames are copied out of the graphics card for the reference, when they can
    /// stay there for the encoder (see [`card_download`]).
    card_frames: Option<&'static str>,
    starts: Vec<Duration>,
    duration: Duration,
    control: &'a JobControl,
    measured: BTreeMap<u8, Measured>,
}

impl Meter for Search<'_> {
    /// The average score of every sample at `level`, and the predicted size. Measured once.
    fn measure(&mut self, level: u8) -> Result<Measured, Halt> {
        if let Some(measured) = self.measured.get(&level) {
            return Ok(*measured);
        }
        let at_once = self.at_once();
        let samples = if at_once > 1 {
            self.samples_together(level, at_once)?
        } else {
            let mut samples = Vec::with_capacity(self.starts.len());
            for index in 0..self.starts.len() {
                samples.push(self.sample(self.starts[index], level)?);
            }
            samples
        };
        let measured = Measured::of(&samples, self.duration);
        info!(
            level,
            score = measured.score,
            video_mb = measured.video_bytes.map(|bytes| (bytes / 1e6).round()),
            "measured a quality level"
        );
        self.measured.insert(level, measured);
        Ok(measured)
    }

    fn measured(&self) -> &BTreeMap<u8, Measured> {
        &self.measured
    }

    /// The samples at `level`, only encoded, as many at once as when measured; their frames are
    /// exactly the frames measured at that level (ADR 0067). A sample that fails leaves the size
    /// unknown: only what is measured next depends on it.
    fn size_alone(&mut self, level: u8) -> Result<Option<f64>, Halt> {
        if let Some(measured) = self.measured.get(&level) {
            return Ok(measured.video_bytes);
        }
        let runs = self.runs_together(&self.starts, level, self.at_once(), false);
        let mut samples = Vec::with_capacity(runs.len());
        for run in runs {
            match run.outcome {
                EncodeOutcome::Finished => samples.push(Sample {
                    score: 0.0,
                    frame_sizes: run.frame_sizes,
                }),
                EncodeOutcome::Cancelled => return Err(Halt::Cancelled),
                EncodeOutcome::Failed { error, .. } => {
                    warn!(level, %error, "could not encode the samples alone");
                    return Ok(None);
                }
                EncodeOutcome::Stalled => {
                    warn!(level, "the samples encoded alone stopped answering");
                    return Ok(None);
                }
            }
        }
        let video = Measured::of(&samples, self.duration).video_bytes;
        info!(
            level,
            video_mb = video.map(|bytes| (bytes / 1e6).round()),
            "measured the size alone"
        );
        Ok(video)
    }
}

impl Search<'_> {
    /// How many samples are measured at once right now (see [`samples_at_once`]).
    fn at_once(&self) -> usize {
        samples_at_once(
            self.starts.len(),
            self.attempt.encoder,
            standard::measured_size(self.picture).or(self.picture),
            share::free_memory(),
            share::easy() || ONE_AT_A_TIME.load(Ordering::Relaxed),
        )
    }

    /// Measures the samples at `level`, `at_once` at a time, each with its share of the threads
    /// the quality measure may use, so all together use no more than one would alone (ADR 0065,
    /// ADR 0067). The scores come back in the samples' order, as one at a time. A sample that
    /// fails is measured again on its own; if the graphics card couldn't take one more encoder,
    /// the samples go one at a time from then on.
    fn samples_together(&mut self, level: u8, at_once: usize) -> Result<Vec<Sample>, Halt> {
        let sleeps = share::sleeps();
        let starts = self.starts.clone();
        let runs = self.runs_together(&starts, level, at_once, true);
        let mut samples = Vec::with_capacity(starts.len());
        for (start, run) in starts.into_iter().zip(runs) {
            let slept = share::sleeps() != sleeps;
            match after_together(&run, slept, self.attempt.hw_decode) {
                Then::Keep => samples.push(run.sample()?),
                Then::Stop => return Err(Halt::Cancelled),
                then => {
                    if then == Then::AloneFromNowOn && !ONE_AT_A_TIME.swap(true, Ordering::Relaxed)
                    {
                        info!("the graphics card can't take samples together; one at a time");
                    }
                    if then == Then::AloneOnTheProcessor {
                        self.attempt.hw_decode = false;
                    }
                    samples.push(self.sample(start, level)?);
                }
            }
        }
        Ok(samples)
    }

    /// Runs the samples starting at `starts` at `level`, `at_once` at a time, in order. Those run
    /// together share the threads one may use for the quality measure as evenly as they go, all
    /// of them in all (ADR 0067); with `measure` false they are only encoded.
    fn runs_together(
        &self,
        starts: &[Duration],
        level: u8,
        at_once: usize,
        measure: bool,
    ) -> Vec<Run> {
        let mut runs = Vec::with_capacity(starts.len());
        for group in starts.chunks(at_once.max(1)) {
            let shares = thread_shares(share::threads_for_one(), group.len());
            thread::scope(|scope| {
                let running: Vec<_> = group
                    .iter()
                    .zip(shares)
                    .map(|(&start, threads)| {
                        let threads = measure.then_some(threads);
                        scope.spawn(move || self.run(start, level, threads))
                    })
                    .collect();
                for sample in running {
                    runs.push(sample.join().unwrap_or_else(|_| Run {
                        outcome: EncodeOutcome::Failed {
                            error: "a sample's thread failed".to_owned(),
                            made_progress: false,
                        },
                        log: String::new(),
                        frame_sizes: Vec::new(),
                    }));
                }
            });
        }
        runs
    }

    /// Runs one sample once, with `threads` for the quality measure, or only encoded, without
    /// measuring its quality, when `None`.
    fn run(&self, start: Duration, level: u8, threads: Option<usize>) -> Run {
        // Measuring above 4K can go much longer between words on a slow computer (ADR 0064).
        let patience = if standard::measured_size(self.picture).is_some() {
            encode::PATIENCE_ABOVE_4K
        } else {
            encode::PATIENCE
        };
        let mut frame_sizes = Vec::with_capacity(256);
        let (outcome, log) = encode::run_ffmpeg_patient(
            self.toolchain,
            self.sample_args(start, level, threads),
            self.control,
            patience,
            &mut |line| frame_sizes.extend(frame_size(line)),
        );
        Run {
            outcome,
            log,
            frame_sizes,
        }
    }

    fn sample(&mut self, start: Duration, level: u8) -> Result<Sample, Halt> {
        // A sample can fail because the computer slept under it; it is tried again, once
        // (ADR 0063).
        let mut tried_again = false;
        loop {
            let sleeps = share::sleeps();
            let run = self.run(start, level, Some(share::threads_for_one()));
            match run.outcome {
                EncodeOutcome::Finished => return run.sample(),
                EncodeOutcome::Cancelled => return Err(Halt::Cancelled),
                EncodeOutcome::Failed { .. } | EncodeOutcome::Stalled
                    if share::sleeps() != sleeps && !tried_again =>
                {
                    info!("the computer slept during a sample; measuring it again");
                    tried_again = true;
                }
                // GPU decoding may not take this video, or may hang on it; the samples then
                // decode on the CPU.
                EncodeOutcome::Failed { .. } | EncodeOutcome::Stalled if self.attempt.hw_decode => {
                    self.attempt.hw_decode = false;
                }
                EncodeOutcome::Failed { error, .. } => return Err(Halt::Failed(error)),
                EncodeOutcome::Stalled => {
                    return Err(Halt::Failed("FFmpeg stopped answering".to_owned()));
                }
            }
        }
    }

    /// Encodes one sample at `level` and measures it against the same part of the source, with
    /// `threads` for the quality measure on the processor; with `None`, only encodes it, exactly
    /// as it would be encoded to be measured (ADR 0067).
    fn sample_args(&self, start: Duration, level: u8, threads: Option<usize>) -> Vec<OsString> {
        let encoder = self.attempt.encoder;
        // With VMAF on the GPU too, the frames never leave it; otherwise they are handed over in
        // ordinary memory, where the reference is measured on the CPU (ADR 0030).
        let on_gpu = self.attempt.hw_decode && self.capabilities.vmaf_cuda;
        // Otherwise, decoded on the card, the frames stay there for the encoder, as in the real
        // encode, and only the reference is copied out (ADR 0065).
        let decoding = plan::card_decoding(encoder).filter(|_| self.attempt.hw_decode);
        let download = self.card_frames.filter(|_| decoding.is_some() && !on_gpu);
        let mut args: Vec<OsString> = Vec::with_capacity(64);
        let mut push = |values: &[&str]| args.extend(values.iter().map(OsString::from));
        push(&["-hide_banner", "-nostdin", "-nostats", "-v", "info"]);
        push(&["-noautorotate"]);
        if on_gpu {
            push(&["-init_hw_device", "cuda=gpu", "-filter_hw_device", "gpu"]);
            push(&["-hwaccel", "cuda", "-hwaccel_output_format", "cuda"]);
            push(&["-hwaccel_device", "gpu"]);
        } else if let Some(decoding) = decoding {
            push(&["-hwaccel", decoding.hwaccel]);
            if download.is_some() {
                push(&["-hwaccel_output_format", decoding.frames]);
            }
        }
        let (start, length) = (start.as_secs_f64(), SAMPLE.as_secs_f64());
        push(&["-ss", &format!("{start:.3}"), "-t", &format!("{length:.3}")]);
        push(&["-i"]);
        args.push(file_url(self.input));

        // The source, changed in size and rate as the real encode will be, feeds both the
        // encoder and the reference.
        let gpu_frames = on_gpu || download.is_some();
        let filters = plan::video_filters(self.rate, gpu_frames, encoder, self.capabilities);
        // The pixel format the encoder needs is changed on its branch alone.
        let format = filters.format.unwrap_or_else(|| "null".to_owned());
        let mut graph = format!("[0:{}]", self.stream);
        if let Some(rate) = &filters.rate {
            graph.push_str(rate);
            graph.push(',');
        }
        let Some(threads) = threads else {
            // Only the encoder's branch, fed the same frames.
            let format = if on_gpu { "null" } else { format.as_str() };
            graph.push_str(&format!("{format}[encode]"));
            args.extend(["-filter_complex", &graph, "-map", "[encode]"].map(OsString::from));
            self.push_encoder(&mut args, level);
            return args;
        };
        graph.push_str("split[source][reference];");
        // Above 4K, both pictures are measured scaled down to 4K (ADR 0063): VMAF's memory and
        // time grow with every pixel, and its model judges what a person sees at normal viewing,
        // which more pixels than 4K don't change.
        let smaller = standard::measured_size(self.picture);
        let (scale_gpu, scale_cpu) = match smaller {
            Some((width, height)) => (
                format!("{width}:{height}:"),
                format!("scale={width}:{height},"),
            ),
            None => (String::new(), String::new()),
        };
        if on_gpu {
            graph.push_str("[source]null[encode];");
            graph.push_str(&format!(
                "[reference]scale_cuda={scale_gpu}format=yuv420p[ref]"
            ));
        } else if let Some(download) = download {
            graph.push_str(&format!("[source]{format}[encode];"));
            graph.push_str(&format!(
                "[reference]hwdownload,format={download},{scale_cpu}format=yuv420p[ref]"
            ));
        } else {
            graph.push_str(&format!("[source]{format}[encode];"));
            graph.push_str(&format!("[reference]{scale_cpu}format=yuv420p[ref]"));
        }
        args.extend(["-filter_complex", &graph, "-map", "[encode]"].map(OsString::from));
        self.push_encoder(&mut args, level);

        // The encoded sample, decoded again, against the reference. Every other frame is enough:
        // the score moves by a few tenths at most, and the measuring takes a third less (ADR 0030).
        let measure = if on_gpu {
            format!(
                "[dec:0]{scale_cpu}format=yuv420p,hwupload_cuda[encoded];[encoded][ref]libvmaf_cuda={VMAF_OPTIONS}[vmaf]"
            )
        } else {
            format!(
                "[dec:0]{scale_cpu}format=yuv420p[encoded];[encoded][ref]libvmaf=n_threads={threads}:{VMAF_OPTIONS}[vmaf]"
            )
        };
        args.extend(
            ["-dec", "0:0", "-filter_complex", &measure, "-map", "[vmaf]"].map(OsString::from),
        );
        args.extend(["-f", "null", "-"].map(OsString::from));
        args
    }

    /// The encoder at `level` for the `[encode]` branch, reporting every encoded frame's number
    /// and size on standard output, its output thrown away.
    fn push_encoder(&self, args: &mut Vec<OsString>, level: u8) {
        args.extend(plan::video_codec_args(
            self.attempt.encoder,
            self.capabilities,
            level,
            0,
            self.picture,
        ));
        args.extend(
            [
                "-stats_enc_post:0",
                "pipe:1",
                "-stats_enc_post_fmt:0",
                "{n} {size}",
            ]
            .map(OsString::from),
        );
        args.extend(["-f", "null", "-"].map(OsString::from));
    }
}

/// How each sample's frames are pooled into one score, and how many are measured.
const VMAF_OPTIONS: &str = "pool=harmonic_mean:n_subsample=2";

/// One run of one sample: how FFmpeg ended, the end of what it said, and the size of every frame
/// it encoded.
struct Run {
    outcome: EncodeOutcome,
    log: String,
    frame_sizes: Vec<u64>,
}

impl Run {
    /// The sample a finished run measured.
    fn sample(self) -> Result<Sample, Halt> {
        match vmaf_score(&self.log) {
            Some(score) => Ok(Sample {
                score,
                frame_sizes: self.frame_sizes,
            }),
            None => Err(Halt::Failed(format!(
                "no score in FFmpeg's output: {}",
                self.log
            ))),
        }
    }
}

/// Most samples of one level measured at once (ADR 0065).
const MOST_AT_ONCE: usize = 4;

const GIB: u64 = 1 << 30;

/// Set once the graphics card couldn't open one more encoder for samples measured together:
/// from then on, for as long as Gracil runs, they go one at a time.
static ONE_AT_A_TIME: AtomicBool = AtomicBool::new(false);

/// How many of a level's `samples` are measured at once (ADR 0065): one with x265 (its samples
/// take the processor), a picture measured above 1440p (`measured`, or not known), less than
/// 4 GB free, or when going `easy` (efficiency mode, a battery, or a card that couldn't take
/// more); otherwise up to [`MOST_AT_ONCE`], with 2 GB free for each. Free memory that isn't known
/// counts as enough, as elsewhere.
fn samples_at_once(
    samples: usize,
    encoder: VideoEncoder,
    measured: Option<(u32, u32)>,
    free: Option<u64>,
    easy: bool,
) -> usize {
    let large = measured.is_none_or(|(width, height)| width.min(height) > 1440);
    if easy || encoder == VideoEncoder::X265 || large || free.is_some_and(|free| free < 4 * GIB) {
        return 1;
    }
    let room = free.map_or(usize::MAX, |free| {
        usize::try_from(free / (2 * GIB)).unwrap_or(usize::MAX)
    });
    samples.min(MOST_AT_ONCE).min(room).max(1)
}

/// `threads` shared by `samples` measured together, as evenly as they go (ADR 0067): six for four
/// is two, two, one and one. Each gets at least one.
fn thread_shares(threads: usize, samples: usize) -> Vec<usize> {
    let samples = samples.max(1);
    let (each, extra) = (threads / samples, threads % samples);
    (0..samples)
        .map(|index| (each + usize::from(index < extra)).max(1))
        .collect()
}

/// What becomes of a sample measured together with others.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Then {
    /// It was measured.
    Keep,
    /// The job was stopped.
    Stop,
    /// Measured again on its own, as one at a time would, since the computer slept under it.
    Alone,
    /// Measured again on its own, decoded on the processor: decoding on the graphics card may
    /// not take this video, as one at a time would conclude.
    AloneOnTheProcessor,
    /// Measured again on its own, and every sample after it too: the graphics card had no room
    /// for one more encoder. Not a decoding failure.
    AloneFromNowOn,
}

/// What becomes of a sample measured together with others that ended as `run`, depending on
/// whether the computer `slept` meanwhile and the samples were decoded on the card (`hw_decode`).
fn after_together(run: &Run, slept: bool, hw_decode: bool) -> Then {
    match run.outcome {
        EncodeOutcome::Finished => Then::Keep,
        EncodeOutcome::Cancelled => Then::Stop,
        EncodeOutcome::Failed { .. } | EncodeOutcome::Stalled => {
            if card_was_full(&run.log) {
                Then::AloneFromNowOn
            } else if hw_decode && !slept {
                Then::AloneOnTheProcessor
            } else {
                Then::Alone
            }
        }
    }
}

/// Whether a sample failed because its encoder couldn't be opened, as when the graphics card has
/// no room for one more encoder at once.
fn card_was_full(log: &str) -> bool {
    [
        "OpenEncodeSessionEx",
        "Error while opening encoder",
        "Could not open encoder",
    ]
    .iter()
    .any(|sign| log.contains(sign))
}

/// The format to copy frames of `pixel_format` out of the graphics card in, for a source whose
/// frames can stay on the card for the encoder: 8-bit or 10-bit 4:2:0, as nearly every camera and
/// phone records, the 10-bit kind only when the encoder writes 10-bit (`ten_bit`). An NVIDIA card
/// hands such frames to the encoder exactly as FFmpeg's own conversion would, so the samples
/// encode and score the same, to the last bit (measured on 8-bit, 10-bit and 4K clips; Intel and
/// AMD cards couldn't be tried). Anything else is copied out first, before both branches.
fn card_download(pixel_format: &str, ten_bit: bool) -> Option<&'static str> {
    match pixel_format {
        "yuv420p" | "nv12" => Some("nv12"),
        "yuv420p10le" | "p010le" if ten_bit => Some("p010le"),
        _ => None,
    }
}

/// Where the samples start: one per [`SAMPLE_EVERY`] of video, up to [`MAX_SAMPLES`], each
/// centred in an equal share of the video.
fn sample_starts(duration: Duration) -> Vec<Duration> {
    let count = (duration.as_secs_f64() / SAMPLE_EVERY.as_secs_f64())
        .clamp(1.0, f64::from(MAX_SAMPLES)) as u32;
    let last_start = duration.saturating_sub(SAMPLE);
    (0..count)
        .map(|i| {
            let centre = duration.mul_f64(f64::from(2 * i + 1) / f64::from(2 * count));
            centre.saturating_sub(SAMPLE / 2).min(last_start)
        })
        .collect()
}

/// Roughly how much the score drops from one level to the next where the search usually ends
/// (measured on real videos: between 0.6 and 1.8).
const POINTS_PER_LEVEL: f64 = 1.0;

/// The next level to measure in the search for the highest level in `levels` reaching `target`,
/// given the `scores` measured so far; `None` once the answer is known. It measures `heavy` first
/// when that is still to be searched (see [`Chooser::heavy`]). Between a level that reaches the
/// target and one that doesn't, it guesses where the target falls on a straight line through
/// their scores; otherwise it tries `hint`, then steps past a level that reaches the target by
/// the score it has to spare, then halves what is left.
fn next_level(
    scores: &BTreeMap<u8, f64>,
    levels: &RangeInclusive<u8>,
    target: f64,
    hint: Option<u8>,
    heavy: Option<u8>,
) -> Option<u8> {
    let reached = reached(scores, target);
    let missed = scores
        .iter()
        .find(|&(&level, &score)| score < target && reached.is_none_or(|r| level > r));
    let low = reached.map_or(*levels.start(), |level| level + 1);
    let high = match missed {
        Some((&level, _)) => level.checked_sub(1)?,
        None => *levels.end(),
    };
    if low > high {
        return None;
    }
    let unmeasured = |level: &u8| (low..=high).contains(level) && !scores.contains_key(level);
    if let Some(heavy) = heavy.filter(unmeasured) {
        return Some(heavy);
    }
    if let (Some(reached), Some((&missed, &missed_score))) = (reached, missed) {
        let reached_score = scores[&reached];
        let share = (reached_score - target) / (reached_score - missed_score);
        let guess = f64::from(reached) + share * f64::from(missed - reached);
        return Some((guess.round() as u8).clamp(low, high));
    }
    let hint = hint.map(|level| level.clamp(*levels.start(), *levels.end()));
    match hint.filter(unmeasured) {
        Some(level) => Some(level),
        // Only levels that reach the target so far: the score the highest has to spare says how
        // much further to go, at the points per level the two highest lost between them
        // (ADR 0067), or roughly a point per level when only one is known.
        None => match reached {
            Some(reached) => {
                let per_level = points_per_level(scores, reached).unwrap_or(POINTS_PER_LEVEL);
                let spare = ((scores[&reached] - target) / per_level).floor();
                let spare = spare.clamp(0.0, f64::from(u8::MAX)) as u8;
                Some(reached.saturating_add(spare).clamp(low, high))
            }
            None => Some(low + (high - low) / 2),
        },
    }
}

/// Levels apart within which two measured levels tell how fast the score falls near them (ADR
/// 0067). Scores fall faster as levels rise: from further apart, the fall would look gentler than
/// it is, and the step too long.
const NEAR: u8 = 3;

/// The points per level the score fell between `reached` and the nearest level below it that was
/// measured, when that one is [`NEAR`] and the score fell.
fn points_per_level(scores: &BTreeMap<u8, f64>, reached: u8) -> Option<f64> {
    let (&below, &below_score) = scores.range(..reached).next_back()?;
    if reached - below > NEAR {
        return None;
    }
    let fall = (below_score - scores[&reached]) / f64::from(reached - below);
    (fall > 0.0).then_some(fall)
}

/// The highest measured level whose score reaches `target`.
fn reached(scores: &BTreeMap<u8, f64>, target: f64) -> Option<u8> {
    scores
        .iter()
        .rev()
        .find(|&(_, &score)| score >= target)
        .map(|(&level, _)| level)
}

/// The size in a `{n} {size}` frame line.
fn frame_size(line: &str) -> Option<u64> {
    line.split_whitespace().nth(1)?.parse().ok()
}

/// The last `VMAF score: 94.87` FFmpeg reported.
fn vmaf_score(log: &str) -> Option<f64> {
    log.rmatch_indices("VMAF score:")
        .next()
        .and_then(|(at, label)| log[at + label.len()..].split_whitespace().next())
        .and_then(|number| number.parse().ok())
        .filter(|score: &f64| score.is_finite())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn samples_spread_through_the_video() {
        let secs = Duration::from_secs;
        assert_eq!(sample_starts(secs(6)), [secs(1)]);
        assert_eq!(
            sample_starts(secs(30)),
            [Duration::from_millis(5_500), Duration::from_millis(20_500)]
        );
        assert_eq!(
            sample_starts(secs(600)),
            [secs(73), secs(223), secs(373), secs(523)]
        );
    }

    #[test]
    fn a_sample_never_runs_past_the_end() {
        let duration = Duration::from_secs(7);
        for start in sample_starts(duration) {
            assert!(start + SAMPLE <= duration);
        }
    }

    #[test]
    fn reads_the_last_score() {
        let log =
            "[Parsed_libvmaf_1 @ 0000026207d06340] VMAF score: 95.145022\n[out#1/null] video:0KiB";
        assert_eq!(vmaf_score(log), Some(95.145022));
        assert_eq!(vmaf_score("Conversion failed!"), None);
        assert_eq!(vmaf_score("VMAF score: nan"), None);
    }

    #[test]
    fn other_encoders_keep_their_fallback() {
        let found = Found::Level {
            encoder: VideoEncoder::Nvenc,
            level: 33,
        };
        assert_eq!(found.level_for(VideoEncoder::Nvenc), 33);
        assert_eq!(found.level_for(VideoEncoder::X265), 26);
        assert_eq!(Found::Fallback.level_for(VideoEncoder::Nvenc), 27);
    }

    /// Runs the search against a score curve, returning the answer and the levels measured.
    fn search(
        curve: impl Fn(u8) -> f64,
        levels: RangeInclusive<u8>,
        target: f64,
        hint: Option<u8>,
        known: &[u8],
    ) -> (Option<u8>, Vec<u8>) {
        let next =
            |scores: &_, levels: &_, target, hint| next_level(scores, levels, target, hint, None);
        search_by(next, curve, levels, target, hint, known)
    }

    /// As [`search`], with the search as it was before ADR 0067: a point per level past a level
    /// that reaches the target, whatever else was measured.
    fn before(
        curve: impl Fn(u8) -> f64,
        levels: RangeInclusive<u8>,
        target: f64,
        hint: Option<u8>,
        known: &[u8],
    ) -> (Option<u8>, Vec<u8>) {
        search_by(next_level_before, curve, levels, target, hint, known)
    }

    /// [`next_level`] before ADR 0067.
    fn next_level_before(
        scores: &BTreeMap<u8, f64>,
        levels: &RangeInclusive<u8>,
        target: f64,
        hint: Option<u8>,
    ) -> Option<u8> {
        let reached = reached(scores, target);
        let missed = scores
            .iter()
            .find(|&(&level, &score)| score < target && reached.is_none_or(|r| level > r));
        let low = reached.map_or(*levels.start(), |level| level + 1);
        let high = match missed {
            Some((&level, _)) => level.checked_sub(1)?,
            None => *levels.end(),
        };
        if low > high {
            return None;
        }
        if let (Some(reached), Some((&missed, &missed_score))) = (reached, missed) {
            let reached_score = scores[&reached];
            let share = (reached_score - target) / (reached_score - missed_score);
            let guess = f64::from(reached) + share * f64::from(missed - reached);
            return Some((guess.round() as u8).clamp(low, high));
        }
        let hint = hint.map(|level| level.clamp(*levels.start(), *levels.end()));
        match hint.filter(|level| (low..=high).contains(level) && !scores.contains_key(level)) {
            Some(level) => Some(level),
            None => match reached {
                Some(reached) => {
                    let spare = ((scores[&reached] - target) / POINTS_PER_LEVEL).floor() as u8;
                    Some(reached.saturating_add(spare).clamp(low, high))
                }
                None => Some(low + (high - low) / 2),
            },
        }
    }

    fn search_by(
        next: impl Fn(&BTreeMap<u8, f64>, &RangeInclusive<u8>, f64, Option<u8>) -> Option<u8>,
        curve: impl Fn(u8) -> f64,
        levels: RangeInclusive<u8>,
        target: f64,
        hint: Option<u8>,
        known: &[u8],
    ) -> (Option<u8>, Vec<u8>) {
        let mut scores: BTreeMap<u8, f64> = known.iter().map(|&l| (l, curve(l))).collect();
        let mut measured = Vec::new();
        while let Some(level) = next(&scores, &levels, target, hint) {
            assert!(!scores.contains_key(&level), "measured {level} twice");
            measured.push(level);
            scores.insert(level, curve(level));
        }
        let in_range: BTreeMap<u8, f64> = scores
            .into_iter()
            .filter(|(level, _)| levels.contains(level))
            .collect();
        (reached(&in_range, target), measured)
    }

    /// Roughly how one of the test videos scores: 99.9 at CQ 24, falling faster as CQ rises.
    fn curve(level: u8) -> f64 {
        let x = f64::from(level) - 24.0;
        99.9 - 0.06 * x * x - 0.2 * x
    }

    #[test]
    fn finds_the_highest_level_reaching_the_target() {
        for target in [95.0, 90.0, 80.0] {
            let expected = (24..=42).rev().find(|&l| curve(l) >= target);
            let (found, measured) = search(curve, 24..=42, target, None, &[]);
            assert_eq!(found, expected, "target {target}");
            assert!(measured.len() <= 5, "target {target}: {measured:?}");
            let (_, earlier) = before(curve, 24..=42, target, None, &[]);
            assert!(measured.len() <= earlier.len(), "{measured:?} {earlier:?}");
        }
    }

    #[test]
    fn content_that_never_reaches_the_target() {
        let (found, measured) = search(|_| 80.0, 24..=38, 95.0, None, &[]);
        assert_eq!(found, None);
        assert_eq!(measured.last(), Some(&24));
        assert!(measured.len() <= before(|_| 80.0, 24..=38, 95.0, None, &[]).1.len());
        let (found, measured) = search(|_| 99.0, 24..=38, 95.0, None, &[]);
        assert_eq!(found, Some(38));
        assert!(measured.len() <= before(|_| 99.0, 24..=38, 95.0, None, &[]).1.len());
    }

    #[test]
    fn earlier_measurements_and_hints_save_steps() {
        let fresh = search(curve, 24..=42, 90.0, None, &[]).1.len();
        assert!(fresh <= before(curve, 24..=42, 90.0, None, &[]).1.len());
        let (found, measured) = search(curve, 24..=42, 90.0, None, &[31, 35, 36]);
        assert_eq!(found, Some(35));
        assert!(measured.is_empty(), "{measured:?}");
        let (found, measured) = search(curve, 24..=42, 90.0, Some(35), &[]);
        assert!(measured.len() <= before(curve, 24..=42, 90.0, Some(35), &[]).1.len());
        assert_eq!(found, Some(35));
        assert_eq!(measured[0], 35);
        assert!(measured.len() <= fresh, "{measured:?}");
    }

    /// The arguments of one NVENC sample of a 1080p video, joined.
    fn sample_line(
        card_frames: Option<&'static str>,
        hw_decode: bool,
        rate: Option<Rate>,
    ) -> String {
        sample_on(VideoEncoder::Nvenc, card_frames, hw_decode, rate)
    }

    #[test]
    fn intel_and_amd_cards_keep_their_frames_too() {
        let intel = sample_on(VideoEncoder::Qsv, Some("nv12"), true, None);
        assert!(
            intel.contains("-hwaccel qsv -hwaccel_output_format qsv -ss"),
            "{intel}"
        );
        assert!(intel.contains(
            "[0:0]split[source][reference];[source]scale_qsv=format=p010[encode];\
             [reference]hwdownload,format=nv12,format=yuv420p[ref]"
        ));
        let amd = sample_on(VideoEncoder::Amf, Some("p010le"), true, None);
        assert!(amd.contains("-hwaccel d3d11va -hwaccel_output_format d3d11 -ss"));
        assert!(amd.contains(
            "[source]null[encode];[reference]hwdownload,format=p010le,format=yuv420p[ref]"
        ));
        // Other sources: copied out of the card first; then converted on the processor.
        let other = sample_on(VideoEncoder::Amf, None, true, None);
        assert!(
            other.contains("-noautorotate -hwaccel d3d11va -ss"),
            "{other}"
        );
        assert!(other.contains("[source]format=p010le[encode]"));
        assert!(!other.contains("scale_d3d11"));
        // Decoding on the processor after the card failed.
        let processor = sample_on(VideoEncoder::Qsv, Some("nv12"), false, None);
        assert!(!processor.contains("hwaccel"));
        assert!(processor.contains("[source]format=p010le[encode]"));
    }

    /// The arguments of one sample of a 1080p video with `encoder`, joined.
    fn sample_on(
        encoder: VideoEncoder,
        card_frames: Option<&'static str>,
        hw_decode: bool,
        rate: Option<Rate>,
    ) -> String {
        args_on(encoder, card_frames, hw_decode, rate, Some(6))
    }

    /// The arguments of one sample of a 1080p video with `encoder`, encoded alone, joined.
    fn alone_on(
        encoder: VideoEncoder,
        card_frames: Option<&'static str>,
        hw_decode: bool,
    ) -> String {
        args_on(encoder, card_frames, hw_decode, None, None)
    }

    fn args_on(
        encoder: VideoEncoder,
        card_frames: Option<&'static str>,
        hw_decode: bool,
        rate: Option<Rate>,
        threads: Option<usize>,
    ) -> String {
        let toolchain = Toolchain::stand_in();
        let control = JobControl::default();
        let capabilities = Capabilities {
            encoder,
            card_decode: true,
            hardware: standard::HardwareLevel::Full,
            x265_10bit: true,
            vmaf: true,
            vmaf_cuda: false,
        };
        let search = Search {
            toolchain: &toolchain,
            input: Path::new(r"C:\v\a.mp4"),
            stream: 0,
            rate,
            attempt: Attempt {
                encoder,
                hw_decode,
                preservation: plan::Preservation::Full,
            },
            capabilities: &capabilities,
            picture: Some((1920, 1080)),
            card_frames,
            starts: Vec::new(),
            duration: Duration::from_secs(60),
            control: &control,
            measured: BTreeMap::new(),
        };
        search
            .sample_args(Duration::from_secs(8), 30, threads)
            .iter()
            .map(|arg| arg.to_str().unwrap_or_default().to_owned())
            .collect::<Vec<_>>()
            .join(" ")
    }

    #[test]
    fn decoded_on_the_card_the_frames_stay_there_for_the_encoder() {
        for (pixel_format, download) in [("yuv420p", "nv12"), ("yuv420p10le", "p010le")] {
            let frames = card_download(pixel_format, true);
            assert_eq!(frames, Some(download));
            let line = sample_line(frames, true, None);
            assert!(
                line.contains("-hwaccel cuda -hwaccel_output_format cuda -ss 8.000"),
                "{line}"
            );
            assert!(
                line.contains(&format!(
                    "[0:0]split[source][reference];[source]null[encode];\
                     [reference]hwdownload,format={download},format=yuv420p[ref]"
                )),
                "{line}"
            );
            assert!(!line.contains("scale_cuda"), "{line}");
        }
        // The frame rate changes on the card, as in the real encode.
        let line = sample_line(Some("nv12"), true, Some(Rate { num: 30, den: 1 }));
        assert!(line.contains("[0:0]fps=30/1,split[source][reference];[source]null[encode]"));
    }

    #[test]
    fn other_sources_are_copied_out_of_the_card_first() {
        // 4:4:4, 12-bit, and 10-bit for an 8-bit encoder.
        assert_eq!(card_download("yuv444p", true), None);
        assert_eq!(card_download("yuv420p12le", true), None);
        assert_eq!(card_download("yuv420p10le", false), None);
        let line = sample_line(None, true, None);
        assert!(line.contains("-noautorotate -hwaccel cuda -ss"), "{line}");
        assert!(line.contains("[source]format=p010le[encode];[reference]format=yuv420p[ref]"));
        // Decoding on the processor after the card failed.
        let line = sample_line(Some("nv12"), false, None);
        assert!(!line.contains("hwaccel"), "{line}");
        assert!(line.contains("[source]format=p010le[encode];[reference]format=yuv420p[ref]"));
    }

    #[test]
    fn a_wrong_hint_never_chooses_a_level_that_misses_the_target() {
        // A score curve with a bump in it, as real ones sometimes have.
        let bumpy = |level: u8| curve(level) + if level == 33 { 1.5 } else { 0.0 };
        for (smooth, score) in [(true, curve as fn(u8) -> f64), (false, bumpy)] {
            for target in [95.0, 90.0] {
                let (fresh, _) = search(score, 24..=42, target, None, &[]);
                for hint in (0..=60).map(Some).chain([None]) {
                    let (found, measured) = search(score, 24..=42, target, hint, &[]);
                    let found = found.unwrap();
                    assert!(score(found) >= target, "hint {hint:?} chose {found}");
                    assert!((24..=42).contains(&found));
                    assert!(measured.iter().all(|level| (24..=42).contains(level)));
                    if smooth {
                        assert_eq!(Some(found), fresh, "hint {hint:?}");
                    }
                    let (_, earlier) = before(score, 24..=42, target, hint, &[]);
                    assert!(
                        measured.len() <= earlier.len(),
                        "hint {hint:?}: {measured:?} {earlier:?}"
                    );
                }
            }
        }
    }

    /// Steadily falling score curves, gentle to steep, as real videos have.
    fn falling(offset: f64, steep: f64) -> impl Fn(u8) -> f64 + Copy {
        move |level| {
            let x = f64::from(level) - 24.0;
            99.9 + offset - steep * (0.06 * x * x + 0.2 * x)
        }
    }

    #[test]
    fn stepping_by_what_the_last_two_levels_lost_measures_no_more() {
        let (mut now, mut earlier) = (0, 0);
        for offset in (-12..=4).map(|tenth| f64::from(tenth) * 0.5) {
            for steep in [0.4, 0.7, 1.0, 1.5] {
                let score = falling(offset, steep);
                for target in [95.0, 90.0] {
                    for levels in [24..=38, 24..=42] {
                        let expected = levels.clone().rev().find(|&l| score(l) >= target);
                        for hint in [None, Some(28), Some(31), Some(35), Some(40)] {
                            let (found, measured) =
                                search(score, levels.clone(), target, hint, &[]);
                            let (old, before) = before(score, levels.clone(), target, hint, &[]);
                            assert_eq!(found, expected, "{offset} {steep} {target} {hint:?}");
                            assert_eq!(found, old);
                            assert!(
                                measured.len() <= before.len(),
                                "{offset} {steep} {target} {hint:?}: {measured:?} {before:?}"
                            );
                            now += measured.len();
                            earlier += before.len();
                        }
                    }
                }
            }
        }
        assert!(now < earlier, "{now} {earlier}");
    }

    /// A video's score and size at each level, measured by reading them off curves.
    struct Curves<S, V> {
        score: S,
        video: V,
        measured: BTreeMap<u8, Measured>,
        alone: Vec<u8>,
    }

    impl<S: Fn(u8) -> f64, V: Fn(u8) -> f64> Meter for Curves<S, V> {
        fn measure(&mut self, level: u8) -> Result<Measured, Halt> {
            let measured = Measured {
                score: (self.score)(level),
                video_bytes: Some((self.video)(level)),
            };
            Ok(*self.measured.entry(level).or_insert(measured))
        }

        fn measured(&self) -> &BTreeMap<u8, Measured> {
            &self.measured
        }

        fn size_alone(&mut self, level: u8) -> Result<Option<f64>, Halt> {
            self.alone.push(level);
            Ok(Some((self.video)(level)))
        }
    }

    /// What Gracil must choose for a video whose scores and sizes fall steadily as levels rise:
    /// the highest level reaching 95 if it is small enough, otherwise the highest reaching 90
    /// if that one is, otherwise nothing.
    fn rightful(
        score: impl Fn(u8) -> f64,
        video: impl Fn(u8) -> f64,
        encoder: VideoEncoder,
        sizes: Sizes,
    ) -> Found {
        let small = |level: u8| sizes.worth_it(sizes.predicted(Some(video(level))));
        let levels = standard::levels(encoder);
        let first = levels
            .clone()
            .rev()
            .find(|&l| score(l) >= standard::TARGET_SCORE)
            .unwrap_or(*levels.start());
        if small(first) {
            return Found::Level {
                encoder,
                level: first,
            };
        }
        match standard::again_levels(encoder)
            .rev()
            .find(|&l| score(l) >= standard::AGAIN_SCORE)
        {
            Some(level) if small(level) => Found::Level { encoder, level },
            _ => Found::AsSmallAsItGets,
        }
    }

    /// [`Chooser::highest`] before ADR 0067, giving up as `give_up` says.
    fn highest_before(
        meter: &mut impl Meter,
        levels: RangeInclusive<u8>,
        target: f64,
        hint: Option<u8>,
        give_up: impl Fn(&Measured) -> bool,
    ) -> Option<Option<u8>> {
        loop {
            let scores: BTreeMap<u8, f64> = meter
                .measured()
                .iter()
                .filter(|(level, _)| levels.contains(level))
                .map(|(&level, measured)| (level, measured.score))
                .collect();
            let Some(level) = next_level_before(&scores, &levels, target, hint) else {
                return Some(reached(&scores, target));
            };
            if give_up(&meter.measure(level).ok()?) {
                return None;
            }
        }
    }

    /// The choice before ADR 0067, and how many levels it measured.
    fn chosen_before(
        meter: &mut impl Meter,
        encoder: VideoEncoder,
        sizes: Sizes,
    ) -> (Found, usize) {
        let small = |measured: &Measured| sizes.worth_it(sizes.predicted(measured.video_bytes));
        let levels = standard::levels(encoder);
        let target = standard::TARGET_SCORE;
        let first = highest_before(meter, levels.clone(), target, None, |measured| {
            measured.score < target && !small(measured)
        });
        let chosen = first.map(|best| best.unwrap_or(*levels.start()));
        if let Some(level) = chosen
            && meter.measure(level).is_ok_and(|measured| small(&measured))
        {
            return (Found::Level { encoder, level }, meter.measured().len());
        }
        let levels = standard::again_levels(encoder);
        let again = highest_before(meter, levels, standard::AGAIN_SCORE, chosen, |_| false);
        let found = match again.flatten() {
            Some(level) if meter.measure(level).is_ok_and(|measured| small(&measured)) => {
                Found::Level { encoder, level }
            }
            _ => Found::AsSmallAsItGets,
        };
        (found, meter.measured().len())
    }

    #[test]
    fn the_level_chosen_is_the_rightful_one_with_fewer_levels_measured() {
        let encoder = VideoEncoder::Nvenc;
        let (mut now, mut alone, mut earlier) = (0, 0, 0);
        for offset in (-12..=4).map(|tenth| f64::from(tenth) * 0.5) {
            for steep in [0.5, 1.0, 1.5] {
                let score = falling(offset, steep);
                for per_level in [0.06, 0.1, 0.15] {
                    let video = move |level: u8| 50e6 * (-per_level * f64::from(level - 24)).exp();
                    for original in [5e6, 8e6, 12e6, 20e6, 40e6, 80e6] {
                        let sizes = Sizes {
                            original: original as u64,
                            kept: 1e6,
                        };
                        let rightful = rightful(score, video, encoder, sizes);
                        let curves = || Curves {
                            score,
                            video,
                            measured: BTreeMap::new(),
                            alone: Vec::new(),
                        };
                        let (old, old_count) = chosen_before(&mut curves(), encoder, sizes);
                        assert_eq!(old, rightful, "{offset} {steep} {per_level} {original}");
                        for compressed in [false, true] {
                            for guess in [None, Some(30), Some(34), Some(37)] {
                                let mut meter = curves();
                                let start = Start { guess, compressed };
                                let key = HintKey {
                                    encoder,
                                    size: 2,
                                    codec: "h264".to_owned(),
                                    again: false,
                                };
                                let found = choose(
                                    &mut meter,
                                    encoder,
                                    Some(sizes),
                                    &Hints::default(),
                                    key,
                                    start,
                                );
                                let case =
                                    format!("{offset} {steep} {per_level} {original} {start:?}");
                                assert_eq!(found.ok(), Some(rightful.clone()), "{case}");
                                assert!(meter.alone.len() <= 1, "{case}");
                                if !compressed && guess.is_none() {
                                    now += meter.measured.len();
                                    alone += meter.alone.len();
                                    earlier += old_count;
                                }
                            }
                        }
                    }
                }
            }
        }
        assert!(
            now + alone < earlier,
            "{now} + {alone} alone, against {earlier}"
        );
    }

    #[test]
    fn a_video_too_big_even_at_the_smallest_level_is_told_at_once() {
        let encoder = VideoEncoder::Nvenc;
        let sizes = Sizes {
            original: 10_000_000,
            kept: 0.0,
        };
        let key = HintKey {
            encoder,
            size: 2,
            codec: "hevc".to_owned(),
            again: false,
        };
        let mut meter = Curves {
            score: falling(0.0, 1.0),
            video: |level: u8| 20e6 * (-0.05 * f64::from(level - 24)).exp(),
            measured: BTreeMap::new(),
            alone: Vec::new(),
        };
        let start = Start {
            guess: None,
            compressed: true,
        };
        let found = choose(
            &mut meter,
            encoder,
            Some(sizes),
            &Hints::default(),
            key.clone(),
            start,
        );
        assert_eq!(found.ok(), Some(Found::AsSmallAsItGets));
        assert!(meter.measured.is_empty(), "{:?}", meter.measured);
        assert_eq!(meter.alone, [42]);
        // Not taken for compressed: once a level too big is estimated to stay too big all the
        // way. The first, 31, shrinks as usual by its estimate; the second, 36, says by how much.
        meter.measured.clear();
        meter.alone.clear();
        let found = choose(
            &mut meter,
            encoder,
            Some(sizes),
            &Hints::default(),
            key,
            Start::default(),
        );
        assert_eq!(found.ok(), Some(Found::AsSmallAsItGets));
        assert_eq!(meter.measured.keys().copied().collect::<Vec<_>>(), [31, 36]);
        assert_eq!(meter.alone, [42]);
    }

    #[test]
    fn the_first_guess_follows_the_bits_per_pixel() {
        let nvenc = VideoEncoder::Nvenc;
        let guesses: Vec<u8> = [0.01, 0.03, 0.06, 0.12, 0.25, 0.5, 1.0, 4.0]
            .into_iter()
            .map(|bits| first_guess(nvenc, bits))
            .collect();
        assert!(
            guesses.windows(2).all(|pair| pair[0] >= pair[1]),
            "{guesses:?}"
        );
        assert!(
            guesses
                .iter()
                .all(|level| standard::levels(nvenc).contains(level))
        );
        for encoder in [VideoEncoder::Qsv, VideoEncoder::Amf, VideoEncoder::X265] {
            assert!(standard::levels(encoder).contains(&first_guess(encoder, 0.0001)));
            assert!(standard::levels(encoder).contains(&first_guess(encoder, 100.0)));
        }
        assert_eq!(
            bits_per_pixel(8e6, Some((1920, 1080)), Some(30.0)).map(|b| (b * 1000.0).round()),
            Some(129.0)
        );
        assert_eq!(bits_per_pixel(8e6, None, Some(30.0)), None);
        assert_eq!(bits_per_pixel(8e6, Some((1920, 1080)), Some(0.0)), None);
    }

    #[test]
    fn hints_are_kept_by_encoder_size_and_codec() {
        let hints = Hints::default();
        let key = |encoder, size, codec: &str| HintKey {
            encoder,
            size: size_step(size),
            codec: codec.to_owned(),
            again: false,
        };
        let full_hd = key(VideoEncoder::Nvenc, Some((1920, 1080)), "h264");
        hints.remember(full_hd.clone(), 33);
        assert_eq!(hints.get(&full_hd), Some(33));
        // A vertical phone video of the same size is the same kind.
        assert_eq!(
            hints.get(&key(VideoEncoder::Nvenc, Some((1080, 1920)), "h264")),
            Some(33)
        );
        for other in [
            key(VideoEncoder::Nvenc, Some((3840, 2160)), "h264"),
            key(VideoEncoder::Nvenc, Some((1920, 1080)), "hevc"),
            key(VideoEncoder::X265, Some((1920, 1080)), "h264"),
            HintKey {
                again: true,
                ..full_hd.clone()
            },
        ] {
            assert_eq!(hints.get(&other), None, "{other:?}");
        }
        assert_eq!(size_step(None), 0);
        assert_eq!(size_step(Some((7680, 4320))), 5);
    }

    #[test]
    fn how_many_samples_are_measured_at_once() {
        let nvenc = VideoEncoder::Nvenc;
        let full_hd = Some((1920, 1080));
        let free = |gib: u64| Some(gib * GIB);
        assert_eq!(samples_at_once(4, nvenc, full_hd, free(16), false), 4);
        assert_eq!(samples_at_once(2, nvenc, full_hd, free(16), false), 2);
        assert_eq!(
            samples_at_once(4, nvenc, Some((1080, 1920)), free(16), false),
            4
        );
        assert_eq!(
            samples_at_once(4, nvenc, Some((2560, 1440)), free(16), false),
            4
        );
        // 2 GB for each.
        assert_eq!(samples_at_once(4, nvenc, full_hd, free(6), false), 3);
        assert_eq!(samples_at_once(4, nvenc, full_hd, None, false), 4);
        // One at a time.
        for (encoder, picture, memory, easy) in [
            (VideoEncoder::X265, full_hd, free(16), false),
            (nvenc, Some((3840, 2160)), free(16), false),
            (nvenc, None, free(16), false),
            (nvenc, full_hd, Some(4 * GIB - 1), false),
            (nvenc, full_hd, free(16), true),
        ] {
            assert_eq!(
                samples_at_once(4, encoder, picture, memory, easy),
                1,
                "{encoder:?} {picture:?} {memory:?} {easy}"
            );
        }
        assert_eq!(samples_at_once(1, nvenc, full_hd, free(16), false), 1);
    }

    #[test]
    fn samples_together_share_every_thread() {
        assert_eq!(thread_shares(6, 4), [2, 2, 1, 1]);
        assert_eq!(thread_shares(8, 4), [2, 2, 2, 2]);
        assert_eq!(thread_shares(6, 1), [6]);
        assert_eq!(thread_shares(9, 2), [5, 4]);
        // Never fewer than one, as before.
        assert_eq!(thread_shares(2, 4), [1, 1, 1, 1]);
        for (threads, samples) in [(6, 4), (12, 3), (5, 2), (7, 4)] {
            assert_eq!(
                thread_shares(threads, samples).iter().sum::<usize>(),
                threads
            );
        }
    }

    #[test]
    fn a_sample_encoded_alone_is_encoded_as_when_measured() {
        let measured = sample_on(VideoEncoder::Nvenc, Some("nv12"), true, None);
        let alone = alone_on(VideoEncoder::Nvenc, Some("nv12"), true);
        assert!(
            !alone.contains("libvmaf") && !alone.contains("-dec"),
            "{alone}"
        );
        assert!(alone.contains("[0:0]null[encode]"), "{alone}");
        let encoder = |line: &str| {
            let at = line.find("-map [encode]").unwrap_or_default();
            line[at..]
                .split(" -f null -")
                .next()
                .unwrap_or_default()
                .to_owned()
        };
        assert_eq!(encoder(&measured), encoder(&alone));
        // Before the input, the same decoding.
        let input = |line: &str| {
            line.split("-filter_complex")
                .next()
                .unwrap_or_default()
                .to_owned()
        };
        assert_eq!(input(&measured), input(&alone));
        let processor = alone_on(VideoEncoder::Nvenc, None, false);
        assert!(
            processor.contains("[0:0]format=p010le[encode]"),
            "{processor}"
        );
    }

    #[test]
    fn a_sample_that_fails_together_is_measured_again_alone() {
        let run = |outcome, log: &str| Run {
            outcome,
            log: log.to_owned(),
            frame_sizes: Vec::new(),
        };
        let failed = || EncodeOutcome::Failed {
            error: String::new(),
            made_progress: false,
        };
        let full = "[hevc_nvenc @ 0] OpenEncodeSessionEx failed: incompatible client key (21)\n\
                    [vost#0:0/hevc_nvenc] Error while opening encoder";
        // The card had no room: one at a time from now on, still decoding on the card.
        assert_eq!(
            after_together(&run(failed(), full), false, true),
            Then::AloneFromNowOn
        );
        // Anything else: as one at a time, on the processor, unless the computer slept.
        let broken = "[h264 @ 0] decode_slice_header error";
        assert_eq!(
            after_together(&run(failed(), broken), false, true),
            Then::AloneOnTheProcessor
        );
        assert_eq!(
            after_together(&run(EncodeOutcome::Stalled, ""), false, true),
            Then::AloneOnTheProcessor
        );
        assert_eq!(
            after_together(&run(failed(), broken), true, true),
            Then::Alone
        );
        assert_eq!(
            after_together(&run(failed(), broken), false, false),
            Then::Alone
        );
        assert_eq!(
            after_together(&run(EncodeOutcome::Finished, ""), false, true),
            Then::Keep
        );
        assert_eq!(
            after_together(&run(EncodeOutcome::Cancelled, full), false, true),
            Then::Stop
        );
    }

    #[test]
    fn reads_frame_sizes() {
        assert_eq!(frame_size("0 48594"), Some(48594));
        assert_eq!(frame_size("12 7444\r"), Some(7444));
        assert_eq!(frame_size("frame=12"), None);
    }

    #[test]
    fn predicts_from_typical_frames_not_key_frames() {
        // Four seconds at 25 fps: one 50 kB key frame, then 99 frames of 10 kB.
        let sample = |score| Sample {
            score,
            frame_sizes: [50_000].into_iter().chain([10_000; 99]).collect(),
        };
        let measured = Measured::of(&[sample(94.0), sample(96.0)], Duration::from_secs(1000));
        assert_eq!(measured.score, 95.0);
        // 25 000 frames of 10 kB, plus a key frame every 250.
        assert_eq!(measured.video_bytes, Some(250_000_000.0 + 5_000_000.0));

        let silent = Sample {
            score: 95.0,
            frame_sizes: Vec::new(),
        };
        assert_eq!(
            Measured::of(&[silent], Duration::from_secs(10)).video_bytes,
            None
        );
    }
}
