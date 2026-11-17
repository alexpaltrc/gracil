use super::*;

#[test]
fn extensions_follow_the_format() {
    let ext = |name: &str, target| output_extension(Path::new(name), target);
    assert_eq!(ext("Foto.JPEG", Target::Jpeg), "JPEG");
    assert_eq!(ext("Foto.jfif", Target::Jpeg), "jfif");
    assert_eq!(ext("Foto.heic", Target::Jpeg), "jpg");
    assert_eq!(ext("Foto.webp", Target::Png), "png");
    assert_eq!(ext("Foto.PNG", Target::Png), "PNG");
}

#[test]
fn formats_are_known_by_their_first_bytes() {
    assert_eq!(Format::sniff(&[0xff, 0xd8, 0xff, 0xe0]), Format::Jpeg);
    assert_eq!(Format::sniff(b"\x89PNG\r\n\x1a\n...."), Format::Png);
    assert_eq!(Format::sniff(b"\0\0\0\x18ftypheic"), Format::Heif);
    assert_eq!(Format::sniff(b"RIFF\0\0\0\0WEBPVP8 "), Format::WebP);
    assert_eq!(Format::sniff(b"II*\0...."), Format::Other);
    assert_eq!(Format::sniff(b"BM......"), Format::Other);
    assert_eq!(Format::sniff(b"GIF89a.."), Format::NotAccepted);
    assert_eq!(
        Format::sniff(b"\0\0\0\x0cJXL \r\n\x87\n"),
        Format::NotAccepted
    );
    assert_eq!(Format::sniff(b"hello"), Format::Unknown);
}

/// A made-up set of results: which distances are small enough, and which score enough.
struct Fake {
    fits: [bool; 4],
    passes: [bool; 4],
    encoded: Vec<usize>,
    measured: Vec<usize>,
}

impl Tries for Fake {
    fn encode(&mut self, ns: &[usize]) -> Result<(), ImageError> {
        for &n in ns {
            if !self.encoded.contains(&n) {
                self.encoded.push(n);
            }
        }
        Ok(())
    }

    fn fits(&self, n: usize) -> bool {
        assert!(
            self.encoded.contains(&n),
            "{n} looked at before it was encoded"
        );
        self.fits[n]
    }

    fn passes(&mut self, n: usize) -> Result<bool, ImageError> {
        assert!(
            self.encoded.contains(&n),
            "{n} measured before it was encoded"
        );
        assert!(!self.measured.contains(&n), "{n} measured twice");
        self.measured.push(n);
        Ok(self.passes[n])
    }
}

/// How the distance was chosen before ADR 0068: every distance reached was encoded and measured.
fn chosen_before(fits: [bool; 4], passes: [bool; 4], start: usize) -> (Option<usize>, Vec<usize>) {
    let mut measured = vec![start];
    let mut best = None;
    if passes[start] {
        if fits[start] {
            best = Some(start);
        }
        for n in start + 1..4 {
            measured.push(n);
            if !passes[n] {
                break;
            }
            if fits[n] {
                best = Some(n);
            }
        }
    } else if fits[start] {
        for n in (0..start).rev() {
            measured.push(n);
            if passes[n] && fits[n] {
                best = Some(n);
                break;
            }
        }
    }
    (best, measured)
}

#[test]
fn compressing_again_chooses_as_before_measuring_less() {
    let start = AGAIN_DISTANCES.iter().position(|&d| d == 2.0).unwrap();
    let bits = |mask: u32| [0, 1, 2, 3].map(|n| mask & (1 << n) != 0);
    for fit_mask in 0..16 {
        for pass_mask in 0..16 {
            let (fits, passes) = (bits(fit_mask), bits(pass_mask));
            let mut fake = Fake {
                fits,
                passes,
                encoded: Vec::new(),
                measured: Vec::new(),
            };
            let chosen = choose_again(&mut fake, start, 4).unwrap();
            let (before, measured_before) = chosen_before(fits, passes, start);
            let case = format!("fits {fits:?}, passes {passes:?}");
            assert_eq!(chosen, before, "{case}");
            assert!(
                fake.measured.iter().all(|n| measured_before.contains(n)),
                "{case}: measured {:?}, before {measured_before:?}",
                fake.measured
            );
            // Nothing is measured unless something small enough could be kept.
            if !fits.iter().any(|&f| f) {
                assert!(fake.measured.is_empty(), "{case}");
            }
        }
    }
}

/// Made-up palettes, in the order plain and dithered for each size: each one's file size (`None`
/// when making it fails) and score. Each palette's one pixel says which it is.
struct FakePalettes {
    sizes: [Option<usize>; 8],
    scores: [f64; 8],
    measured: std::sync::Mutex<Vec<usize>>,
}

fn fake_index(colors: usize, dithered: bool) -> usize {
    let size = png::PALETTE_SIZES
        .iter()
        .position(|&c| c == colors)
        .unwrap();
    size * 2 + usize::from(dithered)
}

impl PaletteSource for FakePalettes {
    fn pixels(&self) -> usize {
        1000
    }

    fn make(&self, colors: usize, dithered: bool) -> Result<png::Palette, String> {
        let t = fake_index(colors, dithered);
        let size = self.sizes[t].ok_or("failed")?;
        Ok(png::Palette {
            png: vec![0; size],
            pixels: vec![[t as u8, 0, 0, 0]],
        })
    }

    fn score(&self, palette: &png::Palette) -> f64 {
        let t = usize::from(palette.pixels[0][0]);
        self.measured.lock().unwrap().push(t);
        self.scores[t]
    }
}

