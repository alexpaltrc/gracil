//! PNG files: reading their chunks, and writing the lossless and reduced-palette versions Gracil
//! chooses between (ADR 0034, items 5 and 6).
//!
//! Every chunk that describes the picture rather than its pixels (color profile, EXIF, text,
//! physical size, dates) is carried over. Lossless optimization keeps every chunk there is.

use exoquant::{Color, convert_to_indexed, ditherer, optimizer};
use oxipng::{BitDepth, ColorType, Options, RawImage, StripChunks};

const SIGNATURE: &[u8] = b"\x89PNG\r\n\x1a\n";

/// Chunks that stay true whatever happens to the pixels.
const DESCRIPTIVE: [&[u8; 4]; 13] = [
    b"iCCP", b"sRGB", b"gAMA", b"cHRM", b"cICP", b"mDCV", b"cLLI", b"pHYs", b"eXIf", b"tEXt",
    b"zTXt", b"iTXt", b"tIME",
];

/// The palette sizes tried, each with and without dithering (ADR 0034, item 5).
pub const PALETTE_SIZES: [usize; 4] = [256, 128, 64, 32];

pub fn is_png(data: &[u8]) -> bool {
    data.starts_with(SIGNATURE)
}

/// The chunks of a PNG file, in order: their type and contents.
pub fn chunks(data: &[u8]) -> Vec<([u8; 4], &[u8])> {
    let mut found = Vec::new();
    let mut at = SIGNATURE.len();
    while let Some(header) = data.get(at..at + 8) {
        let length = u32::from_be_bytes([header[0], header[1], header[2], header[3]]) as usize;
        let kind = [header[4], header[5], header[6], header[7]];
        let Some(contents) = data.get(at + 8..at + 8 + length) else {
            break;
        };
        found.push((kind, contents));
        at += 12 + length;
        if &kind == b"IEND" {
            break;
        }
    }
    found
}

/// An animated PNG: its animation control comes before the picture data.
pub fn is_animated(data: &[u8]) -> bool {
    chunks(data)
        .iter()
        .take_while(|(kind, _)| kind != b"IDAT")
        .any(|(kind, _)| kind == b"acTL")
}

/// Stored as true HDR: its coding-independent code points say PQ or HLG.
pub fn is_hdr(data: &[u8]) -> bool {
    chunks(data)
        .iter()
        .any(|(kind, contents)| kind == b"cICP" && matches!(contents.get(1), Some(16 | 18)))
}

/// The descriptive chunks of `data`, to carry into a new PNG.
pub fn descriptive_chunks(data: &[u8]) -> Vec<([u8; 4], Vec<u8>)> {
    chunks(data)
        .into_iter()
        .filter(|(kind, _)| DESCRIPTIVE.contains(&kind))
        .map(|(kind, contents)| (kind, contents.to_vec()))
        .collect()
}

/// Options for the final files: oxipng's preset 4, keeping every chunk.
pub fn options() -> Options {
    let mut options = Options::from_preset(4);
    options.strip = StripChunks::None;
    options
}

/// Options for trying palettes, quicker: preset 2.
fn quick_options() -> Options {
    let mut options = Options::from_preset(2);
    options.strip = StripChunks::None;
    options
}

/// Lossless: the same pixels, stored as compactly as oxipng can.
pub fn lossless(original: &[u8]) -> Result<Vec<u8>, String> {
    oxipng::optimize_from_memory(original, &options()).map_err(|e| e.to_string())
}

/// A PNG of `pixels` (RGBA, 8 bits), losslessly, carrying `chunks`.
pub fn from_pixels(
    pixels: &[[u8; 4]],
    width: u32,
    height: u32,
    extras: &Extras,
) -> Result<Vec<u8>, String> {
    let data = pixels.iter().flatten().copied().collect();
    let mut raw = RawImage::new(width, height, ColorType::RGBA, BitDepth::Eight, data)
        .map_err(|e| e.to_string())?;
    extras.add_to(&mut raw);
    raw.create_optimized_png(&options())
        .map_err(|e| e.to_string())
}

/// A version of the picture with at most `colors` colors.
pub struct Palette {
    /// The file, ready to write.
    pub png: Vec<u8>,
    /// What it looks like, pixel by pixel, to measure against the original.
    pub pixels: Vec<[u8; 4]>,
}

