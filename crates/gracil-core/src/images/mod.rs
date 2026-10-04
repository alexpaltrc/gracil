//! Compressing one image into a temporary file next to it (ADR 0034).
//!
//! A PNG stays a PNG; every other image becomes a JPEG written by jpegli, or a PNG when it has
//! transparency. The first compression looks the same as the original (jpegli's distance 1.0, or
//! a lossless PNG, or a palette scoring at least 90); when that would not be clearly smaller, the
//! image was already compressed, and Gracil goes a little further without asking (scores of at
//! least 80). Nothing is kept unless it is at most 80% of the original's size.
//!
//! Nothing here opens a window: images are read by the `image` crate inside Gracil, or by FFmpeg
//! (HEIC, HEIF, AVIF), and jpegli runs hidden like FFmpeg. jpegli always does the compressing.

mod exif;
mod heif;
mod jpeg;
mod png;
mod score;

use std::ffi::OsString;
use std::fs;
use std::io::{self, Cursor};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::thread;
use std::time::Duration;

use image::{DynamicImage, ImageDecoder, ImageReader};
use tracing::{info, warn};

use crate::compress::{Engine, Update, reserve_temp};
use crate::encode::{EncodeOutcome, JobControl, run_ffmpeg, run_program};
use crate::leftovers;
use crate::share;
use crate::standard::WORTH_IT;
use crate::toolchain::hidden_command;
use jpeg::Carried;
use png::Extras;
use score::Reference;

/// jpegli's own "visually lossless" distance: every first compression of a photo (item 3).
pub const FIRST_DISTANCE: f32 = 1.0;
/// The looser distances tried when compressing again, tightest first (item 4).
pub const AGAIN_DISTANCES: [f32; 4] = [1.5, 2.0, 2.5, 3.0];
/// SSIMULACRA2 a palette needs on a first compression: no difference even flipping (item 5).
pub const FIRST_SCORE: f64 = 90.0;
/// SSIMULACRA2 anything needs when compressing again: no difference at normal viewing (item 4).
pub const AGAIN_SCORE: f64 = 80.0;
/// About how much memory, in bytes, the palettes being made at the same time may use.
const PALETTE_MEMORY: usize = 1 << 30;
/// About how much memory one SSIMULACRA2 measurement uses for each pixel of the picture.
const SCORE_BYTES_PER_PIXEL: usize = 128;

/// What a compressed image is saved as.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    Jpeg,
    Png,
}

/// The extension the result gets: the source's own spelling when the format does not change
/// (`Photo.JPEG` stays `Photo.JPEG`), otherwise `jpg` or `png`.
pub fn output_extension(source: &Path, target: Target) -> OsString {
    let (same, default) = match target {
        Target::Jpeg => (&["jpg", "jpeg", "jpe", "jfif"][..], "jpg"),
        Target::Png => (&["png"][..], "png"),
    };
    match source.extension() {
        Some(ext) if same.iter().any(|s| ext.eq_ignore_ascii_case(s)) => ext.to_owned(),
        _ => default.into(),
    }
}

#[derive(Debug)]
pub struct CompressedImage {
    /// The finished, checked file. The caller moves it into place or deletes it.
    pub temp: PathBuf,
    pub size: u64,
    /// The extension it is to be saved with (see [`output_extension`]).
    pub extension: OsString,
}

#[derive(Debug, thiserror::Error)]
pub enum ImageError {
    #[error("the image could not be read: {0}")]
    Unreadable(String),
    /// Animated PNG or WebP (item 1).
    #[error("animated images are not compressed yet")]
    Animated,
    /// GIF, camera RAW, JPEG XL (item 1).
    #[error("this kind of image is not compressed")]
    NotAccepted,
    /// Stored as true HDR, PQ or HLG (item 11).
    #[error("the image is stored as HDR")]
    Hdr,
    #[error("jpegli is missing")]
    NoEncoder,
    #[error("could not write a temporary file")]
    Io(#[from] io::Error),
    #[error("cancelled")]
    Cancelled,
    #[error("the image could not be compressed: {0}")]
    Failed(String),
    /// Already compressed, and not even compressing it again would make it clearly smaller.
    #[error("the image is already about as small as it gets")]
    AsSmallAsItGets,
}

/// What a file holds, by its first bytes rather than its name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Format {
    Jpeg,
    Png,
    /// HEIC, HEIF and AVIF: read through FFmpeg.
    Heif,
    /// WebP, TIFF and BMP: read by the `image` crate.
    Other,
    WebP,
    NotAccepted,
    Unknown,
}

