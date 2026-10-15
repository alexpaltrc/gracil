//! SSIMULACRA2, from the `ssimulacra2` crate 0.5.1 by the rav1e contributors (BSD-2-Clause; its
//! notice is in `LICENSE` beside this file and ships with Gracil), changed for Gracil without
//! changing a single bit of any score (ADR 0068):
//!
//! - the original's side of the measure is worked out once, then compared with as many pictures
//!   as needed, instead of being worked out again for each;
//! - the blur, the SSIM map and the edge map run either as built for any 64-bit processor or, on
//!   a processor with fused multiply-add (and AVX2), built for it, chosen as Gracil runs. A fused
//!   multiply-add gives the one correctly rounded result either way, and nothing else is
//!   reordered, so both give the same bits; on any processor the crate's own `mul_add` calls a
//!   function for each, which is most of the time it takes;
//! - the blurs of each scale run side by side, and a product is taken as the blur reads it
//!   rather than written out first;
//! - indexing is checked, with no `unsafe`.
//!
//! The color conversion to XYB is the `yuvxyb` crate's, as the crate itself uses.

mod blur;

use pulp::{Arch, Simd, WithSimd};
use rayon::prelude::*;
use yuvxyb::{LinearRgb, Xyb};

use blur::blur;

/// How often to downscale and score the input images. Each scaling step will downscale by a
/// factor of two.
const NUM_SCALES: usize = 6;

/// How the loops run.
#[derive(Debug, Clone, Copy)]
pub enum Speed {
    /// As built for any 64-bit processor, as the crate runs. Gracil itself always asks for the
    /// best; the tests check this gives the same bits.
    #[cfg_attr(not(test), allow(dead_code))]
    Any,
    /// Built for fused multiply-add and AVX2 where the processor has them, otherwise as `Any`.
    Fused(Arch),
}

impl Speed {
    /// The quickest this processor allows.
    pub fn best() -> Self {
        Self::Fused(Arch::new())
    }

    #[inline(always)]
    fn run<Op: WithSimd>(self, op: Op) -> Op::Output {
        match self {
            Self::Any => op.with_simd(pulp::Scalar),
            Self::Fused(arch) => arch.dispatch(op),
        }
    }
}

/// One scale of the original: its planes, and their blurs.
struct Scale {
    width: usize,
    height: usize,
    img: [Vec<f32>; 3],
    /// `img` blurred.
    mu: [Vec<f32>; 3],
    /// `img` times itself, blurred.
    sigma_sq: [Vec<f32>; 3],
}

/// The original's side of SSIMULACRA2, worked out once.
pub struct Original {
    speed: Speed,
    scales: Vec<Scale>,
}

impl Original {
    /// The original's side, or `None` for a picture under 8 × 8 pixels, which SSIMULACRA2 does
    /// not measure.
    pub fn new(original: LinearRgb, speed: Speed) -> Option<Self> {
        let mut scales = Vec::new();
        each_scale(original, |scale, width, height| {
            let img = planar(scale);
            let blurred: Vec<Vec<f32>> = (0..6)
                .into_par_iter()
                .map(|j| {
                    let c = j % 3;
                    let other = (j >= 3).then_some(&img[c][..]);
                    blur(speed, &img[c], other, width, height)
                })
                .collect();
            let [m0, m1, m2, s0, s1, s2] = <[Vec<f32>; 6]>::try_from(blurred).ok()?;
            scales.push(Scale {
                width,
                height,
                img,
                mu: [m0, m1, m2],
                sigma_sq: [s0, s1, s2],
            });
            Some(())
        })?;
        Some(Self { speed, scales })
    }

