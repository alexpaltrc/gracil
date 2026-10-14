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

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fs;
use std::ops::RangeInclusive;
use std::path::Path;
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

/// Searches the level for `plan`'s main video, encoded the way `plan.attempt` would.
pub fn find_level(
    toolchain: &Toolchain,
    plan: &EncodePlan,
    info: &MediaInfo,
    control: &JobControl,
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

    let mut search = Search {
        toolchain,
        input: &plan.input,
        stream: index,
        rate,
        attempt: plan.attempt,
        capabilities: &plan.capabilities,
        picture: plan.picture,
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
    match choose(&mut search, sizes) {
        Ok(found) => found,
        Err(Halt::Cancelled) => Found::Cancelled,
        Err(Halt::Failed(error)) => {
            warn!(%error, "could not measure quality; using the usual level");
            Found::Fallback
        }
        Err(Halt::GaveUp) => unreachable!("only the first search gives up, and it is handled"),
    }
}

/// Picks the level that looks the same. If that is not worth it, the video was already compressed,
/// and it gets the level that loses a little (ADR 0033), when that one is worth it.
fn choose(search: &mut Search, sizes: Option<Sizes>) -> Result<Found, Halt> {
    let encoder = search.attempt.encoder;
    let levels = standard::levels(encoder);
    // A level that already misses the target and is still too big: every level that reaches the
    // target is bigger still.
    let give_up = |measured: &Measured| {
        measured.score < standard::TARGET_SCORE
            && sizes.is_some_and(|sizes| !sizes.worth_it(sizes.predicted(measured)))
    };
    let chosen = match search.highest(levels.clone(), standard::TARGET_SCORE, None, &give_up) {
        // Content that never reaches the target gets the best quality searched.
        Ok(best) => Some(best.unwrap_or(*levels.start())),
        Err(Halt::GaveUp) => None,
        Err(halt) => return Err(halt),
    };
    let Some(sizes) = sizes else {
        let level = chosen.unwrap_or(*levels.start());
        info!(level, "chose a quality level");
        return Ok(Found::Level { encoder, level });
    };
    if let Some(found) = if_worth_it(search, sizes, chosen, "chose a quality level")? {
        return Ok(found);
    }
    info!(
        original = sizes.original,
        "already compressed; measuring the level for compressing again"
    );

    let never = |_: &Measured| false;
    let again = search.highest(
        standard::again_levels(encoder),
        standard::AGAIN_SCORE,
        None,
        &never,
    )?;
    if let Some(found) = if_worth_it(search, sizes, again, "chose a level for compressing again")? {
        return Ok(found);
    }
    info!(original = sizes.original, "already as small as it gets");
    Ok(Found::AsSmallAsItGets)
}

/// `level`, if its predicted size is worth it.
fn if_worth_it(
    search: &mut Search,
    sizes: Sizes,
    level: Option<u8>,
    chose: &str,
) -> Result<Option<Found>, Halt> {
    let Some(level) = level else {
        return Ok(None);
    };
    let predicted = sizes.predicted(&search.measure(level)?);
    if !sizes.worth_it(predicted) {
        return Ok(None);
    }
    info!(level, ?predicted, original = sizes.original, "{chose}");
    let encoder = search.attempt.encoder;
    Ok(Some(Found::Level { encoder, level }))
}

/// The original's size, and what the result's size is judged against.
#[derive(Debug, Clone, Copy)]
struct Sizes {
    original: u64,
    /// Predicted bytes of everything in the result except the main video.
    kept: f64,
}

impl Sizes {
    fn predicted(self, measured: &Measured) -> Option<u64> {
        measured
            .video_bytes
            .map(|video| (video + self.kept).max(0.0).round() as u64)
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
    /// The caller's condition said the search can't succeed.
    GaveUp,
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
    starts: Vec<Duration>,
    duration: Duration,
    control: &'a JobControl,
    measured: BTreeMap<u8, Measured>,
}

impl<'a> Search<'a> {
    /// The highest level in `levels` whose score reaches `target`, assuming scores fall as
    /// levels rise. Starts from what is already measured, then tries `hint` first if given. Stops
    /// with [`Halt::GaveUp`] as soon as `give_up` holds for a new measurement.
    fn highest(
        &mut self,
        levels: RangeInclusive<u8>,
        target: f64,
        hint: Option<u8>,
        give_up: &dyn Fn(&Measured) -> bool,
    ) -> Result<Option<u8>, Halt> {
        loop {
            let scores: BTreeMap<u8, f64> = self
                .measured
                .iter()
                .filter(|(level, _)| levels.contains(level))
                .map(|(&level, measured)| (level, measured.score))
                .collect();
            let Some(level) = next_level(&scores, &levels, target, hint) else {
                return Ok(reached(&scores, target));
            };
            let measured = self.measure(level)?;
            if give_up(&measured) {
                return Err(Halt::GaveUp);
            }
        }
    }

    /// The average score of every sample at `level`, and the predicted size. Measured once.
    fn measure(&mut self, level: u8) -> Result<Measured, Halt> {
        if let Some(measured) = self.measured.get(&level) {
            return Ok(*measured);
        }
        let mut samples = Vec::with_capacity(self.starts.len());
        for index in 0..self.starts.len() {
            samples.push(self.sample(self.starts[index], level)?);
        }
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

    fn sample(&mut self, start: Duration, level: u8) -> Result<Sample, Halt> {
        // A sample can fail because the computer slept under it; it is tried again, once
        // (ADR 0063).
        let mut tried_again = false;
        loop {
            let sleeps = share::sleeps();
            let mut frame_sizes = Vec::with_capacity(256);
            let (outcome, log) = encode::run_ffmpeg(
                self.toolchain,
                self.sample_args(start, level),
                self.control,
                &mut |line| frame_sizes.extend(frame_size(line)),
            );
            match outcome {
                EncodeOutcome::Finished => {
                    return match vmaf_score(&log) {
                        Some(score) => Ok(Sample { score, frame_sizes }),
                        None => Err(Halt::Failed(format!("no score in FFmpeg's output: {log}"))),
                    };
                }
                EncodeOutcome::Cancelled => return Err(Halt::Cancelled),
                EncodeOutcome::Failed { .. } if share::sleeps() != sleeps && !tried_again => {
                    info!("the computer slept during a sample; measuring it again");
                    tried_again = true;
                }
                // GPU decoding may not take this video; the samples then decode on the CPU.
                EncodeOutcome::Failed { .. } if self.attempt.hw_decode => {
                    self.attempt.hw_decode = false;
                }
                EncodeOutcome::Failed { error, .. } => return Err(Halt::Failed(error)),
            }
        }
    }

    /// Encodes one sample at `level` and measures it against the same part of the source.
    fn sample_args(&self, start: Duration, level: u8) -> Vec<OsString> {
        let encoder = self.attempt.encoder;
        // With VMAF on the GPU too, the frames never leave it; otherwise they are handed over in
        // ordinary memory, where the reference is measured on the CPU (ADR 0030).
        let on_gpu = self.attempt.hw_decode && self.capabilities.vmaf_cuda;
        let mut args: Vec<OsString> = Vec::with_capacity(64);
        let mut push = |values: &[&str]| args.extend(values.iter().map(OsString::from));
        push(&["-hide_banner", "-nostdin", "-nostats", "-v", "info"]);
        push(&["-noautorotate"]);
        if on_gpu {
            push(&["-init_hw_device", "cuda=gpu", "-filter_hw_device", "gpu"]);
            push(&["-hwaccel", "cuda", "-hwaccel_output_format", "cuda"]);
            push(&["-hwaccel_device", "gpu"]);
        } else if self.attempt.hw_decode {
            push(&["-hwaccel", "cuda"]);
        }
        let (start, length) = (start.as_secs_f64(), SAMPLE.as_secs_f64());
        push(&["-ss", &format!("{start:.3}"), "-t", &format!("{length:.3}")]);
        push(&["-i"]);
        args.push(file_url(self.input));

        // The source, changed in size and rate as the real encode will be, feeds both the
        // encoder and the reference.
        let mut changes = plan::video_filters(self.rate, on_gpu, encoder, self.capabilities);
        let format = if on_gpu {
            String::new()
        } else {
            changes.pop().unwrap_or_default()
        };
        let mut graph = format!("[0:{}]", self.stream);
        for change in &changes {
            graph.push_str(change);
            graph.push(',');
        }
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
        } else {
            graph.push_str(&format!("[source]{format}[encode];"));
            graph.push_str(&format!("[reference]{scale_cpu}format=yuv420p[ref]"));
        }
        args.extend(["-filter_complex", &graph, "-map", "[encode]"].map(OsString::from));
        args.extend(plan::video_codec_args(
            encoder,
            self.capabilities,
            level,
            0,
            self.picture,
        ));
        // Every encoded frame's number and size, on standard output.
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

        // The encoded sample, decoded again, against the reference. Every other frame is enough:
        // the score moves by a few tenths at most, and the measuring takes a third less (ADR 0030).
        let measure = if on_gpu {
            format!(
                "[dec:0]{scale_cpu}format=yuv420p,hwupload_cuda[encoded];[encoded][ref]libvmaf_cuda={VMAF_OPTIONS}[vmaf]"
            )
        } else {
            let threads = share::threads_for_one();
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
}

/// How each sample's frames are pooled into one score, and how many are measured.
const VMAF_OPTIONS: &str = "pool=harmonic_mean:n_subsample=2";

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
/// given the `scores` measured so far; `None` once the answer is known. Between a level that
/// reaches the target and one that doesn't, it guesses where the target falls on a straight line
/// through their scores; otherwise it tries `hint`, then steps past a level that reaches the target
/// by the score it has to spare, then halves what is left.
fn next_level(
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
    match hint.filter(|level| (low..=high).contains(level) && !scores.contains_key(level)) {
        Some(level) => Some(level),
        // Only a level that reaches the target so far: the score it has to spare, at roughly a
        // point per level, says how much further to go.
        None => match reached {
            Some(reached) => {
                let spare = ((scores[&reached] - target) / POINTS_PER_LEVEL).floor() as u8;
                Some(reached.saturating_add(spare).clamp(low, high))
            }
            None => Some(low + (high - low) / 2),
        },
    }
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
        let mut scores: BTreeMap<u8, f64> = known.iter().map(|&l| (l, curve(l))).collect();
        let mut measured = Vec::new();
        while let Some(level) = next_level(&scores, &levels, target, hint) {
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
        }
    }

    #[test]
    fn content_that_never_reaches_the_target() {
        let (found, measured) = search(|_| 80.0, 24..=38, 95.0, None, &[]);
        assert_eq!(found, None);
        assert_eq!(measured.last(), Some(&24));
        let (found, _) = search(|_| 99.0, 24..=38, 95.0, None, &[]);
        assert_eq!(found, Some(38));
    }

    #[test]
    fn earlier_measurements_and_hints_save_steps() {
        let fresh = search(curve, 24..=42, 90.0, None, &[]).1.len();
        let (found, measured) = search(curve, 24..=42, 90.0, None, &[31, 35, 36]);
        assert_eq!(found, Some(35));
        assert!(measured.is_empty(), "{measured:?}");
        let (found, measured) = search(curve, 24..=42, 90.0, Some(35), &[]);
        assert_eq!(found, Some(35));
        assert_eq!(measured[0], 35);
        assert!(measured.len() <= fresh, "{measured:?}");
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
