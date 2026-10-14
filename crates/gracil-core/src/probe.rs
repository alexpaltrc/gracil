//! Inspecting a video with FFprobe. Only the few facts Gracil decides on are kept.

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::Path;
use std::time::Duration;

use serde::Deserialize;

use crate::encode::{self, EncodeOutcome, JobControl};
use crate::toolchain::Toolchain;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StreamKind {
    Video,
    Audio,
    Subtitle,
    Data,
    Attachment,
    #[default]
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Stream {
    /// Index of the stream in the source file.
    pub index: u32,
    pub kind: StreamKind,
    /// FFmpeg codec name; empty when FFmpeg does not know the codec.
    pub codec: String,
    /// Cover art stored as a one-frame video stream.
    pub attached_pic: bool,
    pub channels: Option<u32>,
    /// Average frames per second.
    pub frame_rate: Option<Rate>,
    /// Bits per second, when the file records it.
    pub bit_rate: Option<u64>,
    /// A picture's width and height in pixels, as stored, when the file says.
    pub size: Option<(u32, u32)>,
    /// How a picture's pixels are stored, by FFmpeg's name (`yuv420p`, `yuv420p10le`), when
    /// the file says.
    pub pixel_format: Option<String>,
    /// FFmpeg knows the codec but can't decode it, such as the spatial audio iPhones record
    /// (`apple_apac`). Set by [`crate::capabilities::Decoders::mark`].
    pub undecodable: bool,
}

/// A frame rate as FFmpeg writes it, such as `30000/1001`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rate {
    pub num: u32,
    pub den: u32,
}

impl Rate {
    pub fn per_second(self) -> f64 {
        f64::from(self.num) / f64::from(self.den)
    }