impl Format {
    fn sniff(data: &[u8]) -> Self {
        let at = |range: std::ops::Range<usize>| data.get(range).unwrap_or_default();
        if data.starts_with(&[0xff, 0xd8, 0xff]) {
            Self::Jpeg
        } else if png::is_png(data) {
            Self::Png
        } else if at(4..8) == b"ftyp" {
            Self::Heif
        } else if at(0..4) == b"RIFF" && at(8..12) == b"WEBP" {
            Self::WebP
        } else if data.starts_with(b"II*\0")
            || data.starts_with(b"MM\0*")
            || data.starts_with(b"BM")
        {
            Self::Other
        } else if data.starts_with(b"GIF8")
            || data.starts_with(&[0xff, 0x0a])
            || data.starts_with(b"\0\0\0\x0cJXL ")
        {
            Self::NotAccepted
        } else {
            Self::Unknown
        }
    }
}

/// Compresses the image at `source` into a temporary file next to it. The source is never touched.
pub fn compress(
    engine: &Engine,
    source: &Path,
    control: &JobControl,
    on_update: &mut dyn FnMut(Update),
) -> Result<CompressedImage, ImageError> {
    let report = |on_update: &mut dyn FnMut(Update), fraction: f64| {
        on_update(Update {
            fraction: Some(fraction),
            speed: None,
        });
    };
    report(on_update, 0.0);
    // A camera RAW file can start like a TIFF; its name decides before its bytes are read.
    if crate::media::is_refused_image(source) {
        return Err(ImageError::NotAccepted);
    }
    let data = fs::read(source).map_err(|e| ImageError::Unreadable(e.to_string()))?;
    let limit = (data.len() as f64 * WORTH_IT) as u64;
    let mut job = Job {
        engine,
        control,
        scratch: Scratch::default(),
        limit,
    };

    let format = Format::sniff(&data);
    info!(?format, size = data.len(), "compressing image");
    let (bytes, target) = match format {
        Format::NotAccepted => return Err(ImageError::NotAccepted),
        Format::Unknown => return Err(ImageError::Unreadable("unknown format".into())),
        Format::Jpeg => {
            let carried = Carried::from_jpeg(&data);
            let expected = ImageReader::new(Cursor::new(&data))
                .with_guessed_format()
                .ok()
                .and_then(|reader| reader.into_dimensions().ok())
                .ok_or_else(|| ImageError::Unreadable("not a whole JPEG".into()))?;
            let reference = || decode_with_image(&data, false).map(|d| d.reference);
            let bytes = job.make_jpeg(source, &carried, expected, reference, &mut |f| {
                report(on_update, f);
            })?;
            (bytes, Target::Jpeg)
        }
        Format::Png => {
            if png::is_animated(&data) {
                return Err(ImageError::Animated);
            }
            if png::is_hdr(&data) {
                return Err(ImageError::Hdr);
            }
            let decoded = decode_with_image(&data, false)?;
            report(on_update, 0.1);
            let extras = Extras {
                chunks: png::descriptive_chunks(&data),
                icc: None,
            };
            let lossless = png::lossless(&data).map_err(ImageError::Failed)?;
            let bytes = job.make_png(lossless, &decoded.reference, &extras, &mut |f| {
                report(on_update, f);
            })?;
            (bytes, Target::Png)
        }
        Format::Heif | Format::WebP | Format::Other => {
            let decoded = if format == Format::Heif {
                job.decode_with_ffmpeg(source, &data)?
            } else {
                if format == Format::WebP && is_animated_webp(&data) {
                    return Err(ImageError::Animated);
                }
                decode_with_image(&data, true)?
            };
            report(on_update, 0.1);
            if decoded.reference.has_alpha {
                let extras = Extras {
                    chunks: decoded
                        .exif
                        .iter()
                        .map(|tiff| (*b"eXIf", tiff.clone()))
                        .collect(),
                    icc: decoded.icc.clone(),
                };
                let reference = &decoded.reference;
                let lossless = png::from_pixels(
                    &reference.pixels,
                    reference.width,
                    reference.height,
                    &extras,
                )
                .map_err(ImageError::Failed)?;
                let bytes = job.make_png(lossless, reference, &extras, &mut |f| {
                    report(on_update, f);
                })?;
                (bytes, Target::Png)
            } else {
                let input = job.scratch.file("decoded.ppm")?;
                fs::write(&input, ppm(&decoded.reference))?;
                let carried = Carried::from_parts(decoded.exif.as_deref(), decoded.icc.as_deref());
                let expected = (decoded.reference.width, decoded.reference.height);
                let bytes = job.make_jpeg(
                    &input,
                    &carried,
                    expected,
                    || Ok(decoded.reference),
                    &mut |f| {
                        report(on_update, f);
                    },
                )?;
                (bytes, Target::Jpeg)
            }
        }
    };

    let temp = reserve_temp(source)?;
    if let Err(error) = fs::write(&temp, &bytes) {
        let _ = fs::remove_file(&temp);
        return Err(error.into());
    }
    report(on_update, 1.0);
    Ok(CompressedImage {
        temp,
        size: bytes.len() as u64,
        extension: output_extension(source, target),
    })
}

