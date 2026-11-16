//! How everything that moves in Gracil moves: the ring, the pill's stone, the list unfolding and
//! a new view coming in. What follows the work or the pointer eases towards it; what answers a
//! choice rides a spring. One place knows whether Windows' "Animation effects" setting is off,
//! in which case everything goes straight to where it ends.

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

/// A value on a spring: it gathers speed towards where it is going and settles there, so what it
/// moves has weight. What answers a choice moves this way: the pill's stone, a new view coming
/// in, the check mark's green landing.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Spring {
    pub value: f32,
    velocity: f32,
}

/// How a spring moves: about how long it takes to arrive, in seconds, and how far past it goes
/// before settling, from 0 (not at all) towards 1.
#[derive(Debug, Clone, Copy)]
pub struct Feel {
    pub response: f32,
    pub bounce: f32,
}

/// Arrives without going past: a view coming in, the list unfolding.
pub const SMOOTH: Feel = Feel {
    response: 0.32,
    bounce: 0.0,
};

/// Arrives quickly and settles with the slightest give: the pill's stone.
pub const SNAPPY: Feel = Feel {
    response: 0.28,
    bounce: 0.18,
};

/// Lands a little past and comes back: the check mark's green.
pub const LIVELY: Feel = Feel {
    response: 0.42,
    bounce: 0.32,
};

impl Spring {
    /// At rest at `value`.
    pub const fn at(value: f32) -> Self {
        Self {
            value,
            velocity: 0.0,
        }
    }

    /// Moves towards `target` for `elapsed` seconds. Straight to it while things are still.
    pub fn step(&mut self, target: f32, elapsed: f32, feel: Feel) {
        if still() {
            *self = Self::at(target);
            return;
        }
        let stiffness = (std::f32::consts::TAU / feel.response).powi(2);
        let damping = 2.0 * std::f32::consts::TAU * (1.0 - feel.bounce) / feel.response;
        // In small steps, so a long frame moves it as a short one would.
        const STEP: f32 = 1.0 / 240.0;
        let mut left = elapsed;
        while left > 0.0 {
            let step = left.min(STEP);
            self.velocity += (stiffness * (target - self.value) - damping * self.velocity) * step;
            self.value += self.velocity * step;
            left -= step;
        }
        if (target - self.value).abs() < 0.001 && self.velocity.abs() < 0.01 {
            *self = Self::at(target);
        }
    }

    /// Sets it moving at `velocity`, as if tapped.
    pub fn kick(&mut self, velocity: f32) {
        if !still() {
            self.velocity += velocity;
        }
    }

    /// How fast it is going, from 0 (at rest) to 1 (as fast as a spring between 0 and 1 gets).
    pub fn haste(&self) -> f32 {
        (self.velocity.abs() / 8.0).min(1.0)
    }

    /// Whether it rests at `target`.
    pub fn rests_at(&self, target: f32) -> bool {
        self.value == target && self.velocity == 0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_spring_arrives_and_rests_exactly() {
        for feel in [SMOOTH, SNAPPY, LIVELY] {
            let mut spring = Spring::at(0.0);
            let mut furthest: f32 = 0.0;
            for _ in 0..240 {
                spring.step(1.0, 1.0 / 60.0, feel);
                furthest = furthest.max(spring.value);
            }
            assert!(spring.rests_at(1.0), "{feel:?}");
            // Only a spring with bounce goes past, and never far.
            assert_eq!(furthest > 1.0, feel.bounce > 0.0, "{feel:?}");
            assert!(furthest < 1.2, "{feel:?}");
        }
    }

    #[test]
    fn a_long_frame_moves_a_spring_as_short_ones_would() {
        let mut one = Spring::at(0.0);
        one.step(1.0, 0.1, SNAPPY);
        let mut many = Spring::at(0.0);
        for _ in 0..24 {
            many.step(1.0, 0.1 / 24.0, SNAPPY);
        }
        assert!((one.value - many.value).abs() < 0.001);
    }

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
