//! The set of videos and images the user has added, and which ones run next (spec §26–§36).
//!
//! Each file is first got ready, then compressed. Videos and images go side by side, each in the
//! order they were added; how many of each run at once is the app's to decide (ADR 0040). Clear
//! never interrupts the files being worked on. Adding is always additive. A path already in the
//! set is not added twice; the app removes a finished entry first, so a file can always be
//! compressed again (§29).

use std::collections::HashMap;
use std::collections::hash_map::Entry;
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

#[derive(Debug, Default)]
pub struct Queue {
    jobs: Vec<Job>,
    /// Each job by its path (see [`key`]), so finding one never goes through the whole list.
    keys: HashMap<String, JobId>,
    next_id: u64,
}

impl Queue {
    /// Adds every path not already in the set, in order. Returns how many were added.
    pub fn add(&mut self, paths: impl IntoIterator<Item = PathBuf>) -> usize {
        let before = self.jobs.len();
        for path in paths {
            if let Entry::Vacant(slot) = self.keys.entry(key(&path)) {
                self.next_id += 1;
                slot.insert(JobId(self.next_id));
                self.jobs.push(Job {
                    id: JobId(self.next_id),
                    kind: media::kind_of(&path),
                    path,
                    state: JobState::Pending,
                    fraction: None,
                });
            }
        }
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
        self.jobs.iter().find(|job| job.id == id)
    }

    /// The job for `path`, matched the way adding matches duplicates.
    pub fn find(&self, path: &Path) -> Option<&Job> {
        self.keys.get(&key(path)).and_then(|&id| self.get(id))
    }

    /// The first file being worked on, if any. Several can be: one video and a few images, each
    /// getting ready or compressing (ADR 0040).
    pub fn active(&self) -> Option<&Job> {
        self.jobs.iter().find(|job| job.state.is_active())
    }

    pub fn pending_count(&self) -> usize {
        self.jobs
            .iter()
            .filter(|job| job.state == JobState::Pending)
            .count()
    }

    /// How many files of `kind` are in a state `test` accepts.
    pub fn count(&self, kind: Kind, test: impl Fn(JobState) -> bool) -> usize {
        self.jobs
            .iter()
            .filter(|job| job.kind == kind && test(job.state))
            .count()
    }

    /// Starts getting the oldest waiting file of `kind` ready.
    pub fn start_preparing(&mut self, kind: Kind) -> Option<&Job> {
        let at = self
            .jobs
            .iter()
            .position(|job| job.kind == kind && job.state == JobState::Pending)?;
        let job = &mut self.jobs[at];
        job.state = job.state.apply(JobEvent::Start).ok()?;
        Some(job)
    }

    /// The oldest file of `kind` that is ready and waiting its turn to compress.
    pub fn next_prepared(&self, kind: Kind) -> Option<&Job> {
        self.jobs
            .iter()
            .find(|job| job.kind == kind && job.state == JobState::Prepared)
    }

    pub fn apply(&mut self, id: JobId, event: JobEvent) -> Result<JobState, InvalidTransition> {
        let Some(job) = self.jobs.iter_mut().find(|job| job.id == id) else {
            // A job removed from the list has nothing left to update.
            return Ok(JobState::Cancelled);
        };
        job.state = job.state.apply(event)?;
        if job.state.is_finished() {
            job.fraction = None;
        }
        Ok(job.state)
    }

    pub fn set_fraction(&mut self, id: JobId, fraction: Option<f64>) {
        if let Some(job) = self.jobs.iter_mut().find(|job| job.id == id) {
            job.fraction = fraction;
        }
    }

    /// Removes a job that is not running. Removing the running job is a cancellation, which is a
    /// separate, explicit action (§33).
    pub fn remove(&mut self, id: JobId) -> bool {
        let Some(index) = self
            .jobs
            .iter()
            .position(|job| job.id == id && job.state.is_removable())
        else {
            return false;
        };
        let job = self.jobs.remove(index);
        self.keys.remove(&key(&job.path));
        true
    }

    /// Removes everything except the running job (§34).
    pub fn clear(&mut self) {
        let keys = &mut self.keys;
        self.jobs.retain(|job| {
            let keep = !job.state.is_removable();
            if !keep {
                keys.remove(&key(&job.path));
            }
            keep
        });
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
        assert_eq!(queue.pending_count(), 1);
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
}