/// A picture decoded to compare against, with what it carries.
struct Decoded {
    reference: Reference,
    /// EXIF data as a TIFF structure, its orientation already "upright" when the pixels were
    /// turned.
    exif: Option<Vec<u8>>,
    icc: Option<Vec<u8>>,
}

/// Decodes with the `image` crate. With `upright`, the picture is turned as its orientation says
/// and the EXIF data then says "upright"; without, pixels stay as stored, as jpegli keeps them.
fn decode_with_image(data: &[u8], upright: bool) -> Result<Decoded, ImageError> {
    let unreadable = |e: image::ImageError| ImageError::Unreadable(e.to_string());
    let mut decoder = ImageReader::new(Cursor::new(data))
        .with_guessed_format()
        .map_err(|e| ImageError::Unreadable(e.to_string()))?
        .into_decoder()
        .map_err(unreadable)?;
    let icc = decoder.icc_profile().ok().flatten();
    let mut exif = decoder
        .exif_metadata()
        .ok()
        .flatten()
        .map(|raw| exif::strip_prefix(&raw).to_vec());
    let orientation = decoder.orientation().ok();
    let mut picture = DynamicImage::from_decoder(decoder).map_err(unreadable)?;
    if upright {
        if let Some(orientation) = orientation {
            picture.apply_orientation(orientation);
        }
        if let Some(tiff) = exif.as_mut() {
            exif::set_upright(tiff);
        }
    }
    let rgba = picture.into_rgba8();
    let (width, height) = rgba.dimensions();
    let pixels = rgba.as_raw().as_chunks::<4>().0.to_vec();
    Ok(Decoded {
        reference: Reference::new(pixels, width, height),
        exif,
        icc,
    })
}

fn is_animated_webp(data: &[u8]) -> bool {
    image::codecs::webp::WebPDecoder::new(Cursor::new(data)).is_ok_and(|d| d.has_animation())
}

/// The picture as a binary PPM, the plainest input jpegli reads.
fn ppm(reference: &Reference) -> Vec<u8> {
    let mut out = format!("P6\n{} {}\n255\n", reference.width, reference.height).into_bytes();
    out.reserve(reference.pixels.len() * 3);
    for pixel in &reference.pixels {
        out.extend_from_slice(&pixel[..3]);
    }
    out
}

struct Job<'a> {
    engine: &'a Engine,
    control: &'a JobControl,
    scratch: Scratch,
    /// The largest result worth keeping, in bytes.
    limit: u64,
}

