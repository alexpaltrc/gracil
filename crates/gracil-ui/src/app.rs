//! Application state and how it reacts to the user, the window and background work.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use gracil_core::compress::Engine;
use gracil_core::encode::JobControl;
use gracil_core::job::{JobEvent, JobState};
use gracil_core::queue::{Job, JobId, Queue};
use iced::theme::{Mode, Palette};
use iced::{Event, Size, Subscription, Task, Theme, event, system, window};
use tracing::{info, warn};

use crate::logging;
use crate::outside::{self, Outside};
use crate::request::Request;
use crate::style;
use crate::text::{Language, Text};
use crate::work::{self, Choice, Failure, Outcome, Report};

pub struct Gracil {
    pub queue: Queue,
    pub outcomes: HashMap<JobId, Outcome>,
    pub engine: EngineState,
    pub hovering_files: bool,
    pub drawer_open: bool,
    /// The video the user picked in the drawer to look at, if any.
    pub selected: Option<JobId>,
    /// The last video put in place, so the window stays on it once nothing else needs a choice.
    last_placed: Option<JobId>,
    /// The interface's words, in the Windows display language.
    pub text: &'static Text,
    theme: Theme,
    /// The Windows theme being followed; unknown until Windows first answers.
    mode: Option<Mode>,
    /// A change between light and dark in progress: where it started, where it goes and when.
    shift: Option<(Palette, Palette, Instant)>,
    running: Option<(JobId, JobControl)>,
    /// The main window, while it is open.
    main: Option<window::Id>,
    /// Whether the main window was ever shown. A Gracil started only for background work leaves
    /// on its own once that work is done (spec §53).
    window_shown: bool,
    /// Whether any request has arrived yet; until then there is nothing to be done or finished.
    requested: bool,
    /// Choices made before compressing, by File Explorer's commands: the result is placed as soon
    /// as it is ready.
    intents: HashMap<JobId, Choice>,
    /// Leaving was asked for; Gracil leaves as soon as nothing is half done.
    exiting: bool,
}

#[derive(Debug, Clone)]
pub enum EngineState {
    Starting,
    Ready(Arc<Engine>),
    /// FFmpeg could not be found or installed (usually no internet on the very first run).
    /// Retried whenever the user adds videos.
    Unavailable,
}

#[derive(Debug, Clone)]
pub enum Message {
    EngineStarted(Option<Arc<Engine>>),
    FilesHovered,
    FilesHoveredLeft,
    FileDropped(PathBuf),
    ChooseFiles,
    FilesChosen(Vec<PathBuf>),
    Job(JobId, Report),
    Cancel,
    Pause,
    Resume,
    Choose(JobId, Choice),
    ChooseAll(Choice),
    Placed(JobId, Choice, Result<PathBuf, Failure>),
    Select(JobId),
    Remove(JobId),
    Clear,
    ToggleDrawer,
    ThemeChanged(Mode),
    ThemeFrame(Instant),
    /// A window is open; its handle lets Windows give it Gracil's icon.
    WindowReady(u64),
    /// A window moved to a screen with another scale, so its icon is picked again.
    Rescaled(window::Id),
    CloseRequested,
    Outside(Outside),
}

impl Gracil {
    /// Starts without a window: the first request (from the command line) decides whether one
    /// is needed (spec §62).
    pub fn boot() -> (Self, Task<Message>) {
        let state = Self {
            queue: Queue::default(),
            outcomes: HashMap::new(),
            engine: EngineState::Starting,
            hovering_files: false,
            drawer_open: false,
            selected: None,
            last_placed: None,
            text: Text::get(Language::detect()),
            theme: style::theme(style::palette(Mode::Light)),
            mode: None,
            shift: None,
            running: None,
            main: None,
            window_shown: false,
            requested: false,
            intents: HashMap::new(),
            exiting: false,
        };
        let tasks = Task::batch([
            system::theme().map(Message::ThemeChanged),
            work::start_engine().map(Message::EngineStarted),
        ]);
        (state, tasks)
    }

    pub fn title(&self, _window: window::Id) -> String {
        "Gracil".into()
    }

    pub fn theme(&self, _window: window::Id) -> Theme {
        self.theme.clone()
    }

