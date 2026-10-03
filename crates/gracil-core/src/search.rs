//! Finding each video's own quality level (ADR 0028).
//!
//! Short samples spread through the video are encoded at candidate levels and compared with the
//! source using VMAF, both with the same size and frame-rate changes. The highest level (the
//! smallest file) whose average score reaches [`standard::TARGET_SCORE`] wins. Each sample is one
//! FFmpeg process that encodes and measures at once, through a loopback decoder, so nothing is
//! written to disk.

use std::ffi::OsString;
use std::path::Path;
use std::thread;
use std::time::Duration;

use tracing::{info, warn};

use crate::capabilities::{Capabilities, VideoEncoder};
use crate::encode::{self, EncodeOutcome, JobControl};
use crate::plan::{self, Attempt, EncodePlan};
use crate::probe::file_url;
use crate::standard::{self, Shape};
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

/// The level a video will be compressed at.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Found {
    /// Measured for this video with `encoder`.
    Level {
        encoder: VideoEncoder,
        level: u8,
    },
    /// Nothing could be measured; each encoder uses its fallback level.
    Fallback,
    Cancelled,
}

impl Found {
    /// The level to use with `encoder`.
    pub fn level_for(self, encoder: VideoEncoder) -> u8 {
        match self {
            Self::Level { encoder: e, level } if e == encoder => level,
            _ => standard::fallback_level(encoder),
        }
    }
}

/// Searches the level for `plan`'s main video, encoded the way `plan.attempt` would.
pub fn find_level(
    toolchain: &Toolchain,
    plan: &EncodePlan,
    duration: Option<Duration>,
    control: &JobControl,
) -> Found {
    let encoder = plan.attempt.encoder;
    let Some((stream, shape)) = plan.main_video() else {
        return Found::Fallback;
    };
    let shortest = match encoder {
        VideoEncoder::Nvenc => SHORTEST,
        VideoEncoder::X265 => SHORTEST_X265,
    };
    let Some(duration) = duration.filter(|d| *d >= shortest) else {
        return Found::Fallback;
    };
    if !plan.capabilities.vmaf {
        return Found::Fallback;
    }

    let mut search = Search {
        toolchain,
        input: &plan.input,
        stream,
        shape,
        attempt: plan.attempt,
        capabilities: &plan.capabilities,
        starts: sample_starts(duration),
        control,
    };
    let levels = standard::levels(encoder);
    let (mut low, mut high) = (*levels.start(), *levels.end());
    let mut best = None;
    while low <= high {
        let level = low + (high - low) / 2;
        match search.score(level) {
            Scored::Score(score) => {
                info!(level, score, "measured a quality level");
                if score >= standard::TARGET_SCORE {
                    best = Some(level);
                    low = level + 1;
                } else if level == 0 {
                    break;
                } else {
                    high = level - 1;
                }
            }
            Scored::Cancelled => return Found::Cancelled,
            Scored::Failed(error) => {
                warn!(%error, "could not measure quality; using the usual level");
                return Found::Fallback;
            }
        }
    }
    // Content that never reaches the target gets the best quality searched.
    let level = best.unwrap_or(*levels.start());
    info!(level, reached = best.is_some(), "chose a quality level");
    Found::Level { encoder, level }
}

enum Scored {
    Score(f64),
    Cancelled,
    Failed(String),
}

struct Search<'a> {
    toolchain: &'a Toolchain,
    input: &'a Path,
    stream: u32,
    shape: Shape,
    attempt: Attempt,
    capabilities: &'a Capabilities,
    starts: Vec<Duration>,
    control: &'a JobControl,
}

impl Search<'_> {
    /// The average score of every sample at `level`.
    fn score(&mut self, level: u8) -> Scored {
        let mut total = 0.0;
        for index in 0..self.starts.len() {
            match self.score_sample(self.starts[index], level) {
                Scored::Score(score) => total += score,
                other => return other,
            }
        }
        Scored::Score(total / self.starts.len() as f64)
    }

    fn score_sample(&mut self, start: Duration, level: u8) -> Scored {
        let (outcome, log) = encode::run_ffmpeg(
            self.toolchain,
            self.sample_args(start, level),
            self.control,
            &mut |_| {},
        );
        match outcome {
            EncodeOutcome::Finished => match vmaf_score(&log) {
                Some(score) => Scored::Score(score),
                None => Scored::Failed(format!("no score in FFmpeg's output: {log}")),
            },
            EncodeOutcome::Cancelled => Scored::Cancelled,
            // GPU decoding may not take this video; the samples then decode on the CPU.
            EncodeOutcome::Failed { .. } if self.attempt.hw_decode => {
                self.attempt.hw_decode = false;
                self.score_sample(start, level)
            }
            EncodeOutcome::Failed { error, .. } => Scored::Failed(error),
        }
    }

    /// Encodes one sample at `level` and measures it against the same part of the source.
    fn sample_args(&self, start: Duration, level: u8) -> Vec<OsString> {
        let encoder = self.attempt.encoder;
        let mut args: Vec<OsString> = Vec::with_capacity(64);
        let mut push = |values: &[&str]| args.extend(values.iter().map(OsString::from));
        push(&["-hide_banner", "-nostdin", "-nostats", "-v", "info"]);
        push(&["-noautorotate"]);
        if self.attempt.hw_decode {
            // Decoded on the GPU, then handed over in ordinary memory: both branches below need
            // the same frames, and the reference is measured on the CPU.
            push(&["-hwaccel", "cuda"]);
        }
        let (start, length) = (start.as_secs_f64(), SAMPLE.as_secs_f64());
        push(&["-ss", &format!("{start:.3}"), "-t", &format!("{length:.3}")]);
        push(&["-i"]);
        args.push(file_url(self.input));

        // The source, changed in size and rate as the real encode will be, feeds both the
        // encoder and the reference.
        let mut changes = plan::video_filters(self.shape, false, encoder, self.capabilities);
        let format = changes.pop().unwrap_or_default();
        let mut graph = format!("[0:{}]", self.stream);
        for change in &changes {
            graph.push_str(change);
            graph.push(',');
        }
        graph.push_str("split[source][reference];");
        graph.push_str(&format!("[source]{format}[encode];"));
        graph.push_str("[reference]format=yuv420p[ref]");
        args.extend(["-filter_complex", &graph, "-map", "[encode]"].map(OsString::from));
        args.extend(plan::video_codec_args(encoder, self.capabilities, level, 0));
        args.extend(["-f", "null", "-"].map(OsString::from));

        // The encoded sample, decoded again, against the reference.
        let threads = thread::available_parallelism().map_or(4, |n| n.get());
        let measure = format!(
            "[dec:0]format=yuv420p[encoded];[encoded][ref]libvmaf=n_threads={threads}:pool=harmonic_mean[vmaf]"
        );
        args.extend(
            ["-dec", "0:0", "-filter_complex", &measure, "-map", "[vmaf]"].map(OsString::from),
        );
        args.extend(["-f", "null", "-"].map(OsString::from));
        args
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
}
