//! Gracil's quality standard (ADR 0028): encoder settings, the quality search's target and
//! ranges, and the frame-rate and audio rules. Everything here is pure.

use std::ops::RangeInclusive;

use crate::capabilities::VideoEncoder;
use crate::probe::{Rate, Stream};

/// The VMAF score a video's level must reach: no visible difference at normal viewing.
pub const TARGET_SCORE: f64 = 95.0;

/// The VMAF score for a video that is already compressed (ADR 0033): compressing it again loses a
/// little, which is what the user asks for by compressing it again.
pub const AGAIN_SCORE: f64 = 90.0;

/// A compression is worth doing only when its predicted size is at most this share of the
/// original (ADR 0029).
pub const WORTH_IT: f64 = 0.8;

/// The levels searched (CQ for NVENC, ICQ for Quick Sync, QP for AMF, CRF for x265), from best
/// quality to smallest file.
pub fn levels(encoder: VideoEncoder) -> RangeInclusive<u8> {
    match encoder {
        VideoEncoder::Nvenc | VideoEncoder::Amf => 24..=38,
        VideoEncoder::Qsv => 20..=36,
        VideoEncoder::X265 => 20..=34,
    }
}

/// The levels searched for a video that is already compressed: further towards small files.
pub fn again_levels(encoder: VideoEncoder) -> RangeInclusive<u8> {
    match encoder {
        VideoEncoder::Nvenc | VideoEncoder::Amf => 24..=42,
        VideoEncoder::Qsv => 20..=40,
        VideoEncoder::X265 => 20..=38,
    }
}

/// The level used when no measurement is possible: the settings Gracil started with.
pub fn fallback_level(encoder: VideoEncoder) -> u8 {
    match encoder {
        VideoEncoder::Nvenc | VideoEncoder::Amf => 27,
        VideoEncoder::Qsv => 24,
        VideoEncoder::X265 => 26,
    }
}

/// The options that set `encoder`'s quality `level`. AMF has no constant-quality mode for HEVC, so
/// it gets a constant quantizer, which the quality search turns into the same thing (ADR 0052).
pub fn level_options(encoder: VideoEncoder, level: u8) -> Vec<(&'static str, String)> {
    let level = level.to_string();
    match encoder {
        VideoEncoder::Nvenc => vec![("cq", level)],
        VideoEncoder::Qsv => vec![("global_quality", level)],
        VideoEncoder::Amf => vec![("qp_i", level.clone()), ("qp_p", level)],
        VideoEncoder::X265 => vec![("crf", level)],
    }
}

/// How much of a graphics card encoder's feature set this card and driver accept (ADR 0028,
/// ADR 0052). Detected once, best first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HardwareLevel {
    /// 10-bit with the encoder's look-ahead and smarter frame choices where it has them. No
    /// adaptive quantization on NVENC: it made files larger at the same measured quality
    /// (ADR 0029).
    Full,
    /// 10-bit, with NVENC's lookahead and multipass.
    Basic,
    /// The original reference settings, 8-bit.
    #[default]
    Legacy,
}

impl HardwareLevel {
    pub const BEST_FIRST: [Self; 3] = [Self::Full, Self::Basic, Self::Legacy];

    /// `encoder`'s options, without the quality level. Empty for x265, which isn't on the card.
    pub fn options(self, encoder: VideoEncoder) -> &'static [(&'static str, &'static str)] {
        match (encoder, self) {
            (VideoEncoder::Nvenc, Self::Full) => &[
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
            (VideoEncoder::Nvenc, Self::Basic) => &[
                ("preset", "p7"),
                ("tune", "hq"),
                ("rc", "vbr"),
                ("b", "0"),
                ("profile", "main10"),
                ("highbitdepth", "1"),
                ("rc-lookahead", "32"),
                ("multipass", "fullres"),
            ],
            (VideoEncoder::Nvenc, Self::Legacy) => {
                &[("preset", "p6"), ("tune", "hq"), ("rc", "vbr"), ("b", "0")]
            }
            // `global_quality` without a bitrate is Intel's constant quality (ICQ); look-ahead
            // needs the extended rate control.
            (VideoEncoder::Qsv, Self::Full) => &[
                ("preset", "veryslow"),
                ("profile", "main10"),
                ("extbrc", "1"),
                ("look_ahead_depth", "40"),
                ("b_strategy", "1"),
                ("adaptive_i", "1"),
                ("adaptive_b", "1"),
            ],
            (VideoEncoder::Qsv, Self::Basic) => &[("preset", "veryslow"), ("profile", "main10")],
            (VideoEncoder::Qsv, Self::Legacy) => &[("preset", "slow")],
            (VideoEncoder::Amf, Self::Full | Self::Basic) => &[
                ("usage", "transcoding"),
                ("quality", "quality"),
                ("rc", "cqp"),
                ("profile", "main10"),
                ("bitdepth", "10"),
            ],
            (VideoEncoder::Amf, Self::Legacy) => &[
                ("usage", "transcoding"),
                ("quality", "quality"),
                ("rc", "cqp"),
            ],
            (VideoEncoder::X265, _) => &[],
        }
    }

    pub fn ten_bit(self) -> bool {
        self != Self::Legacy
    }
}

/// x265 options, without the quality level. It may use `threads` of the processor (ADR 0040).
pub fn x265_options(threads: usize) -> [(&'static str, String); 2] {
    [
        ("preset", "medium".to_owned()),
        ("x265-params", format!("log-level=error:pools={threads}")),
    ]
}

/// The pixel format frames are converted to before a software-fed encoder.
pub fn pixel_format(ten_bit: bool, encoder: VideoEncoder) -> &'static str {
    match (ten_bit, encoder) {
        (false, _) => "yuv420p",
        (true, VideoEncoder::Nvenc | VideoEncoder::Qsv | VideoEncoder::Amf) => "p010le",
        (true, VideoEncoder::X265) => "yuv420p10le",
    }
}

/// Sources averaging more frames per second than this are brought down to 30; it leaves 29.97
/// and slightly irregular 30 fps phone videos alone.
const MAX_RATE: f64 = 30.5;

/// The frame rate a video leaves with. `None` keeps the source's. Its picture size is always kept
/// (ADR 0033).
pub fn output_rate(stream: &Stream) -> Option<Rate> {
    stream.frame_rate.and_then(slower_rate)
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

    fn video(num: u32, den: u32) -> Stream {
        Stream {
            kind: StreamKind::Video,
            codec: "h264".into(),
            frame_rate: Some(Rate { num, den }),
            ..Stream::default()
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
            assert_eq!(output_rate(&video(num, den)), expected, "{num}/{den}");
        }
    }

    #[test]
    fn an_unknown_rate_stays() {
        let stream = Stream {
            kind: StreamKind::Video,
            ..Stream::default()
        };
        assert_eq!(output_rate(&stream), None);
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
