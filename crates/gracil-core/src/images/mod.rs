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
mod ssimulacra2;

use std::ffi::OsString;
use std::fs;
use std::io::{self, Cursor, Read};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::thread;
use std::time::Duration;

use image::{DynamicImage, ImageDecoder, ImageReader};
use tracing::{info, warn};

use crate::compress::{Engine, Update, reserve_temp};
use crate::encode::{EncodeOutcome, JobControl, run_ffmpeg, run_program};
use crate::leftovers;
use crate::media::{self, JPEG_EXTENSIONS};
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
    match target {
        Target::Jpeg => media::keep_spelling(source, JPEG_EXTENSIONS, "jpg"),
        Target::Png => media::keep_spelling(source, &["png"], "png"),
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
    /// The memory its pixels need could not be had right now (ADR 0063).
    #[error("not enough memory for the image")]
    TooLarge,
}

/// Makes sure about `bytes` of memory can be had right now, without keeping it, so a picture too
/// large for what is free fails quietly instead of ending Gracil (ADR 0063).
fn room_for(bytes: u64) -> Result<(), ImageError> {
    let bytes = usize::try_from(bytes).map_err(|_| ImageError::TooLarge)?;
    Vec::<u8>::new()
        .try_reserve_exact(bytes)
        .map_err(|_| ImageError::TooLarge)
}

