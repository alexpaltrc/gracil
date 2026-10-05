//! How much Gracil takes on at once (spec §77.3, ADR 0040): fast, but never alarming. One video
//! compresses while the next one gets ready; images compress beside it, more of them while the
//! computer is otherwise idle and fewer while the person is using it, and fewer when they are
//! large, so memory never runs short. When the computer runs warm it does fewer images at once
//! and takes short, unnoticed rests, eased up and down to hold the graphics card at a steady
//! temperature, so it never gets hot (ADR 0045, 0046). Windows itself holds Gracil to part of the
//! processor, a smaller part while the person works and on battery (ADR 0049).

use std::time::Duration;

use gracil_core::share;
use gracil_windows::{GraphicsHeat, Heat, Thermometer};
use tracing::{info, warn};

/// Without a touch of the keyboard or mouse for this long, the computer counts as idle.
const IDLE_AFTER: Duration = Duration::from_secs(60);

/// Images read ahead beyond those compressing at once, so a slow drive doesn't leave the
/// processor waiting.
pub const READ_AHEAD: usize = 2;

/// Memory an image needs while it is compressed, per pixel: the decoded picture, the copies the
/// quality measure works on and the encoder's own.
const BYTES_PER_PIXEL: u64 = 48;

/// When an image's size can't be told before decoding it, it is taken to be this large, as big
/// as a phone's largest photos.
const UNKNOWN_PIXELS: u64 = 24_000_000;

/// What the computer is doing right now.
#[derive(Debug, Clone, Copy)]
pub struct Room {
    /// Processor threads in the computer.
    pub threads: usize,
    /// The person isn't using the computer.
    pub idle: bool,
    /// A video is being compressed on the processor rather than the graphics card.
    pub video_on_processor: bool,
    /// Memory free right now, when Windows says.
    pub free_memory: Option<u64>,
    /// The computer is running warm (ADR 0043).
    pub warm: bool,
    /// The computer runs on its battery, or Windows is saving energy (ADR 0049).
    pub saving_energy: bool,
}

impl Room {
    /// Looks at the computer now.
    pub fn now(video_on_processor: bool, warm: bool) -> Self {
        Self {
            threads: share::processor_threads(),
            idle: gracil_windows::idle_for().is_some_and(|idle| idle >= IDLE_AFTER),
            video_on_processor,
            free_memory: gracil_windows::free_memory(),
            warm,
            saving_energy: gracil_windows::saving_energy(),
        }
    }

    /// How many images may compress at once: an eighth of the processor while the person is
    /// using the computer, it runs warm or it saves energy, a quarter while it is idle, one fewer while a video
    /// takes the processor, and always at least one. Each image's tools use several threads of
    /// their own.
    pub fn images_at_once(self) -> usize {
        let share = if self.idle && !self.warm && !self.saving_energy {
            self.threads / 4
        } else {
            self.threads / 8
        };
        share
            .saturating_sub(usize::from(self.video_on_processor))
            .max(1)
    }

    /// Memory images may take on top of what is already in use: half of what is free, so the
    /// person's own programs always keep room.
    pub fn memory_for_images(self) -> u64 {
        self.free_memory.map_or(u64::MAX, |free| free / 2)
    }
}

/// Memory an image of `pixels` needs while it is compressed.
pub fn image_memory(pixels: Option<u64>) -> u64 {
    pixels.unwrap_or(UNKNOWN_PIXELS) * BYTES_PER_PIXEL
}

/// How many of the processor's `threads` Gracil and the tools it starts may run on at all
/// (ADR 0049): three quarters while the computer is idle, half while the person is using it, and
/// a quarter while it runs on its battery or saves energy. Windows holds Gracil to it, so however
/// the work is split, the rest of the processor stays free and cool.
pub fn processors(threads: usize, idle: bool, saving_energy: bool) -> usize {
    let share = if saving_energy {
        threads / 4
    } else if idle {
        threads * 3 / 4
    } else {
        threads / 2
    };
    share.max(1)
}

/// How often Gracil writes down how much memory it holds, so a long run can be checked for
/// memory it never gives back.
const MEMORY_LOG_EVERY: Duration = Duration::from_secs(10 * 60);

