//! The set of videos and images the user has added, and which ones run next (spec §26–§36).
//!
//! Each file is first got ready, then compressed. Videos and images go side by side, each in the
//! order they were added; how many of each run at once is the app's to decide (ADR 0040). Clear
//! never interrupts the files being worked on. Adding is always additive. A path already in the
//! set is not added twice; the app removes a finished entry first, so a file can always be
//! compressed again (§29).
//!
//! The list can hold tens of thousands of files (spec §77.3), and the app asks about it after
//! every change. So the list keeps how many files of each kind are in each state, and where the
//! first waiting file of each kind may be, and answers those without going through it (ADR 0065).

use std::collections::hash_map::Entry;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use crate::job::{InvalidTransition, JobEvent, JobState};
use crate::media::{self, Kind};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct JobId(u64);

#[derive(Debug, Clone, PartialEq)]
pub struct Job {
    pub id: JobId,
    pub path: PathBuf,
    pub kind: Kind,
    pub state: JobState,
    /// Share of the work done, `0.0..=1.0`, while it is known.
    pub fraction: Option<f64>,
}

impl Job {
    pub fn file_name(&self) -> String {
        self.path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default()
    }
}

const STATES: usize = JobState::ALL.len();

/// Lists up to this long are checked against a full count after every change in debug builds;
/// longer ones would make those builds crawl.
const CHECKED_UP_TO: usize = 2048;

#[derive(Debug)]
pub struct Queue {
    jobs: Vec<Job>,
    /// Each job by its path (see [`key`]), so finding one never goes through the whole list.
    keys: HashMap<String, JobId>,
    next_id: u64,
    /// How many jobs of each kind (see [`kind_slot`]) are in each state.
    counts: [[usize; STATES]; 2],
    /// For each kind, where the first job waiting to be got ready may be: none is before it.
    pending_from: [usize; 2],
    /// For each kind, where the first job got ready and waiting its turn may be: none is before it.
    prepared_from: [usize; 2],
    /// Jobs added, taken off, put back or moved to another state since [`Self::take_changed`]
    /// last asked; `None` once that list would be longer than useful.
    changed: Option<Vec<JobId>>,
}

impl Queue {
    /// Adds every path not already in the set, in order. Returns how many were added.
    pub fn add(&mut self, paths: impl IntoIterator<Item = PathBuf>) -> usize {
        let before = self.jobs.len();
        for path in paths {
            if let Entry::Vacant(slot) = self.keys.entry(key(&path)) {
                self.next_id += 1;
                let id = JobId(self.next_id);
                slot.insert(id);
                let job = Job {
                    id,
                    kind: media::kind_of(&path),
                    path,
                    state: JobState::Pending,
                    fraction: None,
                };
                self.counts[kind_slot(job.kind)][job.state.slot()] += 1;
                self.jobs.push(job);
                self.note(id);
            }
        }
        self.check();
        self.jobs.len() - before
    }

    pub fn jobs(&self) -> &[Job] {
        &self.jobs
    }

    pub fn len(&self) -> usize {
        self.jobs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.jobs.is_empty()
    }

    pub fn get(&self, id: JobId) -> Option<&Job> {
        self.index(id).map(|at| &self.jobs[at])
    }

    /// Where `id` is in the list. Jobs are numbered as they are added and never reordered, so
    /// the list is always sorted by number and found by halving it, however long it grows.
    fn index(&self, id: JobId) -> Option<usize> {
        self.jobs.binary_search_by_key(&id, |job| job.id).ok()
    }

    /// The job for `path`, matched the way adding matches duplicates.
    pub fn find(&self, path: &Path) -> Option<&Job> {
        self.keys.get(&key(path)).and_then(|&id| self.get(id))
    }

    /// How many files of `kind` are in a state `test` accepts. Never goes through the list.
    pub fn count(&self, kind: Kind, test: impl Fn(JobState) -> bool) -> usize {
        let counts = &self.counts[kind_slot(kind)];
        JobState::ALL
            .into_iter()
            .filter(|&state| test(state))
            .map(|state| counts[state.slot()])
            .sum()
    }

