//! Application state and how it reacts to the user, the window and background work.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use gracil_core::compress::Engine;
use gracil_core::encode::JobControl;
use gracil_core::job::JobEvent;
use gracil_core::queue::{JobId, Queue};
use iced::theme::Mode;
use iced::{Event, Size, Subscription, Task, Theme, event, system, window};
use tracing::warn;

use crate::style;
use crate::work::{self, Outcome, Report};

pub struct Gracil {
    pub queue: Queue,
    pub outcomes: HashMap<JobId, Outcome>,
    pub engine: EngineState,
    pub hovering_files: bool,
    pub drawer_open: bool,
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
    ToggleDrawer,
    ThemeChanged(Mode),
    CloseRequested,
}

impl Gracil {
    pub fn boot() -> (Self, Task<Message>) {
        let (_, open) = window::open(window::Settings {
            size: Size::new(440.0, 420.0),
            min_size: Some(Size::new(360.0, 320.0)),
            exit_on_close_request: false,
            ..window::Settings::default()
        });
        let mut state = Self {
            queue: Queue::default(),
            outcomes: HashMap::new(),
            engine: EngineState::Starting,
            hovering_files: false,
            drawer_open: false,
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
            Message::ToggleDrawer => {
                self.drawer_open = !self.drawer_open;
                Task::none()
            }
            Message::ThemeChanged(mode) => {
                self.theme = style::theme(mode);
                Task::none()
            }
            Message::CloseRequested => {
                // Until the tray arrives (M5), closing quits. FFmpeg must not outlive Gracil.
                if let Some((_, control)) = &self.running {
                    control.cancel();
                }
                iced::exit()
            }
        }
    }

    fn add(&mut self, paths: Vec<PathBuf>) -> Task<Message> {
        let added = self.queue.add(files(paths));
        if added > 0 && matches!(self.engine, EngineState::Unavailable) {
            self.engine = EngineState::Starting;
            return work::start_engine().map(Message::EngineStarted);
        }
        self.schedule()
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
                if state == Some(gracil_core::job::JobState::Preparing) {
                    self.transition(id, JobEvent::BeginCompressing);
                }
                self.queue.set_fraction(id, fraction);
                Task::none()
            }
            Report::Done(outcome) => {
                let events: &[JobEvent] = match outcome {
                    Outcome::Saved { .. } => &[
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
