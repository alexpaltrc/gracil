//! The tray icon's pictures, drawn from the same shapes as the delivered tray artwork
//! (`assets/icons/vector/piedra-tray-*.svg`): the support and the marble resting on it. Every
//! state is drawn the same way at the size the taskbar asks for, so switching states never shifts
//! a pixel (ADR 0020).
//!
//! Plain arithmetic, no Windows calls, so it can be tested anywhere.

use super::TrayState;

/// The support, in the artwork's own coordinates: an L whose corners are rounded with quadratic
/// curves.
const SUPPORT: &[Segment] = &[
    Segment::Line(43.0, 66.0),
    Segment::Line(73.0, 66.0),
    Segment::Line(73.0, 157.0),
    Segment::Curve(73.0, 183.0, 100.0, 183.0),
    Segment::Line(206.0, 183.0),
    Segment::Line(206.0, 213.0),
    Segment::Line(96.0, 213.0),
    Segment::Curve(43.0, 213.0, 43.0, 159.0),
];

/// The marble: centre and radii, in the artwork's coordinates.
const MARBLE: (f32, f32, f32, f32) = (131.0, 145.5, 37.0, 37.5);

/// How the artwork's coordinates sit in its 48-unit square.
const SCALE: f32 = 0.245_398_77;
const CENTRE: (f32, f32) = (124.5, 139.5);

/// Samples per pixel along each side; their average gives smooth edges.
const SAMPLES: u32 = 4;

/// The share of the marble's radius kept as its rim while a video compresses.
const RIM: f32 = 0.3;

/// Colours as 0xRRGGBB. The support takes the taskbar's contrasting stone tone, as in the
/// artwork; the warning tone is the same amber as the quiet "!" in the window.
struct Inks {
    stone: u32,
    warning: u32,
}

const ON_LIGHT: Inks = Inks {
    stone: 0x4D4B46,
    warning: 0xC28010,
};
const ON_DARK: Inks = Inks {
    stone: 0xDAD8CF,
    warning: 0xF0B44C,
};

#[derive(Clone, Copy)]
enum Segment {
    Line(f32, f32),
    /// A quadratic curve: control point, then end point.
    Curve(f32, f32, f32, f32),
}

/// Draws `state` as `size`×`size` pixels, top row first, each 0xAARRGGBB with straight alpha:
/// the layout Windows expects for a 32-bit icon.
pub(super) fn draw(state: TrayState, light_taskbar: bool, size: u32) -> Vec<u32> {
    let inks = if light_taskbar { &ON_LIGHT } else { &ON_DARK };
    let support = outline(SUPPORT);
    let per_pixel = SAMPLES * SAMPLES;
    let mut pixels = Vec::with_capacity((size * size) as usize);
    for row in 0..size {
        for column in 0..size {
            let (mut covered, mut red, mut green, mut blue) = (0u32, 0u32, 0u32, 0u32);
            for sample in 0..per_pixel {
                let x = column as f32 + ((sample % SAMPLES) as f32 + 0.5) / SAMPLES as f32;
                let y = row as f32 + ((sample / SAMPLES) as f32 + 0.5) / SAMPLES as f32;
                let point = to_artwork(x, y, size);
                if let Some(ink) = ink_at(state, inks, &support, point) {
                    covered += 1;
                    red += (ink >> 16) & 0xFF;
                    green += (ink >> 8) & 0xFF;
                    blue += ink & 0xFF;
                }
            }
            pixels.push(if covered == 0 {
                0
            } else {
                let alpha = covered * 255 / per_pixel;
                (alpha << 24)
                    | ((red / covered) << 16)
                    | ((green / covered) << 8)
                    | (blue / covered)
            });
        }
    }
    pixels
}

/// What colour, if any, the picture has at this point of the artwork.
fn ink_at(
    state: TrayState,
    inks: &Inks,
    support: &[(f32, f32)],
    (x, y): (f32, f32),
) -> Option<u32> {
    if inside(support, x, y) {
        return Some(inks.stone);
    }
    let (cx, cy, rx, ry) = MARBLE;
    let (dx, dy) = ((x - cx) / rx, (y - cy) / ry);
    let reach = (dx * dx + dy * dy).sqrt();
    if reach > 1.0 {
        return None;
    }
    match state {
        TrayState::Idle => Some(inks.stone),
        // The marble fills clockwise from the top as the video advances, inside its rim.
        TrayState::Processing(fraction) => {
            let turn = (dx.atan2(-dy) / std::f32::consts::TAU).rem_euclid(1.0);
            (reach >= 1.0 - RIM || turn <= fraction).then_some(inks.stone)
        }
        // Two bars in place of the marble.
        TrayState::Paused => {
            let bar = (0.2..=0.75).contains(&dx.abs()) && dy.abs() <= 0.8;
            bar.then_some(inks.stone)
        }
        TrayState::Error => Some(inks.warning),
    }
}