/// How long one breath lasts: a rest, then work (ADR 0045, 0046). Short enough that the parts
/// stay at an even temperature through it, the way a dimmer flickers too fast to see.
const BREATH: Duration = Duration::from_secs(1);
/// A rest shorter than this isn't worth taking: it would only stop and start the work.
const SHORTEST_REST: Duration = Duration::from_millis(50);

/// The temperature Gracil keeps a graphics card at, at most: 70 degrees, or 20 below the
/// temperature at which the card starts slowing itself down, whichever is lower (ADR 0046).
const GRAPHICS_TARGET: u32 = 70;
const TARGET_BELOW_SLOWDOWN: u32 = 20;
/// Past this many degrees above the target, the card gets the least work at once rather than step
/// by step.
const FAR_ABOVE_TARGET: u32 = 10;
/// How much the share of work changes each second, per degree away from the target: slowly, so
/// it settles at the share that holds the temperature instead of swinging around it.
const STEP_PER_DEGREE: f64 = 0.02;
/// The least share of each breath spent working: the work slows down, but never stops.
const LEAST_WORK: f64 = 0.25;
/// The processor counts as held back when it is allowed this much less than usual.
const HELD_BACK_BY: f64 = 10.0;
/// Readings in a row the processor must be held back for, so a moment's dip is not mistaken for
/// heat.
const HELD_BACK_READINGS: u8 = 5;

/// Turns readings into the share of each breath Gracil works, remembering what it needs between
/// them (ADR 0046).
#[derive(Debug)]
pub struct Warmer {
    /// The share of each breath worked now.
    work: f64,
    /// The highest the processor's limit has been seen at: its usual value.
    usual_limit: Option<f64>,
    /// Readings in a row with the processor held back.
    held_back: u8,
}

impl Default for Warmer {
    fn default() -> Self {
        Self {
            work: 1.0,
            usual_limit: None,
            held_back: 0,
        }
    }
}

impl Warmer {
    /// Takes in a reading, `elapsed` after the last: the share of each breath to work, from 1
    /// (all of it) down to [`LEAST_WORK`]. Above the target temperature the share goes down a
    /// little, more the further above; below it, it comes back up the same way. So the card
    /// settles at the target, working as much as that temperature allows. A card or processor
    /// slowing itself down because of heat, or a card far above the target, gets the least at
    /// once.
    pub fn read(&mut self, heat: &Heat, elapsed: Duration) -> f64 {
        let processor_held_back = match heat.processor_limit {
            Some(limit) => {
                let usual = self.usual_limit.map_or(limit, |usual| usual.max(limit));
                self.usual_limit = Some(usual);
                limit <= usual - HELD_BACK_BY
            }
            None => false,
        };
        self.held_back = if processor_held_back {
            self.held_back.saturating_add(1)
        } else {
            0
        };
        // Degrees below the target of the card closest to it, or furthest above.
        let below = heat
            .graphics
            .iter()
            .map(|card| i64::from(target(card)) - i64::from(card.celsius))
            .min();
        let urgent = self.held_back >= HELD_BACK_READINGS
            || heat.graphics.iter().any(|card| card.held_back)
            || below.is_some_and(|below| below < -i64::from(FAR_ABOVE_TARGET));
        self.work = if urgent {
            LEAST_WORK
        } else {
            // Without a card to read, the processor alone decides, and it isn't held back: the
            // share comes back up as if well below the target.
            let step = below.unwrap_or(10) as f64 * STEP_PER_DEGREE * elapsed.as_secs_f64();
            (self.work + step).clamp(LEAST_WORK, 1.0)
        };
        self.work
    }
}

/// The temperature `card` is kept at, at most.
fn target(card: &GraphicsHeat) -> u32 {
    card.slowdown.map_or(GRAPHICS_TARGET, |slowdown| {
        GRAPHICS_TARGET.min(slowdown.saturating_sub(TARGET_BELOW_SLOWDOWN))
    })
}

/// What the work does right now, as Gracil breathes (ADR 0045).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Breath {
    /// The computer is running warm: fewer images at once.
    pub warm: bool,
    /// Resting: everything being worked on is frozen for a moment.
    pub resting: bool,
}