    /// How many files of either kind are in a state `test` accepts.
    pub fn count_all(&self, test: impl Fn(JobState) -> bool) -> usize {
        self.count(Kind::Video, &test) + self.count(Kind::Image, &test)
    }

    /// Starts getting the oldest waiting file of `kind` ready.
    pub fn start_preparing(&mut self, kind: Kind) -> Option<&Job> {
        let at = self.first(kind, JobState::Pending)?;
        let next = self.jobs[at].state.apply(JobEvent::Start).ok()?;
        self.set_state(at, next);
        self.check();
        Some(&self.jobs[at])
    }

    /// The oldest file of `kind` that is ready and waiting its turn to compress.
    pub fn next_prepared(&mut self, kind: Kind) -> Option<&Job> {
        let at = self.first(kind, JobState::Prepared)?;
        Some(&self.jobs[at])
    }

    /// The first file, of either kind, that hasn't started compressing yet.
    pub fn first_waiting(&mut self) -> Option<JobId> {
        [Kind::Video, Kind::Image]
            .into_iter()
            .flat_map(|kind| [(kind, JobState::Pending), (kind, JobState::Prepared)])
            .filter_map(|(kind, state)| self.first(kind, state))
            .min()
            .map(|at| self.jobs[at].id)
    }

    pub fn apply(&mut self, id: JobId, event: JobEvent) -> Result<JobState, InvalidTransition> {
        let Some(at) = self.index(id) else {
            // A job removed from the list has nothing left to update.
            return Ok(JobState::Cancelled);
        };
        let next = self.jobs[at].state.apply(event)?;
        self.set_state(at, next);
        let job = &mut self.jobs[at];
        if job.state.is_finished() {
            job.fraction = None;
        }
        self.check();
        Ok(next)
    }

    pub fn set_fraction(&mut self, id: JobId, fraction: Option<f64>) {
        if let Some(job) = self.index(id).map(|at| &mut self.jobs[at]) {
            job.fraction = fraction;
        }
    }

    /// Removes a job that is not running. Removing the running job is a cancellation, which is a
    /// separate, explicit action (§33).
    pub fn remove(&mut self, id: JobId) -> bool {
        let Some(index) = self
            .index(id)
            .filter(|&at| self.jobs[at].state.is_removable())
        else {
            return false;
        };
        let job = self.jobs.remove(index);
        self.keys.remove(&key(&job.path));
        self.counts[kind_slot(job.kind)][job.state.slot()] -= 1;
        // Everything after it moved up one place.
        for cursor in self.pending_from.iter_mut().chain(&mut self.prepared_from) {
            if *cursor > index {
                *cursor -= 1;
            }
        }
        self.note(id);
        self.check();
        true
    }

    /// Removes every job in `ids` that is not running, in one pass however many there are, and
    /// returns them in list order.
    pub fn remove_all(&mut self, ids: &HashSet<JobId>) -> Vec<Job> {
        let mut removed = Vec::new();
        let mut kept = Vec::with_capacity(self.jobs.len());
        for job in std::mem::take(&mut self.jobs) {
            if ids.contains(&job.id) && job.state.is_removable() {
                removed.push(job);
            } else {
                kept.push(job);
            }
        }
        self.jobs = kept;
        for job in &removed {
            self.keys.remove(&key(&job.path));
            self.counts[kind_slot(job.kind)][job.state.slot()] -= 1;
            self.note(job.id);
        }
        self.rewind();
        self.check();
        removed
    }

