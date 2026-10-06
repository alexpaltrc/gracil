//! Reading FFmpeg's `-progress` output.
//!
//! FFmpeg writes blocks of `key=value` lines, each closed by `progress=continue` or
//! `progress=end`. Only the position matters to Gracil.

use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Progress {
    /// How far into the source the encode has reached.
    pub position: Duration,
}

/// Accumulates lines and yields a [`Progress`] at the end of every block.
#[derive(Debug, Default)]
pub struct ProgressParser {
    position: Option<Duration>,
}

impl ProgressParser {
    pub fn feed(&mut self, line: &str) -> Option<Progress> {
        let (key, value) = line.trim().split_once('=')?;
        let value = value.trim();
        match key {
            "out_time_us" | "out_time_ms" => {
                // Both keys are in microseconds; `out_time_ms` is a historical misnomer.
                // Negative or `N/A` values appear before the first frame and are ignored.
                if let Ok(us) = value.parse::<u64>() {
                    self.position = Some(Duration::from_micros(us));
                }
                None
            }
            "progress" => Some(Progress {
                position: self.position.unwrap_or_default(),
            }),
            _ => None,
        }
    }
}

/// Fraction done in `0.0..=1.0`, or `None` when the source length is unknown.
pub fn fraction(position: Duration, total: Option<Duration>) -> Option<f64> {
    let total = total.filter(|t| !t.is_zero())?;
    Some((position.as_secs_f64() / total.as_secs_f64()).clamp(0.0, 1.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(input: &str) -> Vec<Progress> {
        let mut parser = ProgressParser::default();
        input.lines().filter_map(|l| parser.feed(l)).collect()
    }

    #[test]
    fn yields_one_update_per_block() {
        let updates = run("\
frame=10
out_time_us=N/A
speed=N/A
progress=continue
frame=60
out_time_us=2000000
out_time_ms=2000000
speed=3.5x
progress=continue
out_time_us=4500000
speed= 3.61x
progress=end
");
        assert_eq!(updates.len(), 3);
        assert_eq!(updates[0].position, Duration::ZERO);
        assert_eq!(updates[1].position, Duration::from_secs(2));
        assert_eq!(updates[2].position, Duration::from_millis(4500));
    }

    #[test]
    fn negative_times_and_noise_are_ignored() {
        let updates = run("out_time_us=-9223372036854775807\ngarbage\n\nprogress=continue\n");
        assert_eq!(updates[0].position, Duration::ZERO);
    }

    #[test]
    fn fraction_is_clamped_and_needs_a_length() {
        let ten = Some(Duration::from_secs(10));
        assert_eq!(fraction(Duration::from_secs(5), ten), Some(0.5));
        assert_eq!(fraction(Duration::from_secs(11), ten), Some(1.0));
        assert_eq!(fraction(Duration::from_secs(1), None), None);
        assert_eq!(fraction(Duration::from_secs(1), Some(Duration::ZERO)), None);
    }
}
