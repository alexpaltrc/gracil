//! What the window and the tray show about the list, worked out from it.

use super::*;

/// How the whole list went, once it is done.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Finished {
    /// The files that came out smaller, added together.
    pub saved: Sizes,
    pub compressed: usize,
    /// Files that were already about as small as they get.
    pub already: usize,
    /// Files that couldn't be compressed for another reason.
    pub failed: usize,
}

/// How far along the whole list is (see [`Gracil::list_progress`]).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ListProgress {
    /// The round of work it belongs to, by its first file.
    pub round: JobId,
    /// Files in the round.
    pub files: usize,
    /// Share of the whole round done, `0.0..=1.0`.
    pub fraction: f64,
}

/// Counts and picks over the whole list, worked out in one pass after each change rather than each
/// time the window is drawn: with thousands of files on the list, going through it again for
/// every frame and every bit of progress kept the window from keeping up.
#[derive(Debug, Clone, Default)]
pub struct Summary {
    /// Files not finished yet: waiting, getting ready or compressing.
    pub left: usize,
    /// Files that haven't started compressing yet, including those already got ready.
    pub waiting_to_compress: usize,
    /// Results waiting for a choice.
    pub waiting_for_choice: usize,
    /// Every file that came out smaller, added together, and how many there are.
    pub total: (Sizes, usize),
    /// How the whole list went, once it is done (see [`Gracil::finished`]).
    pub finished: Option<Finished>,
    /// The file shown as being worked on (see [`Gracil::working`]).
    pub(super) working: Option<JobId>,
    /// The first result waiting for a choice.
    pub(super) first_waiting_for_choice: Option<JobId>,
    /// The next file to compress.
    pub(super) next_to_compress: Option<JobId>,
    /// The last file finished.
    pub(super) last_finished: Option<JobId>,
    /// The first file of this round of work: the files done since Gracil last started working,
    /// and those still to do (ADR 0034, item 13).
    pub(super) round_from: Option<JobId>,
    /// Files in this round, and how many of them are done.
    pub(super) round: usize,
    pub(super) round_done: usize,
    /// Files being compressed, whose progress moves the ring between whole files.
    pub(super) compressing: Vec<JobId>,
}

impl Gracil {
    /// Whether `job` is shown with the ring: while it is worked on, and, while paused, while it
    /// waits, so Resume is always at hand.
    pub fn shows_dial(&self, job: &Job) -> bool {
        job.state.is_active() || (self.paused && job.state.is_waiting())
    }

    /// Files on the list that haven't started compressing yet, including those already got ready.
    pub fn waiting_to_compress(&self) -> usize {
        self.summary.waiting_to_compress
    }

    /// Files on the list not finished yet; it counts down to nothing as the work goes on.
    pub fn left(&self) -> usize {
        self.summary.left
    }

    /// Goes through the whole list once and works out everything the window and the tray show
    /// about it (see [`Summary`]).
    pub(super) fn summarize(&self) -> Summary {
        let mut summary = Summary::default();
        let mut busy = !self.running.is_empty() || self.gathering > 0;
        let mut finished = Finished::default();
        let (mut video_compressing, mut compressing, mut active) = (None, None, None);
        // A round starts when work begins with nothing left to do, so every file before the first
        // one not done belongs to an earlier round.
        let round_from = self.round_from.or_else(|| {
            self.queue
                .jobs()
                .iter()
                .find(|job| !job.state.is_finished())
                .map(|job| job.id)
        });
        for job in self.queue.jobs() {
            if job.state.is_waiting() {
                summary.waiting_to_compress += 1;
                summary.next_to_compress.get_or_insert(job.id);
            } else if job.state.is_compressing() {
                summary.compressing.push(job.id);
                if job.kind == Kind::Video {
                    video_compressing.get_or_insert(job.id);
                }
                compressing.get_or_insert(job.id);
            }
            if job.state.is_active() {
                active.get_or_insert(job.id);
            }
            if round_from.is_some_and(|from| job.id >= from) {
                summary.round += 1;
                summary.round_done += usize::from(job.state.is_finished());
            }
            if job.state.is_finished() {
                summary.last_finished = Some(job.id);
            } else {
                summary.left += 1;
                busy = true;
            }
            let outcome = self.outcomes.get(&job.id);
            if let Some(sizes) = outcome.and_then(Outcome::sizes) {
                summary.total = (summary.total.0 + sizes, summary.total.1 + 1);
            }
            match outcome {
                Some(Outcome::Ready { placing, .. }) => {
                    busy = true;
                    if !placing {
                        summary.waiting_for_choice += 1;
                        summary.first_waiting_for_choice.get_or_insert(job.id);
                    }
                }
                Some(Outcome::Placed { sizes, .. }) => {
                    finished.saved = finished.saved + *sizes;
                    finished.compressed += 1;
                }
                Some(Outcome::Failed(Failure::AsSmallAsItGets | Failure::NotSmaller)) => {
                    finished.already += 1;
                }
                Some(Outcome::Failed(_)) => finished.failed += 1,
                Some(Outcome::Cancelled) | None => {}
            }
        }
        summary.working = video_compressing.or(compressing).or(active);
        // Everything done: the next file starts a fresh round.
        summary.round_from = round_from.filter(|_| summary.left > 0);
        summary.finished = (!busy && finished.compressed + finished.already + finished.failed > 0)
            .then_some(finished);
        summary
    }

