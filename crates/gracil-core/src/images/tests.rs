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

/// How a palette was chosen before ADR 0068, all eight made first: the index of the one kept
/// (`None` for the lossless version), or `Err` when none is small enough, and those measured.
fn palette_before(
    fake: &FakePalettes,
    lossless: usize,
    limit: u64,
) -> (Result<Option<usize>, ()>, Vec<usize>) {
    let mut palettes: Vec<(usize, usize)> = (0..8)
        .filter_map(|t| fake.sizes[t].filter(|&s| s < lossless).map(|s| (t, s)))
        .collect();
    palettes.sort_by_key(|p| p.1);
    let mut scores = vec![None; palettes.len()];
    let mut measured = Vec::new();
    let mut smallest_passing = |threshold: f64, limit: u64, measured: &mut Vec<usize>| {
        let kind = |dithered: bool| {
            let mut found: Vec<usize> = (0..palettes.len())
                .filter(|&n| (palettes[n].0 % 2 == 1) == dithered)
                .collect();
            found.sort_by_key(|&n| std::cmp::Reverse(png::PALETTE_SIZES[palettes[n].0 / 2]));
            found
        };
        let mut kinds = [kind(false), kind(true)].map(|order| order.into_iter().peekable());
        let mut passing = Vec::new();
        loop {
            let next: Vec<usize> = kinds.iter_mut().filter_map(|k| k.peek().copied()).collect();
            if next.is_empty() {
                break;
            }
            for &n in &next {
                if scores[n].is_none() {
                    measured.push(palettes[n].0);
                    scores[n] = Some(fake.scores[palettes[n].0]);
                }
            }
            for order in &mut kinds {
                let Some(&n) = order.peek() else { continue };
                if scores[n].unwrap_or(0.0) >= threshold {
                    if palettes[n].1 as u64 <= limit {
                        passing.push(n);
                    }
                    order.next();
                } else {
                    order.by_ref().for_each(drop);
                }
            }
        }
        passing.into_iter().min_by_key(|&n| palettes[n].1)
    };
    let first = smallest_passing(FIRST_SCORE, u64::MAX, &mut measured);
    let first_size = first.map_or(lossless, |n| palettes[n].1);
    let chosen = if first_size as u64 <= limit {
        Ok(first)
    } else {
        smallest_passing(AGAIN_SCORE, limit, &mut measured)
            .map(Some)
            .ok_or(())
    };
    (chosen.map(|c| c.map(|n| palettes[n].0)), measured)
}

#[test]
fn palettes_are_chosen_as_before_measuring_less() {
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
        let (before, measured_before) = palette_before(&fake, lossless, limit);
        let mut palettes = Palettes::new(&fake, lossless);
        let chosen = palettes.choose(&|| Ok(()), limit, &mut |_| {});
        let chosen = match chosen {
            Ok(chosen) => Ok(chosen),
            Err(ImageError::AsSmallAsItGets) => Err(()),
            Err(error) => panic!("{error}"),
        };
        let what = format!("sizes {sizes:?}, scores {scores:?}");
        assert_eq!(chosen, before, "{what}");
        let measured = fake.measured.into_inner().unwrap();
        assert!(
            measured.iter().all(|t| measured_before.contains(t)),
            "{what}: measured {measured:?}, before {measured_before:?}"
        );
        let mut once = measured.clone();
        once.sort_unstable();
        once.dedup();
        assert_eq!(once.len(), measured.len(), "{what}: measured twice");
    }
}
