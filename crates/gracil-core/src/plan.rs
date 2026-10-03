//! Turning a probed source into FFmpeg arguments.
//!
//! Everything here is pure, so the exact command Gracil runs is covered by unit tests. The
//! reference behavior (spec §21) is HEVC NVENC, preset P6, HQ tune, VBR at CQ 27 with no bitrate
//! target, audio copied. The rest of the decisions keep as much of the source as the output can
//! hold, and drop the rest instead of rejecting the video (spec §41, §44).

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use crate::capabilities::{Capabilities, VideoEncoder};
use crate::probe::{MediaInfo, Stream, StreamKind, file_url};

/// Every compressed video is an MP4, whatever it was before (ADR 0012).
///
/// Extension for the compressed file: `mp4`, keeping the source's spelling when it already is
/// one (so `Clip.MP4` stays `Clip.MP4` and is replaced in place rather than renamed).
pub fn output_extension(source: &Path) -> OsString {
    match source.extension() {
        Some(ext) if ext.eq_ignore_ascii_case("mp4") => ext.to_owned(),
        _ => "mp4".into(),
    }
}

/// Audio MP4 holds as is; anything else is converted to AAC.
fn mp4_can_copy_audio(codec: &str) -> bool {
    matches!(
        codec,
        "aac" | "mp3" | "ac3" | "eac3" | "alac" | "opus" | "flac"
    )
}

/// How much of the source an attempt tries to carry over.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Preservation {
    /// Every stream and all metadata the output container can hold.
    Full,
    /// Video and audio (re-encoded to AAC) plus metadata. Used only after `Full` failed.
    Essential,
}

/// One way of compressing a file. A job tries [`attempts`] in order until one succeeds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Attempt {
    pub encoder: VideoEncoder,
    pub hw_decode: bool,
    pub preservation: Preservation,
}

