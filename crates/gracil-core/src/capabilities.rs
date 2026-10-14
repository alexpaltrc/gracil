//! What this computer and FFmpeg build can actually do (spec §22, §25).
//!
//! FFmpeg is the authority: an encoder counts only if FFmpeg lists it, and a graphics card encoder
//! (NVENC, Quick Sync, AMF) counts only if a tiny real encode succeeds, because a listed one says
//! nothing about the card or driver. Detection takes about a second and runs once per process.

use std::collections::HashSet;
use std::io;
use std::process::Stdio;
use std::thread;

use tracing::info;

use crate::plan;
use crate::probe::{MediaInfo, StreamKind};
use crate::standard::{self, HardwareLevel};
use crate::toolchain::Toolchain;

/// Encoder names reported by `ffmpeg -encoders`.
#[derive(Debug, Clone, Default)]
pub struct Encoders(HashSet<String>);

impl Encoders {
    pub fn list(toolchain: &Toolchain) -> io::Result<Self> {
        let listing = capture(toolchain, &["-hide_banner", "-encoders"])?;
        Ok(Self::parse(&listing))
    }

    /// Parses lines such as ` V....D libx265   libx265 H.265 / HEVC (codec hevc)`.
    fn parse(listing: &str) -> Self {
        let names = listing
            .lines()
            .filter_map(|line| {
                let mut fields = line.split_whitespace();
                let flags = fields.next()?;
                let name = fields.next()?;
                let is_entry = flags.len() == 6
                    && flags.starts_with(['V', 'A', 'S'])
                    && flags
                        .chars()
                        .skip(1)
                        .all(|c| c.is_ascii_uppercase() || c == '.');
                (is_entry && name != "=").then(|| name.to_owned())
            })
            .collect();
        Self(names)
    }

    pub fn contains(&self, name: &str) -> bool {
        self.0.contains(name)
    }
}

/// Codecs FFmpeg can decode, by the names FFprobe reports (`ffmpeg -codecs`).
#[derive(Debug, Clone, Default)]
pub struct Decoders(HashSet<String>);

impl Decoders {
    pub fn list(toolchain: &Toolchain) -> io::Result<Self> {
        let listing = capture(toolchain, &["-hide_banner", "-codecs"])?;
        Ok(Self::parse(&listing))
    }

    /// Parses lines such as ` DEA.L. aac   AAC ...`, keeping those whose first flag is `D`.
    fn parse(listing: &str) -> Self {
        let names = listing
            .lines()
            .filter_map(|line| {
                let mut fields = line.split_whitespace();
                let flags = fields.next()?;
                let name = fields.next()?;
                let is_entry = flags.len() == 6
                    && flags.starts_with('D')
                    && flags.chars().all(|c| c.is_ascii_uppercase() || c == '.');
                (is_entry && name != "=").then(|| name.to_owned())
            })
            .collect();
        Self(names)
    }

    pub fn can_decode(&self, codec: &str) -> bool {
        self.0.contains(codec)
    }

