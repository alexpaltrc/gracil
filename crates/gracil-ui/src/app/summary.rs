//! What the window and the tray show about the list, worked out from it.

use std::collections::BTreeSet;

use super::*;

/// Lists up to this long have their counts checked against a full count after every change in
/// debug builds; longer ones would make those builds crawl.
const CHECKED_UP_TO: usize = 2048;

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

/// Counts and picks over the whole list, worked out after each change rather than each time the
/// window is drawn: with thousands of files on the list, going through it again for every frame
/// and every bit of progress kept the window from keeping up. Kept up to date from the files that
/// changed alone (see [`Tally`], ADR 0065).
#[derive(Debug, Clone, Default, PartialEq)]
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
    /// Results not put in place yet, chosen for or not.
    pub(super) ready: usize,
    /// Files that can leave the list now (see [`Gracil::can_remove`]).
    pub(super) removable: usize,
    /// Once the list is done with files that went wrong: the most frequent reason, and whether
    /// all of them share it (see [`Gracil::common_fault`]).
    pub(super) common_fault: Option<(Failure, bool)>,
}

/// The results of the files on the list, by file. Remembers which ones changed, so the
/// [`Summary`] is kept up to date from those alone.
#[derive(Debug, Default)]
pub struct Outcomes {
    map: HashMap<JobId, Outcome>,
    touched: Vec<JobId>,
}

impl Outcomes {
    pub fn get(&self, id: &JobId) -> Option<&Outcome> {
        self.map.get(id)
    }

    pub fn values(&self) -> impl Iterator<Item = &Outcome> {
        self.map.values()
    }

    pub(super) fn get_mut(&mut self, id: &JobId) -> Option<&mut Outcome> {
        self.touched.push(*id);
        self.map.get_mut(id)
    }

    pub(super) fn insert(&mut self, id: JobId, outcome: Outcome) {
        self.touched.push(id);
        self.map.insert(id, outcome);
    }

    pub(super) fn remove(&mut self, id: &JobId) -> Option<Outcome> {
        self.touched.push(*id);
        self.map.remove(id)
    }

    fn take_touched(&mut self) -> Vec<JobId> {
        std::mem::take(&mut self.touched)
    }
}

/// What one file adds to the [`Summary`].
#[derive(Debug, Clone, Copy, Default, PartialEq)]
struct Part {
    kind: Option<Kind>,
    state: Option<JobState>,
    in_round: bool,
    /// A result waiting for a choice.
    choosing: bool,
    /// A result not put in place yet.
    ready: bool,
    removable: bool,
    /// Its sizes, when it came out smaller.
    smaller: Option<Sizes>,
    placed: Option<Sizes>,
    already: bool,
    failed: bool,
}

impl Part {
    fn of(job: &Job, outcome: Option<&Outcome>, round_from: Option<JobId>) -> Self {
        Self {
            kind: Some(job.kind),
            state: Some(job.state),
            in_round: round_from.is_some_and(|from| job.id >= from),
            choosing: outcome.is_some_and(Outcome::is_waiting),
            ready: matches!(outcome, Some(Outcome::Ready { .. })),
            removable: can_remove(job, outcome),
            smaller: outcome.and_then(Outcome::sizes),
            placed: match outcome {
                Some(Outcome::Placed { sizes, .. }) => Some(*sizes),
                _ => None,
            },
            already: matches!(
                outcome,
                Some(Outcome::Failed(
                    Failure::AsSmallAsItGets | Failure::NotSmaller
                ))
            ),
            failed: matches!(
                outcome,
                Some(Outcome::Failed(failure))
                    if !matches!(failure, Failure::AsSmallAsItGets | Failure::NotSmaller)
            ),
        }
    }

    fn is(&self, test: fn(JobState) -> bool) -> bool {
        self.state.is_some_and(test)
    }
}

/// Counts that add up over the files.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
struct Counts {
    left: usize,
    waiting: usize,
    choosing: usize,
    ready: usize,
    removable: usize,
    round: usize,
    round_done: usize,
    total: (Sizes, usize),
    finished: Finished,
}

impl Counts {
    /// Adds `part`, or takes it off again.
    fn count(&mut self, part: &Part, add: bool) {
        let step = |count: &mut usize, yes: bool| {
            if yes {
                *count = if add { *count + 1 } else { *count - 1 };
            }
        };
        let sizes = |total: &mut Sizes, sizes: Option<Sizes>| {
            if let Some(sizes) = sizes {
                *total = if add { *total + sizes } else { *total - sizes };
            }
        };
        let finished = part.is(JobState::is_finished);
        step(&mut self.left, part.state.is_some() && !finished);
        step(&mut self.waiting, part.is(JobState::is_waiting));
        step(&mut self.choosing, part.choosing);
        step(&mut self.ready, part.ready);
        step(&mut self.removable, part.removable);
        step(&mut self.round, part.in_round);
        step(&mut self.round_done, part.in_round && finished);
        sizes(&mut self.total.0, part.smaller);
        step(&mut self.total.1, part.smaller.is_some());
        sizes(&mut self.finished.saved, part.placed);
        step(&mut self.finished.compressed, part.placed.is_some());
        step(&mut self.finished.already, part.already);
        step(&mut self.finished.failed, part.failed);
    }
}

