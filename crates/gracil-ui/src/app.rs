//! Application state and how it reacts to the user, the window and background work.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use gracil_core::compress::Engine;
use gracil_core::encode::JobControl;
use gracil_core::job::{JobEvent, JobState};
use gracil_core::queue::{Job, JobId, Queue};
use iced::theme::Mode;
use iced::{Event, Size, Subscription, Task, Theme, event, system, window};
use tracing::warn;

use crate::style;
use crate::work::{self, Choice, Failure, Outcome, Report};

pub struct Gracil {
    pub queue: Queue,
    pub outcomes: HashMap<JobId, Outcome>,
    pub engine: EngineState,
    pub hovering_files: bool,
    pub drawer_open: bool,
    /// The video the user picked in the drawer to look at, if any.
    pub selected: Option<JobId>,
    theme: Theme,
    running: Option<(JobId, JobControl)>,
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
    Choose(JobId, Choice),
    ChooseAll(Choice),
    Placed(JobId, Choice, Result<PathBuf, Failure>),
    Select(JobId),
    Remove(JobId),
    Clear,
    ToggleDrawer,
    ThemeChanged(Mode),
    CloseRequested,
}

impl Gracil {
    pub fn boot() -> (Self, Task<Message>) {
        let (_, open) = window::open(window::Settings {
            size: Size::new(440.0, 440.0),
            min_size: Some(Size::new(360.0, 340.0)),
            exit_on_close_request: false,
            ..window::Settings::default()
        });
        let mut state = Self {
            queue: Queue::default(),
            outcomes: HashMap::new(),
            engine: EngineState::Starting,
            hovering_files: false,
            drawer_open: false,
            selected: None,
            theme: style::theme(Mode::Light),
            running: None,
        };
        // Videos opened with Gracil from File Explorer arrive as arguments; they start once the
        // engine is ready.
        state
            .queue
            .add(files(std::env::args_os().skip(1).map(PathBuf::from)));
        let tasks = Task::batch([
            open.discard(),
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
            event::listen_with(|event, _status, _window| match event {
                Event::Window(window::Event::FileHovered(_)) => Some(Message::FilesHovered),
                Event::Window(window::Event::FilesHoveredLeft) => Some(Message::FilesHoveredLeft),
                Event::Window(window::Event::FileDropped(path)) => Some(Message::FileDropped(path)),
                Event::Window(window::Event::CloseRequested) => Some(Message::CloseRequested),
                _ => None,
            }),
            system::theme_changes().map(Message::ThemeChanged),
        ])
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
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
            Message::ChooseFiles => Task::perform(choose_files(), Message::FilesChosen),
            Message::FilesChosen(paths) => self.add(paths),
            Message::Job(id, report) => self.on_report(id, report),
            Message::Cancel => {
                if let Some((_, control)) = &self.running {
                    control.cancel();
                }
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
                self.theme = style::theme(mode);
                Task::none()
            }
            Message::CloseRequested => {
                // Until the tray arrives (M5), closing quits. FFmpeg must not outlive Gracil, and
                // results nobody chose for are not left behind (ADR 0009).
                if let Some((_, control)) = &self.running {
                    control.cancel();
                }
                for outcome in self.outcomes.values() {
                    if let Outcome::Ready { temp, .. } = outcome {
                        work::discard(temp);
                    }
                }
                iced::exit()
            }
        }
    }

    /// The video shown large: the one the user picked; otherwise the running one; otherwise a
    /// result waiting for a choice; otherwise the next one waiting; otherwise the last one done.
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
        if let Some(Outcome::Ready { temp, .. }) = self.outcomes.remove(&id) {
            work::discard(&temp);
        }
        if self.selected == Some(id) {
            self.selected = None;
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
                self.schedule()
            }
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

async fn choose_files() -> Vec<PathBuf> {
    rfd::AsyncFileDialog::new()
        .set_title("Choose videos")
        .add_filter("Videos", VIDEO_EXTENSIONS)
        .add_filter("All files", &["*"])
        .pick_files()
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|file| file.path().to_owned())
        .collect()
}
