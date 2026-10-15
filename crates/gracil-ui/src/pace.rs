//! How much Gracil takes on at once (spec §77.3, ADR 0040): fast, but never alarming. One video
//! compresses while the next one gets ready; images compress beside it, more of them while the
//! computer is otherwise idle and fewer while the person is using it, and fewer when they are
//! large, so memory never runs short. When the computer runs warm it does fewer images at once
//! and takes short, unnoticed rests, eased up and down to hold the graphics card at a steady
//! temperature, so it never gets hot (ADR 0045, 0046). Windows itself holds Gracil to part of the
//! processor, a smaller part while the person works and on battery (ADR 0049).
//!
//! The computer is only read while there is something to do, and only as much as that needs; a
//! laptop's battery running low stops new work, then all of it, until it is plugged in, and work
//! rests while the computer sleeps (ADR 0063).

use std::sync::{Condvar, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use gracil_core::share;
use gracil_windows::{Card, GraphicsHeat, Heat, Power, Thermometer};
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
    /// The computer has a battery (ADR 0063).
    pub has_battery: bool,
}

impl Room {
    /// Looks at the computer now. Whether it is idle, warm or saving energy comes from the last
    /// breath, read at most a second ago.
    pub fn now(video_on_processor: bool, breath: Breath) -> Self {
        Self {
            threads: share::processor_threads(),
            idle: breath.idle,
            video_on_processor,
            free_memory: gracil_windows::free_memory(),
            warm: breath.warm,
            saving_energy: breath.saving_energy,
            has_battery: breath.has_battery,
        }
    }

