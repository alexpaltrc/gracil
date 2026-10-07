//! Which files are got ready and compressed when, and how their progress is followed
//! (ADR 0040): one video at a time, images beside it, at the pace the computer allows.

use super::*;

impl Gracil {
    /// Starts whatever there is room for (ADR 0040), once the engine is ready. Videos: one
    /// compresses while the next one gets ready. Images: a few at once beside it, as many as
    /// [`pace::Room`] allows, each read ahead of its turn. Nothing new starts while paused,
    /// while resting a moment because the computer runs warm, or once Gracil is exiting.
    pub(super) fn schedule(&mut self) -> Task<Message> {
        if self.exiting || self.held {
            return Task::none();
        }
        let EngineState::Ready(engine) = &self.engine else {
            return Task::none();
        };
        let engine = engine.clone();
        let compressing = JobState::is_compressing;
        let getting_ready = JobState::is_getting_ready;
        let mut tasks = Vec::new();

        // Videos.
        if self.queue.count(Kind::Video, compressing) == 0
            && let Some(id) = self.queue.next_prepared(Kind::Video).map(|job| job.id)
        {
            tasks.push(self.start_compressing(&engine, id));
        }
        if self.queue.count(Kind::Video, getting_ready) == 0
            && let Some(id) = self.queue.start_preparing(Kind::Video).map(|job| job.id)
        {
            tasks.push(self.start_preparing(&engine, id));
        }

        // Images.
        let video_on_processor = self.queue.count(Kind::Video, compressing) > 0
            && plan::attempts(engine.capabilities)
                .first()
                .is_some_and(|attempt| attempt.encoder == VideoEncoder::X265);
        let room = pace::Room::now(video_on_processor, self.breath);
        let at_once = room.images_at_once();
        let mut memory = room.memory_for_images();
        let mut images = self.queue.count(Kind::Image, compressing);
        while images < at_once {
            let Some(id) = self.queue.next_prepared(Kind::Image).map(|job| job.id) else {
                break;
            };
            let need = pace::image_memory(self.prepared.get(&id).and_then(work::Prepared::pixels));
            // A large image waits for others to finish, but one always goes ahead.
            if images > 0 && need > memory {
                break;
            }
            memory = memory.saturating_sub(need);
            tasks.push(self.start_compressing(&engine, id));
            images += 1;
        }
        // Reading is mostly waiting on a network or cloud drive, so a few more are read ahead.
        while self.queue.count(Kind::Image, getting_ready) < at_once + pace::READ_AHEAD {
            let Some(id) = self.queue.start_preparing(Kind::Image).map(|job| job.id) else {
                break;
            };
            tasks.push(self.start_preparing(&engine, id));
        }
        Task::batch(tasks)
    }

    /// Starts getting a file ready; it was just marked as getting ready in the list.
    pub(super) fn start_preparing(&mut self, engine: &Arc<Engine>, id: JobId) -> Task<Message> {
        let Some(path) = self.queue.get(id).map(|job| job.path.clone()) else {
            return Task::none();
        };
        let control = JobControl::default();
        self.running.insert(id, control.clone());
        work::prepare_file(engine.clone(), path, control)
            .map(move |report| Message::Job(id, report))
    }

    /// Starts compressing a file that was got ready.
    pub(super) fn start_compressing(&mut self, engine: &Arc<Engine>, id: JobId) -> Task<Message> {
        let path = self.queue.get(id).map(|job| job.path.clone());
        let (Some(path), Some(prepared)) = (path, self.prepared.remove(&id)) else {
            return Task::none();
        };
        self.transition(id, JobEvent::BeginCompressing);
        let control = JobControl::default();
        self.running.insert(id, control.clone());
        work::compress_file(engine.clone(), path, prepared, control)
            .map(move |report| Message::Job(id, report))
    }