    /// How far along the whole list is, for the ring and the tray, when this round of work has
    /// more than one file (ADR 0034, item 13): one equal part per file, and the files being
    /// compressed partway through theirs. Adding files makes the parts smaller, so the marble
    /// moves back a little. While folders are still being looked through, more files may come,
    /// so it counts one more file than it knows of and never fills before the real end.
    pub fn list_progress(&self) -> Option<ListProgress> {
        let summary = &self.summary;
        let round = summary.round_from.filter(|_| summary.round >= 2)?;
        let parts = summary.round + usize::from(self.gathering > 0);
        let partway: f64 = summary
            .compressing
            .iter()
            .filter_map(|&id| self.queue.get(id)?.fraction)
            .map(|fraction| fraction.clamp(0.0, 1.0))
            .sum();
        Some(ListProgress {
            round,
            files: summary.round,
            fraction: ((summary.round_done as f64 + partway) / parts as f64).min(1.0),
        })
    }

    /// How many files are being worked on, so Exit would stop them.
    pub fn being_worked_on(&self) -> usize {
        self.running.len()
    }

    /// The file shown large: the one the user picked in the list, otherwise [`Self::current`].
    pub fn focused(&self) -> Option<&Job> {
        self.selected
            .and_then(|id| self.queue.get(id))
            .or_else(|| self.current())
    }

    /// What Gracil is busy with, shown large unless the user picked another file: the file being
    /// worked on (see [`Self::working`]); otherwise a result waiting for a choice; otherwise the
    /// next one waiting; otherwise the one just put in place; otherwise the last one done.
    pub fn current(&self) -> Option<&Job> {
        let summary = &self.summary;
        self.working()
            .or_else(|| self.job(summary.first_waiting_for_choice))
            .or_else(|| self.job(summary.next_to_compress))
            .or_else(|| self.job(self.last_placed))
            .or_else(|| self.job(summary.last_finished))
    }

    /// Of the files being worked on, the one shown: the video being compressed, which takes
    /// longest; otherwise the first file being compressed; otherwise the first getting ready.
    pub fn working(&self) -> Option<&Job> {
        self.job(self.summary.working)
    }

    pub(super) fn job(&self, id: Option<JobId>) -> Option<&Job> {
        id.and_then(|id| self.queue.get(id))
    }

    /// The file to offer a way back to while the user looks at another one: only when Gracil is
    /// working on it or it waits for a choice (spec §32.1).
    pub fn way_back(&self) -> Option<&Job> {
        let picked = self.selected?;
        self.current()
            .filter(|job| job.id != picked && (job.state.is_active() || self.is_waiting(job.id)))
    }

    /// Every file in the list that came out smaller, added together, and how many there are.
    pub fn total(&self) -> (Sizes, usize) {
        self.summary.total
    }

    /// The whole list is done, when it is (spec §77.3, ADR 0039): nothing is waiting, being worked
    /// on, waiting for a choice or still being looked for, and at least one file finished rather
    /// than being stopped.
    pub fn finished(&self) -> Option<Finished> {
        self.summary.finished
    }

    /// What the mini-window shows: the video being compressed, else the first result waiting for
    /// a choice, else what the big window shows.
    pub fn mini_job(&self) -> Option<&Job> {
        self.job(self.problem_in_mini)
            .or_else(|| self.working())
            .or_else(|| self.job(self.summary.first_waiting_for_choice))
            .or_else(|| self.focused())
    }

    /// What happens to every finished video without asking; `None` asks (ADR 0025).
    pub fn always(&self) -> Option<Choice> {
        self.settings.always
    }

    pub fn waiting_count(&self) -> usize {
        self.summary.waiting_for_choice
    }

    /// Whether the result of `id` is waiting for a choice.
    pub fn is_waiting(&self, id: JobId) -> bool {
        self.outcomes.get(&id).is_some_and(Outcome::is_waiting)
    }

    /// Whether a video can leave the list now: it is not running and not being put in place.
    pub fn can_remove(&self, job: &Job) -> bool {
        job.state.is_removable()
            && !matches!(
                self.outcomes.get(&job.id),
                Some(Outcome::Ready { placing: true, .. })
            )
    }

    /// Whether `id` failed for a reason worth explaining: not merely already small.
    pub(super) fn is_fault(&self, id: JobId) -> bool {
        matches!(
            self.outcomes.get(&id),
            Some(Outcome::Failed(failure))
                if !matches!(failure, Failure::NotSmaller | Failure::AsSmallAsItGets)
        )
    }
}
