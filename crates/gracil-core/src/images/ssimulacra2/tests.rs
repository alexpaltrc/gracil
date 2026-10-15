use yuvxyb::{ColorPrimaries, Rgb, TransferCharacteristic};

use super::*;

/// A small, fixed pseudo-random sequence.
struct Noise(u64);

impl Noise {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
}

/// A picture of `width` × `height` with smooth areas, edges and fine detail, its channels on
/// a scale of `levels` steps.
fn picture(width: usize, height: usize, levels: u64, seed: u64) -> Vec<[f32; 3]> {
    let mut noise = Noise(seed);
    let top = levels as f32;
    (0..width * height)
        .map(|i| {
            let (x, y) = ((i % width) as f32, (i / width) as f32);
            let wave = ((x * 0.05).sin() * (y * 0.07).cos() + 1.0) / 2.0;
            let edge = if (x as usize / 23 + y as usize / 17).is_multiple_of(2) {
                0.2
            } else {
                0.0
            };
            [0.0f32, 0.3, 0.6].map(|shift| {
                let grain = (noise.next() % 1000) as f32 / 20_000.0;
                let value = ((wave + shift) % 1.0) * 0.75 + edge + grain;
                (value.clamp(0.0, 1.0) * top).round() / top
            })
        })
        .collect()
}

/// `picture` damaged: coarser steps and some noise, as a lossy encoder might leave it.
fn damaged(picture: &[[f32; 3]], levels: f32, seed: u64) -> Vec<[f32; 3]> {
    let mut noise = Noise(seed);
    picture
        .iter()
        .map(|pixel| {
            pixel.map(|value| {
                let grain = ((noise.next() % 1000) as f32 - 500.0) / 50_000.0;
                ((value + grain).clamp(0.0, 1.0) * levels).round() / levels
            })
        })
        .collect()
}

fn rgb(data: Vec<[f32; 3]>, width: usize, height: usize) -> Rgb {
    Rgb::new(
        data,
        width,
        height,
        TransferCharacteristic::SRGB,
        ColorPrimaries::BT709,
    )
    .unwrap()
}

fn linear(rgb: Rgb) -> LinearRgb {
    LinearRgb::try_from(rgb).unwrap()
}

/// Checks the crate's score, and both of Gracil's, for each damaged picture against `original`.
fn same_to_the_bit(original: &[[f32; 3]], damages: &[Vec<[f32; 3]>], width: usize, height: usize) {
    let any = Original::new(linear(rgb(original.to_vec(), width, height)), Speed::Any).unwrap();
    let best = Original::new(linear(rgb(original.to_vec(), width, height)), Speed::best()).unwrap();
    for (n, damaged) in damages.iter().enumerate() {
        let crates = ssimulacra2::compute_frame_ssimulacra2(
            rgb(original.to_vec(), width, height),
            rgb(damaged.clone(), width, height),
        )
        .unwrap();
        let any = any
            .compare(linear(rgb(damaged.clone(), width, height)))
            .unwrap();
        let best = best
            .compare(linear(rgb(damaged.clone(), width, height)))
            .unwrap();
        let what = format!("{width}×{height}, damage {n}: crate {crates}, any {any}, best {best}");
        assert_eq!(crates.to_bits(), any.to_bits(), "{what}");
        assert_eq!(crates.to_bits(), best.to_bits(), "{what}");
    }
}

#[test]
fn eight_bit_scores_are_the_crates_to_the_bit() {
    // Wider than the vertical pass's blocks of 128 and 32 columns, with some left over.
    let (width, height) = (301, 203);
    let original = picture(width, height, 255, 1);
    let damages = [
        damaged(&original, 255.0, 2),
        damaged(&original, 31.0, 3),
        damaged(&original, 7.0, 4),
        original.clone(),
    ];
    same_to_the_bit(&original, &damages, width, height);
}

#[test]
fn sixteen_bit_scores_are_the_crates_to_the_bit() {
    let (width, height) = (160, 97);
    let original = picture(width, height, 65_535, 5);
    let damages = [
        damaged(&original, 65_535.0, 6),
        damaged(&original, 255.0, 7),
    ];
    same_to_the_bit(&original, &damages, width, height);
}

#[test]
fn small_pictures_score_as_the_crate_does() {
    // Down to the smallest measured, where the last scales are under 8 pixels.
    for (width, height) in [(8, 8), (9, 17), (17, 8), (40, 33)] {
        let original = picture(width, height, 255, 8);
        let damages = [damaged(&original, 15.0, 9)];
        same_to_the_bit(&original, &damages, width, height);
    }
}

#[test]
fn too_small_or_another_size_is_not_measured() {
    let small = picture(7, 20, 255, 10);
    assert!(Original::new(linear(rgb(small, 7, 20)), Speed::best()).is_none());
    let original = Original::new(linear(rgb(picture(20, 20, 255, 11), 20, 20)), Speed::best());
    let other = linear(rgb(picture(20, 21, 255, 12), 20, 21));
    assert_eq!(original.unwrap().compare(other), None);
}
