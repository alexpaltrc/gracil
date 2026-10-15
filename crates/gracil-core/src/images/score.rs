//! Measuring how much a compressed picture differs from the original, with SSIMULACRA2 (ADR 0034,
//! item 8): 90 is no visible difference even flipping between the two, about 80 is none at normal
//! viewing.

use std::sync::OnceLock;

use yuvxyb::{ColorPrimaries, LinearRgb, Rgb, TransferCharacteristic};

use super::ssimulacra2::{Original, Speed};

/// The original picture, decoded once and measured against as often as needed.
pub struct Reference {
    pub pixels: Vec<[u8; 4]>,
    pub width: u32,
    pub height: u32,
    pub has_alpha: bool,
    /// The original's side of the measure, over white and over black, worked out the first time
    /// it is needed and kept (ADR 0068); `None` inside when it can't be measured.
    sides: [OnceLock<Option<Original>>; 2],
}

/// The backgrounds a picture is measured over: white, and for a transparent one black too.
const BACKGROUNDS: [f32; 2] = [1.0, 0.0];

impl Reference {
    pub fn new(pixels: Vec<[u8; 4]>, width: u32, height: u32) -> Self {
        let has_alpha = pixels.iter().any(|p| p[3] != u8::MAX);
        Self {
            pixels,
            width,
            height,
            has_alpha,
            sides: Default::default(),
        }
    }

    /// Works out the original's side of the measure now, unless it already was, so a
    /// measurement later only has the other picture's side to work out (ADR 0068).
    pub fn prepare(&self) {
        for n in 0..self.backgrounds() {
            self.side(n);
        }
    }

    /// SSIMULACRA2 of `other` against the original, or `None` when it cannot be measured (a
    /// picture too small, or of another size). A transparent picture is measured over both
    /// white and black, and the worse score counts, so a change hidden by one background is
    /// still noticed on the other.
    pub fn score(&self, other: &[[u8; 4]]) -> Option<f64> {
        if other.len() != self.pixels.len() {
            return None;
        }
        let over = |n: usize| self.side(n)?.compare(self.linear(other, BACKGROUNDS[n])?);
        if self.has_alpha {
            Some(over(0)?.min(over(1)?))
        } else {
            over(0)
        }
    }

    fn backgrounds(&self) -> usize {
        if self.has_alpha { 2 } else { 1 }
    }

    /// The original's side over background `n`, worked out once.
    fn side(&self, n: usize) -> Option<&Original> {
        self.sides[n]
            .get_or_init(|| {
                let linear = self.linear(&self.pixels, BACKGROUNDS[n])?;
                Original::new(linear, Speed::best())
            })
            .as_ref()
    }

    /// `pixels` over `background`, in linear light, as SSIMULACRA2 starts from.
    fn linear(&self, pixels: &[[u8; 4]], background: f32) -> Option<LinearRgb> {
        let unit = |c: u8| f32::from(c) / 255.0;
        let data = pixels
            .iter()
            .map(|p| {
                let a = unit(p[3]);
                [0, 1, 2].map(|i| unit(p[i]) * a + background * (1.0 - a))
            })
            .collect();
        let rgb = Rgb::new(
            data,
            self.width as usize,
            self.height as usize,
            TransferCharacteristic::SRGB,
            ColorPrimaries::BT709,
        )
        .ok()?;
        LinearRgb::try_from(rgb).ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gradient(size: u32, alpha: u8) -> Vec<[u8; 4]> {
        (0..size * size)
            .map(|i| [(i % size * 4) as u8, (i / size * 4) as u8, 100, alpha])
            .collect()
    }

    #[test]
    fn identical_pictures_score_top_and_damage_scores_lower() {
        let reference = Reference::new(gradient(64, 255), 64, 64);
        assert!(!reference.has_alpha);
        let same = reference.score(&gradient(64, 255)).unwrap();
        assert!(same > 99.0, "{same}");
        let mut damaged = gradient(64, 255);
        for pixel in damaged.iter_mut().step_by(3) {
            pixel[0] = pixel[0].wrapping_add(60);
        }
        let worse = reference.score(&damaged).unwrap();
        assert!(worse < 80.0, "{worse}");
    }

    #[test]
    fn transparency_is_measured_and_sizes_must_match() {
        let reference = Reference::new(gradient(64, 128), 64, 64);
        assert!(reference.has_alpha);
        assert!(reference.score(&gradient(64, 128)).unwrap() > 99.0);
        assert_eq!(reference.score(&gradient(32, 128)), None);
    }

    /// The score as it was measured before ADR 0068, with the `ssimulacra2` crate itself.
    fn score_before(reference: &Reference, other: &[[u8; 4]]) -> Option<f64> {
        let rgb = |pixels: &[[u8; 4]], background: f32| {
            let unit = |c: u8| f32::from(c) / 255.0;
            let data = pixels
                .iter()
                .map(|p| {
                    let a = unit(p[3]);
                    [0, 1, 2].map(|i| unit(p[i]) * a + background * (1.0 - a))
                })
                .collect();
            Rgb::new(
                data,
                reference.width as usize,
                reference.height as usize,
                TransferCharacteristic::SRGB,
                ColorPrimaries::BT709,
            )
            .ok()
        };
        let over = |background: f32| {
            ssimulacra2::compute_frame_ssimulacra2(
                rgb(&reference.pixels, background)?,
                rgb(other, background)?,
            )
            .ok()
        };
        if reference.has_alpha {
            Some(over(1.0)?.min(over(0.0)?))
        } else {
            over(1.0)
        }
    }

    #[test]
    fn scores_are_as_before_to_the_bit_with_and_without_transparency() {
        let (width, height) = (150u32, 90u32);
        let picture = |alpha: bool, seed: u32| -> Vec<[u8; 4]> {
            (0..width * height)
                .map(|i| {
                    let (x, y) = (i % width, i / width);
                    let v = i.wrapping_add(seed).wrapping_mul(2_654_435_761) >> 24;
                    let a = if alpha {
                        ((x * 3 + y) % 256) as u8 | 0x10
                    } else {
                        255
                    };
                    [
                        (x * 2) as u8,
                        (y * 3) as u8 ^ (v as u8 & 7),
                        (x + y) as u8,
                        a,
                    ]
                })
                .collect()
        };
        for alpha in [false, true] {
            let reference = Reference::new(picture(alpha, 0), width, height);
            assert_eq!(reference.has_alpha, alpha);
            reference.prepare();
            for seed in [1, 2, 0] {
                let other = picture(alpha, seed);
                let now = reference.score(&other).unwrap();
                let before = score_before(&reference, &other).unwrap();
                assert_eq!(
                    now.to_bits(),
                    before.to_bits(),
                    "alpha {alpha}: {now} {before}"
                );
            }
        }
    }
}
