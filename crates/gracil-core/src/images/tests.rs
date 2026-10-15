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