    /// SSIMULACRA2 of `distorted` against the original, or `None` when it is of another size.
    pub fn compare(&self, distorted: LinearRgb) -> Option<f64> {
        let first = self.scales.first()?;
        if distorted.width() != first.width || distorted.height() != first.height {
            return None;
        }
        let speed = self.speed;
        let mut msssim = Msssim::default();
        let mut scales = self.scales.iter();
        each_scale(distorted, |scale, width, height| {
            let original = scales.next()?;
            let img2 = planar(scale);
            let img1 = &original.img;
            // The distorted picture blurred, times itself blurred, times the original blurred.
            let blurred: Vec<Vec<f32>> = (0..9)
                .into_par_iter()
                .map(|j| {
                    let c = j % 3;
                    match j / 3 {
                        0 => blur(speed, &img2[c], None, width, height),
                        1 => blur(speed, &img2[c], Some(&img2[c]), width, height),
                        _ => blur(speed, &img1[c], Some(&img2[c]), width, height),
                    }
                })
                .collect();
            let [m0, m1, m2, s0, s1, s2, c0, c1, c2] = <[Vec<f32>; 9]>::try_from(blurred).ok()?;
            let (mu2, sigma2_sq, sigma12) = ([m0, m1, m2], [s0, s1, s2], [c0, c1, c2]);
            let (avg_ssim, avg_edgediff) = rayon::join(
                || {
                    speed.run(SsimMap {
                        width,
                        height,
                        m1: &original.mu,
                        m2: &mu2,
                        s11: &original.sigma_sq,
                        s22: &sigma2_sq,
                        s12: &sigma12,
                    })
                },
                || {
                    speed.run(EdgeDiffMap {
                        width,
                        height,
                        img1,
                        mu1: &original.mu,
                        img2: &img2,
                        mu2: &mu2,
                    })
                },
            );
            msssim.scales.push(MsssimScale {
                avg_ssim,
                avg_edgediff,
            });
            Some(())
        })?;
        Some(msssim.score())
    }
}

/// Calls `each` with every scale of `picture` that SSIMULACRA2 measures, from the full size
/// down, converted to XYB, with its width and height: as the crate's `compute_frame_ssimulacra2`
/// goes through them. `None` for a picture under 8 × 8 pixels, or when `each` says `None`.
fn each_scale(
    picture: LinearRgb,
    mut each: impl FnMut(Xyb, usize, usize) -> Option<()>,
) -> Option<()> {
    if picture.width() < 8 || picture.height() < 8 {
        return None;
    }
    let mut picture = picture;
    for scale in 0..NUM_SCALES {
        let (width, height) = (picture.width(), picture.height());
        // The next scale is made from this one before it becomes XYB, when there is one: the
        // crate goes on while the scale before was at least 8 × 8.
        let next =
            (scale + 1 < NUM_SCALES && width >= 8 && height >= 8).then(|| downscale_by_2(&picture));
        let mut xyb = Xyb::from(picture);
        make_positive_xyb(&mut xyb);
        each(xyb, width, height)?;
        match next {
            Some(next) => picture = next,
            None => break,
        }
    }
    Some(())
}

// Get all components in more or less 0..1 range
// Range of Rec2020 with these adjustments:
//  X: 0.017223..0.998838
//  Y: 0.010000..0.855303
//  B: 0.048759..0.989551
// Range of sRGB:
//  X: 0.204594..0.813402
//  Y: 0.010000..0.855308
//  B: 0.272295..0.938012
// The maximum pixel-wise difference has to be <= 1 for the ssim formula to make
// sense.
fn make_positive_xyb(xyb: &mut Xyb) {
    for pix in xyb.data_mut().iter_mut() {
        pix[2] = (pix[2] - pix[1]) + 0.55;
        pix[0] = (pix[0]).mul_add(14.0, 0.42);
        pix[1] += 0.01;
    }
}

/// The picture as three planes, as SSIMULACRA2 works with them.
fn planar(xyb: Xyb) -> [Vec<f32>; 3] {
    let data = xyb.into_data();
    [0, 1, 2].map(|c| data.iter().map(|pixel| pixel[c]).collect())
}

