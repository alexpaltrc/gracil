//! Application state and how it reacts to the user, the window and background work.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use gracil_core::compress::Engine;
use gracil_core::encode::JobControl;
use gracil_core::job::{JobEvent, JobState};
use gracil_core::queue::{Job, JobId, Queue};
use gracil_windows::{Tray, TrayEvent, TrayMenu, TrayState};
use iced::theme::{Mode, Palette};
use iced::window::settings::PlatformSpecific;
use iced::window::settings::platform::CornerPreference;
use iced::{Event, Size, Subscription, Task, Theme, event, system, window};
use tracing::{info, warn};

use crate::logging;
use crate::outside::{self, Outside};
use crate::request::Request;
use crate::settings::Settings;
use crate::style;
use crate::text::{Language, Text};
use crate::work::{self, Choice, Failure, Outcome, Report};

pub struct Gracil {
    pub queue: Queue,
    pub outcomes: HashMap<JobId, Outcome>,
    pub engine: EngineState,
    /// The window files are being dragged over, if any.
    pub hovering: Option<window::Id>,
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
    /// Whether Gracil was started with Windows. It then stays in the tray for the whole session
    /// instead of leaving when background work is done (ADR 0024).
    resident: bool,
    /// Choices made before compressing, by File Explorer's commands: the result is placed as soon
    /// as it is ready.
    intents: HashMap<JobId, Choice>,
    /// Leaving was asked for; Gracil leaves as soon as nothing is half done.
    exiting: bool,
    /// The icon next to the clock, unless Windows refused it.
    tray: Option<Tray>,
    /// What the tray icon was last told to show, so it is only told about changes.
    tray_shown: Option<(TrayState, String)>,
    /// Exit was chosen while work was in progress; the window asks before stopping it (§54.1).
    pub confirming_exit: bool,
    /// Something went wrong while no window was open; the tray icon says so until one opens.
    unseen_problem: bool,
    /// The mini-window next to the tray icon, while it is open (spec §55).
    mini: Option<window::Id>,
    settings: Settings,
}

/// The mini-window's size, in logical pixels.
const MINI_SIZE: Size = Size::new(340.0, 108.0);

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
    FilesHovered(window::Id),
    FilesHoveredLeft(window::Id),
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
    CloseRequested(window::Id),
    /// Exit stands, even though work stops.
    ConfirmExit,
    /// Exit is called off.
    KeepWorking,
    /// The mini-window's "Open Gracil".
    OpenMain,
    CloseMini,
    /// The mini-window exists, still hidden, and can be put next to the tray icon.
    MiniReady(u64),
    /// The mini-window is on screen; it is put in place again in case the screen's scale
    /// changed its size.
    MiniShown(u64),
    Outside(Outside),
}