    /// Puts back jobs taken off the list, where they were, as Undo after Clear does. A job whose
    /// file was added again meanwhile stays out. Returns the ids put back.
    pub fn restore(&mut self, jobs: Vec<Job>) -> Vec<JobId> {
        let mut restored = Vec::new();
        for job in jobs {
            if let Entry::Vacant(slot) = self.keys.entry(key(&job.path)) {
                slot.insert(job.id);
                restored.push(job.id);
                self.counts[kind_slot(job.kind)][job.state.slot()] += 1;
                self.note(job.id);
                self.jobs.push(job);
            }
        }
        self.jobs.sort_by_key(|job| job.id);
        self.rewind();
        self.check();
        restored
    }

    /// Removes everything except the running job (§34).
    pub fn clear(&mut self) {
        let removable: HashSet<JobId> = self
            .jobs
            .iter()
            .filter(|job| job.state.is_removable())
            .map(|job| job.id)
            .collect();
        self.remove_all(&removable);
    }

    /// The jobs added, taken off, put back or moved to another state since the last call, each
    /// at least once; `None` when so much changed that going through the whole list is quicker.
    pub fn take_changed(&mut self) -> Option<Vec<JobId>> {
        self.changed.replace(Vec::new())
    }

    /// Where the first job of `kind` in `state` (waiting, or got ready) is, starting where the
    /// last look ended: each look goes on from there, so finding them all once goes through the
    /// list once.
    fn first(&mut self, kind: Kind, state: JobState) -> Option<usize> {
        let k = kind_slot(kind);
        if self.counts[k][state.slot()] == 0 {
            return None;
        }
        let cursor = match state {
            JobState::Pending => &mut self.pending_from[k],
            JobState::Prepared => &mut self.prepared_from[k],
            _ => {
                return self
                    .jobs
                    .iter()
                    .position(|j| j.kind == kind && j.state == state);
            }
        };
        let found = self
            .jobs
            .get(*cursor..)
            .unwrap_or_default()
            .iter()
            .position(|job| job.kind == kind && job.state == state)
            .map(|offset| *cursor + offset);
        *cursor = found.unwrap_or(self.jobs.len());
        found
    }

    /// Moves the job at `at` to `next`, keeping the counts and where waiting jobs may be.
    fn set_state(&mut self, at: usize, next: JobState) {
        let job = &mut self.jobs[at];
        let k = kind_slot(job.kind);
        self.counts[k][job.state.slot()] -= 1;
        self.counts[k][next.slot()] += 1;
        job.state = next;
        let id = job.id;
        match next {
            JobState::Pending => self.pending_from[k] = self.pending_from[k].min(at),
            JobState::Prepared => self.prepared_from[k] = self.prepared_from[k].min(at),
            _ => {}
        }
        self.note(id);
    }

    /// Looks for waiting jobs from the start again, after jobs were put back or many were taken
    /// off.
    fn rewind(&mut self) {
        self.pending_from = [0; 2];
        self.prepared_from = [0; 2];
    }

    fn note(&mut self, id: JobId) {
        let limit = self.jobs.len() + 1024;
        if let Some(changed) = &mut self.changed {
            if changed.len() >= limit {
                self.changed = None;
            } else {
                changed.push(id);
            }
        }
    }

    /// In debug builds, checks the counts and where waiting jobs may be against the list itself.
    fn check(&self) {
        if !cfg!(debug_assertions) || self.jobs.len() > CHECKED_UP_TO {
            return;
        }
        let mut counts = [[0; STATES]; 2];
        for job in &self.jobs {
            counts[kind_slot(job.kind)][job.state.slot()] += 1;
        }
        debug_assert_eq!(counts, self.counts, "counts drifted from the list");
        for kind in [Kind::Video, Kind::Image] {
            let k = kind_slot(kind);
            for (state, from) in [
                (JobState::Pending, self.pending_from[k]),
                (JobState::Prepared, self.prepared_from[k]),
            ] {
                debug_assert!(from <= self.jobs.len());
                debug_assert!(
                    !self.jobs[..from.min(self.jobs.len())]
                        .iter()
                        .any(|job| job.kind == kind && job.state == state),
                    "a {kind:?} {state:?} before where they are looked for"
                );
            }
        }
    }
}

