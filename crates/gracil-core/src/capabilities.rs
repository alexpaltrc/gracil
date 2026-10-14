//! What this computer and FFmpeg build can actually do (spec §22, §25).
//!
//! FFmpeg is the authority: an encoder counts only if FFmpeg lists it, and a graphics card encoder
//! (NVENC, Quick Sync, AMF) counts only if a tiny real encode succeeds, because a listed one says
//! nothing about the card or driver. Detection takes about a second and runs once per process.

use std::collections::HashSet;
use std::io;
use std::process::Stdio;

use tracing::info;

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
    /// NVDEC decoding through CUDA. Only used together with NVENC.
    pub cuda_decode: bool,
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
    pub fn detect(toolchain: &Toolchain, encoders: &Encoders) -> Self {
        let on_the_card = VideoEncoder::ON_THE_CARD
            .into_iter()
            .filter(|encoder| encoders.contains(encoder.name()))
            .find_map(|encoder| {
                HardwareLevel::BEST_FIRST
                    .into_iter()
                    .find(|&level| card_encodes(toolchain, encoder, level))
                    .map(|level| (encoder, level))
            });
        let (encoder, hardware) =
            on_the_card.unwrap_or((VideoEncoder::X265, HardwareLevel::Legacy));
        let cuda_decode = encoder == VideoEncoder::Nvenc
            && capture(toolchain, &["-hide_banner", "-hwaccels"])
                .is_ok_and(|listing| listing.lines().any(|line| line.trim() == "cuda"));
        let filters = capture(toolchain, &["-hide_banner", "-filters"]).unwrap_or_default();
        let x265_10bit = capture(toolchain, &["-hide_banner", "-h", "encoder=libx265"])
            .is_ok_and(|help| help.contains("yuv420p10le"));

        let capabilities = Self {
            encoder,
            cuda_decode,
            hardware,
            x265_10bit,
            vmaf: lists_filter(&filters, "libvmaf"),
            vmaf_cuda: cuda_decode
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

    #[test]
    fn empty_listing_has_no_encoders() {
        assert!(!Encoders::parse("").contains("libx265"));
    }
}
