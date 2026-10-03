//! What this computer and FFmpeg build can actually do (spec §22, §25).
//!
//! FFmpeg is the authority: an encoder counts only if FFmpeg lists it, and NVENC counts only if a
//! tiny real encode succeeds, because a listed `hevc_nvenc` says nothing about the GPU or driver.
//! Detection takes well under a second and runs once per process.

use std::collections::HashSet;
use std::io;
use std::process::Stdio;

use tracing::info;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VideoEncoder {
    /// NVIDIA hardware HEVC encoder.
    Nvenc,
    /// CPU HEVC encoder; always available because the toolchain is required to have it.
    X265,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Capabilities {
    pub encoder: VideoEncoder,
    /// NVDEC decoding through CUDA. Only used together with NVENC.
    pub cuda_decode: bool,
}

impl Capabilities {
    pub fn detect(toolchain: &Toolchain, encoders: &Encoders) -> Self {
        let nvenc = encoders.contains("hevc_nvenc") && nvenc_works(toolchain);
        let cuda_decode = nvenc
            && capture(toolchain, &["-hide_banner", "-hwaccels"])
                .is_ok_and(|listing| listing.lines().any(|line| line.trim() == "cuda"));

        let capabilities = Self {
            encoder: if nvenc {
                VideoEncoder::Nvenc
            } else {
                VideoEncoder::X265
            },
            cuda_decode,
        };
        info!(?capabilities, "detected encoding capabilities");
        capabilities
    }
}

/// Encodes three frames of a synthetic image with NVENC. Fails without a capable GPU and driver.
fn nvenc_works(toolchain: &Toolchain) -> bool {
    let status = toolchain
        .ffmpeg()
        .args(["-hide_banner", "-v", "error", "-f", "lavfi", "-i"])
        .arg("color=c=black:s=256x256:r=30")
        .args(["-frames:v", "3", "-c:v", "hevc_nvenc", "-f", "null", "-"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    let works = status.as_ref().is_ok_and(|s| s.success());
    info!(works, ?status, "NVENC test encode");
    works
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
    fn empty_listing_has_no_encoders() {
        assert!(!Encoders::parse("").contains("libx265"));
    }
}