impl Default for Queue {
    fn default() -> Self {
        Self {
            jobs: Vec::new(),
            keys: HashMap::new(),
            next_id: 0,
            counts: [[0; STATES]; 2],
            pending_from: [0; 2],
            prepared_from: [0; 2],
            changed: Some(Vec::new()),
        }
    }
}

fn kind_slot(kind: Kind) -> usize {
    match kind {
        Kind::Video => 0,
        Kind::Image => 1,
    }
}

/// Windows paths are case-insensitive, so `A.mp4` and `a.MP4` are the same file.
fn key(path: &Path) -> String {
    path.to_string_lossy().to_lowercase()
}
#[cfg(test)]
mod tests {
    use super::*;

    fn paths(names: &[&str]) -> Vec<PathBuf> {
        names
            .iter()
            .map(|n| PathBuf::from(format!(r"C:\V\{n}")))
            .collect()
    }

    fn queue(names: &[&str]) -> Queue {
        let mut queue = Queue::default();
        queue.add(paths(names));
        queue
    }

    #[test]
    fn finds_a_job_by_path_whatever_its_case() {
        let queue = queue(&["Clip.MOV", "b.mp4"]);
        let found = queue.find(&PathBuf::from(r"c:\v\clip.mov")).unwrap();
        assert_eq!(found.file_name(), "Clip.MOV");
        assert!(queue.find(&PathBuf::from(r"C:\V\other.mp4")).is_none());
    }

    fn finish(queue: &mut Queue, event: JobEvent) -> JobId {
        let id = queue.start_preparing(Kind::Video).unwrap().id;
        if event == JobEvent::Complete {
            queue.apply(id, JobEvent::BeginCompressing).unwrap();
            queue.apply(id, JobEvent::BeginFinalizing).unwrap();
        }
        queue.apply(id, event).unwrap();
        id
    }

    #[test]
    fn adding_is_additive_and_ignores_duplicates() {
        let mut queue = queue(&["a.mp4", "b.mp4"]);
        assert_eq!(queue.add(paths(&["c.mp4", "A.MP4", "c.mp4"])), 1);
        let names: Vec<_> = queue.jobs().iter().map(Job::file_name).collect();
        assert_eq!(names, ["a.mp4", "b.mp4", "c.mp4"]);
    }

    #[test]
    fn each_kind_goes_oldest_first_side_by_side() {
        let mut queue = queue(&["a.mp4", "b.jpg", "c.mp4", "d.png"]);
        let video = queue.start_preparing(Kind::Video).unwrap().id;
        let image = queue.start_preparing(Kind::Image).unwrap().id;
        assert_eq!(queue.get(video).unwrap().file_name(), "a.mp4");
        assert_eq!(queue.get(image).unwrap().file_name(), "b.jpg");
        let next = queue.start_preparing(Kind::Video).unwrap();
        assert_eq!(next.file_name(), "c.mp4");
        assert_eq!(
            queue.count(Kind::Image, |state| state == JobState::Pending),
            1
        );
        assert_eq!(queue.count(Kind::Video, JobState::is_active), 2);
    }

    #[test]
    fn a_ready_file_waits_its_turn() {
        let mut queue = queue(&["a.mp4", "b.mp4"]);
        let first = queue.start_preparing(Kind::Video).unwrap().id;
        let second = queue.start_preparing(Kind::Video).unwrap().id;
        assert!(queue.next_prepared(Kind::Video).is_none());
        queue.apply(second, JobEvent::Ready).unwrap();
        queue.apply(first, JobEvent::Ready).unwrap();
        assert_eq!(queue.next_prepared(Kind::Video).unwrap().id, first);
        assert!(queue.next_prepared(Kind::Image).is_none());
    }

