//! How much Gracil takes on at once (spec §77.3, ADR 0040): fast, but never alarming. One video
//! compresses while the next one gets ready; images compress beside it, more of them while the
//! computer is otherwise idle and fewer while the person is using it, and fewer when they are
//! large, so memory never runs short. When the computer runs warm it does fewer images at once
//! and takes short, unnoticed rests, longer the warmer it is, so it never gets hot (ADR 0045).

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
        }
    }

    /// How many images may compress at once: an eighth of the processor while the person is
    /// using the computer or it runs warm, a quarter while it is idle, one fewer while a video
    /// takes the processor, and always at least one. Each image's tools use several threads of
    /// their own.
    pub fn images_at_once(self) -> usize {
        let share = if self.idle && !self.warm {
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

/// How long one breath lasts: a rest, then work (ADR 0045). Short enough that nothing on screen
/// seems to stop, long enough that the parts cool down a little in every rest.
const BREATH: Duration = Duration::from_secs(3);

/// A graphics card starts breathing from here, or from 20 degrees below the temperature at which
/// it starts slowing itself down, whichever is lower.
const GRAPHICS_WARM: u32 = 70;
const WARM_BELOW_SLOWDOWN: u32 = 20;
/// A card works least from here, or from 10 degrees below its slowdown temperature.
const GRAPHICS_HOT: u32 = 80;
const HOT_BELOW_SLOWDOWN: u32 = 10;
/// The least share of each breath spent working: the work slows down, but never stops.
const LEAST_WORK: f64 = 0.25;
/// The processor counts as held back when it is allowed this much less than usual.
const HELD_BACK_BY: f64 = 10.0;
/// Readings in a row the processor must be held back for, so a moment's dip is not mistaken for
/// heat.
const HELD_BACK_READINGS: u8 = 2;

/// Turns readings into the share of each breath Gracil works, remembering what it needs between
/// them (ADR 0045).
#[derive(Debug, Default)]
pub struct Warmer {
    /// The highest the processor's limit has been seen at: its usual value.
    usual_limit: Option<f64>,
    /// Readings in a row with the processor held back.
    held_back: u8,
}

impl Warmer {
    /// Takes in a reading: the share of each breath to work, from 1 (all of it, the computer is
    /// cool) down to [`LEAST_WORK`], less the warmer the graphics card runs. A card or processor
    /// slowing itself down because of heat gets the least.
    pub fn read(&mut self, heat: &Heat) -> f64 {
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
        if self.held_back >= HELD_BACK_READINGS || heat.graphics.iter().any(|card| card.held_back) {
            return LEAST_WORK;
        }
        heat.graphics.iter().map(work).fold(1.0, f64::min)
    }
}

/// The share of each breath `card` allows working: all of it until it gets warm, then less and
/// less, in proportion, down to [`LEAST_WORK`] once it is hot.
fn work(card: &GraphicsHeat) -> f64 {
    let (warm, hot) = card
        .slowdown
        .map_or((GRAPHICS_WARM, GRAPHICS_HOT), |slowdown| {
            (
                GRAPHICS_WARM.min(slowdown.saturating_sub(WARM_BELOW_SLOWDOWN)),
                GRAPHICS_HOT.min(slowdown.saturating_sub(HOT_BELOW_SLOWDOWN)),
            )
        });
    let span = f64::from(hot.saturating_sub(warm).max(1));
    let past_warm = f64::from(card.celsius.saturating_sub(warm)) / span;
    1.0 - past_warm.min(1.0) * (1.0 - LEAST_WORK)
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
            loop {
                let heat = thermometer.read();
                let work = warmer.read(&heat);
                let step = (work * 20.0).round() as u8;
                if logged != Some(step) {
                    info!(work, ?heat, "warmth");
                    logged = Some(step);
                }
                if work < 1.0 {
                    tell(Breath {
                        warm: true,
                        resting: true,
                    });
                    std::thread::sleep(BREATH.mul_f64(1.0 - work));
                    tell(Breath {
                        warm: true,
                        resting: false,
                    });
                    std::thread::sleep(BREATH.mul_f64(work));
                } else {
                    tell(Breath {
                        warm: false,
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

    #[test]
    fn a_warmer_card_rests_more_but_never_stops() {
        let mut warmer = Warmer::default();
        assert_eq!(warmer.read(&card(60)), 1.0);
        assert_eq!(warmer.read(&card(70)), 1.0);
        let warm = warmer.read(&card(75));
        assert!(LEAST_WORK < warm && warm < 1.0, "{warm}");
        assert_eq!(warmer.read(&card(80)), LEAST_WORK);
        assert_eq!(warmer.read(&card(95)), LEAST_WORK);
    }

    #[test]
    fn a_card_that_slows_down_early_rests_sooner() {
        let mut warmer = Warmer::default();
        let mut heat = card(65);
        heat.graphics[0].slowdown = Some(80);
        assert!(warmer.read(&heat) < 1.0);
        heat.graphics[0].celsius = 70;
        assert_eq!(warmer.read(&heat), LEAST_WORK);
    }

    #[test]
    fn a_card_slowing_itself_down_works_least() {
        let mut warmer = Warmer::default();
        let mut heat = card(60);
        heat.graphics[0].held_back = true;
        assert_eq!(warmer.read(&heat), LEAST_WORK);
    }

    #[test]
    fn a_processor_held_back_for_a_while_works_least() {
        let processor = |limit| Heat {
            graphics: Vec::new(),
            processor_limit: Some(limit),
        };
        let mut warmer = Warmer::default();
        assert_eq!(warmer.read(&processor(100.0)), 1.0);
        // A moment's dip is not heat.
        assert_eq!(warmer.read(&processor(70.0)), 1.0);
        assert_eq!(warmer.read(&processor(100.0)), 1.0);
        assert_eq!(warmer.read(&processor(70.0)), 1.0);
        assert_eq!(warmer.read(&processor(70.0)), LEAST_WORK);
        assert_eq!(warmer.read(&processor(100.0)), 1.0);
    }

    #[test]
    fn a_processor_always_limited_is_not_hot() {
        let mut warmer = Warmer::default();
        let always = Heat {
            graphics: Vec::new(),
            processor_limit: Some(80.0),
        };
        for _ in 0..5 {
            assert_eq!(warmer.read(&always), 1.0);
        }
    }

    #[test]
    fn nothing_to_read_is_cool() {
        let mut warmer = Warmer::default();
        assert_eq!(warmer.read(&Heat::default()), 1.0);
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