fn downscale_by_2(in_data: &LinearRgb) -> LinearRgb {
    const SCALE: usize = 2;
    let in_w = in_data.width();
    let in_h = in_data.height();
    let out_w = in_w.div_ceil(SCALE);
    let out_h = in_h.div_ceil(SCALE);
    let mut out_data = vec![[0.0f32; 3]; out_w * out_h];
    let normalize = 1f32 / (SCALE * SCALE) as f32;

    let in_data = &in_data.data();
    for oy in 0..out_h {
        for ox in 0..out_w {
            for c in 0..3 {
                let mut sum = 0f32;
                for iy in 0..SCALE {
                    for ix in 0..SCALE {
                        let x = (ox * SCALE + ix).min(in_w - 1);
                        let y = (oy * SCALE + iy).min(in_h - 1);
                        let in_pix = in_data[y * in_w + x];

                        sum += in_pix[c];
                    }
                }
                let out_pix = &mut out_data[oy * out_w + ox];
                out_pix[c] = sum * normalize;
            }
        }
    }

    LinearRgb::new(out_data, out_w, out_h).unwrap_or_else(|_| unreachable!("sizes match"))
}

struct SsimMap<'a> {
    width: usize,
    height: usize,
    m1: &'a [Vec<f32>; 3],
    m2: &'a [Vec<f32>; 3],
    s11: &'a [Vec<f32>; 3],
    s22: &'a [Vec<f32>; 3],
    s12: &'a [Vec<f32>; 3],
}

impl WithSimd for SsimMap<'_> {
    type Output = [f64; 3 * 2];

    #[inline(always)]
    fn with_simd<S: Simd>(self, _: S) -> Self::Output {
        const C2: f32 = 0.0009f32;
        let Self {
            width,
            height,
            m1,
            m2,
            s11,
            s22,
            s12,
        } = self;

        let one_per_pixels = 1.0f64 / (width * height) as f64;
        let mut plane_averages = [0f64; 3 * 2];

        for c in 0..3 {
            let mut sum1 = [0.0f64; 2];
            for (row_m1, (row_m2, (row_s11, (row_s22, row_s12)))) in m1[c].chunks_exact(width).zip(
                m2[c].chunks_exact(width).zip(
                    s11[c]
                        .chunks_exact(width)
                        .zip(s22[c].chunks_exact(width).zip(s12[c].chunks_exact(width))),
                ),
            ) {
                for x in 0..width {
                    let mu1 = row_m1[x];
                    let mu2 = row_m2[x];
                    let mu11 = mu1 * mu1;
                    let mu22 = mu2 * mu2;
                    let mu12 = mu1 * mu2;
                    let mu_diff = mu1 - mu2;

                    // Correction applied compared to the original SSIM formula, which has:
                    //   luma_err = 2 * mu1 * mu2 / (mu1^2 + mu2^2)
                    //            = 1 - (mu1 - mu2)^2 / (mu1^2 + mu2^2)
                    // The denominator causes error in the darks (low mu1 and mu2) to weigh
                    // more than error in the brights (high mu1 and mu2). This would make
                    // sense if values correspond to linear luma. However, the actual values
                    // are either gamma-compressed luma (which supposedly is already
                    // perceptually uniform) or chroma (where weighing green more than red
                    // or blue more than yellow does not make any sense at all). So it is
                    // better to simply drop this denominator.
                    let num_m = mu_diff.mul_add(-mu_diff, 1.0f32);
                    let num_s = 2f32.mul_add(row_s12[x] - mu12, C2);
                    let denom_s = (row_s11[x] - mu11) + (row_s22[x] - mu22) + C2;
                    // Use 1 - SSIM' so it becomes an error score instead of a quality
                    // index. This makes it make sense to compute an L_4 norm.
                    let mut d = 1.0f64 - f64::from((num_m * num_s) / denom_s);
                    d = d.max(0.0);
                    sum1[0] += d;
                    sum1[1] += d.powi(4);
                }
            }
            plane_averages[c * 2] = one_per_pixels * sum1[0];
            plane_averages[c * 2 + 1] = (one_per_pixels * sum1[1]).sqrt().sqrt();
        }

        plane_averages
    }
}