    #[test]
    fn files_being_worked_on_cannot_be_removed_or_cleared() {
        let mut queue = queue(&["a.mp4", "b.mp4", "c.jpg"]);
        finish(&mut queue, JobEvent::Complete);
        let video = queue.start_preparing(Kind::Video).unwrap().id;
        let image = queue.start_preparing(Kind::Image).unwrap().id;

        assert!(!queue.remove(video));
        queue.clear();
        let left: Vec<_> = queue.jobs().iter().map(|job| job.id).collect();
        assert_eq!(left, [video, image]);
    }

    #[test]
    fn removing_frees_the_path_to_be_added_again() {
        let mut queue = queue(&["a.mp4", "b.mp4"]);
        let done = finish(&mut queue, JobEvent::Complete);
        assert!(queue.remove(done));
        let pending = queue.jobs()[0].id;
        assert!(queue.remove(pending));
        assert!(queue.is_empty());
        assert_eq!(queue.add(paths(&["a.mp4", "b.mp4"])), 2);
    }

    #[test]
    fn finished_jobs_lose_their_progress() {
        let mut queue = queue(&["a.mp4"]);
        let id = queue.start_preparing(Kind::Video).unwrap().id;
        queue.apply(id, JobEvent::BeginCompressing).unwrap();
        queue.set_fraction(id, Some(0.5));
        queue.apply(id, JobEvent::Cancel).unwrap();
        assert_eq!(queue.get(id).unwrap().fraction, None);
    }

    #[test]
    fn updates_for_removed_jobs_are_ignored() {
        let mut queue = queue(&["a.mp4"]);
        let id = queue.jobs()[0].id;
        queue.remove(id);
        assert!(queue.apply(id, JobEvent::Complete).is_ok());
        queue.set_fraction(id, Some(1.0));
    }

    #[test]
    fn restored_jobs_go_back_where_they_were() {
        let mut queue = queue(&["a.mp4", "b.mp4", "c.mp4"]);
        let taken: Vec<Job> = queue.jobs()[..2].to_vec();
        for job in &taken {
            queue.remove(job.id);
        }
        // Added again meanwhile: it stays as it is now, and its old entry stays out.
        queue.add(paths(&["b.mp4"]));
        let restored = queue.restore(taken.clone());
        assert_eq!(restored, vec![taken[0].id]);
        let names: Vec<String> = queue.jobs().iter().map(Job::file_name).collect();
        assert_eq!(names, ["a.mp4", "c.mp4", "b.mp4"]);
        assert!(queue.find(&PathBuf::from(r"C:\V\a.mp4")).is_some());
    }

    /// The counts, and the first waiting files, worked out the slow way.
    fn assert_matches_a_full_scan(queue: &mut Queue) {
        for kind in [Kind::Video, Kind::Image] {
            for state in JobState::ALL {
                let scanned = queue
                    .jobs()
                    .iter()
                    .filter(|job| job.kind == kind && job.state == state)
                    .count();
                assert_eq!(
                    queue.count(kind, |s| s == state),
                    scanned,
                    "{kind:?} {state:?}"
                );
            }
            let first_prepared = queue
                .jobs()
                .iter()
                .find(|job| job.kind == kind && job.state == JobState::Prepared)
                .map(|job| job.id);
            assert_eq!(queue.next_prepared(kind).map(|job| job.id), first_prepared);
        }
        let first_waiting = queue
            .jobs()
            .iter()
            .find(|job| job.state.is_waiting())
            .map(|job| job.id);
        assert_eq!(queue.first_waiting(), first_waiting);
    }