/// Room for `more` items in `vec`, or [`ImageError::TooLarge`].
fn reserve<T>(vec: &mut Vec<T>, more: usize) -> Result<(), ImageError> {
    vec.try_reserve_exact(more)
        .map_err(|_| ImageError::TooLarge)
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

/// An image read and ready to compress (ADR 0040): its bytes, and its size in pixels when its
/// header tells.
#[derive(Debug)]
pub struct PreparedImage {
    data: Vec<u8>,
    pixels: Option<u64>,
}

impl PreparedImage {
    /// How many pixels the picture has, when it could be told without decoding it.
    pub fn pixels(&self) -> Option<u64> {
        self.pixels
    }

    /// How many bytes of the file are held in memory, waiting to be compressed.
    pub fn bytes(&self) -> u64 {
        self.data.len() as u64
    }
}

/// Compresses the image at `source` into a temporary file next to it. The source is never touched.
pub fn compress(
    engine: &Engine,
    source: &Path,
    control: &JobControl,
    on_update: &mut dyn FnMut(Update),
) -> Result<CompressedImage, ImageError> {
    let prepared = prepare(source)?;
    compress_prepared(engine, source, prepared, control, on_update)
}

/// The first stage: reads the file, which on a network or cloud drive is mostly waiting.
pub fn prepare(source: &Path) -> Result<PreparedImage, ImageError> {
    // A camera RAW file can start like a TIFF; its name decides before its bytes are read.
    if crate::media::is_refused_image(source) {
        return Err(ImageError::NotAccepted);
    }
    let data = read_whole(source)?;
    let pixels = ImageReader::new(Cursor::new(&data))
        .with_guessed_format()
        .ok()
        .and_then(|reader| reader.into_dimensions().ok())
        .map(|(width, height)| u64::from(width) * u64::from(height));
    Ok(PreparedImage { data, pixels })
}

/// Reads all of `source`, with the memory for it asked for first, so a file too large for what is
/// free fails quietly with [`ImageError::TooLarge`] instead of ending Gracil (ADR 0064).
fn read_whole(source: &Path) -> Result<Vec<u8>, ImageError> {
    let unreadable = |error: io::Error| ImageError::Unreadable(error.to_string());
    let mut file = fs::File::open(source).map_err(unreadable)?;
    let len = file.metadata().map_err(unreadable)?.len();
    let mut data = Vec::new();
    reserve(
        &mut data,
        usize::try_from(len).map_err(|_| ImageError::TooLarge)?,
    )?;
    file.read_to_end(&mut data).map_err(unreadable)?;
    Ok(data)
}

/// The second stage: compresses an image [`prepare`] read.
pub fn compress_prepared(
    engine: &Engine,
    source: &Path,
    prepared: PreparedImage,
    control: &JobControl,
    on_update: &mut dyn FnMut(Update),
) -> Result<CompressedImage, ImageError> {
    let report = |on_update: &mut dyn FnMut(Update), fraction: f64| {
        on_update(Update {
            fraction: Some(fraction),
        });
    };
    report(on_update, 0.0);
    let data = prepared.data;
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
            // The tools read a copy on this computer: the original may be on a network or cloud
            // drive, where every read is slow, and each attempt reads it again.
            let input = job.scratch.copy_of(&data, source)?;
            let bytes = job.make_jpeg(&input, &carried, expected, reference, &mut |f| {
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
                let input = job.scratch.copy_of(&data, source)?;
                job.decode_with_ffmpeg(&input, &data)?
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
                fs::write(&input, ppm(&decoded.reference)?)?;
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
    // The decoded picture and its turned copy, then its RGBA copy and the pixels kept.
    let (width, height) = decoder.dimensions();
    let per_pixel = u64::from(decoder.color_type().bytes_per_pixel()) * 2 + 4 + 4;
    room_for(u64::from(width) * u64::from(height) * per_pixel)?;
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
    let chunks = rgba.as_raw().as_chunks::<4>().0;
    let mut pixels = Vec::new();
    reserve(&mut pixels, chunks.len())?;
    pixels.extend_from_slice(chunks);
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
fn ppm(reference: &Reference) -> Result<Vec<u8>, ImageError> {
    let mut out = format!("P6\n{} {}\n255\n", reference.width, reference.height).into_bytes();
    reserve(&mut out, reference.pixels.len() * 3)?;
    for pixel in &reference.pixels {
        out.extend_from_slice(&pixel[..3]);
    }
    Ok(out)
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
        if info.alpha.is_some_and(|alpha| alpha.grid) {
            // FFmpeg reads a grid of tiles one tile at a time when it is not the main picture, so
            // the transparency can't be put together: such a picture stays as it is.
            return Err(ImageError::AsSmallAsItGets);
        }
        let png = self.ffmpeg_png(source, None, "rgba", "decoded.png")?;
        let mut decoded = decode_with_image(&png, false)?;
        if let Some(alpha) = info.alpha {
            // The transparency is a picture of its own, in gray, the size of the main one.
            let name = format!("0:i:{}", alpha.item);
            let png = self.ffmpeg_png(source, Some(&name), "gray", "alpha.png")?;
            let mask = image::load_from_memory(&png)
                .map_err(|e| ImageError::Unreadable(e.to_string()))?
                .into_luma8();
            let (width, height) = (decoded.reference.width, decoded.reference.height);
            if mask.dimensions() != (width, height) {
                return Err(ImageError::Unreadable(
                    "transparency of another size than the picture".into(),
                ));
            }
            let mut pixels = std::mem::take(&mut decoded.reference.pixels);
            for (pixel, value) in pixels.iter_mut().zip(mask.as_raw()) {
                pixel[3] = *value;
            }
            // Measured again from the pixels: a picture whose transparency covers nothing is
            // opaque, and may become a JPEG.
            decoded.reference = Reference::new(pixels, width, height);
        }
        decoded.icc = info.icc;
        decoded.exif = info.exif.map(|mut tiff| {
            exif::set_upright(&mut tiff);
            tiff
        });
        Ok(decoded)
    }

    /// One picture of `source` decoded by FFmpeg into a PNG of `pix_fmt`: the main picture, or the
    /// stream `map` names.
    fn ffmpeg_png(
        &mut self,
        source: &Path,
        map: Option<&str>,
        pix_fmt: &str,
        name: &str,
    ) -> Result<Vec<u8>, ImageError> {
        let output = self.scratch.file(name)?;
        let mut args: Vec<OsString> = vec!["-v".into(), "error".into(), "-i".into()];
        args.push(source.into());
        if let Some(map) = map {
            args.push("-map".into());
            args.push(map.into());
        }
        for arg in [
            "-frames:v",
            "1",
            "-pix_fmt",
            pix_fmt,
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
            EncodeOutcome::Stalled => {
                return Err(ImageError::Failed("FFmpeg stopped answering".into()));
            }
        }
        Ok(fs::read(&output)?)
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
            EncodeOutcome::Stalled => {
                return Err(ImageError::Failed("jpegli stopped answering".into()));
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
    /// to 3.0 that still scores 80 and is clearly smaller, measuring at most three (item 4). A
    /// result is measured only when it, or one it leads to, is small enough to keep (ADR 0068).
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
        let mut again = Again {
            job: self,
            input,
            carried,
            encoded: Default::default(),
            reference: None,
            make_reference: Some(reference),
            report,
            measured: 0,
        };
        let start = AGAIN_DISTANCES.iter().position(|&d| d == 2.0).unwrap_or(0);
        let best = choose_again(&mut again, start, AGAIN_DISTANCES.len())?;
        best.and_then(|n| again.encoded[n].take())
            .ok_or(ImageError::AsSmallAsItGets)
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
        let picture = Picture { reference, extras };
        let mut palettes = Palettes::new(&picture, lossless.len());
        let chosen = palettes.choose(&|| self.checkpoint(), self.limit, report)?;
        Ok(match chosen.and_then(|t| palettes.take(t)) {
            Some(palette) => palette.png,
            None => lossless,
        })
    }
}

/// Where a PNG's palettes come from, and how they are measured.
trait PaletteSource: Sync {
    /// How many pixels the picture has, for the memory making and measuring palettes takes.
    fn pixels(&self) -> usize;
    fn make(&self, colors: usize, dithered: bool) -> Result<png::Palette, String>;
    /// SSIMULACRA2 of `palette` against the original.
    fn score(&self, palette: &png::Palette) -> f64;
}

/// The picture a PNG's palettes are made from.
struct Picture<'r> {
    reference: &'r Reference,
    extras: &'r Extras,
}

impl PaletteSource for Picture<'_> {
    fn pixels(&self) -> usize {
        self.reference.pixels.len()
    }

    fn make(&self, colors: usize, dithered: bool) -> Result<png::Palette, String> {
        let reference = self.reference;
        png::palette(
            &reference.pixels,
            reference.width,
            reference.height,
            colors,
            dithered,
            self.extras,
        )
    }

    fn score(&self, palette: &png::Palette) -> f64 {
        self.reference.score(&palette.pixels).unwrap_or(0.0)
    }
}

/// The palettes a PNG may be reduced to, each with and without dithering (ADR 0034, item 5),
/// made only once they are needed and kept for a second pass (ADR 0068).
struct Palettes<'s, S> {
    source: &'s S,
    /// The lossless version's size: only a palette smaller than it can be chosen.
    lossless: usize,
    /// Every palette size, plain and dithered.
    tries: Vec<(usize, bool)>,
    /// Each of `tries` once made: `None` when it failed or wasn't smaller than the lossless
    /// version.
    made: Vec<Option<Option<png::Palette>>>,
    scores: Vec<Option<f64>>,
    /// How many palettes may be made side by side: each needs a few copies of the picture.
    at_once: usize,
    /// The progress last reported.
    reported: f64,
}

impl<'s, S: PaletteSource> Palettes<'s, S> {
    fn new(source: &'s S, lossless: usize) -> Self {
        let tries: Vec<(usize, bool)> = png::PALETTE_SIZES
            .iter()
            .flat_map(|&colors| [(colors, false), (colors, true)])
            .collect();
        let pixels = source.pixels().max(1);
        let at_once = share::threads_for_one()
            .min(PALETTE_MEMORY / (pixels * 32))
            .clamp(1, tries.len());
        Self {
            source,
            lossless,
            made: tries.iter().map(|_| None).collect(),
            scores: vec![None; tries.len()],
            tries,
            at_once,
            reported: 0.2,
        }
    }

    /// The palette `t`, when it was made and can be chosen.
    fn palette(&self, t: usize) -> Option<&png::Palette> {
        self.made[t].as_ref().and_then(Option::as_ref)
    }

    fn size(&self, t: usize) -> usize {
        self.palette(t).map_or(usize::MAX, |p| p.png.len())
    }

    /// Whether palette `t` may weigh at most `limit` bytes: not when it can't be chosen, yes
    /// when it isn't made yet.
    fn may_fit(&self, t: usize, limit: u64) -> bool {
        match &self.made[t] {
            None => true,
            Some(None) => false,
            Some(Some(palette)) => palette.png.len() as u64 <= limit,
        }
    }

    fn take(&mut self, t: usize) -> Option<png::Palette> {
        self.made[t].take().flatten()
    }

    /// The palette to keep, or `None` for the lossless version: the smallest of the lossless
    /// version and the palettes scoring at least 90; when that weighs more than `limit` bytes,
    /// the smallest palette scoring at least 80 and weighing at most `limit`, if any (items 5
    /// and 6).
    fn choose(
        &mut self,
        checkpoint: &dyn Fn() -> Result<(), ImageError>,
        limit: u64,
        report: &mut dyn FnMut(f64),
    ) -> Result<Option<usize>, ImageError> {
        let first = self.smallest_passing(checkpoint, FIRST_SCORE, u64::MAX, report)?;
        report(0.8);
        self.reported = 0.8;
        let first_size = first.map_or(self.lossless, |t| self.size(t));
        if first_size as u64 <= limit {
            return Ok(first);
        }
        info!(size = first_size, "already compressed; compressing again");
        // Palettes not made yet are made only as needed: making one takes longer than measuring.
        if !(0..self.tries.len()).any(|t| self.may_fit(t, limit)) {
            info!("no palette is small enough");
            return Err(ImageError::AsSmallAsItGets);
        }
        let again = self.smallest_passing(checkpoint, AGAIN_SCORE, limit, report)?;
        Ok(Some(again.ok_or(ImageError::AsSmallAsItGets)?))
    }

    /// Makes those of `wanted` not made yet, side by side as far as the processor and memory
    /// allow.
    fn make(
        &mut self,
        checkpoint: &dyn Fn() -> Result<(), ImageError>,
        wanted: &[usize],
        report: &mut dyn FnMut(f64),
    ) -> Result<(), ImageError> {
        let wanted: Vec<usize> = wanted
            .iter()
            .copied()
            .filter(|&t| self.made[t].is_none())
            .collect();
        let source = self.source;
        for group in wanted.chunks(self.at_once) {
            checkpoint()?;
            let made: Vec<_> = thread::scope(|scope| {
                let handles: Vec<_> = group
                    .iter()
                    .map(|&t| {
                        let (colors, dithered) = self.tries[t];
                        scope.spawn(move || {
                            share::worker_started();
                            (t, source.make(colors, dithered))
                        })
                    })
                    .collect();
                handles.into_iter().filter_map(|h| h.join().ok()).collect()
            });
            for (t, made) in made {
                let (colors, dithered) = self.tries[t];
                self.made[t] = Some(match made {
                    // Only a palette smaller than the lossless version can be chosen.
                    Ok(palette) => {
                        let size = palette.png.len();
                        info!(colors, dithered, size, "palette made");
                        (size < self.lossless).then_some(palette)
                    }
                    Err(error) => {
                        warn!(%error, colors, dithered, "palette failed");
                        None
                    }
                });
            }
            let done = self.made.iter().filter(|m| m.is_some()).count();
            let fraction = 0.2 + 0.4 * (done as f64 / self.tries.len() as f64);
            // Never back: palettes made for a second pass come after the first's measurements.
            if fraction > self.reported {
                self.reported = fraction;
                report(fraction);
            }
        }
        Ok(())
    }

    /// The smallest palette scoring at least `threshold` and weighing at most `limit` bytes.
    ///
    /// Fewer colors hardly ever look better, so each kind of palette, plain and dithered, is
    /// measured from the most colors down, and stops at the first that falls short: the ones with
    /// fewer colors would fall short too. That usually takes two measurements instead of eight,
    /// which matters because each one takes about a second for a large picture. A palette is made
    /// only when it is next to be measured, the next of each kind side by side; a kind stops too
    /// once none of its palettes left may be small enough (ADR 0068). The two kinds are measured
    /// side by side when memory allows. Scores already known are kept.
    fn smallest_passing(
        &mut self,
        checkpoint: &dyn Fn() -> Result<(), ImageError>,
        threshold: f64,
        limit: u64,
        report: &mut dyn FnMut(f64),
    ) -> Result<Option<usize>, ImageError> {
        let kind = |dithered: bool| {
            let mut found: Vec<usize> = (0..self.tries.len())
                .filter(|&t| self.tries[t].1 == dithered)
                .collect();
            found.sort_by_key(|&t| std::cmp::Reverse(self.tries[t].0));
            found
        };
        let mut kinds = [kind(false), kind(true)].map(|order| order.into_iter().peekable());
        let pixels = self.source.pixels().max(1);
        let side_by_side = PALETTE_MEMORY / (pixels * SCORE_BYTES_PER_PIXEL) >= 2;
        let mut passing = Vec::new();
        loop {
            // A kind none of whose palettes left may be small enough adds nothing.
            for order in &mut kinds {
                if !order.clone().any(|t| self.may_fit(t, limit)) {
                    order.by_ref().for_each(drop);
                }
            }
            // The next palette of each kind that can be chosen, made if it wasn't yet.
            let next: Vec<usize> = kinds.iter_mut().filter_map(|k| k.peek().copied()).collect();
            // Two at a time: when only one is needed, the one after it of the same kind is made
            // beside it, never more than the eight there are.
            let mut wanted: Vec<usize> = next
                .iter()
                .copied()
                .filter(|&t| self.made[t].is_none())
                .collect();
            if let [only] = wanted[..] {
                let after = kinds
                    .iter()
                    .filter_map(|order| {
                        let mut order = order.clone();
                        order.next_if_eq(&only)?;
                        order.find(|&t| self.made[t].is_none())
                    })
                    .next();
                wanted.extend(after);
            }
            self.make(checkpoint, &wanted, report)?;
            let mut skipped = false;
            for order in &mut kinds {
                if order.next_if(|&t| self.palette(t).is_none()).is_some() {
                    skipped = true;
                }
            }
            if skipped {
                continue;
            }
            if next.is_empty() {
                break;
            }
            checkpoint()?;
            let unknown: Vec<usize> = next
                .iter()
                .copied()
                .filter(|&t| self.scores[t].is_none())
                .collect();
            let source = self.source;
            let to_measure: Vec<&png::Palette> =
                unknown.iter().filter_map(|&t| self.palette(t)).collect();
            let measured: Vec<f64> = if side_by_side && to_measure.len() > 1 {
                thread::scope(|scope| {
                    let handles: Vec<_> = to_measure
                        .iter()
                        .map(|&palette| {
                            scope.spawn(move || {
                                share::worker_started();
                                source.score(palette)
                            })
                        })
                        .collect();
                    handles
                        .into_iter()
                        .map(|h| h.join().unwrap_or(0.0))
                        .collect()
                })
            } else {
                to_measure
                    .iter()
                    .map(|&palette| source.score(palette))
                    .collect()
            };
            for (&t, score) in unknown.iter().zip(measured) {
                let (colors, dithered) = self.tries[t];
                info!(colors, dithered, size = self.size(t), score, "measured");
                self.scores[t] = Some(score);
            }
            for order in &mut kinds {
                let Some(&t) = order.peek() else { continue };
                if self.scores[t].unwrap_or(0.0) >= threshold {
                    if self.size(t) as u64 <= limit {
                        passing.push(t);
                    }
                    order.next();
                } else {
                    // Fewer colors would fall short too: this kind is done.
                    order.by_ref().for_each(drop);
                }
            }
        }
        Ok(passing.into_iter().min_by_key(|&t| self.size(t)))
    }
}

/// What choosing a distance for compressing again needs: results encoded, and measured.
trait Tries {
    /// Encodes each of the distances `ns` (indices into the list) not encoded yet.
    fn encode(&mut self, ns: &[usize]) -> Result<(), ImageError>;
    /// Whether distance `n`, already encoded, is small enough to keep.
    fn fits(&self, n: usize) -> bool;
    /// Whether distance `n`, already encoded, scores enough. Each is measured at most once.
    fn passes(&mut self, n: usize) -> Result<bool, ImageError>;
}

/// The loosest of `count` distances, tightest first, that scores enough and is small enough,
/// starting at `start` and moving one step looser or tighter (ADR 0034, item 4). It chooses what
/// measuring every distance it reaches would, but measures a result only when it, or a looser one
/// it would lead to, is small enough to keep, and measures nothing when nothing is (ADR 0068).
fn choose_again(
    tries: &mut impl Tries,
    start: usize,
    count: usize,
) -> Result<Option<usize>, ImageError> {
    let looser: Vec<usize> = (start + 1..count).collect();
    tries.encode(&[start])?;
    if !tries.fits(start) {
        // Measured only to know whether a looser distance may be kept: when none is small
        // enough either, none can be kept.
        tries.encode(&looser)?;
        if !looser.iter().any(|&n| tries.fits(n)) {
            info!("no distance is small enough");
            return Ok(None);
        }
    }
    let mut best = None;
    if tries.passes(start)? {
        if tries.fits(start) {
            best = Some(start);
        }
        for (at, &n) in looser.iter().enumerate() {
            tries.encode(&[n])?;
            if !tries.fits(n) {
                // Its score matters only to go on to a looser one small enough to keep.
                let later = &looser[at + 1..];
                tries.encode(later)?;
                if !later.iter().any(|&l| tries.fits(l)) {
                    break;
                }
            }
            if !tries.passes(n)? {
                break;
            }
            if tries.fits(n) {
                best = Some(n);
            }
        }
    } else if tries.fits(start) {
        // Tighter distances only grow: once one is too big, so are the tighter ones.
        for n in (0..start).rev() {
            tries.encode(&[n])?;
            if !tries.fits(n) {
                break;
            }
            if tries.passes(n)? {
                best = Some(n);
                break;
            }
        }
    }
    Ok(best)
}

/// Compressing a JPEG again: the distances of [`AGAIN_DISTANCES`] encoded by jpegli and measured
/// against the original, decoded only once something is to be measured.
struct Again<'j, 'a, F> {
    job: &'j mut Job<'a>,
    input: &'j Path,
    carried: &'j Carried,
    encoded: [Option<Vec<u8>>; AGAIN_DISTANCES.len()],
    reference: Option<Reference>,
    make_reference: Option<F>,
    report: &'j mut dyn FnMut(f64),
    measured: u32,
}

impl<F: FnOnce() -> Result<Reference, ImageError>> Tries for Again<'_, '_, F> {
    fn encode(&mut self, ns: &[usize]) -> Result<(), ImageError> {
        for &n in ns {
            if self.encoded[n].is_none() {
                let distance = AGAIN_DISTANCES[n];
                let bytes = self.job.jpegli(self.input, distance, self.carried)?;
                info!(distance, size = bytes.len(), "encoded");
                self.encoded[n] = Some(bytes);
            }
        }
        Ok(())
    }

    fn fits(&self, n: usize) -> bool {
        self.encoded[n]
            .as_ref()
            .is_some_and(|bytes| bytes.len() as u64 <= self.job.limit)
    }

    fn passes(&mut self, n: usize) -> Result<bool, ImageError> {
        self.job.checkpoint()?;
        if let Some(make) = self.make_reference.take() {
            self.reference = Some(make()?);
            (self.report)(0.4);
        }
        let (Some(reference), Some(bytes)) = (&self.reference, &self.encoded[n]) else {
            return Err(ImageError::Failed("measured before it was encoded".into()));
        };
        let pixels = decode_with_image(bytes, false)?.reference.pixels;
        let score = reference.score(&pixels);
        self.measured += 1;
        (self.report)(0.4 + 0.18 * f64::from(self.measured));
        info!(
            distance = AGAIN_DISTANCES[n],
            size = bytes.len(),
            ?score,
            "measured"
        );
        Ok(score.is_some_and(|s| s >= AGAIN_SCORE))
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
        let id = COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "gracil-image-{}-{id:x}-{name}",
            leftovers::run_tag()
        ));
        leftovers::record_scratch(&path);
        self.files.push(path.clone());
        Ok(path)
    }

    /// A copy of the image `data` read from `source`, with the same extension, on this computer.
    fn copy_of(&mut self, data: &[u8], source: &Path) -> io::Result<PathBuf> {
        let extension = source.extension().map_or_else(String::new, |extension| {
            extension.to_string_lossy().into_owned()
        });
        let path = self.file(&format!("source.{extension}"))?;
        fs::write(&path, data)?;
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