/// The [`Summary`]'s counts, kept up to date file by file (ADR 0065): after a change, only the
/// files that changed are counted again, however long the list.
#[derive(Debug, Default, PartialEq)]
pub(super) struct Tally {
    parts: HashMap<JobId, Part>,
    counts: Counts,
    /// The files in each of these situations, in list order, for the first or last of them.
    compressing: BTreeSet<JobId>,
    video_compressing: BTreeSet<JobId>,
    active: BTreeSet<JobId>,
    choosing: BTreeSet<JobId>,
    finished: BTreeSet<JobId>,
    /// The first file of the round the parts were counted for.
    round_from: Option<JobId>,
}

impl Tally {
    /// Counts the whole list again.
    fn of(queue: &Queue, outcomes: &Outcomes, round_from: Option<JobId>) -> Self {
        let mut tally = Self {
            round_from,
            ..Self::default()
        };
        for job in queue.jobs() {
            tally.set(
                job.id,
                Some(Part::of(job, outcomes.get(&job.id), round_from)),
            );
        }
        tally
    }

    /// Counts again the files in `ids`, as they are now.
    fn recount(
        &mut self,
        queue: &Queue,
        outcomes: &Outcomes,
        ids: impl IntoIterator<Item = JobId>,
    ) {
        let round_from = self.round_from;
        for id in ids {
            let part = queue
                .get(id)
                .map(|job| Part::of(job, outcomes.get(&id), round_from));
            self.set(id, part);
        }
    }

    /// Puts `part` in place of what file `id` added before; `None` when it left the list.
    fn set(&mut self, id: JobId, part: Option<Part>) {
        if let Some(old) = self.parts.remove(&id) {
            if part == Some(old) {
                self.parts.insert(id, old);
                return;
            }
            self.counts.count(&old, false);
            for set in [
                &mut self.compressing,
                &mut self.video_compressing,
                &mut self.active,
                &mut self.choosing,
                &mut self.finished,
            ] {
                set.remove(&id);
            }
        }
        let Some(part) = part else { return };
        self.counts.count(&part, true);
        let compressing = part.is(JobState::is_compressing);
        for (set, yes) in [
            (&mut self.compressing, compressing),
            (
                &mut self.video_compressing,
                compressing && part.kind == Some(Kind::Video),
            ),
            (&mut self.active, part.is(JobState::is_active)),
            (&mut self.choosing, part.choosing),
            (&mut self.finished, part.is(JobState::is_finished)),
        ] {
            if yes {
                set.insert(id);
            }
        }
        self.parts.insert(id, part);
    }
}

/// The most frequent reason among `outcomes` that went wrong, not merely already small, and
/// whether every one of them shares it. A tie goes to the reason counted first.
fn common_fault<'a>(outcomes: impl Iterator<Item = &'a Outcome>) -> Option<(Failure, bool)> {
    let mut counts: Vec<(Failure, usize)> = Vec::new();
    for outcome in outcomes {
        let Outcome::Failed(failure) = outcome else {
            continue;
        };
        if matches!(failure, Failure::NotSmaller | Failure::AsSmallAsItGets) {
            continue;
        }
        match counts.iter_mut().find(|(known, _)| known == failure) {
            Some((_, count)) => *count += 1,
            None => counts.push((*failure, 1)),
        }
    }
    let shared_by_all = counts.len() == 1;
    let (failure, _) = counts
        .into_iter()
        .reduce(|most, next| if next.1 > most.1 { next } else { most })?;
    Some((failure, shared_by_all))
}