/// Attempts in order of preference. Each step gives up a little (GPU decoding, secondary streams,
/// the GPU encoder) so that any video FFmpeg can read ends up compressed.
pub fn attempts(capabilities: Capabilities) -> Vec<Attempt> {
    use Preservation::{Essential, Full};
    use VideoEncoder::{Nvenc, X265};
    let attempt = |encoder, hw_decode, preservation| Attempt {
        encoder,
        hw_decode,
        preservation,
    };
    match capabilities.encoder {
        Nvenc => {
            let mut list = Vec::with_capacity(4);
            if capabilities.cuda_decode {
                list.push(attempt(Nvenc, true, Full));
            }
            list.extend([
                attempt(Nvenc, false, Full),
                attempt(Nvenc, false, Essential),
                attempt(X265, false, Essential),
            ]);
            list
        }
        X265 => vec![attempt(X265, false, Full), attempt(X265, false, Essential)],
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamAction {
    EncodeVideo,
    Copy,
    ToMovText,
    ToAac { kbps: u32 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MappedStream {
    pub source_index: u32,
    pub action: StreamAction,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncodePlan {
    pub input: PathBuf,
    pub output: PathBuf,
    pub attempt: Attempt,
    pub streams: Vec<MappedStream>,
    /// Source streams left out because the output cannot hold them. Logged, never shown.
    pub dropped: Vec<u32>,
    /// iPhone-style `com.apple.*` keys only survive when written as `mdta` metadata.
    pub keep_apple_metadata: bool,
}

impl EncodePlan {
    pub fn new(input: &Path, output: &Path, info: &MediaInfo, attempt: Attempt) -> Self {
        let mut streams = Vec::with_capacity(info.streams.len());
        let mut dropped = Vec::new();
        for stream in &info.streams {
            match stream_action(stream, attempt.preservation) {
                Some(action) => streams.push(MappedStream {
                    source_index: stream.index,
                    action,
                }),
                None => dropped.push(stream.index),
            }
        }
        Self {
            input: input.to_owned(),
            output: output.to_owned(),
            attempt,
            streams,
            dropped,
            keep_apple_metadata: info.tag_keys.iter().any(|k| k.starts_with("com.apple.")),
        }
    }

    /// The complete FFmpeg argument list. The output file must already exist (Gracil reserves it),
    /// which is why `-y` is safe here.
    pub fn args(&self) -> Vec<OsString> {
        let mut args: Vec<OsString> = Vec::with_capacity(32 + self.streams.len() * 12);
        let mut push = |values: &[&str]| args.extend(values.iter().map(OsString::from));

        push(&["-hide_banner", "-nostdin", "-v", "error", "-nostats"]);
        push(&["-progress", "pipe:1", "-y"]);
        // Keep the pixels as recorded and carry the rotation over as metadata (spec §41). It also
        // keeps a rotation filter from running on GPU frames, which it cannot do.
        push(&["-noautorotate"]);
        if self.attempt.hw_decode {
            push(&["-hwaccel", "cuda", "-hwaccel_output_format", "cuda"]);
        }
        push(&["-i"]);
        args.push(file_url(&self.input));

        for stream in &self.streams {
            args.push("-map".into());
            args.push(format!("0:{}", stream.source_index).into());
        }
        // Explicit global mapping: FFmpeg's default mapping leaves out `creation_time`.
        args.extend(["-map_metadata", "0", "-map_chapters", "0"].map(OsString::from));
        for (out, stream) in self.streams.iter().enumerate() {
            args.push(format!("-map_metadata:s:{out}").into());
            args.push(format!("0:s:{}", stream.source_index).into());
        }

        for (out, stream) in self.streams.iter().enumerate() {
            self.push_codec_args(&mut args, out, stream.action);
        }

        if self.keep_apple_metadata {
            args.extend(["-movflags", "+use_metadata_tags"].map(OsString::from));
        }
        args.extend(["-f", "mp4"].map(OsString::from));
        args.push(file_url(&self.output));
        args
    }

    fn push_codec_args(&self, args: &mut Vec<OsString>, out: usize, action: StreamAction) {
        let mut option = |name: &str, value: &str| {
            args.push(format!("-{name}:{out}").into());
            args.push(value.into());
        };
        match action {
            StreamAction::EncodeVideo => {
                match self.attempt.encoder {
                    VideoEncoder::Nvenc => {
                        option("c", "hevc_nvenc");
                        option("preset", "p6");
                        option("tune", "hq");
                        option("rc", "vbr");
                        option("cq", "27");
                        option("b", "0");
                    }
                    VideoEncoder::X265 => {
                        option("c", "libx265");
                        option("preset", "medium");
                        option("crf", "26");
                        option("x265-params", "log-level=error");
                    }
                }
                // `hvc1` is the HEVC tag Apple devices and Windows players expect.
                option("tag", "hvc1");
            }
            StreamAction::Copy => option("c", "copy"),
            StreamAction::ToMovText => option("c", "mov_text"),
            StreamAction::ToAac { kbps } => {
                option("c", "aac");
                option("b", &format!("{kbps}k"));
            }
        }
    }
}

const TEXT_SUBTITLES: [&str; 6] = ["mov_text", "subrip", "srt", "ass", "ssa", "webvtt"];

/// What to do with one source stream, or `None` to leave it out.
fn stream_action(stream: &Stream, preservation: Preservation) -> Option<StreamAction> {
    let codec = stream.codec.as_str();
    let essential = preservation == Preservation::Essential;
    match stream.kind {
        StreamKind::Video if !stream.attached_pic => Some(StreamAction::EncodeVideo),
        StreamKind::Audio if essential => Some(aac_for(stream)),
        StreamKind::Audio if mp4_can_copy_audio(codec) => Some(StreamAction::Copy),
        StreamKind::Audio => Some(aac_for(stream)),
        _ if essential => None,
        StreamKind::Video => matches!(codec, "mjpeg" | "png").then_some(StreamAction::Copy),
        StreamKind::Subtitle if codec == "mov_text" => Some(StreamAction::Copy),
        StreamKind::Subtitle => TEXT_SUBTITLES
            .contains(&codec)
            .then_some(StreamAction::ToMovText),
        // Bitmap subtitles, fonts and data tracks have no place in MP4.
        StreamKind::Attachment | StreamKind::Data | StreamKind::Other => None,
    }
}

/// AAC at 64 kbps per channel, between 128 and 512 kbps: transparent for normal listening.
fn aac_for(stream: &Stream) -> StreamAction {
    let channels = stream.channels.unwrap_or(2);
    StreamAction::ToAac {
        kbps: (64 * channels).clamp(128, 512),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use StreamAction::*;

    fn stream(index: u32, kind: StreamKind, codec: &str) -> Stream {
        Stream {
            index,
            kind,
            codec: codec.into(),
            attached_pic: false,
            channels: None,
        }
    }

    fn cover(index: u32, codec: &str) -> Stream {
        Stream {
            attached_pic: true,
            ..stream(index, StreamKind::Video, codec)
        }
    }

    fn info(streams: Vec<Stream>) -> MediaInfo {
        MediaInfo {
            duration: None,
            streams,
            tag_keys: Vec::new(),
        }
    }

    fn nvenc_full() -> Attempt {
        Attempt {
            encoder: VideoEncoder::Nvenc,
            hw_decode: true,
            preservation: Preservation::Full,
        }
    }

    fn plan(info: &MediaInfo, attempt: Attempt) -> EncodePlan {
        EncodePlan::new(
            Path::new(r"C:\in\a.mov"),
            Path::new(r"C:\in\a.part"),
            info,
            attempt,
        )
    }

    fn actions(plan: &EncodePlan) -> Vec<(u32, StreamAction)> {
        plan.streams
            .iter()
            .map(|s| (s.source_index, s.action))
            .collect()
    }

    fn joined(plan: &EncodePlan) -> String {
        plan.args()
            .iter()
            .map(|a| a.to_str().unwrap())
            .collect::<Vec<_>>()
            .join(" ")
    }

    #[test]
    fn every_output_is_mp4() {
        let cases = [
            ("a.mp4", "mp4"),
            ("a.MP4", "MP4"),
            ("a.m4v", "mp4"),
            ("a.MOV", "mp4"),
            ("a.mkv", "mp4"),
            ("a.avi", "mp4"),
            ("noext", "mp4"),
        ];
        for (name, expected) in cases {
            assert_eq!(output_extension(Path::new(name)), expected, "{name}");
        }
    }

    #[test]
    fn reference_nvenc_settings() {
        let info = info(vec![
            stream(0, StreamKind::Video, "h264"),
            stream(1, StreamKind::Audio, "aac"),
        ]);
        let args = joined(&plan(&info, nvenc_full()));
        assert!(args.contains(
            "-c:0 hevc_nvenc -preset:0 p6 -tune:0 hq -rc:0 vbr -cq:0 27 -b:0 0 -tag:0 hvc1"
        ));
        assert!(args.contains("-c:1 copy"));
        assert!(args.contains("-noautorotate -hwaccel cuda -hwaccel_output_format cuda -i file:"));
        assert!(args.ends_with(r"-f mp4 file:C:\in\a.part"));
    }

    #[test]
    fn metadata_and_chapters_are_mapped_explicitly() {
        let info = info(vec![
            stream(0, StreamKind::Data, "bin_data"),
            stream(1, StreamKind::Video, "h264"),
            stream(2, StreamKind::Audio, "aac"),
        ]);
        let args = joined(&plan(&info, nvenc_full()));
        assert!(args.contains("-map 0:1 -map 0:2 -map_metadata 0 -map_chapters 0"));
        assert!(args.contains("-map_metadata:s:0 0:s:1 -map_metadata:s:1 0:s:2"));
        assert!(!args.contains("0:0"));
    }

    #[test]
    fn x265_without_gpu_decoding() {
        let attempt = Attempt {
            encoder: VideoEncoder::X265,
            hw_decode: false,
            preservation: Preservation::Full,
        };
        let info = info(vec![stream(0, StreamKind::Video, "h264")]);
        let args = joined(&plan(&info, attempt));
        assert!(args.contains("-c:0 libx265 -preset:0 medium -crf:0 26"));
        assert!(args.contains("-tag:0 hvc1"));
        assert!(!args.contains("hwaccel"));
    }

    #[test]
    fn mp4_keeps_what_it_can_and_drops_the_rest() {
        let mut flac = stream(3, StreamKind::Audio, "flac");
        flac.channels = Some(6);
        let info = info(vec![
            stream(0, StreamKind::Video, "h264"),
            stream(1, StreamKind::Audio, "aac"),
            stream(2, StreamKind::Audio, "pcm_s16le"),
            flac,
            stream(4, StreamKind::Subtitle, "subrip"),
            stream(5, StreamKind::Subtitle, "hdmv_pgs_subtitle"),
            stream(6, StreamKind::Data, ""),
            cover(7, "mjpeg"),
            cover(8, "bmp"),
            stream(9, StreamKind::Attachment, "ttf"),
        ]);
        let plan = plan(&info, nvenc_full());
        assert_eq!(
            actions(&plan),
            [
                (0, EncodeVideo),
                (1, Copy),
                (2, ToAac { kbps: 128 }),
                (3, Copy),
                (4, ToMovText),
                (7, Copy),
            ]
        );
        assert_eq!(plan.dropped, [5, 6, 8, 9]);
    }

    #[test]
    fn mkv_sources_keep_what_mp4_can_hold() {
        let info = info(vec![
            stream(0, StreamKind::Video, "hevc"),
            stream(1, StreamKind::Audio, "dts"),
            stream(2, StreamKind::Subtitle, "hdmv_pgs_subtitle"),
            stream(3, StreamKind::Subtitle, "ass"),
            stream(4, StreamKind::Subtitle, "mov_text"),
            stream(5, StreamKind::Attachment, "ttf"),
        ]);
        let plan = plan(&info, nvenc_full());
        assert_eq!(
            actions(&plan),
            [
                (0, EncodeVideo),
                (1, ToAac { kbps: 128 }),
                (3, ToMovText),
                (4, Copy)
            ]
        );
        assert_eq!(plan.dropped, [2, 5]);
    }

    #[test]
    fn essential_keeps_only_video_and_aac_audio() {
        let mut surround = stream(1, StreamKind::Audio, "ac3");
        surround.channels = Some(6);
        let info = info(vec![
            stream(0, StreamKind::Video, "h264"),
            surround,
            stream(2, StreamKind::Subtitle, "mov_text"),
            cover(3, "mjpeg"),
        ]);
        let attempt = Attempt {
            preservation: Preservation::Essential,
            ..nvenc_full()
        };
        let plan = plan(&info, attempt);
        assert_eq!(actions(&plan), [(0, EncodeVideo), (1, ToAac { kbps: 384 })]);
        assert_eq!(plan.dropped, [2, 3]);
        assert!(joined(&plan).contains("-c:1 aac -b:1 384k"));
    }

    #[test]
    fn apple_metadata_kept_only_when_present() {
        let mut iphone = info(vec![stream(0, StreamKind::Video, "hevc")]);
        iphone.tag_keys = vec!["com.apple.quicktime.make".into()];
        assert!(plan(&iphone, nvenc_full()).keep_apple_metadata);
        assert!(joined(&plan(&iphone, nvenc_full())).contains("+use_metadata_tags"));

        let plain = info(vec![stream(0, StreamKind::Video, "hevc")]);
        assert!(!plan(&plain, nvenc_full()).keep_apple_metadata);
    }

    #[test]
    fn unicode_paths_are_passed_untouched() {
        let info = info(vec![stream(0, StreamKind::Video, "h264")]);
        let plan = EncodePlan::new(
            Path::new(r"C:\日本語\Mi cumpleaños 🎂 (it's).mp4"),
            Path::new(r"C:\日本語\out.part"),
            &info,
            nvenc_full(),
        );
        let args = plan.args();
        assert!(args.contains(&OsString::from(
            r"file:C:\日本語\Mi cumpleaños 🎂 (it's).mp4"
        )));
    }

    #[test]
    fn attempt_order_with_full_nvidia_support() {
        let caps = Capabilities {
            encoder: VideoEncoder::Nvenc,
            cuda_decode: true,
        };
        let list = attempts(caps);
        assert_eq!(list.len(), 4);
        assert_eq!(list[0], nvenc_full());
        assert!(!list[1].hw_decode);
        assert_eq!(list[3].encoder, VideoEncoder::X265);
        assert_eq!(list[3].preservation, Preservation::Essential);
    }

    #[test]
    fn attempt_order_without_nvidia() {
        let caps = Capabilities {
            encoder: VideoEncoder::X265,
            cuda_decode: false,
        };
        let list = attempts(caps);
        assert!(
            list.iter()
                .all(|a| a.encoder == VideoEncoder::X265 && !a.hw_decode)
        );
        assert_eq!(list.len(), 2);
    }
}
