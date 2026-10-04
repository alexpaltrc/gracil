//! Gracil's quality standard (ADR 0028): encoder settings, the quality search's target and
//! ranges, and the size, frame-rate and audio rules. Everything here is pure.

use std::ops::RangeInclusive;

use crate::capabilities::VideoEncoder;
use crate::probe::{Rate, Stream};

/// The VMAF score a video's level must reach: no visible difference at normal viewing.
pub const TARGET_SCORE: f64 = 95.0;

/// The VMAF score of the lighter choices offered for an already-compressed video (ADR 0029): some
/// loss the user accepts to get a real reduction.
pub const LIGHTER_SCORE: f64 = 90.0;

/// A compression is worth doing only when its predicted size is at most this share of the
/// original (ADR 0029).
pub const WORTH_IT: f64 = 0.8;

/// The levels searched (CQ for NVENC, CRF for x265), from best quality to smallest file.
pub fn levels(encoder: VideoEncoder) -> RangeInclusive<u8> {
    match encoder {
        VideoEncoder::Nvenc => 24..=38,
        VideoEncoder::X265 => 20..=34,
    }
}

/// The levels searched for the lighter choices: further towards small files.
pub fn lighter_levels(encoder: VideoEncoder) -> RangeInclusive<u8> {
    match encoder {
        VideoEncoder::Nvenc => 24..=42,
        VideoEncoder::X265 => 20..=38,
    }
}

/// The level used when no measurement is possible: the settings Gracil started with.
pub fn fallback_level(encoder: VideoEncoder) -> u8 {
    match encoder {
        VideoEncoder::Nvenc => 27,
        VideoEncoder::X265 => 26,
    }
}

/// How much of the NVENC feature set this GPU and driver accept. Detected once, best first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NvencLevel {
    /// 10-bit with lookahead, B-frame references and multipass. No adaptive quantization: it made
    /// files larger at the same measured quality (ADR 0029).
    Full,
    /// 10-bit with lookahead and multipass.
    Basic,
    /// The original reference settings, 8-bit.
    #[default]
    Legacy,
}

impl NvencLevel {
    pub const BEST_FIRST: [Self; 3] = [Self::Full, Self::Basic, Self::Legacy];

    /// Encoder options, without the quality level.
    pub fn options(self) -> &'static [(&'static str, &'static str)] {
        match self {
            Self::Full => &[
                ("preset", "p7"),
                ("tune", "hq"),
                ("rc", "vbr"),
                ("b", "0"),
                ("profile", "main10"),
                ("highbitdepth", "1"),
                ("rc-lookahead", "32"),
                ("bf", "4"),
                ("b_ref_mode", "middle"),
                ("multipass", "fullres"),
            ],
            Self::Basic => &[
                ("preset", "p7"),
                ("tune", "hq"),
                ("rc", "vbr"),
                ("b", "0"),
                ("profile", "main10"),
                ("highbitdepth", "1"),
                ("rc-lookahead", "32"),
                ("multipass", "fullres"),
            ],
            Self::Legacy => &[("preset", "p6"), ("tune", "hq"), ("rc", "vbr"), ("b", "0")],
        }
    }

    pub fn ten_bit(self) -> bool {
        self != Self::Legacy
    }
}

/// x265 options, without the quality level.
pub const X265_OPTIONS: [(&str, &str); 2] =
    [("preset", "medium"), ("x265-params", "log-level=error")];

/// The pixel format frames are converted to before a software-fed encoder.
pub fn pixel_format(ten_bit: bool, encoder: VideoEncoder) -> &'static str {
    match (ten_bit, encoder) {
        (false, _) => "yuv420p",
        (true, VideoEncoder::Nvenc) => "p010le",
        (true, VideoEncoder::X265) => "yuv420p10le",
    }
}

/// Picture size and frame rate a video leaves with. `None` keeps the source's.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Shape {
    pub size: Option<(u32, u32)>,
    pub rate: Option<Rate>,
}

/// The short side a video is brought down to, never up.
const MAX_SHORT_SIDE: u32 = 1080;

/// The short sides a video steps down to when the user asks for a smaller size.
const SMALLER_STEPS: [u32; 2] = [720, 480];

/// Sources averaging more frames per second than this are brought down to 30; it leaves 29.97
/// and slightly irregular 30 fps phone videos alone.
const MAX_RATE: f64 = 30.5;

impl Shape {
    pub fn of(stream: &Stream) -> Self {
        Self {
            size: stream
                .width
                .zip(stream.height)
                .and_then(|size| smaller_size(size, MAX_SHORT_SIDE)),
            rate: stream.frame_rate.and_then(slower_rate),
        }
    }

    /// Like [`Self::of`], one size down (ADR 0029): the short side becomes the next of
    /// [`SMALLER_STEPS`] below the one [`Self::of`] gives. `None` at 480 or smaller, or when the
    /// size is unknown.
    pub fn one_size_down(stream: &Stream) -> Option<Self> {
        let size = stream.width.zip(stream.height)?;
        let short = size.0.min(size.1).min(MAX_SHORT_SIDE);
        let step = SMALLER_STEPS.into_iter().find(|&step| step < short)?;
        Some(Self {
            size: smaller_size(size, step),
            ..Self::of(stream)
        })
    }

    /// The short side of the picture this shape gives `stream`, when known.
    pub fn short_side(self, stream: &Stream) -> Option<u32> {
        self.size
            .or(stream.width.zip(stream.height))
            .map(|(width, height)| width.min(height))
    }

    pub fn is_unchanged(self) -> bool {
        self == Self::default()
    }
}

