//! How everything that moves in Gracil eases: the dial, the pill's stone, the list unfolding and
//! the change of colors. One rule for all of them, and one place that knows whether Windows'
//! "Animation effects" setting is off, in which case everything goes straight to where it ends.

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

static STILL: AtomicBool = AtomicBool::new(false);

/// How quickly what answers the pointer or the keyboard eases, in seconds (the time to cover
/// about two thirds): a control lighting up, the pill's stone, the dial's symbols, the list
/// unfolding, and a new view coming in. One pace, so everything feels of one piece.
pub const QUICK: f32 = 0.06;

/// How quickly what follows the work eases: the dial filling, the marble breathing.
pub const STEADY: f32 = 0.28;

/// Follows Windows' "Animation effects" setting: when it is off, nothing in Gracil moves.
pub fn follow_windows() {
    STILL.store(!gracil_windows::animations(), Ordering::Relaxed);
}

/// Whether things should change at once instead of moving.
pub fn still() -> bool {
    STILL.load(Ordering::Relaxed)
}

/// Seconds since the last frame, or none for the first. Capped, so a window that was out of sight
/// doesn't jump.
pub fn elapsed(last_frame: Option<Instant>, now: Instant) -> f32 {
    last_frame.map_or(0.0, |last| {
        now.saturating_duration_since(last).as_secs_f32().min(0.1)
    })
}

/// Moves `value` towards `target`, covering about two thirds of the way every `ease` seconds,
/// and snapping once the difference can't be seen. Straight to `target` while things are still.
pub fn approach(value: f32, target: f32, elapsed: f32, ease: f32) -> f32 {
    if still() {
        return target;
    }
    let next = value + (target - value) * (1.0 - (-elapsed / ease).exp());
    if (target - next).abs() < 0.002 {
        target
    } else {
        next
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn values_ease_and_then_settle_exactly() {
        let mut value = 0.0;
        for _ in 0..200 {
            value = approach(value, 1.0, 1.0 / 60.0, 0.28);
        }
        assert_eq!(value, 1.0);
        assert!(approach(0.0, 1.0, 1.0 / 60.0, 0.28) < 0.1);
    }
}