    pub fn subscription(&self) -> Subscription<Message> {
        Subscription::batch([
            event::listen_with(|event, _status, window| match event {
                Event::Window(window::Event::FileHovered(_)) => Some(Message::FilesHovered),
                Event::Window(window::Event::FilesHoveredLeft) => Some(Message::FilesHoveredLeft),
                Event::Window(window::Event::FileDropped(path)) => Some(Message::FileDropped(path)),
                Event::Window(window::Event::CloseRequested) => Some(Message::CloseRequested),
                Event::Window(window::Event::Rescaled(_)) => Some(Message::Rescaled(window)),
                _ => None,
            }),
            system::theme_changes().map(Message::ThemeChanged),
            outside::subscription().map(Message::Outside),
            // Frames only while the theme is changing (spec §51).
            if self.shift.is_some() {
                window::frames().map(Message::ThemeFrame)
            } else {
                Subscription::none()
            },
        ])
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        let task = self.handle(message);
        if self.exiting && self.settled() {
            self.leave();
        }
        if self.background_work_done() {
            info!("background work done; leaving");
            self.leave();
        }
        task
    }

    fn handle(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::EngineStarted(engine) => {
                self.engine = match engine {
                    Some(engine) => EngineState::Ready(engine),
                    None => EngineState::Unavailable,
                };
                self.schedule()
            }
            Message::FilesHovered => {
                self.hovering_files = true;
                Task::none()
            }
            Message::FilesHoveredLeft => {
                self.hovering_files = false;
                Task::none()
            }
            Message::FileDropped(path) => {
                self.hovering_files = false;
                self.add(vec![path])
            }
            Message::ChooseFiles => Task::perform(choose_files(self.text), Message::FilesChosen),
            Message::FilesChosen(paths) => self.add(paths),
            Message::Job(id, report) => self.on_report(id, report),
            Message::Cancel => {
                if let Some((_, control)) = &self.running {
                    control.cancel();
                }
                Task::none()
            }
            Message::Pause => {
                self.pause(true);
                Task::none()
            }
            Message::Resume => {
                self.pause(false);
                Task::none()
            }
            Message::Choose(id, choice) => self.choose(id, choice),
            Message::ChooseAll(choice) => {
                let waiting: Vec<JobId> = self
                    .queue
                    .jobs()
                    .iter()
                    .map(|job| job.id)
                    .filter(|id| self.outcomes.get(id).is_some_and(Outcome::is_waiting))
                    .collect();
                Task::batch(waiting.into_iter().map(|id| self.choose(id, choice)))
            }
            Message::Placed(id, choice, placed) => {
                self.placed(id, choice, placed);
                Task::none()
            }
            Message::Select(id) => {
                self.selected = Some(id);
                Task::none()
            }
            Message::Remove(id) => {
                self.remove(id);
                Task::none()
            }
            Message::Clear => {
                let ids: Vec<JobId> = self.queue.jobs().iter().map(|job| job.id).collect();
                for id in ids {
                    self.remove(id);
                }
                if self.queue.is_empty() {
                    self.drawer_open = false;
                }
                Task::none()
            }
            Message::ToggleDrawer => {
                self.drawer_open = !self.drawer_open;
                Task::none()
            }
            Message::ThemeChanged(mode) => {
                self.follow_theme(mode, Instant::now());
                Task::none()
            }
            Message::ThemeFrame(now) => {
                self.shift_theme(now);
                Task::none()
            }
            Message::WindowReady(handle) => {
                if let Err(error) = gracil_windows::set_window_icon(handle) {
                    warn!("could not set the window icon: {error}");
                }
                Task::none()
            }
            Message::Rescaled(window) => {
                window::raw_id::<Message>(window).map(Message::WindowReady)
            }
            Message::CloseRequested => {
                // Until the tray arrives, closing quits: the running compression is cancelled and
                // Gracil leaves once it has tidied up (see `update`).
                self.exiting = true;
                if let Some((_, control)) = &self.running {
                    control.cancel();
                }
                Task::none()
            }
            Message::Outside(Outside::Request(request)) => self.on_request(request),
        }
    }

    /// Adds what was asked for, and shows the window when the request wants it. A choice made in
    /// advance applies to each of its videos, including one already on the list.
    fn on_request(&mut self, request: Request) -> Task<Message> {
        self.requested = true;
        let show = if request.shows_window() {
            self.show_main()
        } else {
            Task::none()
        };
        let paths: Vec<PathBuf> = files(request.paths).collect();
        let added = self.add(paths.clone());
        let mut tasks = vec![show, added];
        if let Some(choice) = request.choice {
            let ids: Vec<JobId> = paths
                .iter()
                .filter_map(|path| self.queue.find(path).map(|job| job.id))
                .collect();
            for id in ids {
                self.intents.insert(id, choice);
                if self.outcomes.get(&id).is_some_and(Outcome::is_waiting) {
                    tasks.push(self.choose(id, choice));
                }
            }
        }
        Task::batch(tasks)
    }

    /// Opens the main window, or brings it forward if it is open already.
    fn show_main(&mut self) -> Task<Message> {
        if let Some(id) = self.main {
            return Task::batch([window::minimize(id, false), window::gain_focus(id)]);
        }
        let (id, open) = window::open(window::Settings {
            size: Size::new(440.0, 440.0),
            min_size: Some(Size::new(360.0, 340.0)),
            exit_on_close_request: false,
            ..window::Settings::default()
        });
        self.main = Some(id);
        self.window_shown = true;
        open.then(window::raw_id::<Message>)
            .map(Message::WindowReady)
    }

    /// Whether nothing is in the middle of being compressed or put in place, so leaving loses no
    /// work and leaves no half-written file.
    fn settled(&self) -> bool {
        self.running.is_none()
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
    fn leave(&self) -> ! {
        for outcome in self.outcomes.values() {
            if let Outcome::Ready { temp, .. } = outcome {
                work::discard(temp);
            }
        }
        info!("Gracil exited");
        logging::finish();
        std::process::exit(0)
    }

    /// Whether this Gracil was started only for background work and that work is over: nothing
    /// waiting or running, and no result left for anyone to decide on (spec §53).
    fn background_work_done(&self) -> bool {
        self.requested
            && !self.window_shown
            && self.running.is_none()
            && !self
                .queue
                .jobs()
                .iter()
                .any(|job| job.state == JobState::Pending)
            && !self
                .outcomes
                .values()
                .any(|outcome| matches!(outcome, Outcome::Ready { .. }))
    }

    /// The video shown large: the one the user picked; otherwise the running one; otherwise a
    /// result waiting for a choice; otherwise the next one waiting; otherwise the one just put in
    /// place; otherwise the last one done.
    pub fn focused(&self) -> Option<&Job> {
        let jobs = self.queue.jobs();
        self.selected
            .and_then(|id| self.queue.get(id))
            .or_else(|| self.queue.active())
            .or_else(|| {
                jobs.iter()
                    .find(|job| self.outcomes.get(&job.id).is_some_and(Outcome::is_waiting))
            })
            .or_else(|| jobs.iter().find(|job| job.state == JobState::Pending))
            .or_else(|| self.last_placed.and_then(|id| self.queue.get(id)))
            .or_else(|| jobs.iter().rev().find(|job| job.state.is_finished()))
    }

    /// How many results are waiting for the user to choose replace or copy.
    pub fn waiting_count(&self) -> usize {
        self.outcomes.values().filter(|o| o.is_waiting()).count()
    }

    /// Whether a video can leave the list now: it is not running and not being put in place.
    pub fn can_remove(&self, job: &Job) -> bool {
        job.state.is_removable()
            && !matches!(
                self.outcomes.get(&job.id),
                Some(Outcome::Ready { placing: true, .. })
            )
    }

    fn add(&mut self, paths: Vec<PathBuf>) -> Task<Message> {
        let added = self.queue.add(files(paths));
        if added > 0 && matches!(self.engine, EngineState::Unavailable) {
            self.engine = EngineState::Starting;
            return work::start_engine().map(Message::EngineStarted);
        }
        self.schedule()
    }

    fn choose(&mut self, id: JobId, choice: Choice) -> Task<Message> {
        let Some(source) = self.queue.get(id).map(|job| job.path.clone()) else {
            return Task::none();
        };
        let Some(Outcome::Ready {
            temp,
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
        work::place(temp.clone(), source, choice)
            .map(move |placed| Message::Placed(id, choice, placed))
    }

    fn placed(&mut self, id: JobId, choice: Choice, placed: Result<PathBuf, Failure>) {
        let Some(outcome) = self.outcomes.get_mut(&id) else {
            return;
        };
        let Outcome::Ready {
            smaller_by,
            placing,
            problem,
            ..
        } = outcome
        else {
            return;
        };
        match placed {
            Ok(path) => {
                let smaller_by = *smaller_by;
                self.last_placed = Some(id);
                *outcome = Outcome::Placed {
                    choice,
                    path,
                    smaller_by,
                };
            }
            Err(failure) => {
                *placing = false;
                *problem = Some(failure);
            }
        }
    }

    /// Takes a video off the list. A result nobody chose for is deleted; the original stays.
    fn remove(&mut self, id: JobId) {
        let removable = self.queue.get(id).is_some_and(|job| self.can_remove(job));
        if !removable || !self.queue.remove(id) {
            return;
        }
        self.intents.remove(&id);
        if let Some(Outcome::Ready { temp, .. }) = self.outcomes.remove(&id) {
            work::discard(&temp);
        }
        if self.selected == Some(id) {
            self.selected = None;
        }
        if self.last_placed == Some(id) {
            self.last_placed = None;
        }
    }

    /// Starts the next video when the engine is ready and nothing is running.
    fn schedule(&mut self) -> Task<Message> {
        let EngineState::Ready(engine) = &self.engine else {
            return Task::none();
        };
        let Some(job) = self.queue.start_next() else {
            return Task::none();
        };
        let (id, path) = (job.id, job.path.clone());
        let control = JobControl::default();
        self.running = Some((id, control.clone()));
        work::compress_video(engine.clone(), path, control)
            .map(move |report| Message::Job(id, report))
    }

    fn on_report(&mut self, id: JobId, report: Report) -> Task<Message> {
        match report {
            Report::Progress(fraction) => {
                let state = self.queue.get(id).map(|job| job.state);
                if state == Some(JobState::Preparing) {
                    self.transition(id, JobEvent::BeginCompressing);
                }
                self.queue.set_fraction(id, fraction);
                Task::none()
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
                self.outcomes.insert(id, outcome);
                self.running = None;
                let place = match self.intents.remove(&id) {
                    Some(choice) => self.choose(id, choice),
                    None => Task::none(),
                };
                Task::batch([place, self.schedule()])
            }
        }
    }

    /// Freezes or thaws the running compression (spec §47). Only a compression that has started
    /// can pause; a failure leaves it as it was and is only logged.
    /// Follows Windows between light and dark. The first answer is applied at once, so Gracil
    /// opens in the right colors; later changes fade softly (ADR 0015).
    fn follow_theme(&mut self, mode: Mode, now: Instant) {
        if self.mode == Some(mode) {
            return;
        }
        let first = self.mode.is_none();
        self.mode = Some(mode);
        let target = style::palette(mode);
        if first {
            self.theme = style::theme(target);
            self.shift = None;
        } else {
            // Starting from what is on screen, so changing back halfway is still smooth.
            self.shift = Some((self.theme.palette(), target, now));
        }
    }

    fn shift_theme(&mut self, now: Instant) {
        let Some((from, to, start)) = self.shift else {
            return;
        };
        let progress = (now.saturating_duration_since(start).as_secs_f32()
            / style::THEME_SHIFT.as_secs_f32())
        .min(1.0);
        self.theme = style::theme(style::blend(from, to, ease_in_out(progress)));
        if progress >= 1.0 {
            self.shift = None;
        }
    }

    fn pause(&mut self, paused: bool) {
        let Some((id, control)) = &self.running else {
            return;
        };
        let id = *id;
        let state = self.queue.get(id).map(|job| job.state);
        let (result, event) = match (paused, state) {
            (true, Some(JobState::Compressing)) => (
                control.pause(gracil_windows::set_suspended),
                JobEvent::Pause,
            ),
            (false, Some(JobState::Paused)) => (control.resume(), JobEvent::Resume),
            _ => return,
        };
        match result {
            Ok(()) => self.transition(id, event),
            Err(error) => warn!(%error, paused, "could not pause or resume"),
        }
    }

    fn transition(&mut self, id: JobId, event: JobEvent) {
        if let Err(error) = self.queue.apply(id, event) {
            warn!(%error, "ignored job transition");
        }
    }
}

/// Folders and anything else that is not a file are ignored; FFprobe later decides what is video.
fn files(paths: impl IntoIterator<Item = PathBuf>) -> impl Iterator<Item = PathBuf> {
    paths.into_iter().filter(|path| path.is_file())
}

/// What the file picker shows by default. Only a convenience: any file FFmpeg reads is accepted.
const VIDEO_EXTENSIONS: &[&str] = &[
    "mp4", "m4v", "mov", "mkv", "avi", "wmv", "webm", "mts", "m2ts", "ts", "3gp", "flv", "mpg",
    "mpeg",
];

async fn choose_files(text: &'static Text) -> Vec<PathBuf> {
    rfd::AsyncFileDialog::new()
        .set_title(text.choose_videos)
        .add_filter(text.videos_filter, VIDEO_EXTENSIONS)
        .add_filter(text.all_files_filter, &["*"])
        .pick_files()
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|file| file.path().to_owned())
        .collect()
}

/// Slow at both ends, so the change of theme settles in rather than snapping.
fn ease_in_out(t: f32) -> f32 {
    t * t * (3.0 - 2.0 * t)
}