/// Scales so the short side is `max_short`, keeping the aspect ratio and even dimensions.
fn smaller_size((width, height): (u32, u32), max_short: u32) -> Option<(u32, u32)> {
    let (short, long) = (width.min(height), width.max(height));
    if short <= max_short {
        return None;
    }
    let scaled = f64::from(long) * f64::from(max_short) / f64::from(short);
    // At most the long side itself, so the result fits in a `u32`.
    let long = ((scaled / 2.0).round() as u32 * 2).max(2);
    Some(if width >= height {
        (long, max_short)
    } else {
        (max_short, long)
    })
}

/// 30 fps, or 29.97 for the NTSC family (59.94, 119.88) so the timing stays regular.
fn slower_rate(rate: Rate) -> Option<Rate> {
    let fps = rate.per_second();
    if fps <= MAX_RATE {
        return None;
    }
    let ntsc_whole = (fps * 1.001).round();
    let is_ntsc = (fps - ntsc_whole / 1.001).abs() < 0.01 && (fps - fps.round()).abs() > 0.01;
    Some(if is_ntsc {
        Rate {
            num: 30000,
            den: 1001,
        }
    } else {
        Rate { num: 30, den: 1 }
    })
}

/// AAC bitrate for a track: 64 kbps per channel, between 128 and 512.
pub fn aac_kbps(channels: Option<u32>) -> u32 {
    (64 * channels.unwrap_or(2)).clamp(128, 512)
}

/// Audio worth converting to AAC even though MP4 could hold it: lossless audio, or a track
/// using more than twice what AAC would.
pub fn audio_is_heavy(stream: &Stream) -> bool {
    let lossless =
        matches!(stream.codec.as_str(), "flac" | "alac") || stream.codec.starts_with("pcm_");
    let limit = u64::from(aac_kbps(stream.channels)) * 2 * 1000;
    lossless || stream.bit_rate.is_some_and(|b| b > limit)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::probe::StreamKind;

    fn video(width: u32, height: u32, num: u32, den: u32) -> Stream {
        Stream {
            kind: StreamKind::Video,
            codec: "h264".into(),
            width: Some(width),
            height: Some(height),
            frame_rate: Some(Rate { num, den }),
            ..Stream::default()
        }
    }

    #[test]
    fn only_videos_above_1080p_shrink() {
        let cases = [
            ((3840, 2160), Some((1920, 1080))),
            ((2160, 3840), Some((1080, 1920))),
            ((2560, 1440), Some((1920, 1080))),
            ((4096, 2160), Some((2048, 1080))),
            ((1920, 1080), None),
            ((1080, 1920), None),
            ((854, 480), None),
            ((1920, 1088), Some((1906, 1080))),
            ((2880, 1440), Some((2160, 1080))),
        ];
        for ((w, h), expected) in cases {
            assert_eq!(Shape::of(&video(w, h, 30, 1)).size, expected, "{w}x{h}");
        }
    }

    #[test]
    fn one_size_down_never_goes_below_480() {
        let cases = [
            ((3840, 2160), Some((1280, 720))),
            ((1920, 1080), Some((1280, 720))),
            ((1080, 1920), Some((720, 1280))),
            ((1320, 720), Some((880, 480))),
            ((1024, 600), Some((820, 480))),
            ((854, 480), None),
            ((640, 360), None),
        ];
        for ((w, h), expected) in cases {
            let shape = Shape::one_size_down(&video(w, h, 60, 1));
            assert_eq!(shape.map(|s| s.size), expected.map(Some), "{w}x{h}");
            if let Some(shape) = shape {
                assert_eq!(
                    shape.rate,
                    Some(Rate { num: 30, den: 1 }),
                    "keeps the rate rule"
                );
            }
        }
    }

    #[test]
    fn only_videos_above_30_fps_slow_down() {
        let thirty = Some(Rate { num: 30, den: 1 });
        let ntsc = Some(Rate {
            num: 30000,
            den: 1001,
        });
        let cases = [
            ((60, 1), thirty),
            ((60000, 1001), ntsc),
            ((120000, 1001), ntsc),
            ((240, 1), thirty),
            ((50, 1), thirty),
            ((30, 1), None),
            ((30000, 1001), None),
            ((244, 10), None),
            ((24000, 1001), None),
            ((3018, 100), None),
        ];
        for ((num, den), expected) in cases {
            assert_eq!(
                Shape::of(&video(1280, 720, num, den)).rate,
                expected,
                "{num}/{den}"
            );
        }
    }

    #[test]
    fn unknown_size_and_rate_stay() {
        let stream = Stream {
            kind: StreamKind::Video,
            ..Stream::default()
        };
        assert!(Shape::of(&stream).is_unchanged());
    }

    #[test]
    fn heavy_audio() {
        let audio = |codec: &str, channels, bit_rate| Stream {
            kind: StreamKind::Audio,
            codec: codec.into(),
            channels,
            bit_rate,
            ..Stream::default()
        };
        assert!(audio_is_heavy(&audio("pcm_s24le", Some(2), None)));
        assert!(audio_is_heavy(&audio("flac", Some(6), None)));
        assert!(audio_is_heavy(&audio("alac", None, None)));
        assert!(audio_is_heavy(&audio("mp3", Some(2), Some(320_000))));
        assert!(!audio_is_heavy(&audio("aac", Some(2), Some(256_000))));
        assert!(!audio_is_heavy(&audio("ac3", Some(6), Some(640_000))));
        assert!(!audio_is_heavy(&audio("opus", Some(2), None)));
    }
}