/// A pixel position in an icon of `size` pixels, in the artwork's coordinates.
fn to_artwork(x: f32, y: f32, size: u32) -> (f32, f32) {
    let unit = 48.0 / size as f32;
    (
        (x * unit - 24.0) / SCALE + CENTRE.0,
        (y * unit - 24.0) / SCALE + CENTRE.1,
    )
}

/// The path as straight edges, curves cut into short lines.
fn outline(path: &[Segment]) -> Vec<(f32, f32)> {
    const STEPS: u32 = 12;
    let mut points: Vec<(f32, f32)> = Vec::new();
    for &segment in path {
        match segment {
            Segment::Line(x, y) => points.push((x, y)),
            Segment::Curve(cx, cy, x, y) => {
                let (sx, sy) = points.last().copied().unwrap_or((cx, cy));
                for step in 1..=STEPS {
                    let t = step as f32 / STEPS as f32;
                    let u = 1.0 - t;
                    points.push((
                        u * u * sx + 2.0 * u * t * cx + t * t * x,
                        u * u * sy + 2.0 * u * t * cy + t * t * y,
                    ));
                }
            }
        }
    }
    points
}

/// Whether the point lies inside the closed outline (even-odd rule).
fn inside(outline: &[(f32, f32)], x: f32, y: f32) -> bool {
    let mut inside = false;
    let mut previous = outline.len().wrapping_sub(1);
    for (current, &(xi, yi)) in outline.iter().enumerate() {
        let (xj, yj) = outline[previous];
        if (yi > y) != (yj > y) && x < (xj - xi) * (y - yi) / (yj - yi) + xi {
            inside = !inside;
        }
        previous = current;
    }
    inside
}

#[cfg(test)]
mod tests {
    use super::*;

    const SIZE: u32 = 32;

    fn alpha(pixels: &[u32], x: u32, y: u32) -> u32 {
        pixels[(y * SIZE + x) as usize] >> 24
    }

    fn colour(pixels: &[u32], x: u32, y: u32) -> u32 {
        pixels[(y * SIZE + x) as usize] & 0xFF_FFFF
    }

    /// The marble's centre, in pixels of a 32-pixel icon.
    fn marble_centre() -> (u32, u32) {
        let unit = SIZE as f32 / 48.0;
        let x = ((MARBLE.0 - CENTRE.0) * SCALE + 24.0) * unit;
        let y = ((MARBLE.1 - CENTRE.1) * SCALE + 24.0) * unit;
        (x as u32, y as u32)
    }

    #[test]
    fn idle_shows_the_support_and_a_solid_marble() {
        let pixels = draw(TrayState::Idle, true, SIZE);
        let (cx, cy) = marble_centre();
        assert_eq!(alpha(&pixels, cx, cy), 255);
        assert_eq!(colour(&pixels, cx, cy), ON_LIGHT.stone);
        // The foot of the support, near the bottom right.
        assert_eq!(alpha(&pixels, 27, 27), 255);
        // The top right corner is empty.
        assert_eq!(alpha(&pixels, 30, 2), 0);
    }

    #[test]
    fn processing_starts_hollow_and_fills_up() {
        let (cx, cy) = marble_centre();
        let empty = draw(TrayState::Processing(0.0), true, SIZE);
        assert_eq!(alpha(&empty, cx, cy), 0);
        let full = draw(TrayState::Processing(1.0), true, SIZE);
        assert_eq!(alpha(&full, cx, cy), 255);
        // Halfway, the right half is filled and the left half not yet.
        let half = draw(TrayState::Processing(0.5), true, SIZE);
        assert!(alpha(&half, cx + 2, cy) > 200);
        assert_eq!(alpha(&half, cx - 3, cy - 1), 0);
    }

    #[test]
    fn paused_and_error_differ_from_idle() {
        let idle = draw(TrayState::Idle, false, SIZE);
        let paused = draw(TrayState::Paused, false, SIZE);
        let error = draw(TrayState::Error, false, SIZE);
        let (cx, cy) = marble_centre();
        // The gap between the pause bars.
        assert_eq!(alpha(&paused, cx, cy), 0);
        assert_eq!(colour(&error, cx, cy), ON_DARK.warning);
        assert_ne!(idle, paused);
        assert_ne!(idle, error);
    }

    #[test]
    fn the_support_follows_the_taskbar() {
        let on_light = draw(TrayState::Idle, true, SIZE);
        let on_dark = draw(TrayState::Idle, false, SIZE);
        assert_eq!(colour(&on_light, 27, 27), ON_LIGHT.stone);
        assert_eq!(colour(&on_dark, 27, 27), ON_DARK.stone);
    }

    #[test]
    fn any_size_is_drawn() {
        for size in [16, 20, 24, 32, 40, 48, 64] {
            let pixels = draw(TrayState::Idle, true, size);
            assert_eq!(pixels.len(), (size * size) as usize);
            assert!(pixels.iter().any(|pixel| pixel >> 24 == 255));
        }
    }
}