impl Gracil {
    /// Starts without a window: the first request (from the command line) decides whether one
    /// is needed (spec §62).
    pub fn boot() -> (Self, Task<Message>) {
        let text = Text::get(Language::detect());
        let settings = Settings::load();
        let menu = TrayMenu {
            open: text.open_gracil.to_owned(),
            explorer_mini_window: text.explorer_mini_window.to_owned(),
            exit: text.exit.to_owned(),
        };
        let tray = Tray::start(menu, &text.tray_tip(TrayState::Idle), |event| {
            outside::send(Outside::Tray(event));
        })
        .inspect_err(|error| warn!(%error, "no tray icon; closing the window will exit"))
        .ok();
        if let Some(tray) = &tray {
            tray.check_explorer_mini_window(settings.explorer_mini_window);
        }
        let state = Self {
            queue: Queue::default(),
            outcomes: HashMap::new(),
            engine: EngineState::Starting,
            hovering: None,
            drawer_open: false,
            selected: None,
            last_placed: None,
            text,
            theme: style::theme(style::palette(Mode::Light)),
            mode: None,
            shift: None,
            running: None,
            main: None,
            window_shown: false,
            requested: false,
            resident: false,
            intents: HashMap::new(),
            exiting: false,
            tray,
            tray_shown: None,
            confirming_exit: false,
            unseen_problem: false,
            mini: None,
            settings,
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
                Event::Window(window::Event::FileHovered(_)) => Some(Message::FilesHovered(window)),
                Event::Window(window::Event::FilesHoveredLeft) => {
                    Some(Message::FilesHoveredLeft(window))
                }
                Event::Window(window::Event::FileDropped(path)) => Some(Message::FileDropped(path)),
                Event::Window(window::Event::CloseRequested) => {
                    Some(Message::CloseRequested(window))
                }
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
        // The work finished while Exit was being asked about: nothing is left to stop, so the
        // question goes away.
        if self.confirming_exit && self.running.is_none() && self.waiting_to_compress() == 0 {
            self.confirming_exit = false;
        }
        if self.exiting && self.settled() {
            self.leave();
        }
        if self.background_work_done() {
            info!("background work done; leaving");
            self.leave();
        }
        self.sync_tray();
        task
    }

    /// Tells the tray icon what to show, when that changed (spec §54).
    fn sync_tray(&mut self) {
        let Some(tray) = &self.tray else {
            return;
        };
        let state = match &self.running {
            Some((id, _)) => match self.queue.get(*id) {
                Some(job) if job.state == JobState::Paused => TrayState::Paused,
                Some(job) => {
                    // Steps of 1/40 are plenty for a ring 16 pixels wide.
                    let done = job.fraction.unwrap_or(0.0).clamp(0.0, 1.0) as f32;
                    TrayState::Processing((done * 40.0).floor() / 40.0)
                }
                None => TrayState::Processing(0.0),
            },
            None if self.unseen_problem => TrayState::Error,
            None => TrayState::Idle,
        };
        // The tip shows whole percentages, so it follows the work more finely than the picture.
        let tip = match (state, self.running.as_ref()) {
            (TrayState::Processing(_), Some((id, _))) => {
                let done = self.queue.get(*id).and_then(|job| job.fraction);
                self.text
                    .tray_tip(TrayState::Processing(done.unwrap_or(0.0) as f32))
            }
            _ => self.text.tray_tip(state),
        };
        let look = (state, tip);
        if self.tray_shown.as_ref() != Some(&look) {
            tray.show(look.0, &look.1);
            self.tray_shown = Some(look);
        }
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
            Message::FilesHovered(window) => {
                self.hovering = Some(window);
                Task::none()
            }
            Message::FilesHoveredLeft(window) => {
                if self.hovering == Some(window) {
                    self.hovering = None;
                }
                Task::none()
            }
            Message::FileDropped(path) => {
                self.hovering = None;
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
            Message::CloseRequested(id) if self.mini == Some(id) => self.close_mini(),
            Message::CloseRequested(id) => {
                if self.tray.is_none() {
                    // Without the icon there would be no way back, so closing exits instead.
                    return self.ask_to_exit();
                }
                // Closing only hides Gracil; the work goes on in the tray (spec §54.1).
                if self.main == Some(id) {
                    self.main = None;
                    self.confirming_exit = false;
                }
                window::close(id)
            }
            Message::ConfirmExit => {
                self.exit_now();
                Task::none()
            }
            Message::KeepWorking => {
                self.confirming_exit = false;
                Task::none()
            }
            Message::OpenMain => self.show_main(),
            Message::CloseMini => self.close_mini(),
            Message::MiniReady(handle) => {
                self.place_mini(handle);
                match self.mini {
                    Some(id) => window::set_mode::<Message>(id, window::Mode::Windowed)
                        .chain(Task::done(Message::MiniShown(handle))),
                    None => Task::none(),
                }
            }
            Message::MiniShown(handle) => {
                self.place_mini(handle);
                Task::none()
            }
            Message::Outside(Outside::Request(request)) => self.on_request(request),
            Message::Outside(Outside::Tray(TrayEvent::Toggle)) => match self.mini {
                Some(_) => self.close_mini(),
                None => self.open_mini(),
            },
            Message::Outside(Outside::Tray(TrayEvent::Open)) => self.show_main(),
            Message::Outside(Outside::Tray(TrayEvent::ExplorerMiniWindow)) => {
                self.settings.explorer_mini_window = !self.settings.explorer_mini_window;
                if let Some(tray) = &self.tray {
                    tray.check_explorer_mini_window(self.settings.explorer_mini_window);
                }
                if let Err(error) = self.settings.save() {
                    warn!(%error, "could not save the settings");
                }
                Task::none()
            }
            Message::Outside(Outside::Tray(TrayEvent::Exit)) => self.ask_to_exit(),
        }
    }

    /// Opens the mini-window next to the tray icon (spec §55). It opens hidden, is put in place,
    /// and only then shown, so it never appears somewhere else first.
    fn open_mini(&mut self) -> Task<Message> {
        // What it shows includes whatever went wrong.
        self.unseen_problem = false;
        if let Some(id) = self.mini {
            return window::gain_focus(id);
        }
        let (id, open) = window::open(window::Settings {
            size: MINI_SIZE,
            resizable: false,
            minimizable: false,
            decorations: false,
            visible: false,
            level: window::Level::AlwaysOnTop,
            exit_on_close_request: false,
            platform_specific: PlatformSpecific {
                skip_taskbar: true,
                undecorated_shadow: true,
                corner_preference: CornerPreference::Round,
                ..PlatformSpecific::default()
            },
            ..window::Settings::default()
        });
        self.mini = Some(id);
        open.then(window::raw_id::<Message>).map(Message::MiniReady)
    }

    fn close_mini(&mut self) -> Task<Message> {
        match self.mini.take() {
            Some(id) => window::close(id),
            None => Task::none(),
        }
    }

    fn place_mini(&self, handle: u64) {
        if let Err(error) = gracil_windows::place_near_tray(handle, self.tray.as_ref()) {
            warn!(%error, "could not place the mini-window");
        }
    }

    /// Whether `window` is the mini-window.
    pub fn is_mini(&self, window: window::Id) -> bool {
        self.mini == Some(window)
    }

    /// Exit (spec §54.1): straight away when nothing is in progress; otherwise the window opens
    /// and says what would stop.
    fn ask_to_exit(&mut self) -> Task<Message> {
        if self.running.is_none() && self.waiting_to_compress() == 0 {
            self.exit_now();
            return Task::none();
        }
        self.confirming_exit = true;
        self.show_main()
    }

    /// Stops the running compression, leaving its original as it was, and leaves once it has
    /// tidied up (see `update`). Videos still waiting are not started.
    fn exit_now(&mut self) {
        self.exiting = true;
        self.confirming_exit = false;
        if let Some((_, control)) = &self.running {
            control.cancel();
        }
    }

    /// Videos on the list that haven't started compressing yet.
    pub fn waiting_to_compress(&self) -> usize {
        self.queue
            .jobs()
            .iter()
            .filter(|job| job.state == JobState::Pending)
            .count()
    }

    /// Whether a compression is under way, so Exit would stop it.
    pub fn compressing(&self) -> bool {
        self.running.is_some()
    }

    /// Adds what was asked for, and shows the window when the request wants it. A choice made in
    /// advance applies to each of its videos, including one already on the list.
    fn on_request(&mut self, request: Request) -> Task<Message> {
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
        // Whatever went wrong is in plain view now.
        self.unseen_problem = false;
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
        if let Some(tray) = &self.tray {
            tray.remove();
        }
        info!("Gracil exited");
        logging::finish();
        std::process::exit(0)
    }

    /// Whether this Gracil was started only for background work and that work is over: nothing
    /// waiting or running, and no result left for anyone to decide on (spec §53). While the
    /// mini-window is open, the user is still looking, so Gracil waits for it to close.
    fn background_work_done(&self) -> bool {
        self.requested
            && !self.resident
            && !self.window_shown
            && self.mini.is_none()
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
                self.note_problem();
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

    /// Marks a problem the user hasn't seen, when no window is open.
    fn note_problem(&mut self) {
        if self.main.is_none() && self.mini.is_none() {
            self.unseen_problem = true;
        }
    }

    /// Starts the next video when the engine is ready and nothing is running. Nothing new starts
    /// once Gracil is exiting.
    fn schedule(&mut self) -> Task<Message> {
        if self.exiting {
            return Task::none();
        }
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
                // A result that isn't smaller is no fault; the original simply stays.
                if matches!(outcome, Outcome::Failed(failure) if failure != Failure::NotSmaller) {
                    self.note_problem();
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

    /// Freezes or thaws the running compression (spec §47). Only a compression that has started
    /// can pause; a failure leaves it as it was and is only logged.
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

/// The picker shows videos by default. Only a convenience: any file FFmpeg reads is accepted.
async fn choose_files(text: &'static Text) -> Vec<PathBuf> {
    rfd::AsyncFileDialog::new()
        .set_title(text.choose_videos)
        .add_filter(text.videos_filter, gracil_core::media::VIDEO_EXTENSIONS)
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
