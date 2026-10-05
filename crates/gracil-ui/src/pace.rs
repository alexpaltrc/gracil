//! How much Gracil takes on at once (spec §77.3, ADR 0040): fast, but never alarming. One video
//! compresses while the next one gets ready; images compress beside it, more of them while the
//! computer is otherwise idle and fewer while the person is using it, and fewer when they are
//! large, so memory never runs short. When the computer runs warm it does less at once, and when
//! it gets too hot everything waits for it to cool down (ADR 0043).

use std::time::{Duration, Instant};

use gracil_core::share;
use gracil_windows::{GraphicsHeat, Heat, Thermometer};
use tracing::{info, warn};

/// Without a touch of the keyboard or mouse for this long, the computer counts as idle.
const IDLE_AFTER: Duration = Duration::from_secs(60);

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

/// How often the computer's warmth is read.
const READ_EVERY: Duration = Duration::from_secs(5);

/// A graphics card is too hot from here on, or from a little below the temperature at which it
/// starts slowing itself down, whichever is lower.
const GRAPHICS_HOT: u32 = 80;
/// How far below its own slowdown temperature a card counts as too hot.
const BELOW_SLOWDOWN: u32 = 8;
/// How far below "too hot" a card counts as warm, and Gracil does less at once.
const WARM_BELOW_HOT: u32 = 5;
/// How far below "too hot" a card must come back down before work carries on.
const COOL_BELOW_HOT: u32 = 10;
/// The processor counts as held back when it is allowed this much less than usual.
const HELD_BACK_BY: f64 = 10.0;
/// Readings in a row the processor must be held back for, so a moment's dip is not mistaken for
/// heat.
const HELD_BACK_READINGS: u8 = 2;
/// Once too hot, Gracil rests for at least this long, so it doesn't stop and start every few
/// seconds.
const SHORTEST_REST: Duration = Duration::from_secs(30);

/// How warm the computer is running, for Gracil's purposes (ADR 0043).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Warmth {
    /// Nothing to worry about.
    Cool,
    /// Getting warm: fewer images at once, as when the person is using the computer.
    Warm,
    /// Too hot: everything waits until the computer has cooled down.
    Hot,
}

/// Turns readings into [`Warmth`], remembering what it needs between them.
#[derive(Debug, Default)]
pub struct Warmer {
    /// Too hot since this reading, while it lasts.
    hot_since: Option<Instant>,
    /// The highest the processor's limit has been seen at: its usual value.
    usual_limit: Option<f64>,
    /// Readings in a row with the processor held back.
    held_back: u8,
}

impl Warmer {
    /// Takes in a reading made at `now`.
    pub fn read(&mut self, heat: &Heat, now: Instant) -> Warmth {
        let hot_at = |card: &GraphicsHeat| {
            card.slowdown.map_or(GRAPHICS_HOT, |slowdown| {
                GRAPHICS_HOT.min(slowdown.saturating_sub(BELOW_SLOWDOWN))
            })
        };
        let too_hot = heat
            .graphics
            .iter()
            .any(|card| card.held_back || card.celsius >= hot_at(card));
        let warm = heat
            .graphics
            .iter()
            .any(|card| card.celsius + WARM_BELOW_HOT >= hot_at(card));
        let cooled = heat
            .graphics
            .iter()
            .all(|card| !card.held_back && card.celsius + COOL_BELOW_HOT <= hot_at(card));

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
        let processor_hot = self.held_back >= HELD_BACK_READINGS;

        match self.hot_since {
            None if too_hot || processor_hot => {
                self.hot_since = Some(now);
                Warmth::Hot
            }
            Some(since)
                if !cooled || processor_held_back || now.duration_since(since) < SHORTEST_REST =>
            {
                Warmth::Hot
            }
            _ => {
                self.hot_since = None;
                if warm { Warmth::Warm } else { Warmth::Cool }
            }
        }
    }
}

/// Reads the computer's warmth every few seconds on a thread of its own, for as long as Gracil
/// runs, and hands each change to `changed`, starting with the first reading.
pub fn watch(changed: impl Fn(Warmth) + Send + 'static) {
    let spawned = std::thread::Builder::new()
        .name("warmth".to_owned())
        .spawn(move || {
            let mut thermometer = Thermometer::new();
            let mut warmer = Warmer::default();
            let mut last = None;
            loop {
                let heat = thermometer.read();
                let warmth = warmer.read(&heat, Instant::now());
                if last != Some(warmth) {
                    info!(?warmth, ?heat, "warmth");
                    changed(warmth);
                    last = Some(warmth);
                }
                std::thread::sleep(READ_EVERY);
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
    fn a_hot_card_rests_until_it_has_cooled() {
        let start = Instant::now();
        let at = |seconds| start + Duration::from_secs(seconds);
        let mut warmer = Warmer::default();
        assert_eq!(warmer.read(&card(60), at(0)), Warmth::Cool);
        assert_eq!(warmer.read(&card(76), at(5)), Warmth::Warm);
        assert_eq!(warmer.read(&card(80), at(10)), Warmth::Hot);
        // Cooler, but not yet cool enough.
        assert_eq!(warmer.read(&card(75), at(45)), Warmth::Hot);
        // Cool enough, but it rests at least a while.
        let mut quick = Warmer::default();
        assert_eq!(quick.read(&card(85), at(0)), Warmth::Hot);
        assert_eq!(quick.read(&card(50), at(10)), Warmth::Hot);
        assert_eq!(quick.read(&card(50), at(40)), Warmth::Cool);
        assert_eq!(warmer.read(&card(70), at(50)), Warmth::Cool);
    }

    #[test]
    fn a_card_that_slows_down_early_is_hot_sooner() {
        let mut warmer = Warmer::default();
        let mut heat = card(75);
        heat.graphics[0].slowdown = Some(82);
        assert_eq!(warmer.read(&heat, Instant::now()), Warmth::Hot);
    }

    #[test]
    fn a_card_slowing_itself_down_is_hot() {
        let mut warmer = Warmer::default();
        let mut heat = card(60);
        heat.graphics[0].held_back = true;
        assert_eq!(warmer.read(&heat, Instant::now()), Warmth::Hot);
    }

    #[test]
    fn a_processor_held_back_for_a_while_is_hot() {
        let start = Instant::now();
        let at = |seconds| start + Duration::from_secs(seconds);
        let processor = |limit| Heat {
            graphics: Vec::new(),
            processor_limit: Some(limit),
        };
        let mut warmer = Warmer::default();
        assert_eq!(warmer.read(&processor(100.0), at(0)), Warmth::Cool);
        // A moment's dip is not heat.
        assert_eq!(warmer.read(&processor(70.0), at(5)), Warmth::Cool);
        assert_eq!(warmer.read(&processor(100.0), at(10)), Warmth::Cool);
        assert_eq!(warmer.read(&processor(70.0), at(15)), Warmth::Cool);
        assert_eq!(warmer.read(&processor(70.0), at(20)), Warmth::Hot);
        assert_eq!(warmer.read(&processor(100.0), at(60)), Warmth::Cool);
    }

    #[test]
    fn a_processor_always_limited_is_not_hot() {
        let mut warmer = Warmer::default();
        let always = Heat {
            graphics: Vec::new(),
            processor_limit: Some(80.0),
        };
        for _ in 0..5 {
            assert_eq!(warmer.read(&always, Instant::now()), Warmth::Cool);
        }
    }

    #[test]
    fn nothing_to_read_is_cool() {
        let mut warmer = Warmer::default();
        assert_eq!(warmer.read(&Heat::default(), Instant::now()), Warmth::Cool);
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