    #[test]
    fn counts_and_first_files_follow_every_change() {
        let names: Vec<String> = (0..40)
            .map(|i| format!("{i}.{}", if i % 3 == 0 { "jpg" } else { "mp4" }))
            .collect();
        let names: Vec<&str> = names.iter().map(String::as_str).collect();
        let mut queue = queue(&names);
        assert_matches_a_full_scan(&mut queue);

        // Got ready out of order, so a file further down is ready before one above it.
        let mut started = Vec::new();
        for _ in 0..6 {
            started.push(queue.start_preparing(Kind::Video).unwrap().id);
            started.push(queue.start_preparing(Kind::Image).unwrap().id);
        }
        for &id in started.iter().rev().step_by(2) {
            queue.apply(id, JobEvent::Ready).unwrap();
            assert_matches_a_full_scan(&mut queue);
        }
        for &id in started.iter().step_by(4) {
            queue.apply(id, JobEvent::Ready).unwrap();
            assert_matches_a_full_scan(&mut queue);
        }
        let next = queue.next_prepared(Kind::Video).unwrap().id;
        queue.apply(next, JobEvent::BeginCompressing).unwrap();
        assert_matches_a_full_scan(&mut queue);

        // Taking files off moves everything after them up.
        let first = queue.jobs()[0].id;
        assert!(queue.remove(first));
        let pending = queue
            .jobs()
            .iter()
            .find(|job| job.state == JobState::Pending)
            .unwrap()
            .id;
        assert!(queue.remove(pending));
        assert_matches_a_full_scan(&mut queue);

        // Taken off together and put back.
        let taken: Vec<Job> = queue.jobs()[..10]
            .iter()
            .filter(|job| job.state.is_removable())
            .cloned()
            .collect();
        let ids: HashSet<JobId> = taken.iter().map(|job| job.id).collect();
        assert_eq!(queue.remove_all(&ids), taken);
        assert_matches_a_full_scan(&mut queue);
        queue.restore(taken);
        assert_matches_a_full_scan(&mut queue);

        queue.clear();
        assert_matches_a_full_scan(&mut queue);
        assert!(queue.jobs().iter().all(|job| job.state.is_active()));
    }

    #[test]
    fn says_which_files_changed() {
        let mut queue = queue(&["a.mp4", "b.mp4", "c.jpg"]);
        assert_eq!(queue.take_changed().map(|ids| ids.len()), Some(3));
        assert_eq!(queue.take_changed(), Some(Vec::new()));
        let id = queue.start_preparing(Kind::Image).unwrap().id;
        queue.set_fraction(id, Some(0.5));
        assert_eq!(queue.take_changed(), Some(vec![id]));
        let pending = queue.jobs()[0].id;
        queue.remove(pending);
        assert_eq!(queue.take_changed(), Some(vec![pending]));
        // Far more changes than files: going through the whole list is quicker.
        let mut long = Queue::default();
        long.add((0..2000).map(|i| PathBuf::from(format!(r"C:\V\{i}.mp4"))));
        long.clear();
        assert_eq!(long.take_changed().map(|ids| ids.len()), None);
        assert_eq!(long.take_changed(), Some(Vec::new()));
    }

    /// Thirty thousand files, each got ready and compressed in turn, as the scheduler does: the
    /// counts and the next file are found without going through the list each time.
    #[test]
    fn a_huge_list_is_never_gone_through_for_each_file() {
        let mut queue = Queue::default();
        queue.add((0..30_000).map(|i| PathBuf::from(format!(r"C:\P\{i}.jpg"))));
        let started = std::time::Instant::now();
        while let Some(id) = queue.start_preparing(Kind::Image).map(|job| job.id) {
            queue.apply(id, JobEvent::Ready).unwrap();
            assert_eq!(queue.count(Kind::Image, JobState::is_getting_ready), 1);
            let next = queue.next_prepared(Kind::Image).unwrap().id;
            assert_eq!(next, id);
            queue.apply(next, JobEvent::BeginCompressing).unwrap();
            queue.apply(next, JobEvent::BeginFinalizing).unwrap();
            queue.apply(next, JobEvent::Complete).unwrap();
            queue.first_waiting();
        }
        let took = started.elapsed();
        assert_eq!(queue.count_all(JobState::is_finished), 30_000);
        // Going through the list for each file took seconds; a quarter of a second is plenty
        // even in a debug build.
        assert!(took < std::time::Duration::from_millis(250), "{took:?}");
    }
}
