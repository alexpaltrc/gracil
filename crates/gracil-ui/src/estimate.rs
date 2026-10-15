//! A quiet estimate of the time left in a round of work (ADR 0069): coarse, rounded, and never
//! creeping up.
//!
//! It is worked out from the work itself, not from the count of files: one video can take longer
//! than a thousand photos. Each file weighs its size in bytes, and each kind (videos, images) has
//! its own pace, measured from what this round has done so far, the file being compressed
//! included, partway. The two kinds are worked on side by side (ADR 0040: one video compresses
//! while the next gets ready, and images go beside it), so the time left is the longer of the two.
//! Measuring the pace over the whole time a kind is under way takes all of that in: getting the
//! next video ready alongside, the images sharing the computer, resting while it runs warm.
//!
//! Each reading costs the same however long the list: it is fed counts kept up to date as the list
//! changes (see `app::summary`), never the list itself.

use std::time::{Duration, Instant};

use gracil_core::queue::JobId;

/// How often the pace of each kind is measured, at most.
const SAMPLE: Duration = Duration::from_secs(10);

/// How much each new measure of the pace counts against those before it: about the last minute
/// matters most.
const SMOOTHING: f64 = 0.2;

/// Measures of the pace a kind needs before its time left is trusted.
const SAMPLES_NEEDED: u32 = 3;

/// Files done in the round before anything is shown.
const FILES_NEEDED: usize = 3;

/// Nothing is shown with this little left: the end is in sight anyway.
const HIDDEN_BELOW: f64 = 2.0;

/// Once hidden near the end, it comes back only with at least this much left.
const BACK_FROM: f64 = 5.0;

/// The shown time may go up only when the new estimate is at least this much larger, as well as
/// at least the next step up.
const GOING_UP: f64 = 1.25;

/// The time left as it is shown, rounded: about 5, 10, 20, 30 or 45 minutes, then whole hours.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum TimeLeft {
    Minutes(u32),
    Hours(u32),
}

impl TimeLeft {
    /// The step for `minutes` left: the nearest of 5, 10, 20, 30 and 45 minutes, or the nearest
    /// whole hour from an hour on.
    pub fn rounded(minutes: f64) -> Self {
        match minutes {
            m if m < 7.5 => Self::Minutes(5),
            m if m < 15.0 => Self::Minutes(10),
            m if m < 25.0 => Self::Minutes(20),
            m if m < 37.5 => Self::Minutes(30),
            m if m < 52.5 => Self::Minutes(45),
            m if m < 90.0 => Self::Hours(1),
            m => Self::Hours((m / 60.0).round() as u32),
        }
    }

    fn minutes(self) -> f64 {
        match self {
            Self::Minutes(minutes) => f64::from(minutes),
            Self::Hours(hours) => f64::from(hours) * 60.0,
        }
    }

    /// The next step up.
    fn next(self) -> Self {
        match self {
            Self::Minutes(5) => Self::Minutes(10),
            Self::Minutes(10) => Self::Minutes(20),
            Self::Minutes(20) => Self::Minutes(30),
            Self::Minutes(30) => Self::Minutes(45),
            Self::Minutes(_) => Self::Hours(1),
            Self::Hours(hours) => Self::Hours(hours + 1),
        }
    }

    /// What to show next, with `shown` shown now and `minutes` estimated: it holds or goes down
    /// freely, but goes up only by a clear margin, at least a whole step and a quarter more, so it
    /// never creeps up in small steps.
    pub fn after(shown: Option<Self>, minutes: f64) -> Self {
        let step = Self::rounded(minutes);
        match shown {
            Some(shown) if step > shown => {
                let clearly_more =
                    minutes >= shown.next().minutes() && minutes >= shown.minutes() * GOING_UP;
                if clearly_more { step } else { shown }
            }
            _ => step,
        }
    }
}

/// One kind's work in the round, in bytes: all of it, and what is done, the file being compressed
/// counted partway.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Work {
    pub total: f64,
    pub done: f64,
}

impl Work {
    fn left(self) -> f64 {
        (self.total - self.done).max(0.0)
    }
}

/// How the round stands, at one moment.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Reading {
    /// The round, by its first file; `None` with nothing left to do.
    pub round: Option<JobId>,
    /// Videos, then images.
    pub work: [Work; 2],
    /// Files done in the round.
    pub files_done: usize,
    /// Work is going on: not paused, not resting, not waiting for the laptop to be plugged in.
    pub working: bool,
    /// More files may still come, as folders are looked through.
    pub gathering: bool,
}

/// The pace of one kind, in bytes a second.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
struct Pace {
    /// When the measure under way started, and how much was done then.
    since: Option<(Instant, f64)>,
    /// The pace so far, smoothed.
    smoothed: Option<f64>,
    samples: u32,
}