impl Job<'_> {
    /// Waits while paused; stops when cancelled. Called between steps that run inside Gracil.
    fn checkpoint(&self) -> Result<(), ImageError> {
        while self.control.is_paused() && !self.control.is_cancelled() {
            thread::sleep(Duration::from_millis(100));
        }
        if self.control.is_cancelled() {
            return Err(ImageError::Cancelled);
        }
        Ok(())
    }

    /// Reads a HEIC, HEIF or AVIF picture through FFmpeg, which turns it upright, and its EXIF
    /// data and color profile from the file itself.
    fn decode_with_ffmpeg(&mut self, source: &Path, data: &[u8]) -> Result<Decoded, ImageError> {
        let info = heif::read(data);
        if info.hdr {
            return Err(ImageError::Hdr);
        }
        if info.alpha {
            // FFmpeg reads only the main picture; its transparency would be lost.
            return Err(ImageError::Unreadable(
                "transparency in a HEIF picture".into(),
            ));
        }
        let output = self.scratch.file("decoded.png")?;
        let mut args: Vec<OsString> = vec!["-v".into(), "error".into(), "-i".into()];
        args.push(source.into());
        for arg in [
            "-frames:v",
            "1",
            "-pix_fmt",
            "rgba",
            "-c:v",
            "png",
            "-compression_level",
            "0",
            "-update",
            "1",
            "-y",
        ] {
            args.push(arg.into());
        }
        args.push(output.clone().into());
        let (outcome, log) = run_ffmpeg(&self.engine.toolchain, args, self.control, &mut |_| {});
        match outcome {
            EncodeOutcome::Finished => {}
            EncodeOutcome::Cancelled => return Err(ImageError::Cancelled),
            EncodeOutcome::Failed { error, .. } => {
                warn!(%error, %log, "FFmpeg could not read the image");
                return Err(ImageError::Unreadable(error));
            }
        }
        let png = fs::read(&output)?;
        let mut decoded = decode_with_image(&png, false)?;
        decoded.icc = info.icc;
        decoded.exif = info.exif.map(|mut tiff| {
            exif::set_upright(&mut tiff);
            tiff
        });
        Ok(decoded)
    }

    /// jpegli at `distance`, from `input` (a JPEG or a PPM), with `carried` inserted.
    fn jpegli(
        &mut self,
        input: &Path,
        distance: f32,
        carried: &Carried,
    ) -> Result<Vec<u8>, ImageError> {
        self.checkpoint()?;
        let program = self.engine.jpegli.as_ref().ok_or(ImageError::NoEncoder)?;
        let output = self.scratch.file(&format!("d{distance}.jpg"))?;
        let args = vec![
            input.as_os_str().to_owned(),
            output.as_os_str().to_owned(),
            "-d".into(),
            distance.to_string().into(),
            "--quiet".into(),
        ];
        let (outcome, log) = run_program(hidden_command(program), args, self.control, &mut |_| {});
        match outcome {
            EncodeOutcome::Finished => {}
            EncodeOutcome::Cancelled => return Err(ImageError::Cancelled),
            EncodeOutcome::Failed { error, .. } => {
                warn!(%error, %log, "jpegli failed");
                return Err(ImageError::Failed(error));
            }
        }
        let encoded = fs::read(&output)?;
        let _ = fs::remove_file(&output);
        if carried.is_empty() {
            return Ok(encoded);
        }
        carried
            .insert_into(&encoded)
            .ok_or_else(|| ImageError::Failed("jpegli wrote something other than a JPEG".into()))
    }

    /// A JPEG: distance 1.0 when that is clearly smaller; otherwise the loosest distance from 1.5
    /// to 3.0 that still scores 80 and is clearly smaller, measuring at most three (item 4).
    fn make_jpeg(
        &mut self,
        input: &Path,
        carried: &Carried,
        expected: (u32, u32),
        reference: impl FnOnce() -> Result<Reference, ImageError>,
        report: &mut dyn FnMut(f64),
    ) -> Result<Vec<u8>, ImageError> {
        let first = self.jpegli(input, FIRST_DISTANCE, carried)?;
        report(0.3);
        if first.len() as u64 <= self.limit {
            check_jpeg(&first, expected)?;
            return Ok(first);
        }
        info!(size = first.len(), "already compressed; compressing again");
        self.checkpoint()?;
        let reference = reference()?;
        report(0.4);

        let mut measured = 0;
        let mut try_distance =
            |job: &mut Self, distance: f32| -> Result<(Vec<u8>, Option<f64>), ImageError> {
                let bytes = job.jpegli(input, distance, carried)?;
                job.checkpoint()?;
                let pixels = decode_with_image(&bytes, false)?.reference.pixels;
                let score = reference.score(&pixels);
                measured += 1;
                report(0.4 + 0.18 * f64::from(measured));
                info!(distance, size = bytes.len(), ?score, "measured");
                Ok((bytes, score))
            };
        let passes = |score: Option<f64>| score.is_some_and(|s| s >= AGAIN_SCORE);
        let limit = self.limit;
        let small = |bytes: &[u8]| bytes.len() as u64 <= limit;

        let start = AGAIN_DISTANCES.iter().position(|&d| d == 2.0).unwrap_or(0);
        let (bytes, score) = try_distance(self, AGAIN_DISTANCES[start])?;
        let mut best = None;
        if passes(score) {
            if small(&bytes) {
                best = Some(bytes);
            }
            for &distance in &AGAIN_DISTANCES[start + 1..] {
                let (bytes, score) = try_distance(self, distance)?;
                if !passes(score) {
                    break;
                }
                if small(&bytes) {
                    best = Some(bytes);
                }
            }
        } else if small(&bytes) {
            // Tighter distances only grow; one is worth trying only while this one was small.
            for &distance in AGAIN_DISTANCES[..start].iter().rev() {
                let (bytes, score) = try_distance(self, distance)?;
                if passes(score) && small(&bytes) {
                    best = Some(bytes);
                    break;
                }
            }
        }
        best.ok_or(ImageError::AsSmallAsItGets)
    }

    /// A PNG: the smallest of the lossless version and the palettes scoring at least 90; when
    /// that is not clearly smaller, the smallest palette scoring at least 80 (items 5 and 6).
    fn make_png(
        &mut self,
        lossless: Vec<u8>,
        reference: &Reference,
        extras: &Extras,
        report: &mut dyn FnMut(f64),
    ) -> Result<Vec<u8>, ImageError> {
        report(0.2);
        let tries: Vec<(usize, bool)> = png::PALETTE_SIZES
            .iter()
            .flat_map(|&colors| [(colors, false), (colors, true)])
            .collect();
        // Palettes are made side by side, as many at once as the processor and memory allow:
        // each needs a few copies of the picture.
        let pixels = reference.pixels.len().max(1);
        let at_once = share::threads_for_one()
            .min(PALETTE_MEMORY / (pixels * 32))
            .clamp(1, tries.len());
        let mut palettes = Vec::new();
        for (batch, group) in tries.chunks(at_once).enumerate() {
            self.checkpoint()?;
            let made: Vec<_> = thread::scope(|scope| {
                let handles: Vec<_> = group
                    .iter()
                    .map(|&(colors, dithered)| {
                        scope.spawn(move || {
                            let made = png::palette(
                                &reference.pixels,
                                reference.width,
                                reference.height,
                                colors,
                                dithered,
                                extras,
                            );
                            (colors, dithered, made)
                        })
                    })
                    .collect();
                handles.into_iter().filter_map(|h| h.join().ok()).collect()
            });
            for (colors, dithered, made) in made {
                match made {
                    // Only a palette smaller than the lossless version can be chosen.
                    Ok(palette) if palette.png.len() < lossless.len() => palettes.push(palette),
                    Ok(_) => {}
                    Err(error) => warn!(%error, colors, dithered, "palette failed"),
                }
            }
            let done = ((batch + 1) * at_once).min(tries.len());
            report(0.2 + 0.4 * (done as f64 / tries.len() as f64));
        }
        palettes.sort_by_key(|p| p.png.len());

        let mut scores = vec![None; palettes.len()];

        // `None` is the lossless version.
        let first =
            self.smallest_passing(&palettes, reference, FIRST_SCORE, u64::MAX, &mut scores)?;
        report(0.8);
        let first_size = first.map_or(lossless.len(), |n| palettes[n].png.len());
        let chosen = if first_size as u64 <= self.limit {
            first
        } else {
            info!(size = first_size, "already compressed; compressing again");
            let again =
                self.smallest_passing(&palettes, reference, AGAIN_SCORE, self.limit, &mut scores)?;
            Some(again.ok_or(ImageError::AsSmallAsItGets)?)
        };
        Ok(match chosen {
            Some(n) => palettes.swap_remove(n).png,
            None => lossless,
        })
    }

    /// The smallest palette scoring at least `threshold` and weighing at most `limit` bytes.
    ///
    /// Fewer colors hardly ever look better, so each kind of palette, plain and dithered, is
    /// measured from the most colors down, and stops at the first that falls short: the ones with
    /// fewer colors would fall short too. That usually takes two measurements instead of eight,
    /// which matters because each one takes about a second for a large picture. The two kinds are
    /// measured side by side when memory allows. Scores already known are kept in `scores`.
    fn smallest_passing(
        &self,
        palettes: &[png::Palette],
        reference: &Reference,
        threshold: f64,
        limit: u64,
        scores: &mut [Option<f64>],
    ) -> Result<Option<usize>, ImageError> {
        let kind = |dithered: bool| {
            let mut found: Vec<usize> = (0..palettes.len())
                .filter(|&n| palettes[n].dithered == dithered)
                .collect();
            found.sort_by_key(|&n| std::cmp::Reverse(palettes[n].colors));
            found
        };
        let mut kinds = [kind(false), kind(true)].map(|order| order.into_iter().peekable());
        let pixels = reference.pixels.len().max(1);
        let side_by_side = PALETTE_MEMORY / (pixels * SCORE_BYTES_PER_PIXEL) >= 2;
        let mut passing = Vec::new();
        loop {
            let next: Vec<usize> = kinds.iter_mut().filter_map(|k| k.peek().copied()).collect();
            if next.is_empty() {
                break;
            }
            self.checkpoint()?;
            let unknown: Vec<usize> = next
                .iter()
                .copied()
                .filter(|&n| scores[n].is_none())
                .collect();
            let measure = |n: usize| reference.score(&palettes[n].pixels).unwrap_or(0.0);
            let measured: Vec<f64> = if side_by_side && unknown.len() > 1 {
                thread::scope(|scope| {
                    let handles: Vec<_> = unknown
                        .iter()
                        .map(|&n| scope.spawn(move || measure(n)))
                        .collect();
                    handles
                        .into_iter()
                        .map(|h| h.join().unwrap_or(0.0))
                        .collect()
                })
            } else {
                unknown.iter().map(|&n| measure(n)).collect()
            };
            for (&n, score) in unknown.iter().zip(measured) {
                let palette = &palettes[n];
                info!(
                    colors = palette.colors,
                    dithered = palette.dithered,
                    size = palette.png.len(),
                    score,
                    "measured"
                );
                scores[n] = Some(score);
            }
            for order in &mut kinds {
                let Some(&n) = order.peek() else { continue };
                if scores[n].unwrap_or(0.0) >= threshold {
                    if palettes[n].png.len() as u64 <= limit {
                        passing.push(n);
                    }
                    order.next();
                } else {
                    // Fewer colors would fall short too: this kind is done.
                    order.by_ref().for_each(drop);
                }
            }
        }
        Ok(passing.into_iter().min_by_key(|&n| palettes[n].png.len()))
    }
}