    /// Parses `num/den`; `0/0` and other unusable values are `None`.
    fn parse(text: &str) -> Option<Self> {
        let (num, den) = text.split_once('/')?;
        let rate = Self {
            num: num.parse().ok()?,
            den: den.parse().ok()?,
        };
        (rate.num > 0 && rate.den > 0).then_some(rate)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct MediaInfo {
    pub duration: Option<Duration>,
    pub streams: Vec<Stream>,
    /// Container-level metadata keys (values are not needed and not kept).
    pub tag_keys: Vec<String>,
}

impl MediaInfo {
    /// Real video streams, excluding cover art.
    pub fn has_video(&self) -> bool {
        self.streams
            .iter()
            .any(|s| s.kind == StreamKind::Video && !s.attached_pic)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ProbeError {
    #[error("FFprobe could not read the file: {0}")]
    Unreadable(String),
    #[error("FFprobe returned unexpected output")]
    Parse(#[from] serde_json::Error),
    #[error("cancelled")]
    Cancelled,
}

pub fn probe(toolchain: &Toolchain, path: &Path) -> Result<MediaInfo, ProbeError> {
    probe_under(toolchain, path, &JobControl::default())
}

/// [`probe`] under `control`, like any other tool: it can be paused and cancelled, and it is
/// stopped if it goes silent (ADR 0064).
pub fn probe_under(
    toolchain: &Toolchain,
    path: &Path,
    control: &JobControl,
) -> Result<MediaInfo, ProbeError> {
    let args = [
        "-v",
        "error",
        "-print_format",
        "json",
        "-show_format",
        "-show_streams",
    ];
    let mut args: Vec<OsString> = args.map(OsString::from).to_vec();
    // `file:` stops FFprobe from treating a name like `http:...` or `pipe:` as a protocol.
    args.push(file_url(path));
    let mut json = String::new();
    let (outcome, _) = encode::run_program(toolchain.ffprobe(), args, control, &mut |line| {
        json.push_str(line);
        json.push('\n');
    });
    match outcome {
        EncodeOutcome::Finished => parse(json.as_bytes()),
        EncodeOutcome::Cancelled => Err(ProbeError::Cancelled),
        EncodeOutcome::Failed { error, .. } => Err(ProbeError::Unreadable(error)),
        EncodeOutcome::Stalled => Err(ProbeError::Unreadable(
            "FFprobe stopped answering".to_owned(),
        )),
    }
}

/// Prefixes `file:` so FFmpeg tools never interpret a path as a protocol or device.
pub(crate) fn file_url(path: &Path) -> std::ffi::OsString {
    let mut url = std::ffi::OsString::from("file:");
    url.push(path.as_os_str());
    url
}

#[derive(Deserialize)]
struct RawOutput {
    #[serde(default)]
    streams: Vec<RawStream>,
    #[serde(default)]
    format: RawFormat,
}

#[derive(Deserialize, Default)]
struct RawFormat {
    duration: Option<String>,
    #[serde(default)]
    tags: BTreeMap<String, String>,
}

#[derive(Deserialize)]
struct RawStream {
    index: u32,
    #[serde(default)]
    codec_type: String,
    #[serde(default)]
    codec_name: String,
    channels: Option<u32>,
    #[serde(default)]
    avg_frame_rate: String,
    #[serde(default)]
    r_frame_rate: String,
    bit_rate: Option<String>,
    width: Option<u32>,
    height: Option<u32>,
    pix_fmt: Option<String>,
    #[serde(default)]
    disposition: BTreeMap<String, u8>,
}

fn parse(json: &[u8]) -> Result<MediaInfo, ProbeError> {
    let raw: RawOutput = serde_json::from_slice(json)?;
    let streams = raw
        .streams
        .into_iter()
        .map(|s| Stream {
            index: s.index,
            kind: match s.codec_type.as_str() {
                "video" => StreamKind::Video,
                "audio" => StreamKind::Audio,
                "subtitle" => StreamKind::Subtitle,
                "data" => StreamKind::Data,
                "attachment" => StreamKind::Attachment,
                _ => StreamKind::Other,
            },
            attached_pic: s.disposition.get("attached_pic") == Some(&1),
            codec: s.codec_name,
            channels: s.channels,
            frame_rate: Rate::parse(&s.avg_frame_rate).or_else(|| Rate::parse(&s.r_frame_rate)),
            bit_rate: s.bit_rate.and_then(|b| b.parse().ok()).filter(|&b| b > 0),
            size: s.width.zip(s.height).filter(|&(w, h)| w > 0 && h > 0),
            pixel_format: s.pix_fmt.filter(|format| !format.is_empty()),
            undecodable: false,
        })
        .collect();
    let duration = raw
        .format
        .duration
        .and_then(|d| d.parse::<f64>().ok())
        .filter(|d| d.is_finite() && *d > 0.0)
        .map(Duration::from_secs_f64);
    Ok(MediaInfo {
        duration,
        streams,
        tag_keys: raw.format.tags.into_keys().collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_streams_duration_and_tags() {
        let json = br#"{
            "streams": [
                {"index": 0, "codec_type": "video", "codec_name": "h264", "width": 3840, "height": 2160, "pix_fmt": "yuv420p10le",
                 "avg_frame_rate": "60000/1001", "r_frame_rate": "60/1", "disposition": {"attached_pic": 0}},
                {"index": 1, "codec_type": "audio", "codec_name": "aac", "channels": 2,
                 "avg_frame_rate": "0/0", "bit_rate": "128000"},
                {"index": 2, "codec_type": "data"},
                {"index": 3, "codec_type": "video", "codec_name": "mjpeg", "disposition": {"attached_pic": 1}}
            ],
            "format": {"duration": "12.500000", "tags": {"com.apple.quicktime.make": "Apple", "title": "x"}}
        }"#;
        let info = parse(json).unwrap();
        assert_eq!(info.duration, Some(Duration::from_millis(12_500)));
        assert_eq!(info.streams.len(), 4);
        assert_eq!(info.streams[1].channels, Some(2));
        let video = &info.streams[0];
        assert_eq!(
            video.frame_rate,
            Some(Rate {
                num: 60000,
                den: 1001
            })
        );
        assert_eq!(video.size, Some((3840, 2160)));
        assert_eq!(video.pixel_format.as_deref(), Some("yuv420p10le"));
        assert_eq!(info.streams[1].size, None);
        assert_eq!(info.streams[1].frame_rate, None);
        assert_eq!(info.streams[1].bit_rate, Some(128_000));
        assert_eq!(info.streams[2].kind, StreamKind::Data);
        assert_eq!(info.streams[2].codec, "");
        assert!(info.streams[3].attached_pic);
        assert!(info.has_video());
        assert_eq!(info.tag_keys, ["com.apple.quicktime.make", "title"]);
    }

    #[test]
    fn missing_or_invalid_duration_is_none() {
        for json in [
            &br#"{"format": {}}"#[..],
            br#"{"format": {"duration": "N/A"}}"#,
        ] {
            assert_eq!(parse(json).unwrap().duration, None);
        }
    }

    #[test]
    fn cover_art_alone_is_not_video() {
        let json = br#"{"streams": [{"index": 0, "codec_type": "video", "codec_name": "png", "disposition": {"attached_pic": 1}}]}"#;
        assert!(!parse(json).unwrap().has_video());
    }

    #[test]
    fn file_url_keeps_path_exactly() {
        let url = file_url(Path::new(r"C:\Vídeos\Mi cumpleaños 🎂.mp4"));
        assert_eq!(url, r"file:C:\Vídeos\Mi cumpleaños 🎂.mp4");
    }
}