    /// How many images may compress at once: a quarter of the processor while the computer is
    /// idle; while the person is using it, a sixth on a desktop computer plugged in (one without
    /// a battery, ADR 0065) and an eighth on a laptop; an eighth too whenever it runs warm or
    /// saves energy; one fewer while a video takes the processor, and always at least one. Each
    /// image's tools use several threads of their own, within Gracil's share of the processor
    /// (ADR 0049).
    pub fn images_at_once(self) -> usize {
        let calm = !self.warm && !self.saving_energy;
        let share = if self.idle && calm {
            self.threads / 4
        } else if calm && !self.has_battery {
            self.threads / 6
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

/// Computers with this much memory or less never get the next video ready while one compresses
/// (ADR 0063): the two together could fill it.
const SMALL_MEMORY: u64 = 8 << 30;

/// Memory getting a video ready takes, at most, while another compresses: a 4K picture's frames
/// in flight in the quality measure and its encoder.
const VIDEO_PREPARING_MEMORY: u64 = 2 << 30;

/// Whether the next video may get ready while one compresses (ADR 0063): only with well over the
/// memory it takes free, twice as much, and never on a computer with little memory. Unknown
/// memory counts as enough, as before.
pub fn room_to_prepare_beside(total: Option<u64>, free: Option<u64>) -> bool {
    total.is_none_or(|total| total > SMALL_MEMORY)
        && free.is_none_or(|free| free >= 2 * VIDEO_PREPARING_MEMORY)
}

/// How many of the processor's `threads` Gracil and the tools it starts may run on at all
/// (ADR 0049): three quarters while the computer is idle, half while the person is using it or
/// it has a battery (a laptop's cooling is modest, ADR 0063), and a quarter while it runs on its
/// battery or saves energy. Windows holds Gracil to it, so however the work is split, the rest of
/// the processor stays free and cool.
pub fn processors(threads: usize, idle: bool, saving_energy: bool, has_battery: bool) -> usize {
    let share = if saving_energy {
        threads / 4
    } else if idle && !has_battery {
        threads * 3 / 4
    } else {
        threads / 2
    };
    share.max(1)
}

/// Whether the tools run in Windows' efficiency mode (ADR 0051): while the person uses the
/// computer, it saves energy or it has a battery at all (ADR 0063), so they keep to the
/// efficient cores and lower clock speeds and the computer stays cool and quiet. Left to Windows
/// while a computer without a battery is idle.
pub fn efficiency_mode(idle: bool, saving_energy: bool, has_battery: bool) -> bool {
    !idle || saving_energy || has_battery
}

/// Whether the computer counts as idle (ADR 0063): untouched for a while, nothing on it that
/// doesn't need the keyboard or mouse (a film, a game, a presentation, a call), and its warmth
/// readable. A computer whose warmth can't be read at all is treated as in use: Gracil couldn't
/// tell if it were getting hot.
pub fn idle(untouched: bool, busy: bool, heat: &Heat) -> bool {
    let blind = heat.graphics.is_empty() && heat.processor_limit.is_none();
    untouched && !busy && !blind
}

/// How full a laptop's battery is, as far as the work goes (ADR 0063).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum Battery {
    /// Plugged in, no battery, or enough left.
    #[default]
    Fine,
    /// Low: the files being worked on finish, and no new one starts.
    Low,
    /// Very low: everything waits.
    VeryLow,
}

/// Below this many percent, on battery, no new file starts.
const LOW_BATTERY: u8 = 30;
/// Below this many percent, on battery, everything waits.
const VERY_LOW_BATTERY: u8 = 15;

/// The battery now, after `last`. Once low it stays low until the computer is plugged in, so a
/// battery that recovers a point or two doesn't start and stop the work.
pub fn battery(last: Battery, power: Power) -> Battery {
    if !power.on_battery {
        return Battery::Fine;
    }
    let now = match power.battery_percent {
        Some(percent) if percent < VERY_LOW_BATTERY => Battery::VeryLow,
        Some(percent) if percent < LOW_BATTERY => Battery::Low,
        _ => Battery::Fine,
    };
    now.max(last)
}

/// How often Gracil writes down how much memory it holds, so a long run can be checked for
/// memory it never gives back.
const MEMORY_LOG_EVERY: Duration = Duration::from_secs(10 * 60);

/// How long one breath lasts: a rest, then work (ADR 0045, 0046). Short enough that the parts
/// stay at an even temperature through it, the way a dimmer flickers too fast to see.
const BREATH: Duration = Duration::from_secs(1);
/// A rest shorter than this isn't worth taking: it would only stop and start the work.
const SHORTEST_REST: Duration = Duration::from_millis(50);
/// How often the battery is read while files wait to start.
const WAITING_EVERY: Duration = Duration::from_secs(5);
/// How long the work keeps resting after the computer wakes up, while its drives and graphics
/// card come back.
const SETTLE: Duration = Duration::from_secs(10);

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
/// The processor's usual limit, until it is seen otherwise: its full speed (ADR 0063).
const FULL_LIMIT: f64 = 100.0;
/// Readings in a row the processor's limit must hold within [`STEADY_WITHIN`] of the same value
/// to be taken as its usual one: some laptops always hold the processor back a little, and that
/// is not heat.
const STEADY_READINGS: u32 = 60;
const STEADY_WITHIN: f64 = 1.0;

/// Turns readings into the share of each breath Gracil works, remembering what it needs between
/// them (ADR 0046).
#[derive(Debug)]
pub struct Warmer {
    /// The share of each breath worked now.
    work: f64,
    /// The processor's usual limit: full speed, or what it has held steady at for a while.
    usual_limit: f64,
    /// The limit the processor holds steady at, and for how many readings.
    steady: Option<(f64, u32)>,
    /// Readings in a row with the processor held back.
    held_back: u8,
}

impl Default for Warmer {
    fn default() -> Self {
        Self {
            work: 1.0,
            usual_limit: FULL_LIMIT,
            steady: None,
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
                let steady = match self.steady {
                    Some((at, readings)) if (limit - at).abs() <= STEADY_WITHIN => {
                        (at, readings + 1)
                    }
                    _ => (limit, 1),
                };
                self.steady = Some(steady);
                if steady.1 >= STEADY_READINGS {
                    self.usual_limit = steady.0;
                }
                self.usual_limit = self.usual_limit.max(limit);
                limit <= self.usual_limit - HELD_BACK_BY
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Breath {
    /// The computer is running warm: fewer images at once.
    pub warm: bool,
    /// Resting: everything being worked on is frozen for a moment, or, while the computer
    /// sleeps or its battery is very low, for longer.
    pub resting: bool,
    /// The person isn't using the computer.
    pub idle: bool,
    /// The computer runs on its battery, or Windows is saving energy (ADR 0049).
    pub saving_energy: bool,
    /// The computer has a battery: a laptop, whose cooling is modest (ADR 0063).
    pub has_battery: bool,
    /// How full a laptop's battery is (ADR 0063).
    pub battery: Battery,
}

/// What Gracil is doing, as the window tells [`working`]. The computer is read only as often as
/// that needs (ADR 0063).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Activity {
    /// Nothing to do: nothing is read, except now and then how much memory Gracil holds.
    #[default]
    Nothing,
    /// Files wait to start: only the battery is read, every few seconds.
    Waiting,
    /// Files are being worked on: everything is read every breath. `card` is the graphics card
    /// a tool works on, if any; its warmth is the one watched.
    Working { card: Option<Card> },
}

/// What [`watch`] is told from outside its thread.
#[derive(Debug, Clone, Copy)]
struct Told {
    activity: Activity,
    /// The computer is going to sleep, or asleep.
    asleep: bool,
    /// When it last woke up.
    woke: Option<Instant>,
    /// Something changed since [`watch`] last looked.
    news: bool,
}

static TOLD: Mutex<Told> = Mutex::new(Told {
    activity: Activity::Nothing,
    asleep: false,
    woke: None,
    news: false,
});
static NEWS: Condvar = Condvar::new();

fn told() -> MutexGuard<'static, Told> {
    TOLD.lock().unwrap_or_else(PoisonError::into_inner)
}

fn tell_watch(change: impl FnOnce(&mut Told)) {
    let mut told = told();
    change(&mut told);
    told.news = true;
    NEWS.notify_all();
}

/// Tells [`watch`] what Gracil is doing now. Cheap when nothing changed.
pub fn working(activity: Activity) {
    if told().activity != activity {
        tell_watch(|told| told.activity = activity);
    }
}

/// Tells [`watch`] the computer is going to sleep (`true`) or has woken up. Called on a thread of
/// Windows', which must not wait: it only notes it.
pub fn sleeping(asleep: bool) {
    if asleep {
        share::note_sleep();
    } else {
        share::note_wake();
    }
    tell_watch(|told| {
        told.asleep = asleep;
        if !asleep {
            told.woke = Some(Instant::now());
        }
    });
}

/// Waits `timeout`, or less if [`working`] or [`sleeping`] has news.
fn wait(timeout: Duration) {
    let told = told();
    let _ = NEWS
        .wait_timeout_while(told, timeout, |told| !told.news)
        .map_err(PoisonError::into_inner);
}

/// Reads the computer on a thread of its own, for as long as Gracil runs: every breath while
/// files are worked on, the battery alone every few seconds while files wait, and nothing while
/// there is nothing to do. While the computer is warm, each breath is a short rest followed by
/// work, the rest longer the warmer it is; `changed` hears every change, starting with the
/// first.
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
            let threads = gracil_windows::usable_threads().unwrap_or_else(share::processor_threads);
            let mut allowed = None;
            let mut efficient = None;
            let mut memory_logged: Option<Instant> = None;
            let mut low = Battery::Fine;
            let mut breath = Breath::default();
            loop {
                let now = {
                    let mut told = told();
                    told.news = false;
                    *told
                };
                if memory_logged.is_none_or(|at| at.elapsed() >= MEMORY_LOG_EVERY) {
                    info!(bytes = gracil_windows::own_memory(), "memory held");
                    memory_logged = Some(Instant::now());
                }
                let settling = now.asleep || now.woke.is_some_and(|at| at.elapsed() < SETTLE);
                let card = match now.activity {
                    Activity::Nothing => {
                        tell(Breath {
                            resting: settling,
                            ..breath
                        });
                        wait(MEMORY_LOG_EVERY);
                        continue;
                    }
                    Activity::Waiting => {
                        let power = gracil_windows::power();
                        let battery = battery(low, power);
                        if battery != low {
                            info!(?battery, percent = power.battery_percent, "battery");
                            low = battery;
                        }
                        breath.battery = battery;
                        breath.saving_energy = power.saving_energy;
                        breath.has_battery = power.has_battery;
                        tell(Breath {
                            resting: settling || battery == Battery::VeryLow,
                            ..breath
                        });
                        wait(WAITING_EVERY);
                        continue;
                    }
                    Activity::Working { card } => card,
                };

                let power = gracil_windows::power();
                let battery = battery(low, power);
                if battery != low {
                    info!(?battery, percent = power.battery_percent, "battery");
                    low = battery;
                }
                let untouched = gracil_windows::idle_for().is_some_and(|idle| idle >= IDLE_AFTER);
                let heat = thermometer.read(card);
                // Only asked once untouched for a while: it is the rarer case.
                let idle = idle(untouched, untouched && gracil_windows::person_busy(), &heat);
                let share_now = processors(threads, idle, power.saving_energy, power.has_battery);
                if allowed != Some(share_now) {
                    share::allow_threads(share_now);
                    match gracil_windows::use_processors(share_now) {
                        Ok(()) => info!(
                            threads = share_now,
                            of = threads,
                            idle,
                            saving_energy = power.saving_energy,
                            has_battery = power.has_battery,
                            "processor share"
                        ),
                        Err(error) => warn!(%error, "the processor share can't be set"),
                    }
                    allowed = Some(share_now);
                }
                // Given to the tools running now when it changes; tools started later are given
                // it as they start.
                let mode = efficiency_mode(idle, power.saving_energy, power.has_battery);
                // The quality measure then takes its samples one at a time (ADR 0065).
                share::go_easy(mode || power.on_battery);
                if efficient != Some(mode) {
                    match gracil_windows::use_efficiency_mode(mode) {
                        Ok(()) => info!(on = mode, "efficiency mode"),
                        Err(error) => warn!(%error, "efficiency mode can't be set"),
                    }
                    efficient = Some(mode);
                }
                let work = warmer.read(&heat, BREATH);
                let step = (work * 20.0).round() as u8;
                if logged != Some(step) {
                    info!(work, ?heat, "warmth");
                    logged = Some(step);
                }
                breath = Breath {
                    warm: work < 1.0,
                    resting: false,
                    idle,
                    saving_energy: power.saving_energy,
                    has_battery: power.has_battery,
                    battery,
                };
                if settling || battery == Battery::VeryLow {
                    tell(Breath {
                        resting: true,
                        ..breath
                    });
                    wait(BREATH);
                    continue;
                }
                let rest = BREATH.mul_f64(1.0 - work);
                if rest >= SHORTEST_REST {
                    tell(Breath {
                        resting: true,
                        ..breath
                    });
                    wait(rest);
                    tell(breath);
                    wait(BREATH - rest);
                } else {
                    tell(breath);
                    wait(BREATH);
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
            has_battery: false,
        }
    }

    #[test]
    fn more_images_while_the_computer_is_idle() {
        assert_eq!(room(16, false, false).images_at_once(), 2);
        assert_eq!(room(16, true, false).images_at_once(), 4);
        assert_eq!(room(32, true, false).images_at_once(), 8);
    }

    #[test]
    fn a_few_more_images_on_a_desktop_in_use() {
        // Twelve threads, as a Ryzen 5 5600G: two images on a desktop plugged in, one on a
        // laptop, one whenever it is warm or saves energy.
        let desktop = room(12, false, false);
        assert_eq!(desktop.images_at_once(), 2);
        let laptop = Room {
            has_battery: true,
            ..desktop
        };
        assert_eq!(laptop.images_at_once(), 1);
        for calmer in [
            Room {
                warm: true,
                ..desktop
            },
            Room {
                saving_energy: true,
                ..desktop
            },
        ] {
            assert_eq!(calmer.images_at_once(), 1);
        }
        assert_eq!(room(24, false, false).images_at_once(), 4);
        assert_eq!(room(12, true, false).images_at_once(), 3);
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

    #[test]
    fn the_next_video_gets_ready_beside_only_with_memory_to_spare() {
        const GB: u64 = 1 << 30;
        assert!(room_to_prepare_beside(Some(32 * GB), Some(16 * GB)));
        assert!(room_to_prepare_beside(None, None));
        assert!(!room_to_prepare_beside(Some(8 * GB), Some(6 * GB)));
        assert!(!room_to_prepare_beside(Some(32 * GB), Some(3 * GB)));
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

    fn processor(limit: f64) -> Heat {
        Heat {
            graphics: Vec::new(),
            processor_limit: Some(limit),
        }
    }

    #[test]
    fn a_processor_held_back_for_a_while_works_least() {
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
    fn a_processor_held_back_from_the_start_is_noticed() {
        // Its full speed is the usual, not the first reading.
        let mut warmer = Warmer::default();
        for _ in 0..HELD_BACK_READINGS {
            warmer.read(&processor(70.0), SECOND);
        }
        assert_eq!(warmer.read(&processor(70.0), SECOND), LEAST_WORK);
    }

    #[test]
    fn a_processor_always_limited_is_not_hot() {
        // Held steady at 80 for a minute, 80 is its usual, and the work comes back.
        let mut warmer = Warmer::default();
        for _ in 0..STEADY_READINGS + 200 {
            warmer.read(&processor(80.0), SECOND);
        }
        assert_eq!(warmer.read(&processor(80.0), SECOND), 1.0);
        // Falling well below that is heat again.
        for _ in 0..HELD_BACK_READINGS {
            warmer.read(&processor(65.0), SECOND);
        }
        assert_eq!(warmer.read(&processor(65.0), SECOND), LEAST_WORK);
    }

    #[test]
    fn nothing_to_read_is_cool_but_never_idle() {
        let mut warmer = Warmer::default();
        assert_eq!(warmer.read(&Heat::default(), SECOND), 1.0);
        // Gracil couldn't tell if it got hot, so it takes what it takes while the person works.
        assert!(!idle(true, false, &Heat::default()));
        assert!(idle(true, false, &processor(100.0)));
        assert!(idle(
            true,
            false,
            &Heat {
                processor_limit: None,
                ..card(50)
            }
        ));
    }

    #[test]
    fn a_film_or_a_game_is_not_idle() {
        assert!(!idle(true, true, &processor(100.0)));
        assert!(!idle(false, false, &processor(100.0)));
    }

    #[test]
    fn less_of_the_processor_while_in_use_and_on_battery() {
        assert_eq!(processors(12, true, false, false), 9);
        assert_eq!(processors(12, false, false, false), 6);
        assert_eq!(processors(12, true, true, true), 3);
        assert_eq!(processors(4, false, true, true), 1);
        assert_eq!(processors(1, true, false, false), 1);
    }

    #[test]
    fn a_laptop_never_takes_more_than_half() {
        assert_eq!(processors(12, true, false, true), 6);
    }

    #[test]
    fn efficiency_mode_unless_idle_plugged_in_without_a_battery() {
        assert!(!efficiency_mode(true, false, false));
        assert!(efficiency_mode(false, false, false));
        assert!(efficiency_mode(true, true, false));
        assert!(efficiency_mode(false, true, false));
        assert!(efficiency_mode(true, false, true));
    }

    #[test]
    fn a_low_battery_stops_new_work_then_all_of_it() {
        let on_battery = |percent| Power {
            saving_energy: true,
            on_battery: true,
            has_battery: true,
            battery_percent: Some(percent),
        };
        assert_eq!(battery(Battery::Fine, on_battery(80)), Battery::Fine);
        assert_eq!(battery(Battery::Fine, on_battery(29)), Battery::Low);
        assert_eq!(battery(Battery::Fine, on_battery(14)), Battery::VeryLow);
        // It stays low while unplugged, even if it recovers a little.
        assert_eq!(battery(Battery::Low, on_battery(31)), Battery::Low);
        assert_eq!(battery(Battery::VeryLow, on_battery(16)), Battery::VeryLow);
        // Plugged in, the work goes on.
        let plugged = Power {
            on_battery: false,
            ..on_battery(10)
        };
        assert_eq!(battery(Battery::VeryLow, plugged), Battery::Fine);
        // Without a level, nothing is held back.
        let unknown = Power {
            battery_percent: None,
            ..on_battery(10)
        };
        assert_eq!(battery(Battery::Fine, unknown), Battery::Fine);
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