/// The picture reduced to `colors` colors, with or without dithering.
pub fn palette(
    pixels: &[[u8; 4]],
    width: u32,
    height: u32,
    colors: usize,
    dithered: bool,
    extras: &Extras,
) -> Result<Palette, String> {
    let input: Vec<Color> = pixels
        .iter()
        .map(|&[r, g, b, a]| Color::new(r, g, b, a))
        .collect();
    let (palette, indices) = if dithered {
        convert_to_indexed(
            &input,
            width as usize,
            colors,
            &optimizer::KMeans,
            &ditherer::FloydSteinberg::new(),
        )
    } else {
        convert_to_indexed(
            &input,
            width as usize,
            colors,
            &optimizer::KMeans,
            &ditherer::None,
        )
    };
    let shown = indices
        .iter()
        .map(|&i| {
            let c = palette[usize::from(i)];
            [c.r, c.g, c.b, c.a]
        })
        .collect();
    let palette = palette
        .iter()
        .map(|c| rgb::RGBA8::new(c.r, c.g, c.b, c.a))
        .collect();
    let mut raw = RawImage::new(
        width,
        height,
        ColorType::Indexed { palette },
        BitDepth::Eight,
        indices,
    )
    .map_err(|e| e.to_string())?;
    extras.add_to(&mut raw);
    let png = raw
        .create_optimized_png(&quick_options())
        .map_err(|e| e.to_string())?;
    Ok(Palette { png, pixels: shown })
}

/// What a new PNG carries besides its pixels.
#[derive(Debug, Default, Clone)]
pub struct Extras {
    /// Descriptive chunks, as they were in the original PNG.
    pub chunks: Vec<([u8; 4], Vec<u8>)>,
    /// A color profile from a picture of another format.
    pub icc: Option<Vec<u8>>,
}

impl Extras {
    fn add_to(&self, raw: &mut RawImage) {
        for (kind, contents) in &self.chunks {
            raw.add_png_chunk(*kind, contents.clone());
        }
        if let Some(icc) = &self.icc {
            raw.add_icc_profile(icc);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chunk(kind: &[u8; 4], contents: &[u8]) -> Vec<u8> {
        let mut out = (contents.len() as u32).to_be_bytes().to_vec();
        out.extend_from_slice(kind);
        out.extend_from_slice(contents);
        out.extend_from_slice(&[0; 4]);
        out
    }

    fn file(kinds: &[(&[u8; 4], &[u8])]) -> Vec<u8> {
        let mut data = SIGNATURE.to_vec();
        for (kind, contents) in kinds {
            data.extend(chunk(kind, contents));
        }
        data
    }

    #[test]
    fn recognizes_animation_and_hdr() {
        let still = file(&[(b"IHDR", &[0; 13]), (b"IDAT", &[]), (b"IEND", &[])]);
        let animated = file(&[(b"IHDR", &[0; 13]), (b"acTL", &[0; 8]), (b"IDAT", &[])]);
        let pq = file(&[
            (b"IHDR", &[0; 13]),
            (b"cICP", &[9, 16, 0, 1]),
            (b"IDAT", &[]),
        ]);
        assert!(is_png(&still));
        assert!(!is_animated(&still) && !is_hdr(&still));
        assert!(is_animated(&animated));
        assert!(is_hdr(&pq));
    }

    #[test]
    fn keeps_descriptive_chunks_only() {
        let data = file(&[
            (b"IHDR", &[0; 13]),
            (b"iCCP", b"profile"),
            (b"PLTE", &[0; 3]),
            (b"tRNS", &[0]),
            (b"eXIf", b"MM\0*"),
            (b"IDAT", &[]),
            (b"tEXt", b"Title\0Hi"),
            (b"IEND", &[]),
        ]);
        let kept: Vec<[u8; 4]> = descriptive_chunks(&data).into_iter().map(|c| c.0).collect();
        assert_eq!(kept, [*b"iCCP", *b"eXIf", *b"tEXt"]);
    }

    #[test]
    fn writes_palettes_that_keep_the_information() {
        let (width, height) = (16, 16);
        let pixels: Vec<[u8; 4]> = (0..width * height)
            .map(|i| [(i % 16 * 16) as u8, (i / 16 * 16) as u8, 128, 255])
            .collect();
        let text = Extras {
            chunks: vec![(*b"tEXt", b"Title\0Hi".to_vec())],
            icc: None,
        };
        let reduced = palette(&pixels, width, height, 32, false, &text).unwrap();
        assert_eq!(reduced.pixels.len(), pixels.len());
        assert!(chunks(&reduced.png).iter().any(|(kind, _)| kind == b"tEXt"));

        let full = from_pixels(&pixels, width, height, &text).unwrap();
        let decoded = image::load_from_memory(&full).unwrap().to_rgba8();
        assert_eq!(decoded.as_raw(), &pixels.concat());
    }
}
