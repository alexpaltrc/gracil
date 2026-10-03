//! Gracil's quality standard (ADR 0028): encoder settings, the quality search's target and
//! ranges, and the size, frame-rate and audio rules. Everything here is pure.

use std::ops::RangeInclusive;

use crate::capabilities::VideoEncoder;
use crate::probe::{Rate, Stream};

/// The VMAF score a video's level must reach: no visible difference at normal viewing.
pub const TARGET_SCORE: f64 = 95.0;

/// The levels searched (CQ for NVENC, CRF for x265), from best quality to smallest file.
pub fn levels(encoder: VideoEncoder) -> RangeInclusive<u8> {
    match encoder {
        VideoEncoder::Nvenc => 24..=38,
        VideoEncoder::X265 => 20..=34,
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
    /// 10-bit with adaptive quantization, lookahead, B-frame references, multipass and the
    /// temporal filter.
    Full,
    /// 10-bit with spatial adaptive quantization, lookahead and multipass.
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
                ("spatial-aq", "1"),
                ("temporal-aq", "1"),
                ("rc-lookahead", "32"),
                ("bf", "4"),
                ("b_ref_mode", "middle"),
                ("multipass", "fullres"),
                ("tf_level", "4"),
            ],
            Self::Basic => &[
                ("preset", "p7"),
                ("tune", "hq"),
                ("rc", "vbr"),
                ("b", "0"),
                ("profile", "main10"),
                ("highbitdepth", "1"),
                ("spatial-aq", "1"),
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

/// Sources averaging more frames per second than this are brought down to 30; it leaves 29.97
/// and slightly irregular 30 fps phone videos alone.
const MAX_RATE: f64 = 30.5;

impl Shape {
    pub fn of(stream: &Stream) -> Self {
        Self {
            size: stream.width.zip(stream.height).and_then(smaller_size),
            rate: stream.frame_rate.and_then(slower_rate),
        }
    }

    pub fn is_unchanged(self) -> bool {
        self == Self::default()
    }
}

/// Scales so the short side is 1080, keeping the aspect ratio and even dimensions.
fn smaller_size((width, height): (u32, u32)) -> Option<(u32, u32)> {
    let (short, long) = (width.min(height), width.max(height));
    if short <= MAX_SHORT_SIDE {
        return None;
    }
    let scaled = f64::from(long) * f64::from(MAX_SHORT_SIDE) / f64::from(short);
    // At most the long side itself, so the result fits in a `u32`.
    let long = ((scaled / 2.0).round() as u32 * 2).max(2);
    Some(if width >= height {
        (long, MAX_SHORT_SIDE)
    } else {
        (MAX_SHORT_SIDE, long)
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
