//! Leaving Gracil: asking first when work would be lost, keeping the list of unfinished
//! files (ADR 0042), and tidying up before the process ends.

use super::*;

/// The longest Gracil waits, on leaving, to tidy up the files it no longer needs.
const TIDY_BEFORE_LEAVING: Duration = Duration::from_secs(3);

/// A quick likeness of the list of unfinished files: when it changes, the list is kept again.
pub(super) type Kept = (usize, usize, usize, usize);

/// The list of unfinished files is kept at most this often while it keeps changing, so a long
/// list isn't written out for every file done. A sudden close loses at most this much: those
/// files are simply compressed again.
const KEEP_EVERY: Duration = Duration::from_secs(30);

impl Gracil {
    /// Exit (spec §54.1): straight away when nothing is in progress; otherwise the window opens
    /// and says what would stop.
    pub(super) fn ask_to_exit(&mut self) -> Task<Message> {
        if self.nothing_to_lose() {
            self.exit_now();
            return Task::none();
        }
        self.confirming_exit = true;
        self.show_main()
    }

    /// Whether leaving now loses nothing: no file is being worked on or waiting its turn, and no
    /// result waits for a choice (each would be let go, to be made again next time).
    pub(super) fn nothing_to_lose(&self) -> bool {
        self.running.is_empty() && self.waiting_to_compress() == 0 && self.waiting_count() == 0
    }

    /// Stops every file being worked on, leaving their originals as they were, and leaves once
    /// they have tidied up (see `update`). Files still waiting are not started.
    pub(super) fn exit_now(&mut self) {
        self.exiting = true;
        self.confirming_exit = false;
        // Stopping marks the files being worked on as stopped; they are still to be done.
        let list = self.unfinished();
        unfinished::save_now(&list);
        self.leaving_with = Some(list);
        // Nothing is waited for but a result being put in place (ADR 0044): work stuck reading a
        // slow drive ends with Gracil, and what it leaves behind is removed at the next start.
        for control in self.running.values() {
            control.cancel();
        }
        self.running.clear();
    }

    /// Whether nothing is in the middle of being compressed or put in place, so leaving loses no
    /// work and leaves no half-written file.
    pub(super) fn settled(&self) -> bool {
        self.running.is_empty()
            && !self
                .outcomes
                .values()
                .any(|outcome| matches!(outcome, Outcome::Ready { placing: true, .. }))
    }

    /// Ends Gracil. Results nobody chose for are deleted; their originals stay as they were
    /// (ADR 0009).
    ///
    /// The process ends here rather than through `iced::exit`: Iced 0.14's daemon does not return
    /// from `run` once it has no window, so a Gracil working only in the background would stay.
    pub(super) fn leave(&self) -> ! {
        match &self.leaving_with {
            Some(list) => unfinished::save_now(list),
            None => unfinished::save_now(&self.unfinished()),
        }
        if let Some(tray) = &self.tray {
            tray.remove();
        }
        if self.preparing_to_delete {
            place::prepare_to_delete();
        }
        let unchosen: Vec<PathBuf> = self
            .outcomes
            .values()
            .filter_map(|outcome| match outcome {
                Outcome::Ready { temp, .. } => Some(temp.clone()),
                _ => None,
            })
            .collect();
        // Deleting on a slow drive, and a replaced video's parked original still in another
        // program's hands, are waited for only so long (ADR 0044); anything left after this is
        // removed at the next start.
        let (done, tidied) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            for temp in &unchosen {
                work::discard(temp);
            }
            gracil_core::output::finish_tidying(TIDY_BEFORE_LEAVING);
            let _ = done.send(());
        });
        let _ = tidied.recv_timeout(TIDY_BEFORE_LEAVING);
        info!("Gracil exited");
        logging::finish();
        std::process::exit(0)
    }

    /// Whether this Gracil was started only for background work and that work is over: nothing
    /// waiting or running, and no result left for anyone to decide on (spec §53). While the
    /// mini-window is open, the user is still looking, so Gracil waits for it to close.
    pub(super) fn background_work_done(&self) -> bool {
        self.requested
            && !self.resident
            && !self.window_shown
            && self.mini.is_none()
            && self.running.is_empty()
            && self.waiting_to_compress() == 0
            && !self
                .outcomes
                .values()
                .any(|outcome| matches!(outcome, Outcome::Ready { .. }))
    }

    /// The files not finished yet, to carry on with if Gracil closes now (ADR 0042): those still
    /// offered from last time, every file waiting or being worked on, and every result nobody
    /// chose for yet, which is deleted on leaving and so has to be made again.
    pub(super) fn unfinished(&self) -> Vec<Unfinished> {
        let mut list = self.offered.clone();
        list.extend(
            self.queue
                .jobs()
                .iter()
                .filter(|job| {
                    !job.state.is_finished()
                        || matches!(self.outcomes.get(&job.id), Some(Outcome::Ready { .. }))
                })
                .map(|job| Unfinished {
                    path: job.path.clone(),
                    choice: self.intents.get(&job.id).copied(),
                }),
        );
        list
    }

    /// Keeps the list of unfinished files when it has changed, at most every [`KEEP_EVERY`],
    /// and at once when nothing is left, so a finished list is never offered again.
    pub(super) fn keep_unfinished(&mut self) {
        let likeness = (
            self.summary.left,
            self.summary.waiting_for_choice,
            self.queue.len(),
            self.offered.len(),
        );
        let nothing_left = likeness.0 == 0 && likeness.1 == 0 && likeness.3 == 0;
        let due = match self.kept {
            Some((kept, _)) if kept == likeness => false,
            Some((_, at)) => nothing_left || at.elapsed() >= KEEP_EVERY,
            // Nothing kept yet this time: a list from last time stays until it is let go.
            None => !nothing_left,
        };
        if due {
            unfinished::save(self.unfinished());
            self.kept = Some((likeness, Instant::now()));
        }
    }
}