/// Checks jpegli's result really is a whole picture of the expected size.
fn check_jpeg(bytes: &[u8], expected: (u32, u32)) -> Result<(), ImageError> {
    let dimensions = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .ok()
        .and_then(|reader| reader.into_dimensions().ok())
        .ok_or_else(|| ImageError::Failed("jpegli's result cannot be read".into()))?;
    if dimensions != expected {
        return Err(ImageError::Failed(format!(
            "jpegli's result is {dimensions:?}, expected {expected:?}"
        )));
    }
    Ok(())
}

/// Intermediate files of one image, in the system's temporary folder, deleted when done. Each is
/// noted as a leftover too, so a crash never leaves them behind for good (ADR 0017).
#[derive(Default)]
struct Scratch {
    files: Vec<PathBuf>,
}

impl Scratch {
    fn file(&mut self, name: &str) -> io::Result<PathBuf> {
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let dir = std::env::temp_dir().join("Gracil");
        fs::create_dir_all(&dir)?;
        let id = COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = dir.join(format!("image-{:x}-{id:x}-{name}", std::process::id()));
        leftovers::record_temp(&path);
        self.files.push(path.clone());
        Ok(path)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        for file in &self.files {
            if let Err(error) = fs::remove_file(file)
                && error.kind() != io::ErrorKind::NotFound
            {
                warn!(%error, ?file, "could not remove an intermediate image file");
            }
        }
    }
}

#[cfg(test)]
mod tests;
