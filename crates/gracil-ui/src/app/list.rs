//! Files coming onto the list, the choice made for each finished one, and files leaving it.

use super::*;

/// How long Undo stays after Clear.
const UNDO_FOR: Duration = Duration::from_secs(6);

/// A file Clear took off the list, with all Gracil knew of it, kept a moment for Undo.
pub struct Taken {
    job: Job,
    outcome: Option<Outcome>,
    intent: Option<Choice>,
    prepared: Option<work::Prepared>,
}

impl Taken {
    /// Lets it go for good: a result nobody chose for is deleted; the original stays.
    fn let_go(self) {
        if let Some(Outcome::Ready { temp, .. }) = self.outcome {
            work::discard(&temp);
        }
    }
}

/// The picker shows videos and images by default. Only a convenience: any video FFmpeg reads is
/// accepted.
pub(super) async fn choose_files(text: &'static Text) -> Vec<PathBuf> {
    let media: Vec<&str> = gracil_core::media::VIDEO_EXTENSIONS
        .iter()
        .chain(gracil_core::media::IMAGE_EXTENSIONS)
        .copied()
        .collect();
    rfd::AsyncFileDialog::new()
        .set_title(text.choose_files)
        .add_filter(text.media_filter, &media)
        .add_filter(text.all_files_filter, &["*"])
        .pick_files()
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|file| file.path().to_owned())
        .collect()
}

impl Gracil {
    /// Adds what was asked for, and shows the window when the request wants it. A choice made in
    /// advance applies to each of its videos, including one already on the list.
    pub(super) fn on_request(&mut self, mut request: Request) -> Task<Message> {
        // File Explorer leaves the choice to **Always** (ADR 0058).
        if request.from_explorer && request.choice.is_none() {
            request.choice = self.settings.always;
        }
        self.requested = true;
        self.resident |= request.at_startup;
        let show = if request.shows_window() {
            self.show_main()
        } else if request.choice.is_some()
            && self.settings.explorer_mini_window
            && self.main.is_none()
        {
            // Compressing from File Explorer: the mini-window shows it happening, unless Gracil's
            // window already does (spec §56).
            self.open_mini()
        } else {
            Task::none()
        };
        let gather = self.look_through(request.paths, request.only, request.choice);
        Task::batch([show, gather])
    }

    /// Adds the files left unfinished last time, each with the choice File Explorer made for it.
    /// Whether they are still there is checked away from the window, as for File Explorer's
    /// files, since there can be thousands of them.
    pub(super) fn continue_unfinished(&mut self) -> Task<Message> {
        let offered = std::mem::take(&mut self.offered);
        let mut tasks = Vec::new();
        for choice in [None, Some(Choice::Replace), Some(Choice::SaveCopy)] {
            let paths: Vec<PathBuf> = offered
                .iter()
                .filter(|file| file.choice == choice)
                .map(|file| file.path.clone())
                .collect();
            tasks.push(self.look_through(paths, None, choice));
        }
        Task::batch(tasks)
    }

    /// Looks through files and folders away from the window, which a slow drive would otherwise
    /// freeze, however they came: dropped, chosen, from File Explorer or left unfinished. A folder
    /// brings in the videos and images inside it (ADR 0055), and files join the list folder by
    /// folder (ADR 0035, 0037), each with `choice` if File Explorer made one.
    pub(super) fn look_through(
        &mut self,
        paths: Vec<PathBuf>,
        only: Option<Kind>,
        choice: Option<Choice>,
    ) -> Task<Message> {
        if paths.is_empty() {
            return Task::none();
        }
        self.gathering += 1;
        let stops = self.stops;
        work::gather(paths, only).map(move |found| Message::Found(stops, choice, found))
    }

    pub(super) fn on_found(
        &mut self,
        stops: u64,
        choice: Option<Choice>,
        found: work::Found,
    ) -> Task<Message> {
        if stops != self.stops {
            // Started before Stop: what it finds is kept to carry on with later, not compressed.
            match found {
                work::Found::Files(paths) => self
                    .offered
                    .extend(paths.into_iter().map(|path| Unfinished { path, choice })),
                work::Found::Done => self.abandoned = self.abandoned.saturating_sub(1),
            }
            return Task::none();
        }
        let paths = match found {
            work::Found::Files(paths) => paths,
            work::Found::Done => {
                self.gathering = self.gathering.saturating_sub(1);
                return Task::none();
            }
        };
        let Some(choice) = choice else {
            return self.add(paths);
        };
        // File Explorer's choice goes with each file, found again once it is on the list.
        let mut tasks = vec![self.add(paths.clone())];
        let ids: Vec<JobId> = paths
            .iter()
            .filter_map(|path| self.queue.find(path).map(|job| job.id))
            .collect();
        for id in ids {
            self.intents.insert(id, choice);
            if self.is_waiting(id) {
                tasks.push(self.choose(id, choice));
            }
        }
        Task::batch(tasks)
    }

    /// Adds files to the list, once [`Self::look_through`] found them. A file the list is done
    /// with (put in place, failed, cancelled) leaves it first, so the list never stops a file from
    /// being compressed again (spec §29). One still waiting, running or waiting for a choice stays
    /// as it is.
    pub(super) fn add(&mut self, paths: Vec<PathBuf>) -> Task<Message> {
        let done_with: Vec<JobId> = paths
            .iter()
            .filter_map(|path| self.queue.find(path))
            .filter(|job| {
                job.state.is_finished()
                    && !matches!(self.outcomes.get(&job.id), Some(Outcome::Ready { .. }))
            })
            .map(|job| job.id)
            .collect();
        for id in done_with {
            self.remove(id);
        }
        // A file left unfinished and added again is on the list now, so it isn't offered too.
        if !self.offered.is_empty() {
            let adding: HashSet<&PathBuf> = paths.iter().collect();
            self.offered
                .retain(|unfinished| !adding.contains(&unfinished.path));
        }
        let added = self.queue.add(paths);
        if added > 0 && matches!(self.engine, EngineState::Unavailable) {
            self.engine = EngineState::Starting;
            return work::start_engine().map(Message::EngineStarted);
        }
        self.schedule()
    }