/// Whether a file can leave the list now: it is not running and not being put in place.
fn can_remove(job: &Job, outcome: Option<&Outcome>) -> bool {
    job.state.is_removable() && !matches!(outcome, Some(Outcome::Ready { placing: true, .. }))
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

    /// Works out everything the window and the tray show about the list (see [`Summary`]),
    /// counting again only the files that changed since the last time (ADR 0065).
    pub(super) fn summarize(&mut self) -> Summary {
        let changed = self.queue.take_changed();
        let touched = self.outcomes.take_touched();
        let whole = changed.is_none();
        let ids = changed.into_iter().flatten().chain(touched);
        self.tally.recount(&self.queue, &self.outcomes, ids);
        // A round starts when work begins with nothing left to do, so every file before the first
        // one not done belongs to an earlier round.
        let next_to_compress = self.queue.first_waiting();
        let first_unfinished = next_to_compress
            .into_iter()
            .chain(self.tally.active.first().copied())
            .min();
        let round_from = self.round_from.or(first_unfinished);
        if whole || round_from != self.tally.round_from {
            self.tally = Tally::of(&self.queue, &self.outcomes, round_from);
        } else if cfg!(debug_assertions) && self.queue.len() <= CHECKED_UP_TO {
            debug_assert_eq!(
                self.tally,
                Tally::of(&self.queue, &self.outcomes, round_from),
                "the summary drifted from the list"
            );
        }

        let tally = &self.tally;
        let counts = tally.counts;
        let busy =
            !self.running.is_empty() || self.gathering > 0 || counts.left > 0 || counts.ready > 0;
        let finished = (!busy
            && counts.finished.compressed + counts.finished.already + counts.finished.failed > 0)
            .then_some(counts.finished);
        // Only once the list is done, when little else changes: it goes through the results.
        let common_fault = finished
            .filter(|finished| finished.failed > 0)
            .and_then(|_| common_fault(self.outcomes.values()));
        Summary {
            left: counts.left,
            waiting_to_compress: counts.waiting,
            waiting_for_choice: counts.choosing,
            total: counts.total,
            finished,
            common_fault,
            working: [&tally.video_compressing, &tally.compressing, &tally.active]
                .into_iter()
                .find_map(|set| set.first().copied()),
            first_waiting_for_choice: tally.choosing.first().copied(),
            next_to_compress,
            last_finished: tally.finished.last().copied(),
            // Everything done: the next file starts a fresh round.
            round_from: round_from.filter(|_| counts.left > 0),
            round: counts.round,
            round_done: counts.round_done,
            compressing: tally.compressing.iter().copied().collect(),
            ready: counts.ready,
            removable: counts.removable,
        }
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

    /// Whether the mini-window shows the finished list, once it is done (ADR 0066): unless it
    /// opened on a file that went wrong, or a file was picked to look at.
    pub fn mini_shows_done(&self) -> bool {
        self.problem_in_mini.is_none() && self.selected.is_none()
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
        can_remove(job, self.outcomes.get(&job.id))
    }

    /// Whether any file can leave the list now, so Clear has something to do.
    pub fn any_removable(&self) -> bool {
        self.summary.removable > 0
    }

    /// Opens the list on a file that went wrong, with what happened: the first one, or, pressed
    /// again, the one after the last shown, round to the first after the last (ADR 0066).
    pub(super) fn show_problem(&mut self) -> Task<Message> {
        let jobs = self.queue.jobs();
        let after = self
            .last_problem
            .and_then(|last| jobs.binary_search_by_key(&last, |job| job.id).ok())
            .map_or(0, |index| index + 1);
        let found = (after..jobs.len())
            .chain(0..after)
            .find(|&index| self.is_fault(jobs[index].id));
        self.selected = found.map(|index| jobs[index].id);
        self.last_problem = self.selected;
        self.drawer_open = true;
        // The list opens on that file, however far down it is.
        match found {
            Some(index) => {
                self.drawer_scroll = self.drawer_scroll.showing(index);
                self.drawer_scroll.restore()
            }
            None => Task::none(),
        }
    }

    /// Whether a file kept only online is being downloaded to be got ready (ADR 0066).
    pub fn is_downloading(&self, id: JobId) -> bool {
        self.downloading.contains(&id)
    }

    /// Whether the finished list went well, so it is shown with the green middle and the check
    /// mark: something came out smaller, and Stop set nothing aside in this round of work. Files
    /// left unfinished from an earlier time don't count against it (ADR 0066).
    pub fn went_well(&self, finished: &Finished) -> bool {
        finished.compressed > 0 && !(self.stopped_in_round && !self.offered.is_empty())
    }

    /// Whether Gracil rests while the computer cools down, has just woken up or its battery is
    /// very low: then nothing in the window breathes, as nothing moves (ADR 0066).
    pub fn is_resting(&self) -> bool {
        self.breath.resting || self.breath.battery == pace::Battery::VeryLow
    }

    /// Why the files that went wrong did, once the list is done (ADR 0066): the most frequent
    /// reason, and whether every one of them shares it.
    pub fn common_fault(&self) -> Option<(Failure, bool)> {
        self.summary.common_fault
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Counts again only what changed, as [`Gracil::summarize`] does.
    fn follow(tally: &mut Tally, queue: &mut Queue, outcomes: &mut Outcomes) {
        let changed = queue.take_changed().unwrap_or_default();
        let ids = changed.into_iter().chain(outcomes.take_touched());
        tally.recount(queue, outcomes, ids);
    }

    fn sizes(original: u64, compressed: u64) -> Sizes {
        Sizes {
            original,
            compressed,
        }
    }

    #[test]
    fn counting_what_changed_matches_counting_everything() {
        let mut queue = Queue::default();
        let mut outcomes = Outcomes::default();
        queue.add((0..30).map(|i| {
            let extension = if i % 4 == 0 { "jpg" } else { "mp4" };
            PathBuf::from(format!(r"C:\V\{i}.{extension}"))
        }));
        let from = queue.jobs()[3].id;
        let mut tally = Tally::of(&queue, &outcomes, Some(from));
        queue.take_changed();
        let check = |tally: &mut Tally, queue: &mut Queue, outcomes: &mut Outcomes| {
            follow(tally, queue, outcomes);
            assert_eq!(*tally, Tally::of(queue, outcomes, Some(from)));
        };

        let mut done = Vec::new();
        for (round, kind) in [Kind::Video, Kind::Image, Kind::Video]
            .into_iter()
            .enumerate()
        {
            let id = queue.start_preparing(kind).unwrap().id;
            check(&mut tally, &mut queue, &mut outcomes);
            queue.apply(id, JobEvent::Ready).unwrap();
            queue.apply(id, JobEvent::BeginCompressing).unwrap();
            check(&mut tally, &mut queue, &mut outcomes);
            queue.apply(id, JobEvent::BeginFinalizing).unwrap();
            queue.apply(id, JobEvent::Complete).unwrap();
            let outcome = match round {
                0 => Outcome::Placed {
                    choice: Choice::Replace,
                    path: PathBuf::from(r"C:\V\done.mp4"),
                    sizes: sizes(100, 40),
                },
                1 => Outcome::Failed(Failure::NotSmaller),
                _ => Outcome::Failed(Failure::CouldNotCompress),
            };
            outcomes.insert(id, outcome);
            check(&mut tally, &mut queue, &mut outcomes);
            done.push(id);
        }
        assert_eq!(tally.counts.finished.compressed, 1);
        assert_eq!(tally.counts.finished.already, 1);
        assert_eq!(tally.counts.finished.failed, 1);
        assert_eq!(tally.counts.total, (sizes(100, 40), 1));
        assert_eq!(tally.finished.last(), done.last());

        // A file leaving the list takes its result with it.
        queue.remove(done[0]);
        outcomes.remove(&done[0]);
        check(&mut tally, &mut queue, &mut outcomes);
        assert_eq!(tally.counts.total, (Sizes::default(), 0));

        let ids: HashSet<JobId> = queue.jobs()[..10].iter().map(|job| job.id).collect();
        let taken = queue.remove_all(&ids);
        check(&mut tally, &mut queue, &mut outcomes);
        queue.restore(taken);
        check(&mut tally, &mut queue, &mut outcomes);
        assert_eq!(tally.counts.removable, queue.len());
    }

    #[test]
    fn the_reason_most_files_failed_for_is_found() {
        let failed = |failure| Outcome::Failed(failure);
        let same = [
            failed(Failure::InUse),
            failed(Failure::NotSmaller),
            failed(Failure::InUse),
        ];
        assert_eq!(common_fault(same.iter()), Some((Failure::InUse, true)));
        let mixed = [
            failed(Failure::Gone),
            failed(Failure::NoRoom),
            failed(Failure::NoRoom),
            failed(Failure::AsSmallAsItGets),
        ];
        assert_eq!(common_fault(mixed.iter()), Some((Failure::NoRoom, false)));
        assert_eq!(common_fault([failed(Failure::NotSmaller)].iter()), None);
    }

    /// The window's counts over thirty thousand files, kept up to date as each file is got ready
    /// and compressed: each change costs about the same however long the list is.
    #[test]
    fn a_huge_list_is_counted_file_by_file() {
        let mut queue = Queue::default();
        let mut outcomes = Outcomes::default();
        queue.add((0..30_000).map(|i| PathBuf::from(format!(r"C:\P\{i}.jpg"))));
        let first = queue.first_waiting();
        let mut tally = Tally::of(&queue, &outcomes, first);
        queue.take_changed();
        let started = Instant::now();
        while let Some(id) = queue.start_preparing(Kind::Image).map(|job| job.id) {
            follow(&mut tally, &mut queue, &mut outcomes);
            for event in [
                JobEvent::Ready,
                JobEvent::BeginCompressing,
                JobEvent::BeginFinalizing,
                JobEvent::Complete,
            ] {
                queue.apply(id, event).unwrap();
            }
            outcomes.insert(id, Outcome::Failed(Failure::NotSmaller));
            follow(&mut tally, &mut queue, &mut outcomes);
        }
        let took = started.elapsed();
        assert_eq!(tally.counts.finished.already, 30_000);
        assert_eq!(tally.counts.round_done, 30_000);
        // Counting the whole list after each change took minutes in all.
        assert!(took < Duration::from_secs(2), "{took:?}");
    }
}