impl Pace {
    fn follow(&mut self, now: Instant, work: Work, working: bool) {
        // Only while this kind is being worked on: time paused, resting or with nothing of this
        // kind left doesn't count.
        if !working || work.left() <= 0.0 {
            self.since = None;
            return;
        }
        let Some((since, done_then)) = self.since else {
            self.since = Some((now, work.done));
            return;
        };
        let took = now.duration_since(since);
        if took < SAMPLE {
            return;
        }
        self.since = Some((now, work.done));
        // Files taken off the list take their work with them: that measure is let go.
        if work.done < done_then {
            return;
        }
        let pace = (work.done - done_then) / took.as_secs_f64();
        self.smoothed = Some(match self.smoothed {
            Some(before) => before + SMOOTHING * (pace - before),
            None => pace,
        });
        self.samples += 1;
    }

    /// Seconds left for `work`, once the pace is known well enough.
    fn seconds_left(&self, work: Work) -> Option<f64> {
        let pace = self.smoothed.filter(|&pace| pace > 0.0)?;
        (self.samples >= SAMPLES_NEEDED).then(|| work.left() / pace)
    }
}

/// The time left in the round, followed as the work goes on (ADR 0069).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Estimate {
    round: Option<JobId>,
    paces: [Pace; 2],
    /// What was shown last, kept while it is hidden for a moment (paused, say).
    shown: Option<TimeLeft>,
    /// Hidden because little is left: it comes back only with clearly more.
    ending: bool,
    /// What to show now, if anything.
    now: Option<TimeLeft>,
}

impl Estimate {
    /// Takes in how the round stands at `now`.
    pub fn follow(&mut self, now: Instant, reading: &Reading) {
        if reading.round != self.round || reading.round.is_none() {
            *self = Self {
                round: reading.round,
                ..Self::default()
            };
        }
        for (pace, work) in self.paces.iter_mut().zip(reading.work) {
            pace.follow(now, work, reading.working);
        }
        self.now = self.shown_for(reading);
    }

    /// The time left to show, if any.
    pub fn time_left(&self) -> Option<TimeLeft> {
        self.now
    }