    pub(super) fn choose(&mut self, id: JobId, choice: Choice) -> Task<Message> {
        let Some(source) = self.queue.get(id).map(|job| job.path.clone()) else {
            return Task::none();
        };
        let Some(Outcome::Ready {
            temp,
            extension,
            placing,
            problem,
            ..
        }) = self.outcomes.get_mut(&id)
        else {
            return Task::none();
        };
        if *placing {
            return Task::none();
        }
        *placing = true;
        *problem = None;
        work::place(temp.clone(), source, extension.clone(), choice)
            .map(move |placed| Message::Placed(id, choice, placed))
    }

    pub(super) fn placed(&mut self, id: JobId, choice: Choice, placed: Result<PathBuf, Failure>) {
        let Some(outcome) = self.outcomes.get_mut(&id) else {
            return;
        };
        let Outcome::Ready {
            sizes,
            placing,
            problem,
            ..
        } = outcome
        else {
            return;
        };
        match placed {
            Ok(path) => {
                let sizes = *sizes;
                self.last_placed = Some(id);
                *outcome = Outcome::Placed {
                    choice,
                    path,
                    sizes,
                };
            }
            Err(failure) => {
                *placing = false;
                *problem = Some(failure);
                self.note_problem(id);
            }
        }
    }

    /// Takes a video off the list. A result nobody chose for is deleted; the original stays.
    pub(super) fn remove(&mut self, id: JobId) {
        if let Some(Taken {
            outcome: Some(Outcome::Ready { temp, .. }),
            ..
        }) = self.take(id)
        {
            work::discard(&temp);
        }
    }

    /// Takes a file off the list with all Gracil knows of it, so it can be put back.
    fn take(&mut self, id: JobId) -> Option<Taken> {
        let job = self
            .queue
            .get(id)
            .filter(|job| self.can_remove(job))
            .cloned()?;
        if !self.queue.remove(id) {
            return None;
        }
        if self.selected == Some(id) {
            self.selected = None;
        }
        if self.last_placed == Some(id) {
            self.last_placed = None;
        }
        Some(Taken {
            job,
            outcome: self.outcomes.remove(&id),
            intent: self.intents.remove(&id),
            prepared: self.prepared.remove(&id),
        })
    }

    /// Clear: everything not being worked on leaves the list, and for a few seconds Undo puts it
    /// back. Results nobody chose for are deleted only once Undo is gone.
    pub(super) fn clear(&mut self) -> Task<Message> {
        self.let_go_cleared();
        let ids: Vec<JobId> = self.queue.jobs().iter().map(|job| job.id).collect();
        let taken: Vec<Taken> = ids.into_iter().filter_map(|id| self.take(id)).collect();
        if self.queue.is_empty() {
            // The empty window has no list to fold away.
            self.drawer_open = false;
            self.drawer_folding = false;
        }
        self.drawer_scroll = DrawerScroll::default();
        if taken.is_empty() {
            return Task::none();
        }
        self.clears += 1;
        self.cleared = Some(taken);
        Task::batch([
            after(UNDO_FOR, Message::UndoGone(self.clears)),
            crate::focus::restart(crate::focus::Scope::Main),
        ])
    }

    /// Undo: what Clear took off the list comes back where it was.
    pub(super) fn undo_clear(&mut self) -> Task<Message> {
        let Some(taken) = self.cleared.take() else {
            return Task::none();
        };
        let jobs = taken.iter().map(|taken| taken.job.clone()).collect();
        let restored: HashSet<JobId> = self.queue.restore(jobs).into_iter().collect();
        for taken in taken {
            let id = taken.job.id;
            if !restored.contains(&id) {
                // Its file was added again meanwhile; the new entry stands.
                taken.let_go();
                continue;
            }
            if let Some(outcome) = taken.outcome {
                self.outcomes.insert(id, outcome);
            }
            if let Some(intent) = taken.intent {
                self.intents.insert(id, intent);
            }
            if let Some(prepared) = taken.prepared {
                self.prepared.insert(id, prepared);
            }
        }
        Task::batch([
            self.schedule(),
            crate::focus::restart(crate::focus::Scope::Main),
        ])
    }

    /// Undo is gone: what Clear took off the list is let go for good.
    pub(super) fn let_go_cleared(&mut self) {
        for taken in self.cleared.take().into_iter().flatten() {
            taken.let_go();
        }
    }

    /// Results kept for Undo, which Gracil deletes on leaving like any other nobody chose for.
    pub(super) fn cleared_results(&self) -> impl Iterator<Item = &PathBuf> {
        self.cleared
            .iter()
            .flatten()
            .filter_map(|taken| match &taken.outcome {
                Some(Outcome::Ready { temp, .. }) => Some(temp),
                _ => None,
            })
    }

    /// Marks a problem the user hasn't seen, when no window is open.
    pub(super) fn note_problem(&mut self, id: JobId) {
        if self.main.is_none() && !self.mini_shown {
            self.unseen_problem = Some(id);
        }
    }
}
