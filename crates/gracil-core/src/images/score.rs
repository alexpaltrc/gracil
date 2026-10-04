//! Measuring how much a compressed picture differs from the original, with SSIMULACRA2 (ADR 0034,
//! item 8): 90 is no visible difference even flipping between the two, about 80 is none at normal
//! viewing.

use ssimulacra2::{ColorPrimaries, Rgb, TransferCharacteristic, compute_frame_ssimulacra2};

/// The original picture, decoded once and measured against as often as needed.
pub struct Reference {
    pub pixels: Vec<[u8; 4]>,
    pub width: u32,
    pub height: u32,
    pub has_alpha: bool,
}

impl Reference {
    pub fn new(pixels: Vec<[u8; 4]>, width: u32, height: u32) -> Self {
        let has_alpha = pixels.iter().any(|p| p[3] != u8::MAX);
        Self {
            pixels,
            width,
            height,
            has_alpha,
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
        let over = |background: f32| {
            compute_frame_ssimulacra2(
                self.rgb(&self.pixels, background)?,
                self.rgb(other, background)?,
            )
            .ok()
        };
        if self.has_alpha {
            Some(over(1.0)?.min(over(0.0)?))
        } else {
            over(1.0)
        }
    }

    fn rgb(&self, pixels: &[[u8; 4]], background: f32) -> Option<Rgb> {
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
            self.width as usize,
            self.height as usize,
            TransferCharacteristic::SRGB,
            ColorPrimaries::BT709,
        )
        .ok()
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
}