struct EdgeDiffMap<'a> {
    width: usize,
    height: usize,
    img1: &'a [Vec<f32>; 3],
    mu1: &'a [Vec<f32>; 3],
    img2: &'a [Vec<f32>; 3],
    mu2: &'a [Vec<f32>; 3],
}

impl WithSimd for EdgeDiffMap<'_> {
    type Output = [f64; 3 * 4];

    #[inline(always)]
    fn with_simd<S: Simd>(self, _: S) -> Self::Output {
        let Self {
            width,
            height,
            img1,
            mu1,
            img2,
            mu2,
        } = self;
        let one_per_pixels = 1.0f64 / (width * height) as f64;
        let mut plane_averages = [0f64; 3 * 4];

        for c in 0..3 {
            let mut sum1 = [0.0f64; 4];
            for (row1, (row2, (rowm1, rowm2))) in img1[c].chunks_exact(width).zip(
                img2[c]
                    .chunks_exact(width)
                    .zip(mu1[c].chunks_exact(width).zip(mu2[c].chunks_exact(width))),
            ) {
                for x in 0..width {
                    let d1: f64 = (1.0 + f64::from((row2[x] - rowm2[x]).abs()))
                        / (1.0 + f64::from((row1[x] - rowm1[x]).abs()))
                        - 1.0;

                    // d1 > 0: distorted has an edge where original is smooth
                    //         (indicating ringing, color banding, blockiness, etc)
                    let artifact = d1.max(0.0);
                    sum1[0] += artifact;
                    sum1[1] += artifact.powi(4);

                    // d1 < 0: original has an edge where distorted is smooth
                    //         (indicating smoothing, blurring, smearing, etc)
                    let detail_lost = (-d1).max(0.0);
                    sum1[2] += detail_lost;
                    sum1[3] += detail_lost.powi(4);
                }
            }
            plane_averages[c * 4] = one_per_pixels * sum1[0];
            plane_averages[c * 4 + 1] = (one_per_pixels * sum1[1]).sqrt().sqrt();
            plane_averages[c * 4 + 2] = one_per_pixels * sum1[2];
            plane_averages[c * 4 + 3] = (one_per_pixels * sum1[3]).sqrt().sqrt();
        }

        plane_averages
    }
}

#[derive(Debug, Clone, Default)]
struct Msssim {
    pub scales: Vec<MsssimScale>,
}

#[derive(Debug, Clone, Copy, Default)]
struct MsssimScale {
    pub avg_ssim: [f64; 3 * 2],
    pub avg_edgediff: [f64; 3 * 4],
}