    fn shown_for(&mut self, reading: &Reading) -> Option<TimeLeft> {
        if !reading.working || reading.gathering || reading.files_done < FILES_NEEDED {
            return None;
        }
        // The longer of the two kinds, since they are worked on side by side; a kind with work
        // left whose pace isn't known yet keeps it hidden.
        let mut seconds = 0.0_f64;
        for (pace, work) in self.paces.iter().zip(reading.work) {
            if work.left() > 0.0 {
                seconds = seconds.max(pace.seconds_left(work)?);
            }
        }
        let minutes = seconds / 60.0;
        if minutes <= HIDDEN_BELOW || (self.ending && minutes < BACK_FROM) {
            self.ending = true;
            self.shown = None;
            return None;
        }
        self.ending = false;
        let shown = TimeLeft::after(self.shown, minutes);
        self.shown = Some(shown);
        Some(shown)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_time_left_is_rounded_coarsely() {
        let rounded = |minutes| TimeLeft::rounded(minutes);
        assert_eq!(rounded(2.5), TimeLeft::Minutes(5));
        assert_eq!(rounded(7.4), TimeLeft::Minutes(5));
        assert_eq!(rounded(7.5), TimeLeft::Minutes(10));
        assert_eq!(rounded(18.0), TimeLeft::Minutes(20));
        assert_eq!(rounded(26.0), TimeLeft::Minutes(30));
        assert_eq!(rounded(40.0), TimeLeft::Minutes(45));
        assert_eq!(rounded(55.0), TimeLeft::Hours(1));
        assert_eq!(rounded(89.0), TimeLeft::Hours(1));
        assert_eq!(rounded(90.0), TimeLeft::Hours(2));
        assert_eq!(rounded(200.0), TimeLeft::Hours(3));
    }

    #[test]
    fn it_goes_down_freely_but_up_only_by_a_clear_margin() {
        let twenty = Some(TimeLeft::Minutes(20));
        // Down at once.
        assert_eq!(TimeLeft::after(twenty, 12.0), TimeLeft::Minutes(10));
        // A little more holds what is shown, even past the next step's boundary.
        assert_eq!(TimeLeft::after(twenty, 26.0), TimeLeft::Minutes(20));
        assert_eq!(TimeLeft::after(twenty, 29.0), TimeLeft::Minutes(20));
        // A whole step and a quarter more goes up.
        assert_eq!(TimeLeft::after(twenty, 30.0), TimeLeft::Minutes(30));
        assert_eq!(TimeLeft::after(twenty, 50.0), TimeLeft::Minutes(45));
        // From five minutes, ten are needed; from an hour, two.
        let five = Some(TimeLeft::Minutes(5));
        assert_eq!(TimeLeft::after(five, 9.0), TimeLeft::Minutes(5));
        assert_eq!(TimeLeft::after(five, 10.0), TimeLeft::Minutes(10));
        let hour = Some(TimeLeft::Hours(1));
        assert_eq!(TimeLeft::after(hour, 100.0), TimeLeft::Hours(1));
        assert_eq!(TimeLeft::after(hour, 120.0), TimeLeft::Hours(2));
        assert_eq!(TimeLeft::after(None, 26.0), TimeLeft::Minutes(30));
    }

    /// Two files' ids, to stand for rounds.
    fn ids() -> [JobId; 2] {
        let mut queue = gracil_core::queue::Queue::default();
        queue.add([r"C:\V\a.mp4", r"C:\V\b.mp4"].map(std::path::PathBuf::from));
        [queue.jobs()[0].id, queue.jobs()[1].id]
    }

    /// A round of `total` bytes of video, read every second.
    struct Round {
        estimate: Estimate,
        at: Instant,
        reading: Reading,
    }

    impl Round {
        fn new(total: f64) -> Self {
            Self {
                estimate: Estimate::default(),
                at: Instant::now(),
                reading: Reading {
                    round: Some(ids()[0]),
                    work: [Work { total, done: 0.0 }, Work::default()],
                    files_done: 3,
                    working: true,
                    gathering: false,
                },
            }
        }

        /// Works on for `seconds` at `pace`, returning what is shown at the end.
        fn go(&mut self, seconds: u32, pace: f64) -> Option<TimeLeft> {
            for _ in 0..seconds {
                self.at += Duration::from_secs(1);
                if self.reading.working {
                    let video = &mut self.reading.work[0];
                    video.done = (video.done + pace).min(video.total);
                }
                self.estimate.follow(self.at, &self.reading);
            }
            self.estimate.time_left()
        }
    }

    #[test]
    fn it_shows_once_the_pace_is_known() {
        // An hour's work at 1000 bytes a second.
        let mut round = Round::new(3_600_000.0);
        assert_eq!(round.go(20, 1000.0), None, "too few measures yet");
        assert_eq!(round.go(30, 1000.0), Some(TimeLeft::Hours(1)));
        // Fewer than three files done: nothing yet.
        let mut round = Round::new(3_600_000.0);
        round.reading.files_done = 2;
        assert_eq!(round.go(60, 1000.0), None);
    }

    #[test]
    fn it_hides_while_paused_or_looking() {
        let mut round = Round::new(3_600_000.0);
        assert_eq!(round.go(60, 1000.0), Some(TimeLeft::Hours(1)));
        round.reading.working = false;
        assert_eq!(round.go(600, 1000.0), None, "paused or resting");
        round.reading.working = true;
        assert_eq!(
            round.go(1, 1000.0),
            Some(TimeLeft::Hours(1)),
            "paused time didn't slow the pace"
        );
        round.reading.gathering = true;
        assert_eq!(round.go(1, 1000.0), None, "more files may come");
        round.reading.gathering = false;
        assert_eq!(round.go(1, 1000.0), Some(TimeLeft::Hours(1)));
        // A new round starts afresh.
        round.reading.round = Some(ids()[1]);
        assert_eq!(round.go(1, 1000.0), None);
    }

    #[test]
    fn it_hides_near_the_end_and_comes_back_only_with_clearly_more() {
        // Under seven minutes' work.
        let mut round = Round::new(400_000.0);
        assert_eq!(round.go(40, 1000.0), Some(TimeLeft::Minutes(5)));
        assert_eq!(round.go(250, 1000.0), None, "under two minutes left");
        // A few files more, under five minutes in all: still hidden.
        round.reading.work[0].total += 150_000.0;
        assert_eq!(round.go(1, 1000.0), None);
        // Clearly more.
        round.reading.work[0].total += 200_000.0;
        assert_eq!(round.go(1, 1000.0), Some(TimeLeft::Minutes(10)));
    }

    #[test]
    fn it_never_creeps_up() {
        let mut round = Round::new(3_000_000.0);
        // 50 minutes at 1000 bytes a second, then a little slower: the estimate grows, but
        // what is shown never goes up by less than a step and a quarter.
        let mut shown = round.go(60, 1000.0).unwrap();
        assert_eq!(shown, TimeLeft::Minutes(45));
        for _ in 0..30 {
            let now = round.go(10, 850.0).unwrap();
            assert!(now <= shown, "{now:?} after {shown:?}");
            shown = now;
        }
    }

    #[test]
    fn the_longer_kind_decides() {
        let mut estimate = Estimate::default();
        let at = Instant::now();
        let mut reading = Reading {
            round: Some(ids()[0]),
            work: [
                Work {
                    total: 1_200_000.0,
                    done: 0.0,
                },
                Work {
                    total: 1_000_000.0,
                    done: 0.0,
                },
            ],
            files_done: 10,
            working: true,
            gathering: false,
        };
        for second in 1..=40_u32 {
            // Videos at 1000 bytes a second (20 minutes), images at 10000 (under 2).
            reading.work[0].done = 1000.0 * f64::from(second);
            reading.work[1].done = 10_000.0 * f64::from(second);
            estimate.follow(at + Duration::from_secs(second.into()), &reading);
        }
        assert_eq!(estimate.time_left(), Some(TimeLeft::Minutes(20)));
    }
}