/// What the standard chooses, said plainly, as if every palette were made and measured: each
/// kind from the most colors down, past the ones that couldn't be made, until one scores short;
/// the dithered kind only when the plain palette of the most colors scores within
/// [`DITHER_HELPS`] of what is needed (ADR 0078). Gives the size of the one kept (`None` for the
/// lossless version), or `Err` when none is small enough, and those it had to measure.
fn palette_by_the_rule(
    fake: &FakePalettes,
    lossless: usize,
    limit: u64,
) -> (Result<Option<usize>, ()>, Vec<usize>) {
    let mut measured = Vec::new();
    let mut smallest_passing = |threshold: f64, limit: u64| {
        let most = fake_index(png::PALETTE_SIZES[0], false);
        let far_short = fake.sizes[most].is_some() && fake.scores[most] < threshold - DITHER_HELPS;
        let mut passing = Vec::new();
        for dithered in [false, true] {
            if dithered && far_short {
                continue;
            }
            for &colors in &png::PALETTE_SIZES {
                let t = fake_index(colors, dithered);
                let Some(size) = fake.sizes[t] else { continue };
                if !measured.contains(&t) {
                    measured.push(t);
                }
                if fake.scores[t] < threshold {
                    break;
                }
                if size < lossless && size as u64 <= limit {
                    passing.push(size);
                }
            }
        }
        passing.into_iter().min()
    };
    let first = smallest_passing(FIRST_SCORE, u64::MAX);
    let chosen = if first.unwrap_or(lossless) as u64 <= limit {
        Ok(first)
    } else {
        smallest_passing(AGAIN_SCORE, limit).map(Some).ok_or(())
    };
    (chosen, measured)
}

#[test]
fn palettes_are_chosen_by_the_rule_making_and_measuring_less() {
    // A small, fixed pseudo-random sequence.
    let mut state = 0x2545_f491_4f6c_dd1d_u64;
    let mut next = move |below: u64| {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state % below
    };
    let (lossless, limit) = (1000, 800);
    for case in 0..2000 {
        let sizes = [(); 8].map(|()| match next(12) {
            0 => None,
            // Some ties, around the limit and the lossless version's size.
            n => Some(500 + 50 * n as usize + if case % 2 == 0 { 0 } else { next(50) as usize }),
        });
        let scores = [(); 8].map(|()| 70.0 + next(30) as f64);
        let fake = FakePalettes {
            sizes,
            scores,
            measured: std::sync::Mutex::default(),
        };
        let (by_the_rule, to_measure) = palette_by_the_rule(&fake, lossless, limit);
        let mut palettes = Palettes::new(&fake, lossless);
        let chosen = palettes.choose(&|| Ok(()), limit, &mut |_| {});
        let chosen = match chosen {
            Ok(chosen) => Ok(chosen.map(|t| sizes[t].unwrap_or(usize::MAX))),
            Err(ImageError::AsSmallAsItGets) => Err(()),
            Err(error) => panic!("{error}"),
        };
        let what = format!("sizes {sizes:?}, scores {scores:?}");
        assert_eq!(chosen, by_the_rule, "{what}");
        let measured = fake.measured.into_inner().unwrap();
        assert!(
            measured.iter().all(|t| to_measure.contains(t)),
            "{what}: measured {measured:?}, the rule {to_measure:?}"
        );
        let mut once = measured.clone();
        once.sort_unstable();
        once.dedup();
        assert_eq!(once.len(), measured.len(), "{what}: measured twice");
    }
}

/// How many palettes of each kind `fake` was asked to make.
fn made(palettes: &Palettes<'_, FakePalettes>) -> (usize, usize) {
    let count = |dithered: bool| {
        (0..palettes.tries.len())
            .filter(|&t| palettes.tries[t].1 == dithered && palettes.made[t].is_some())
            .count()
    };
    (count(false), count(true))
}

#[test]
fn a_plain_palette_far_short_spares_the_dithered_ones() {
    // A picture of many colors: the plain palette scores far below what is needed.
    let fake = FakePalettes {
        sizes: [
            Some(300),
            Some(900),
            Some(250),
            Some(800),
            Some(200),
            Some(700),
            Some(150),
            Some(600),
        ],
        scores: [46.0, 58.0, 40.0, 57.0, 35.0, 55.0, 30.0, 50.0],
        measured: std::sync::Mutex::default(),
    };
    let mut palettes = Palettes::new(&fake, 1000);
    assert_eq!(
        palettes.choose(&|| Ok(()), 2000, &mut |_| {}).ok(),
        Some(None)
    );
    assert_eq!(made(&palettes), (1, 0));
    assert_eq!(fake.measured.into_inner().unwrap(), [0]);
}

#[test]
fn a_dithered_palette_too_big_to_keep_still_says_whether_to_go_on() {
    // The plain palette falls just short, so dithering is tried; the dithered one is bigger than
    // the lossless version and scores short too, so the ones with fewer colors are not made.
    let fake = FakePalettes {
        sizes: [
            Some(300),
            Some(1400),
            Some(250),
            Some(900),
            Some(200),
            Some(800),
            Some(150),
            Some(700),
        ],
        scores: [85.5, 86.0, 80.0, 84.0, 75.0, 82.0, 70.0, 80.0],
        measured: std::sync::Mutex::default(),
    };
    let mut palettes = Palettes::new(&fake, 1000);
    assert_eq!(
        palettes.choose(&|| Ok(()), 2000, &mut |_| {}).ok(),
        Some(None)
    );
    assert_eq!(made(&palettes), (1, 1));
    assert_eq!(fake.measured.into_inner().unwrap(), [0, 1]);
}