    pub(super) fn on_report(&mut self, id: JobId, report: Report) -> Task<Message> {
        if !self.running.contains_key(&id) {
            // Stopped already (ADR 0044): a result made meanwhile is deleted, away from the
            // window, since it may be on a slow drive.
            if let Report::Done(Outcome::Ready { temp, .. }) = report {
                std::thread::spawn(move || work::discard(&temp));
            }
            return Task::none();
        }
        match report {
            Report::Progress(fraction) => {
                self.queue.set_fraction(id, fraction);
                Task::none()
            }
            Report::Prepared(prepared) => {
                self.running.remove(&id);
                self.transition(id, JobEvent::Ready);
                self.prepared.insert(id, prepared);
                self.schedule()
            }
            Report::Done(outcome) => {
                let events: &[JobEvent] = match outcome {
                    Outcome::Ready { .. } | Outcome::Placed { .. } => &[
                        // It may have finished just as it was paused.
                        JobEvent::Resume,
                        JobEvent::BeginCompressing,
                        JobEvent::BeginFinalizing,
                        JobEvent::Complete,
                    ],
                    Outcome::Failed(_) => &[JobEvent::Fail],
                    Outcome::Cancelled => &[JobEvent::Cancel],
                };
                for &event in events {
                    // Events that already happened (e.g. compressing had begun) are skipped.
                    let _ = self.queue.apply(id, event);
                }
                // A result that isn't smaller is no fault; the original simply stays.
                let fault = matches!(
                    outcome,
                    Outcome::Failed(failure)
                        if !matches!(failure, Failure::NotSmaller | Failure::AsSmallAsItGets)
                );
                if fault {
                    self.note_problem(id);
                }
                self.outcomes.insert(id, outcome);
                self.running.remove(&id);
                self.prepared.remove(&id);
                // File Explorer's choice first; otherwise the "Always" setting (ADR 0025).
                let intent = self.intents.remove(&id).or(self.settings.always);
                let place = match intent {
                    Some(choice) => self.choose(id, choice),
                    None => Task::none(),
                };
                Task::batch([place, self.schedule()])
            }
        }
    }

    /// Pauses or resumes at the person's request (spec §47, §77.3). Pausing with nothing being
    /// worked on does nothing.
    pub(super) fn pause(&mut self, paused: bool) {
        if self.paused == paused || (paused && self.running.is_empty()) {
            return;
        }
        self.paused = paused;
        self.hold();
    }

    /// Rests a moment or works on, as the computer runs warm (ADR 0045). Resting is never shown:
    /// the work only goes a little slower.
    pub(super) fn breathe(&mut self, breath: Breath) -> Task<Message> {
        self.breath = breath;
        self.hold();
        self.schedule()
    }

    /// Freezes or thaws every file being worked on, all together, while paused or resting a
    /// moment. Nothing new starts while frozen. Only the person's own pause shows. A failure
    /// leaves a file as it was and is only logged.
    pub(super) fn hold(&mut self) {
        let held = self.paused || self.breath.resting;
        if self.held != held {
            self.held = held;
            for control in self.running.values() {
                let result = if held {
                    control.pause(gracil_windows::set_suspended)
                } else {
                    control.resume()
                };
                if let Err(error) = result {
                    warn!(%error, held, "could not pause or resume");
                }
            }
        }
        let shown: Vec<JobId> = self.running.keys().copied().collect();
        for id in shown {
            match (self.paused, self.queue.get(id).map(|job| job.state)) {
                (true, Some(JobState::Compressing)) => self.transition(id, JobEvent::Pause),
                (false, Some(JobState::Paused)) => self.transition(id, JobEvent::Resume),
                _ => {}
            }
        }
    }

    pub(super) fn transition(&mut self, id: JobId, event: JobEvent) {
        if let Err(error) = self.queue.apply(id, event) {
            warn!(%error, "ignored job transition");
        }
    }

    /// Stop (ADR 0044): the whole list stops at once. Every file not finished leaves the list
    /// and is offered to carry on with later, with the choice made for it. Work under way is told
    /// to stop, and whatever it still reports is let go. A result being put in place finishes.
    pub(super) fn stop(&mut self) {
        self.stops += 1;
        self.abandoned += std::mem::take(&mut self.gathering);
        for control in self.running.values() {
            control.cancel();
        }
        self.running.clear();
        self.prepared.clear();
        let stopped: Vec<(JobId, PathBuf)> = self
            .queue
            .jobs()
            .iter()
            .filter(|job| !job.state.is_finished())
            .map(|job| (job.id, job.path.clone()))
            .collect();
        for (id, path) in stopped {
            let _ = self.queue.apply(id, JobEvent::Cancel);
            if self.queue.remove(id) {
                let choice = self.intents.remove(&id);
                self.offered.push(Unfinished { path, choice });
            }
        }
        if self.selected.is_some_and(|id| self.queue.get(id).is_none()) {
            self.selected = None;
        }
        self.paused = false;
        self.hold();
    }
}
