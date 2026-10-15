//! The blur SSIMULACRA2 measures with: "Recursive Implementation of the Gaussian Filter Using
//! Truncated Cosine Functions" by Charalampidis [2016], from the `ssimulacra2` crate's
//! `blur/gaussian.rs`, computing the same numbers in the same order.

use pulp::{Simd, WithSimd};
use rayon::prelude::*;

use super::Speed;

// The constants the crate's build script works out for a sigma of 1.5, as it writes them.
const RADIUS: isize = 5;
const VERT_MUL_IN_1: f32 = 0.055_295_236;
const VERT_MUL_IN_3: f32 = -0.058_836_687;
const VERT_MUL_IN_5: f32 = 0.012_955_819;
const VERT_MUL_PREV_1: f32 = -1.902_113_1;
const VERT_MUL_PREV_3: f32 = -1.175_570_5;
const VERT_MUL_PREV_5: f32 = -0.000_000_000_000_000_122_464_69;
const MUL_IN_1: f32 = 0.055_295_236;
const MUL_IN_3: f32 = -0.058_836_687;
const MUL_IN_5: f32 = 0.012_955_819;
const MUL_PREV_1: f32 = 1.902_113_1;
const MUL_PREV_3: f32 = 1.175_570_5;
const MUL_PREV_5: f32 = 0.000_000_000_000_000_122_464_69;
const MUL_PREV2_1: f32 = -1.0;
const MUL_PREV2_3: f32 = -1.0;
const MUL_PREV2_5: f32 = -1.0;

/// The plane `a` of `width` × `height` blurred, or with `b`, their product pixel by pixel
/// blurred: the crate's `image_multiply` followed by its `Blur::blur_plane`, the product taken
/// as the blur reads it.
pub fn blur(speed: Speed, a: &[f32], b: Option<&[f32]>, width: usize, height: usize) -> Vec<f32> {
    let mut temp = vec![0f32; width * height];
    temp.par_chunks_exact_mut(width)
        .enumerate()
        .for_each(|(y, output)| {
            let a = &a[y * width..][..width];
            let b = b.map(|b| &b[y * width..][..width]);
            speed.run(Horizontal { a, b, output });
        });
    let mut output = vec![0f32; width * height];
    speed.run(Vertical {
        input: &temp,
        output: &mut output,
        width,
        height,
    });
    output
}

/// One row through the horizontal pass.
struct Horizontal<'a> {
    a: &'a [f32],
    b: Option<&'a [f32]>,
    output: &'a mut [f32],
}

impl WithSimd for Horizontal<'_> {
    type Output = ();

    #[inline(always)]
    fn with_simd<S: Simd>(self, _: S) {
        match self.b {
            Some(b) => horizontal_row(self.output, |i| self.a[i] * b[i]),
            None => horizontal_row(self.output, |i| self.a[i]),
        }
    }
}

#[inline(always)]
fn horizontal_row(output: &mut [f32], input: impl Fn(usize) -> f32) {
    let width = output.len() as isize;
    let mut prev_1 = 0f32;
    let mut prev_3 = 0f32;
    let mut prev_5 = 0f32;
    let mut prev2_1 = 0f32;
    let mut prev2_3 = 0f32;
    let mut prev2_5 = 0f32;

    let mut n = (-RADIUS) + 1;
    while n < width {
        let left = n - RADIUS - 1;
        let right = n + RADIUS - 1;
        let left_val = if left >= 0 {
            input(left as usize)
        } else {
            0f32
        };
        let right_val = if right < width {
            input(right as usize)
        } else {
            0f32
        };
        let sum = left_val + right_val;

        let mut out_1 = sum * MUL_IN_1;
        let mut out_3 = sum * MUL_IN_3;
        let mut out_5 = sum * MUL_IN_5;

        out_1 = MUL_PREV2_1.mul_add(prev2_1, out_1);
        out_3 = MUL_PREV2_3.mul_add(prev2_3, out_3);
        out_5 = MUL_PREV2_5.mul_add(prev2_5, out_5);
        prev2_1 = prev_1;
        prev2_3 = prev_3;
        prev2_5 = prev_5;

        out_1 = MUL_PREV_1.mul_add(prev_1, out_1);
        out_3 = MUL_PREV_3.mul_add(prev_3, out_3);
        out_5 = MUL_PREV_5.mul_add(prev_5, out_5);
        prev_1 = out_1;
        prev_3 = out_3;
        prev_5 = out_5;

        if n >= 0 {
            output[n as usize] = out_1 + out_3 + out_5;
        }

        n += 1;
    }
}

/// A whole plane through the vertical pass.
struct Vertical<'a> {
    input: &'a [f32],
    output: &'a mut [f32],
    width: usize,
    height: usize,
}

impl WithSimd for Vertical<'_> {
    type Output = ();

    #[inline(always)]
    fn with_simd<S: Simd>(self, _: S) {
        vertical_pass_chunked::<128, 32>(self.input, self.output, self.width, self.height);
    }
}

#[inline(always)]
fn vertical_pass_chunked<const J: usize, const K: usize>(
    input: &[f32],
    output: &mut [f32],
    width: usize,
    height: usize,
) {
    assert!(J > K);
    assert!(K > 0);

    assert_eq!(input.len(), output.len());

    let mut x = 0;
    while x + J <= width {
        vertical_pass::<J>(&input[x..], &mut output[x..], width, height);
        x += J;
    }

    while x + K <= width {
        vertical_pass::<K>(&input[x..], &mut output[x..], width, height);
        x += K;
    }

    while x < width {
        vertical_pass::<1>(&input[x..], &mut output[x..], width, height);
        x += 1;
    }
}

/// The vertical pass on `COLUMNS` columns at a time.
#[inline(always)]
fn vertical_pass<const COLUMNS: usize>(
    input: &[f32],
    output: &mut [f32],
    width: usize,
    height: usize,
) {
    assert_eq!(input.len(), output.len());

    let zeroes = vec![0f32; COLUMNS];
    let mut prev = vec![0f32; 3 * COLUMNS];
    let mut prev2 = vec![0f32; 3 * COLUMNS];
    let mut out = vec![0f32; 3 * COLUMNS];

    let mut n = (-RADIUS) + 1;
    while n < height as isize {
        let top = n - RADIUS - 1;
        let bottom = n + RADIUS - 1;
        let top_row = if top >= 0 {
            &input[top as usize * width..][..COLUMNS]
        } else {
            &zeroes
        };

        let bottom_row = if bottom < height as isize {
            &input[bottom as usize * width..][..COLUMNS]
        } else {
            &zeroes
        };

        for i in 0..COLUMNS {
            let sum = top_row[i] + bottom_row[i];

            let i1 = i;
            let i3 = i1 + COLUMNS;
            let i5 = i3 + COLUMNS;

            let out1 = prev[i1].mul_add(VERT_MUL_PREV_1, prev2[i1]);
            let out3 = prev[i3].mul_add(VERT_MUL_PREV_3, prev2[i3]);
            let out5 = prev[i5].mul_add(VERT_MUL_PREV_5, prev2[i5]);

            let out1 = sum.mul_add(VERT_MUL_IN_1, -out1);
            let out3 = sum.mul_add(VERT_MUL_IN_3, -out3);
            let out5 = sum.mul_add(VERT_MUL_IN_5, -out5);

            out[i1] = out1;
            out[i3] = out3;
            out[i5] = out5;

            if n >= 0 {
                output[n as usize * width + i] = out1 + out3 + out5;
            }
        }

        prev2.copy_from_slice(&prev);
        prev.copy_from_slice(&out);

        n += 1;
    }
}