impl Msssim {
    // The final score is based on a weighted sum of 108 sub-scores:
    // - for 6 scales (1:1 to 1:32, downsampled in linear RGB)
    // - for 3 components (X, Y, B-Y, rescaled to 0..1 range)
    // - using 2 norms (the 1-norm and the 4-norm)
    // - over 3 error maps:
    //     - SSIM' (SSIM without the spurious gamma correction term)
    //     - "ringing" (distorted edges where there are no orig edges)
    //     - "blurring" (orig edges where there are no distorted edges)
    // The weights were obtained by running Nelder-Mead simplex search,
    // optimizing to minimize MSE for the CID22 training set and to
    // maximize Kendall rank correlation (and with a lower weight,
    // also Pearson correlation) with the CID22 training set and the
    // TID2013, Kadid10k and KonFiG-IQA datasets.
    // Validation was done on the CID22 validation set.
    // Final results after tuning (Kendall | Spearman | Pearson):
    //    CID22:     0.6903 | 0.8805 | 0.8583
    //    TID2013:   0.6590 | 0.8445 | 0.8471
    //    KADID-10k: 0.6175 | 0.8133 | 0.8030
    //    KonFiG(F): 0.7668 | 0.9194 | 0.9136
    #[allow(clippy::too_many_lines)]
    pub fn score(&self) -> f64 {
        const WEIGHT: [f64; 108] = [
            0.0,
            0.000_737_660_670_740_658_6,
            0.0,
            0.0,
            0.000_779_348_168_286_730_9,
            0.0,
            0.0,
            0.000_437_115_573_010_737_9,
            0.0,
            1.104_172_642_665_734_6,
            0.000_662_848_341_292_71,
            0.000_152_316_327_837_187_52,
            0.0,
            0.001_640_643_745_659_975_4,
            0.0,
            1.842_245_552_053_929_8,
            11.441_172_603_757_666,
            0.0,
            0.000_798_910_943_601_516_3,
            0.000_176_816_438_078_653,
            0.0,
            1.878_759_497_954_638_7,
            10.949_069_906_051_42,
            0.0,
            0.000_728_934_699_150_807_2,
            0.967_793_708_062_683_3,
            0.0,
            0.000_140_034_242_854_358_84,
            0.998_176_697_785_496_7,
            0.000_319_497_559_344_350_53,
            0.000_455_099_211_379_206_3,
            0.0,
            0.0,
            0.001_364_876_616_324_339_8,
            0.0,
            0.0,
            0.0,
            0.0,
            0.0,
            7.466_890_328_078_848,
            0.0,
            17.445_833_984_131_262,
            0.000_623_560_163_404_146_6,
            0.0,
            0.0,
            6.683_678_146_179_332,
            0.000_377_244_079_796_112_96,
            1.027_889_937_768_264,
            225.205_153_008_492_74,
            0.0,
            0.0,
            19.213_238_186_143_016,
            0.001_140_152_458_661_836_1,
            0.001_237_755_635_509_985,
            176.393_175_984_506_94,
            0.0,
            0.0,
            24.433_009_998_704_76,
            0.285_208_026_121_177_57,
            0.000_448_543_692_383_340_8,
            0.0,
            0.0,
            0.0,
            34.779_063_444_837_72,
            44.835_625_328_877_896,
            0.0,
            0.0,
            0.0,
            0.0,
            0.0,
            0.0,
            0.0,
            0.0,
            0.000_868_055_657_329_169_8,
            0.0,
            0.0,
            0.0,
            0.0,
            0.0,
            0.000_531_319_187_435_874_7,
            0.0,
            0.000_165_338_141_613_791_12,
            0.0,
            0.0,
            0.0,
            0.0,
            0.0,
            0.000_417_917_180_325_133_6,
            0.001_729_082_823_472_283_3,
            0.0,
            0.002_082_700_584_663_643_7,
            0.0,
            0.0,
            8.826_982_764_996_862,
            23.192_433_439_989_26,
            0.0,
            95.108_049_881_108_6,
            0.986_397_803_440_068_2,
            0.983_438_279_246_535_3,
            0.001_228_640_504_827_849_3,
            171.266_725_589_730_7,
            0.980_785_887_243_537_9,
            0.0,
            0.0,
            0.0,
            0.000_513_006_458_899_067_9,
            0.0,
            0.000_108_540_578_584_115_37,
        ];

        let mut ssim = 0.0f64;

        let mut i = 0usize;
        for c in 0..3 {
            for scale in &self.scales {
                for n in 0..2 {
                    ssim = WEIGHT[i].mul_add(scale.avg_ssim[c * 2 + n].abs(), ssim);
                    i += 1;
                    ssim = WEIGHT[i].mul_add(scale.avg_edgediff[c * 4 + n].abs(), ssim);
                    i += 1;
                    ssim = WEIGHT[i].mul_add(scale.avg_edgediff[c * 4 + n + 2].abs(), ssim);
                    i += 1;
                }
            }
        }

        ssim *= 0.956_238_261_683_484_4_f64;
        ssim = (6.248_496_625_763_138e-5 * ssim * ssim).mul_add(
            ssim,
            2.326_765_642_916_932f64.mul_add(ssim, -0.020_884_521_182_843_837 * ssim * ssim),
        );

        if ssim > 0.0f64 {
            ssim = ssim
                .powf(0.627_633_646_783_138_7)
                .mul_add(-10.0f64, 100.0f64);
        } else {
            ssim = 100.0f64;
        }

        ssim
    }
}

#[cfg(test)]
mod tests;