/// Reads the computer's warmth every breath on a thread of its own, for as long as Gracil runs.
/// While it is warm, each breath is a short rest followed by work, the rest longer the warmer it
/// is; `changed` hears every change, starting with the first.
pub fn watch(changed: impl Fn(Breath) + Send + 'static) {
    let spawned = std::thread::Builder::new()
        .name("warmth".to_owned())
        .spawn(move || {
            let mut thermometer = Thermometer::new();
            let mut warmer = Warmer::default();
            let mut last = None;
            let mut tell = |breath: Breath| {
                if last != Some(breath) {
                    changed(breath);
                    last = Some(breath);
                }
            };
            // Logged in steps of a twentieth, so a degree up and down isn't written each time.
            let mut logged = None;
            let threads = share::processor_threads();
            let mut allowed = None;
            let mut memory_logged: Option<std::time::Instant> = None;
            loop {
                let idle = gracil_windows::idle_for().is_some_and(|idle| idle >= IDLE_AFTER);
                let saving_energy = gracil_windows::saving_energy();
                let now = processors(threads, idle, saving_energy);
                if allowed != Some(now) {
                    match gracil_windows::use_processors(now) {
                        Ok(()) => info!(
                            threads = now,
                            of = threads,
                            idle,
                            saving_energy,
                            "processor share"
                        ),
                        Err(error) => warn!(%error, "the processor share can't be set"),
                    }
                    allowed = Some(now);
                }
                if memory_logged.is_none_or(|at| at.elapsed() >= MEMORY_LOG_EVERY) {
                    info!(bytes = gracil_windows::own_memory(), "memory held");
                    memory_logged = Some(std::time::Instant::now());
                }
                let heat = thermometer.read();
                let work = warmer.read(&heat, BREATH);
                let step = (work * 20.0).round() as u8;
                if logged != Some(step) {
                    info!(work, ?heat, "warmth");
                    logged = Some(step);
                }
                let rest = BREATH.mul_f64(1.0 - work);
                if rest >= SHORTEST_REST {
                    tell(Breath {
                        warm: true,
                        resting: true,
                    });
                    std::thread::sleep(rest);
                    tell(Breath {
                        warm: true,
                        resting: false,
                    });
                    std::thread::sleep(BREATH - rest);
                } else {
                    tell(Breath {
                        warm: work < 1.0,
                        resting: false,
                    });
                    std::thread::sleep(BREATH);
                }
            }
        });
    if let Err(error) = spawned {
        warn!(%error, "the computer's warmth can't be watched");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn room(threads: usize, idle: bool, video_on_processor: bool) -> Room {
        Room {
            threads,
            idle,
            video_on_processor,
            free_memory: None,
            warm: false,
            saving_energy: false,
        }
    }

    #[test]
    fn more_images_while_the_computer_is_idle() {
        assert_eq!(room(16, false, false).images_at_once(), 2);
        assert_eq!(room(16, true, false).images_at_once(), 4);
        assert_eq!(room(32, true, false).images_at_once(), 8);
    }

    #[test]
    fn a_video_on_the_processor_leaves_less_room() {
        assert_eq!(room(16, true, true).images_at_once(), 3);
    }

    #[test]
    fn always_at_least_one() {
        assert_eq!(room(2, false, true).images_at_once(), 1);
        assert_eq!(room(1, true, false).images_at_once(), 1);
    }

    #[test]
    fn large_images_need_more_memory() {
        assert!(image_memory(Some(48_000_000)) > image_memory(Some(12_000_000)));
        assert_eq!(image_memory(None), image_memory(Some(UNKNOWN_PIXELS)));
        let free = Room {
            free_memory: Some(8 << 30),
            ..room(8, true, false)
        };
        assert_eq!(free.memory_for_images(), 4 << 30);
    }

    fn card(celsius: u32) -> Heat {
        Heat {
            graphics: vec![GraphicsHeat {
                celsius,
                slowdown: Some(91),
                held_back: false,
            }],
            processor_limit: Some(100.0),
        }
    }

    const SECOND: Duration = Duration::from_secs(1);

    #[test]
    fn a_card_below_the_target_works_fully() {
        let mut warmer = Warmer::default();
        for celsius in [40, 60, 70] {
            assert_eq!(warmer.read(&card(celsius), SECOND), 1.0);
        }
    }

    #[test]
    fn above_the_target_the_work_eases_off_step_by_step() {
        let mut warmer = Warmer::default();
        let first = warmer.read(&card(73), SECOND);
        let second = warmer.read(&card(73), SECOND);
        assert!(second < first && first < 1.0, "{first} {second}");
        // Further above, faster.
        let mut other = Warmer::default();
        assert!(other.read(&card(78), SECOND) < first);
        // Never below the least.
        for _ in 0..200 {
            warmer.read(&card(75), SECOND);
        }
        assert_eq!(warmer.read(&card(75), SECOND), LEAST_WORK);
    }

    #[test]
    fn below_the_target_the_work_comes_back_gradually() {
        let mut warmer = Warmer::default();
        for _ in 0..200 {
            warmer.read(&card(75), SECOND);
        }
        let back = warmer.read(&card(67), SECOND);
        assert!(LEAST_WORK < back && back < 1.0, "{back}");
        for _ in 0..200 {
            warmer.read(&card(67), SECOND);
        }
        assert_eq!(warmer.read(&card(67), SECOND), 1.0);
    }

    #[test]
    fn it_settles_where_the_temperature_holds() {
        // A card whose temperature follows the work: 50 degrees idle, 90 at full work.
        let mut warmer = Warmer::default();
        let mut celsius = 50.0_f64;
        let mut highest = 0.0_f64;
        for _ in 0..600 {
            let work = warmer.read(&card(celsius.round() as u32), SECOND);
            // It warms and cools toward that temperature over about twenty seconds.
            celsius += (50.0 + 40.0 * work - celsius) / 20.0;
            highest = highest.max(celsius);
        }
        assert!((celsius - 70.0).abs() <= 2.0, "{celsius}");
        assert!(highest <= 75.0, "{highest}");
    }

    #[test]
    fn a_card_that_slows_down_early_is_kept_cooler() {
        let mut warmer = Warmer::default();
        let mut heat = card(60);
        heat.graphics[0].slowdown = Some(78);
        assert!(warmer.read(&heat, SECOND) < 1.0);
    }

    #[test]
    fn far_above_the_target_or_slowing_itself_down_works_least() {
        let mut warmer = Warmer::default();
        assert_eq!(warmer.read(&card(85), SECOND), LEAST_WORK);
        let mut warmer = Warmer::default();
        let mut heat = card(60);
        heat.graphics[0].held_back = true;
        assert_eq!(warmer.read(&heat, SECOND), LEAST_WORK);
    }

    #[test]
    fn a_processor_held_back_for_a_while_works_least() {
        let processor = |limit| Heat {
            graphics: Vec::new(),
            processor_limit: Some(limit),
        };
        let mut warmer = Warmer::default();
        assert_eq!(warmer.read(&processor(100.0), SECOND), 1.0);
        // A moment's dip is not heat.
        for _ in 0..HELD_BACK_READINGS - 1 {
            assert_eq!(warmer.read(&processor(70.0), SECOND), 1.0);
        }
        assert_eq!(warmer.read(&processor(100.0), SECOND), 1.0);
        for _ in 0..HELD_BACK_READINGS - 1 {
            assert_eq!(warmer.read(&processor(70.0), SECOND), 1.0);
        }
        assert_eq!(warmer.read(&processor(70.0), SECOND), LEAST_WORK);
        // Then back up, gradually.
        let back = warmer.read(&processor(100.0), SECOND);
        assert!(LEAST_WORK < back && back < 1.0, "{back}");
    }

    #[test]
    fn a_processor_always_limited_is_not_hot() {
        let mut warmer = Warmer::default();
        let always = Heat {
            graphics: Vec::new(),
            processor_limit: Some(80.0),
        };
        for _ in 0..10 {
            assert_eq!(warmer.read(&always, SECOND), 1.0);
        }
    }

    #[test]
    fn nothing_to_read_is_cool() {
        let mut warmer = Warmer::default();
        assert_eq!(warmer.read(&Heat::default(), SECOND), 1.0);
    }

    #[test]
    fn less_of_the_processor_while_in_use_and_on_battery() {
        assert_eq!(processors(12, true, false), 9);
        assert_eq!(processors(12, false, false), 6);
        assert_eq!(processors(12, true, true), 3);
        assert_eq!(processors(4, false, true), 1);
        assert_eq!(processors(1, true, false), 1);
    }

    #[test]
    fn fewer_images_on_battery() {
        let battery = Room {
            saving_energy: true,
            ..room(16, true, false)
        };
        assert_eq!(battery.images_at_once(), 2);
    }

    #[test]
    fn fewer_images_while_warm() {
        let warm = Room {
            warm: true,
            ..room(16, true, false)
        };
        assert_eq!(warm.images_at_once(), 2);
    }
}