    /// Marks the audio streams of `info` that FFmpeg can't decode. An empty list (FFmpeg didn't
    /// answer) marks nothing, so every stream is tried as before.
    pub fn mark(&self, info: &mut MediaInfo) {
        if self.0.is_empty() {
            return;
        }
        for stream in &mut info.streams {
            stream.undecodable =
                stream.kind == StreamKind::Audio && !self.can_decode(&stream.codec);
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VideoEncoder {
    /// NVIDIA hardware HEVC encoder.
    Nvenc,
    /// Intel Quick Sync hardware HEVC encoder (ADR 0052).
    Qsv,
    /// AMD hardware HEVC encoder (ADR 0052).
    Amf,
    /// CPU HEVC encoder; always available because the toolchain is required to have it.
    X265,
}

impl VideoEncoder {
    /// Graphics card encoders, in the order they are tried: a computer with an NVIDIA card next to
    /// Intel graphics uses the NVIDIA card.
    pub const ON_THE_CARD: [Self; 3] = [Self::Nvenc, Self::Qsv, Self::Amf];

    /// FFmpeg's name for this encoder.
    pub fn name(self) -> &'static str {
        match self {
            Self::Nvenc => "hevc_nvenc",
            Self::Qsv => "hevc_qsv",
            Self::Amf => "hevc_amf",
            Self::X265 => "libx265",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Capabilities {
    pub encoder: VideoEncoder,
    /// The card decodes for its own encoder: NVIDIA through CUDA; Quick Sync and AMF once a test
    /// frame went through their decoding and encoding (ADR 0065).
    pub card_decode: bool,
    /// The settings this card and driver accept for `encoder` (ADR 0028, ADR 0052); `Legacy` for
    /// x265.
    pub hardware: HardwareLevel,
    /// x265 can encode 10-bit video.
    pub x265_10bit: bool,
    /// VMAF, to measure each video's quality level.
    pub vmaf: bool,
    /// VMAF on the GPU (`libvmaf_cuda`), used instead of the CPU when this FFmpeg has it and the
    /// video is decoded there. No public Windows build has it today (ADR 0030).
    pub vmaf_cuda: bool,
}

impl Capabilities {
    /// Every listing and every card's test encodes run at once, each card's tiers still best
    /// first, so detecting takes about as long as the slowest card (ADR 0065). Nothing is kept
    /// from one run of Gracil to the next: a new driver or card is seen at the next start.
    pub fn detect(toolchain: &Toolchain, encoders: &Encoders) -> Self {
        let listed: Vec<VideoEncoder> = VideoEncoder::ON_THE_CARD
            .into_iter()
            .filter(|encoder| encoders.contains(encoder.name()))
            .collect();
        let listing = |args: &'static [&'static str]| move || capture(toolchain, args);
        let (tiers, hwaccels, filters, x265_help) = thread::scope(|scope| {
            let cards: Vec<_> = listed
                .iter()
                .map(|&encoder| {
                    scope.spawn(move || {
                        HardwareLevel::BEST_FIRST
                            .into_iter()
                            .find(|&level| card_encodes(toolchain, encoder, level))
                    })
                })
                .collect();
            let hwaccels = scope.spawn(listing(&["-hide_banner", "-hwaccels"]));
            let filters = scope.spawn(listing(&["-hide_banner", "-filters"]));
            let x265 = scope.spawn(listing(&["-hide_banner", "-h", "encoder=libx265"]));
            let joined = |result: thread::Result<io::Result<String>>| {
                result.unwrap_or_else(|_| Err(io::Error::other("a listing thread failed")))
            };
            let tiers: Vec<Option<HardwareLevel>> = cards
                .into_iter()
                .map(|card| card.join().ok().flatten())
                .collect();
            (
                tiers,
                joined(hwaccels.join()),
                joined(filters.join()),
                joined(x265.join()),
            )
        });
        // The first card in order of preference whose test passed.
        let on_the_card = listed
            .into_iter()
            .zip(tiers)
            .find_map(|(encoder, level)| level.map(|level| (encoder, level)));
        let (encoder, hardware) =
            on_the_card.unwrap_or((VideoEncoder::X265, HardwareLevel::Legacy));
        let filters = filters.unwrap_or_default();
        let hwaccels = hwaccels.unwrap_or_default();
        let lists_hwaccel = |name| hwaccels.lines().any(|line| line.trim() == name);
        // Quick Sync and AMF decode for their encoder only once a frame went through both on
        // this card (ADR 0065); NVIDIA's decoding has been relied on since the start.
        let card_decode = match plan::card_decoding(encoder) {
            Some(decoding) if encoder == VideoEncoder::Nvenc => lists_hwaccel(decoding.hwaccel),
            Some(decoding) => {
                lists_hwaccel(decoding.hwaccel)
                    && decoding
                        .converter
                        .is_none_or(|converter| lists_filter(&filters, converter))
                    && card_decodes(toolchain, encoder, hardware)
            }
            None => false,
        };
        let x265_10bit = x265_help.is_ok_and(|help| help.contains("yuv420p10le"));

        let capabilities = Self {
            encoder,
            card_decode,
            hardware,
            x265_10bit,
            vmaf: lists_filter(&filters, "libvmaf"),
            vmaf_cuda: encoder == VideoEncoder::Nvenc
                && card_decode
                && ["libvmaf_cuda", "scale_cuda", "hwupload_cuda"]
                    .iter()
                    .all(|name| lists_filter(&filters, name)),
        };
        info!(?capabilities, "detected encoding capabilities");
        capabilities
    }

    /// Whether `encoder` will write 10-bit video here.
    pub fn ten_bit(&self, encoder: VideoEncoder) -> bool {
        match encoder {
            VideoEncoder::X265 => self.x265_10bit,
            _ => self.hardware.ten_bit(),
        }
    }
}

/// Encodes three frames of a synthetic image with `encoder` and `level`'s settings, fed the way
/// Gracil feeds it. Fails without a capable card and driver, or when they don't accept those
/// settings: a listed encoder says nothing about the card.
fn card_encodes(toolchain: &Toolchain, encoder: VideoEncoder, level: HardwareLevel) -> bool {
    let mut command = toolchain.ffmpeg();
    command
        .args(["-hide_banner", "-v", "error", "-f", "lavfi", "-i"])
        .arg("color=c=black:s=256x256:r=30")
        .args(["-frames:v", "3", "-vf"])
        .arg(format!(
            "format={}",
            standard::pixel_format(level.ten_bit(), encoder)
        ))
        .args(["-c:v", encoder.name()]);
    let level_options = standard::level_options(encoder, standard::fallback_level(encoder));
    let options = level
        .options(encoder)
        .iter()
        .map(|&(n, v)| (n, v.to_owned()));
    for (name, value) in options.chain(level_options) {
        command.arg(format!("-{name}:v")).arg(value);
    }
    let status = command
        .args(["-f", "null", "-"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    let works = status.as_ref().is_ok_and(|s| s.success());
    info!(
        works,
        ?encoder,
        ?level,
        ?status,
        "graphics card test encode"
    );
    works
}

/// Decodes one frame of H.264, the most common kind of video, on the card that encodes with
/// `encoder`, and encodes it there with `level`'s settings, the frames staying on the card as
/// Gracil would keep them (ADR 0065). Fails where the card, its driver or FFmpeg can't, as on a
/// computer without that card: nothing changes there.
fn card_decodes(toolchain: &Toolchain, encoder: VideoEncoder, level: HardwareLevel) -> bool {
    let Some(decoding) = plan::card_decoding(encoder) else {
        return false;
    };
    // The frame to decode, made on the processor.
    let mut maker = toolchain.ffmpeg();
    maker
        .args(["-hide_banner", "-v", "error", "-f", "lavfi", "-i"])
        .arg("testsrc2=s=256x256:r=30")
        .args(["-frames:v", "1", "-pix_fmt", "yuv420p", "-c:v", "libx264"])
        .args(["-f", "h264", "pipe:1"])
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    let Ok(mut made) = maker.spawn() else {
        return false;
    };
    let Some(frame) = made.stdout.take() else {
        let _ = made.kill();
        let _ = made.wait();
        return false;
    };

    let mut command = toolchain.ffmpeg();
    command
        .args(["-hide_banner", "-v", "error"])
        .args(["-hwaccel", decoding.hwaccel])
        .args(["-hwaccel_output_format", decoding.frames])
        .args(["-f", "h264", "-i", "pipe:0"]);
    if let Some(conversion) = plan::card_conversion(encoder, level.ten_bit()) {
        command.args(["-vf", &conversion]);
    }
    command.args(["-c:v", encoder.name()]);
    let level_options = standard::level_options(encoder, standard::fallback_level(encoder));
    let options = level
        .options(encoder)
        .iter()
        .map(|&(n, v)| (n, v.to_owned()));
    for (name, value) in options.chain(level_options) {
        command.arg(format!("-{name}:v")).arg(value);
    }
    let status = command
        .args(["-f", "null", "-"])
        .stdin(Stdio::from(frame))
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    // Gracil's own end of the pipe goes with the command, so the first FFmpeg never waits on
    // a reader that is gone.
    drop(command);
    let _ = made.wait();
    let works = status.as_ref().is_ok_and(|s| s.success());
    info!(
        works,
        ?encoder,
        ?level,
        ?status,
        "graphics card test decode"
    );
    works
}

/// Whether an `ffmpeg -filters` listing has `name`, in lines like ` .. libvmaf  VV->V  ...`.
fn lists_filter(listing: &str, name: &str) -> bool {
    listing
        .lines()
        .any(|line| line.split_whitespace().nth(1) == Some(name))
}

fn capture(toolchain: &Toolchain, args: &[&str]) -> io::Result<String> {
    let output = toolchain
        .ffmpeg()
        .args(args)
        .stderr(Stdio::null())
        .output()?;
    if !output.status.success() {
        return Err(io::Error::other(format!(
            "ffmpeg {args:?} exited with {}",
            output.status
        )));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    const LISTING: &str = "\
Encoders:
 V..... = Video
 A..... = Audio
 S..... = Subtitle
 .F.... = Frame-level multithreading
 ------
 V....D libx264              libx264 H.264 / AVC / MPEG-4 AVC / MPEG-4 part 10 (codec h264)
 V....D libx265              libx265 H.265 / HEVC (codec hevc)
 V....D hevc_nvenc           NVIDIA NVENC hevc encoder (codec hevc)
 A....D aac                  AAC (Advanced Audio Coding)
 S..... mov_text             3GPP Timed Text subtitle
";

    #[test]
    fn parses_encoder_names_only() {
        let encoders = Encoders::parse(LISTING);
        for name in ["libx264", "libx265", "hevc_nvenc", "aac", "mov_text"] {
            assert!(encoders.contains(name), "{name}");
        }
        for not_a_name in ["=", "Video", "------", "Encoders:", "H.265"] {
            assert!(!encoders.contains(not_a_name), "{not_a_name}");
        }
    }

    #[test]
    fn knows_which_codecs_decode() {
        let listing = "\
Codecs:
 D..... = Decoding supported
 .E.... = Encoding supported
 -------
 DEA.L. aac                  AAC (Advanced Audio Coding) (decoders: aac aac_fixed)
 D.AI.S apac                 Marian's A-pac audio
 ..A.L. apple_apac           Apple Positional Audio Codec
";
        let decoders = Decoders::parse(listing);
        assert!(decoders.can_decode("aac"));
        assert!(decoders.can_decode("apac"));
        assert!(!decoders.can_decode("apple_apac"));
        assert!(!decoders.can_decode("="));
        assert!(!decoders.can_decode("Decoding"));
    }

    #[test]
    fn finds_filters_by_name() {
        let listing = "Filters:\n  T.. = Timeline support\n ... libvmaf           VV->V      Calculate the VMAF\n ... scale             V->V       Scale the input video\n";
        assert!(lists_filter(listing, "libvmaf"));
        assert!(lists_filter(listing, "scale"));
        assert!(!lists_filter(listing, "scale_cuda"));
        assert!(!lists_filter(listing, "Timeline"));
    }

    /// Where a card can't encode, as Intel's and AMD's on a computer without them, decoding on it
    /// fails too, by itself and quickly, so nothing changes there.
    #[test]
    fn the_card_decode_test_fails_cleanly_without_the_card() {
        let Some(found) = crate::toolchain::find() else {
            eprintln!("skipped: no FFmpeg installed");
            return;
        };
        for encoder in [VideoEncoder::Qsv, VideoEncoder::Amf] {
            let level = HardwareLevel::Full;
            if found.encoders.contains(encoder.name())
                && !card_encodes(&found.toolchain, encoder, level)
            {
                let started = std::time::Instant::now();
                assert!(
                    !card_decodes(&found.toolchain, encoder, level),
                    "{encoder:?}"
                );
                assert!(started.elapsed() < std::time::Duration::from_secs(30));
            }
        }
    }

    #[test]
    fn empty_listing_has_no_encoders() {
        assert!(!Encoders::parse("").contains("libx265"));
    }
}
