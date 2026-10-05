//! Application state and how it reacts to the user, the window and background work.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use gracil_core::capabilities::VideoEncoder;
use gracil_core::compress::Engine;
use gracil_core::encode::JobControl;
use gracil_core::job::{JobEvent, JobState};
use gracil_core::media::Kind;
use gracil_core::plan;
use gracil_core::queue::{Job, JobId, Queue};
use gracil_windows::{Tray, TrayEvent, TrayMenu, TrayState};
use iced::theme::{Mode, Palette};
use iced::window::settings::PlatformSpecific;
use iced::window::settings::platform::CornerPreference;
use iced::{Event, Size, Subscription, Task, Theme, event, system, window};
use tracing::{info, warn};

use crate::logging;
use crate::outside::{self, Outside};
use crate::pace::{self, Warmth};
use crate::request::Request;
use crate::settings::Settings;
use crate::style;
use crate::text::{Language, Text};
use crate::unfinished::{self, Unfinished};
use crate::view::DrawerScroll;
use crate::work::{self, Choice, Failure, Outcome, Report, Sizes};

pub struct Gracil {
    pub queue: Queue,
    pub outcomes: HashMap<JobId, Outcome>,
    pub engine: EngineState,
    /// The window files are being dragged over, if any.
    pub hovering: Option<window::Id>,
    pub drawer_open: bool,
    /// How far down the list is scrolled and how tall its visible part is, so only the rows in
    /// sight are built (spec §77.3).
    pub drawer_scroll: DrawerScroll,
    /// The video the user picked in the drawer to look at, if any.
    pub selected: Option<JobId>,
    /// The last video put in place, so the window stays on it once nothing else needs a choice.
    last_placed: Option<JobId>,
    /// The interface's words, in the Windows display language.
    pub text: &'static Text,
    theme: Theme,
    /// The Windows theme being followed; unknown until Windows first answers.
    mode: Option<Mode>,
    /// The user's accent color in Windows, followed as it changes.
    accent: style::Accent,
    /// A change of colors in progress: where it started, where it goes and when.
    shift: Option<(Palette, Palette, Instant)>,
    /// Files with work in flight, getting ready or compressing, and how to pause or stop each.
    running: HashMap<JobId, JobControl>,
    /// Files got ready and waiting their turn to compress (ADR 0040).
    prepared: HashMap<JobId, work::Prepared>,
    /// Pause holds every file being worked on, and nothing new starts until Resume.
    pub paused: bool,
    /// How warm the computer is running (ADR 0043). Too hot holds everything, like Pause.
    pub warmth: Warmth,
    /// Every file being worked on is frozen: paused, or waiting for the computer to cool down.
    held: bool,
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
    /// How many requests from File Explorer are still being looked through (ADR 0037).
    pub gathering: usize,
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
    /// Windows just shown but still out of sight, until they have drawn themselves, so they never
    /// appear blank (white) first: their handle, how many frames they have started and where
    /// they belong.
    revealing: HashMap<window::Id, (u64, u8, (i32, i32))>,
    settings: Settings,
    /// Counts over the whole list, as of the last change.
    summary: Summary,
    /// Where this round of work started, kept until everything in it is done.
    round_from: Option<JobId>,
    /// The files left unfinished the last time Gracil closed, offered until they are taken up or
    /// let go (ADR 0042).
    pub offered: Vec<Unfinished>,
    /// What the list of unfinished files looked like when it was last kept, and when.
    kept: Option<(Kept, Instant)>,
    /// The unfinished files as they were when Exit stopped the work, kept as they were.
    leaving_with: Option<Vec<Unfinished>>,
}

/// Frames a new window starts before it comes into view: by the last, the first ones are drawn.
const FRAMES_BEFORE_REVEAL: u8 = 3;

/// How long after its first frame a new window comes into view, if it draws no more frames
/// meanwhile. A window with nothing moving in it draws only one frame; this leaves Windows a few
/// screen refreshes to put that frame on screen.
const SETTLE_AFTER_FIRST_FRAME: Duration = Duration::from_millis(50);

/// If a new window draws no frames, it comes into view anyway after this long. Generous, since the
/// very first frame after starting the computer can take a while, and a white window is worse
/// than one that appears a moment later.
const REVEAL_AT_THE_LATEST: Duration = Duration::from_secs(3);

/// A quick likeness of the list of unfinished files: when it changes, the list is kept again.
type Kept = (usize, usize, usize, usize);

/// The list of unfinished files is kept at most this often while it keeps changing, so a long
/// list isn't written out for every file done. A sudden close loses at most this much: those
/// files are simply compressed again.
const KEEP_EVERY: Duration = Duration::from_secs(30);

/// The mini-window's size, in logical pixels.
const MINI_SIZE: Size = Size::new(340.0, 108.0);

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
    /// Files in the round, each an equal part of the circle, and how many are done.
    pub parts: usize,
    pub done: usize,
    /// Share of the whole round done, `0.0..=1.0`.
    pub fraction: f64,
}

/// Counts and picks over the whole list, worked out in one pass after each change rather than each
/// time the window is drawn: with thousands of files on the list, going through it again for
/// every frame and every bit of progress kept the window from keeping up.
#[derive(Debug, Clone, Default)]
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
    working: Option<JobId>,
    /// The first result waiting for a choice.
    first_waiting_for_choice: Option<JobId>,
    /// The next file to compress.
    next_to_compress: Option<JobId>,
    /// The last file finished.
    last_finished: Option<JobId>,
    /// The first file of this round of work: the files done since Gracil last started working,
    /// and those still to do (ADR 0034, item 13).
    round_from: Option<JobId>,
    /// Files in this round, and how many of them are done.
    round: usize,
    round_done: usize,
    /// Files being compressed, whose progress moves the ring between whole files.
    compressing: Vec<JobId>,
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
    /// The "Always" pill: replace, ask (`None`) or save a copy for every video that finishes.
    Always(Option<Choice>),
    Placed(JobId, Choice, Result<PathBuf, Failure>),
    /// Files found in what File Explorer sent, with the choice it came with.
    Found(Option<Choice>, work::Found),
    /// A file picked in the list is shown large; picking it again goes back to the work.
    Select(JobId),
    /// Back from a file picked in the list to what Gracil is working on.
    ShowCurrent,
    Remove(JobId),
    Clear,
    ToggleDrawer,
    DrawerScrolled(DrawerScroll),
    ThemeChanged(Mode),
    ThemeFrame(Instant),
    /// The main window exists, still hidden; it is given its icon and shown out of sight.
    MainReady(window::Id, u64),
    /// A window's icon is picked again for its new scale.
    WindowReady(u64),
    /// A window is about to draw a frame; counted while it is being revealed.
    Frame(window::Id),
    /// A new window drew its first frame a moment ago: it comes into view.
    Drawn(window::Id),
    /// A new window drew nothing for too long: it comes into view anyway.
    Reveal(window::Id),
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
    /// Carry on with the files left unfinished last time (ADR 0042).
    ContinueUnfinished,
    /// Let the files left unfinished last time go.
    LetGoUnfinished,
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
        // Read from Windows itself: Iced only learns the theme once a window exists, so its first
        // answer would come too late and the first window would fade from light (ADR 0015).
        let mode =
            gracil_windows::light_apps().map(|light| if light { Mode::Light } else { Mode::Dark });
        let accent = windows_accent();
        gracil_windows::set_icon_theme(mode != Some(Mode::Dark));
        let state = Self {
            queue: Queue::default(),
            outcomes: HashMap::new(),
            engine: EngineState::Starting,
            hovering: None,
            drawer_open: false,
            drawer_scroll: DrawerScroll::default(),
            selected: None,
            last_placed: None,
            text,
            theme: style::theme(style::palette(mode.unwrap_or(Mode::Light), accent)),
            mode,
            accent,
            shift: None,
            running: HashMap::new(),
            prepared: HashMap::new(),
            paused: false,
            warmth: Warmth::Cool,
            held: false,
            main: None,
            window_shown: false,
            requested: false,
            resident: false,
            intents: HashMap::new(),
            gathering: 0,
            exiting: false,
            tray,
            tray_shown: None,
            confirming_exit: false,
            unseen_problem: false,
            mini: None,
            revealing: HashMap::new(),
            settings,
            summary: Summary::default(),
            round_from: None,
            offered: unfinished::load(),
            kept: None,
            leaving_with: None,
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
            // Frames only while a new window waits out of sight.
            if self.revealing.is_empty() {
                Subscription::none()
            } else {
                event::listen_raw(|event, _status, window| match event {
                    Event::Window(window::Event::RedrawRequested(_)) => {
                        Some(Message::Frame(window))
                    }
                    _ => None,
                })
            },
        ])
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        // Progress, frames and scrolling change nothing the counts depend on, and they are by far
        // the most frequent messages.
        let recount = !matches!(
            message,
            Message::Job(_, Report::Progress(_))
                | Message::ThemeFrame(_)
                | Message::Frame(_)
                | Message::DrawerScrolled(_)
        );
        let task = self.handle(message);
        if recount {
            self.summary = self.summarize();
            self.round_from = self.summary.round_from;
        }
        self.keep_unfinished();
        // The work finished while Exit was being asked about: nothing is left to stop, so the
        // question goes away.
        if self.confirming_exit && self.running.is_empty() && self.waiting_to_compress() == 0 {
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
        // Like the ring: the whole list when there is more than one file (ADR 0034, item 13).
        let done = self.working().map(|job| {
            self.list_progress()
                .map_or(job.fraction.unwrap_or(0.0), |list| list.fraction)
                .clamp(0.0, 1.0) as f32
        });
        let state = match done {
            Some(_) if self.paused => TrayState::Paused,
            // Steps of 1/40 are plenty for a ring 16 pixels wide.
            Some(done) => TrayState::Processing((done * 40.0).floor() / 40.0),
            None if self.unseen_problem => TrayState::Error,
            None => TrayState::Idle,
        };
        // The tip shows whole percentages, so it follows the work more finely than the picture.
        let tip = match (state, done) {
            (TrayState::Processing(_), Some(done)) => {
                self.text.tray_tip(TrayState::Processing(done))
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
                // Stop acts on every file being worked on; the files still waiting go on.
                for control in self.running.values() {
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
                self.schedule()
            }
            Message::Choose(id, choice) => self.choose(id, choice),
            Message::Always(always) => {
                self.settings.always = always;
                if let Err(error) = self.settings.save() {
                    warn!(%error, "could not save the settings");
                }
                Task::none()
            }
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
                self.selected = (self.selected != Some(id)).then_some(id);
                Task::none()
            }
            Message::Found(choice, found) => self.on_found(choice, found),
            Message::ShowCurrent => {
                self.selected = None;
                Task::none()
            }
            Message::Remove(id) => {
                self.remove(id);
                Task::none()
            }
            Message::ContinueUnfinished => self.continue_unfinished(),
            Message::LetGoUnfinished => {
                self.offered.clear();
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
                // A list opened again starts at the top.
                self.drawer_scroll = DrawerScroll::default();
                // Closing the list puts away what was picked in it, too.
                if !self.drawer_open {
                    self.selected = None;
                }
                Task::none()
            }
            Message::DrawerScrolled(scroll) => {
                self.drawer_scroll = scroll;
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
            Message::MainReady(id, handle) => {
                if let Err(error) = gracil_windows::set_window_icon(handle) {
                    warn!("could not set the window icon: {error}");
                }
                self.reveal(id, handle)
            }
            Message::WindowReady(handle) => {
                if let Err(error) = gracil_windows::set_window_icon(handle) {
                    warn!("could not set the window icon: {error}");
                }
                Task::none()
            }
            Message::Frame(id) => {
                let Some((_, frames, _)) = self.revealing.get_mut(&id) else {
                    return Task::none();
                };
                *frames += 1;
                match *frames {
                    1 => after(SETTLE_AFTER_FIRST_FRAME, Message::Drawn(id)),
                    FRAMES_BEFORE_REVEAL.. => {
                        self.bring_into_view(id);
                        Task::none()
                    }
                    _ => Task::none(),
                }
            }
            Message::Drawn(id) => {
                self.bring_into_view(id);
                Task::none()
            }
            Message::Reveal(id) => {
                if let Some((_, frames, _)) = self.revealing.get(&id) {
                    warn!(frames, "a new window took too long to draw; shown anyway");
                }
                self.bring_into_view(id);
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
                self.revealing.remove(&id);
                dismiss(id)
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
                    Some(id) => self
                        .reveal(id, handle)
                        .chain(Task::done(Message::MiniShown(handle))),
                    None => Task::none(),
                }
            }
            Message::MiniShown(handle) => {
                if !self.mini.is_some_and(|id| self.revealing.contains_key(&id)) {
                    self.place_mini(handle);
                }
                Task::none()
            }
            Message::Outside(Outside::Request(request)) => self.on_request(request),
            Message::Outside(Outside::Warmth(warmth)) => self.warmth(warmth),
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
            Message::Outside(Outside::Tray(TrayEvent::Colors)) => {
                self.follow_accent(Instant::now());
                Task::none()
            }
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
            Some(id) => {
                self.revealing.remove(&id);
                dismiss(id)
            }
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
        if self.running.is_empty() && self.waiting_to_compress() == 0 {
            self.exit_now();
            return Task::none();
        }
        self.confirming_exit = true;
        self.show_main()
    }

    /// The files not finished yet, to carry on with if Gracil closes now (ADR 0042): those still
    /// offered from last time, every file waiting or being worked on, and every result nobody
    /// chose for yet, which is deleted on leaving and so has to be made again.
    fn unfinished(&self) -> Vec<Unfinished> {
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
    fn keep_unfinished(&mut self) {
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

    /// Stops every file being worked on, leaving their originals as they were, and leaves once
    /// they have tidied up (see `update`). Files still waiting are not started.
    fn exit_now(&mut self) {
        self.exiting = true;
        self.confirming_exit = false;
        // Stopping marks the files being worked on as stopped; they are still to be done.
        let list = self.unfinished();
        unfinished::save_now(&list);
        self.leaving_with = Some(list);
        for control in self.running.values() {
            control.cancel();
        }
    }

    /// Whether `job` is shown with the ring: while it is worked on, and, while paused, while it
    /// waits, so Resume is always at hand.
    pub fn shows_dial(&self, job: &Job) -> bool {
        job.state.is_active()
            || (self.paused && matches!(job.state, JobState::Pending | JobState::Prepared))
    }

    /// Files on the list that haven't started compressing yet, including those already got ready.
    pub fn waiting_to_compress(&self) -> usize {
        self.summary.waiting_to_compress
    }

    /// Files on the list not finished yet; it counts down to nothing as the work goes on.
    pub fn left(&self) -> usize {
        self.summary.left
    }

    /// Goes through the whole list once and works out everything the window and the tray show
    /// about it (see [`Summary`]).
    fn summarize(&self) -> Summary {
        let mut summary = Summary::default();
        let mut busy = !self.running.is_empty() || self.gathering > 0;
        let mut finished = Finished::default();
        let (mut video_compressing, mut compressing, mut active) = (None, None, None);
        // A round starts when work begins with nothing left to do, so every file before the first
        // one not done belongs to an earlier round.
        let round_from = self.round_from.or_else(|| {
            self.queue
                .jobs()
                .iter()
                .find(|job| !job.state.is_finished())
                .map(|job| job.id)
        });
        for job in self.queue.jobs() {
            match job.state {
                JobState::Pending | JobState::Prepared => {
                    summary.waiting_to_compress += 1;
                    summary.next_to_compress.get_or_insert(job.id);
                }
                JobState::Compressing | JobState::Paused | JobState::Finalizing => {
                    summary.compressing.push(job.id);
                    if job.kind == Kind::Video {
                        video_compressing.get_or_insert(job.id);
                    }
                    compressing.get_or_insert(job.id);
                }
                _ => {}
            }
            if job.state.is_active() {
                active.get_or_insert(job.id);
            }
            if round_from.is_some_and(|from| job.id >= from) {
                summary.round += 1;
                summary.round_done += usize::from(job.state.is_finished());
            }
            if job.state.is_finished() {
                summary.last_finished = Some(job.id);
            } else {
                summary.left += 1;
                busy = true;
            }
            let outcome = self.outcomes.get(&job.id);
            if let Some(sizes) = outcome.and_then(Outcome::sizes) {
                summary.total = (summary.total.0 + sizes, summary.total.1 + 1);
            }
            match outcome {
                Some(Outcome::Ready { placing, .. }) => {
                    busy = true;
                    if !placing {
                        summary.waiting_for_choice += 1;
                        summary.first_waiting_for_choice.get_or_insert(job.id);
                    }
                }
                Some(Outcome::Placed { sizes, .. }) => {
                    finished.saved = finished.saved + *sizes;
                    finished.compressed += 1;
                }
                Some(Outcome::Failed(Failure::AsSmallAsItGets | Failure::NotSmaller)) => {
                    finished.already += 1;
                }
                Some(Outcome::Failed(_)) => finished.failed += 1,
                Some(Outcome::Cancelled) | None => {}
            }
        }
        summary.working = video_compressing.or(compressing).or(active);
        // Everything done: the next file starts a fresh round.
        summary.round_from = round_from.filter(|_| summary.left > 0);
        summary.finished = (!busy && finished.compressed + finished.already + finished.failed > 0)
            .then_some(finished);
        summary
    }

    /// How far along the whole list is, for the ring and the tray, when this round of work has
    /// more than one file (ADR 0034, item 13): one equal part per file, and the files being
    /// compressed partway through theirs. Adding files makes the parts smaller, so the marble
    /// moves back a little.
    pub fn list_progress(&self) -> Option<ListProgress> {
        let summary = &self.summary;
        let round = summary.round_from.filter(|_| summary.round >= 2)?;
        let partway: f64 = summary
            .compressing
            .iter()
            .filter_map(|&id| self.queue.get(id)?.fraction)
            .map(|fraction| fraction.clamp(0.0, 1.0))
            .sum();
        Some(ListProgress {
            round,
            parts: summary.round,
            done: summary.round_done,
            fraction: ((summary.round_done as f64 + partway) / summary.round as f64).min(1.0),
        })
    }

    /// How many files are being worked on, so Exit would stop them.
    pub fn being_worked_on(&self) -> usize {
        self.running.len()
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
        // Folders from File Explorer are looked through away from the window, which a slow
        // drive would otherwise freeze; files join the list folder by folder (ADR 0035, 0037).
        self.gathering += 1;
        let choice = request.choice;
        let gather = work::gather(request.paths, request.only)
            .map(move |found| Message::Found(choice, found));
        Task::batch([show, gather])
    }

    /// Adds the files left unfinished last time, each with the choice File Explorer made for it.
    /// Whether they are still there is checked away from the window, as for File Explorer's
    /// files, since there can be thousands of them.
    fn continue_unfinished(&mut self) -> Task<Message> {
        let offered = std::mem::take(&mut self.offered);
        let mut tasks = Vec::new();
        for choice in [None, Some(Choice::Replace), Some(Choice::SaveCopy)] {
            let paths: Vec<PathBuf> = offered
                .iter()
                .filter(|file| file.choice == choice)
                .map(|file| file.path.clone())
                .collect();
            if paths.is_empty() {
                continue;
            }
            self.gathering += 1;
            tasks.push(work::gather(paths, None).map(move |found| Message::Found(choice, found)));
        }
        Task::batch(tasks)
    }

    fn on_found(&mut self, choice: Option<Choice>, found: work::Found) -> Task<Message> {
        let paths = match found {
            work::Found::Files(paths) => paths,
            work::Found::Done => {
                self.gathering = self.gathering.saturating_sub(1);
                return Task::none();
            }
        };
        // Found in a folder listing or checked already, so nothing is asked of the drive again.
        let added = self.add_checked(paths.clone());
        let mut tasks = vec![added];
        if let Some(choice) = choice {
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
            visible: false,
            exit_on_close_request: false,
            ..window::Settings::default()
        });
        self.main = Some(id);
        self.window_shown = true;
        open.then(window::raw_id::<Message>)
            .map(move |handle| Message::MainReady(id, handle))
    }

    /// Shows a window that opened hidden, out of sight until it has drawn itself, so it never
    /// appears white first (`gracil_windows::park`). If it can't be moved aside it is simply shown.
    fn reveal(&mut self, id: window::Id, handle: u64) -> Task<Message> {
        let show = window::set_mode::<Message>(id, window::Mode::Windowed);
        let home = match gracil_windows::park(handle) {
            Ok(home) => home,
            Err(error) => {
                warn!(%error, "could not move a new window aside");
                return show;
            }
        };
        self.revealing.insert(id, (handle, 0, home));
        Task::batch([show, after(REVEAL_AT_THE_LATEST, Message::Reveal(id))])
    }

    fn bring_into_view(&mut self, id: window::Id) {
        let Some((handle, _, home)) = self.revealing.remove(&id) else {
            return;
        };
        if self.mini == Some(id) {
            // Placed again: the screen's scale may have changed its size meanwhile.
            self.place_mini(handle);
        } else if let Err(error) = gracil_windows::unpark(handle, home) {
            warn!(%error, "could not bring a new window into view");
        }
    }

    /// Whether nothing is in the middle of being compressed or put in place, so leaving loses no
    /// work and leaves no half-written file.
    fn settled(&self) -> bool {
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
    fn leave(&self) -> ! {
        match &self.leaving_with {
            Some(list) => unfinished::save_now(list),
            None => unfinished::save_now(&self.unfinished()),
        }
        for outcome in self.outcomes.values() {
            if let Outcome::Ready { temp, .. } = outcome {
                work::discard(temp);
            }
        }
        if let Some(tray) = &self.tray {
            tray.remove();
        }
        // A replaced video's parked original may still be in another program's hands for a
        // moment; anything left after this is removed at the next start.
        gracil_core::output::finish_tidying(Duration::from_secs(5));
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
            && self.running.is_empty()
            && self.waiting_to_compress() == 0
            && !self
                .outcomes
                .values()
                .any(|outcome| matches!(outcome, Outcome::Ready { .. }))
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

    fn job(&self, id: Option<JobId>) -> Option<&Job> {
        id.and_then(|id| self.queue.get(id))
    }

    /// The file to offer a way back to while the user looks at another one: only when Gracil is
    /// working on it or it waits for a choice (spec §32.1).
    pub fn way_back(&self) -> Option<&Job> {
        let picked = self.selected?;
        self.current().filter(|job| {
            job.id != picked
                && (job.state.is_active()
                    || self.outcomes.get(&job.id).is_some_and(Outcome::is_waiting))
        })
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
        self.working()
            .or_else(|| self.job(self.summary.first_waiting_for_choice))
            .or_else(|| self.focused())
    }

    /// What happens to every finished video without asking; `None` asks (ADR 0025).
    pub fn always(&self) -> Option<Choice> {
        self.settings.always
    }

    pub fn waiting_count(&self) -> usize {
        self.summary.waiting_for_choice
    }

    /// Whether a video can leave the list now: it is not running and not being put in place.
    pub fn can_remove(&self, job: &Job) -> bool {
        job.state.is_removable()
            && !matches!(
                self.outcomes.get(&job.id),
                Some(Outcome::Ready { placing: true, .. })
            )
    }

    /// Adds videos to the list. A video the list is done with (put in place, failed, cancelled)
    /// leaves it first, so the list never stops a video from being compressed again (spec §29).
    /// One still waiting, running or waiting for a choice stays as it is.
    fn add(&mut self, paths: Vec<PathBuf>) -> Task<Message> {
        self.add_checked(files(paths).collect())
    }

    /// [`Self::add`] for paths known to be files already.
    fn add_checked(&mut self, paths: Vec<PathBuf>) -> Task<Message> {
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
        let added = self.queue.add(paths);
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

    fn placed(&mut self, id: JobId, choice: Choice, placed: Result<PathBuf, Failure>) {
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
        self.prepared.remove(&id);
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

    /// Starts whatever there is room for (ADR 0040), once the engine is ready. Videos: one
    /// compresses while the next one gets ready. Images: a few at once beside it, as many as
    /// [`pace::Room`] allows, each read ahead of its turn. Nothing new starts while paused,
    /// while the computer cools down, or once Gracil is exiting.
    fn schedule(&mut self) -> Task<Message> {
        if self.exiting || self.held {
            return Task::none();
        }
        let EngineState::Ready(engine) = &self.engine else {
            return Task::none();
        };
        let engine = engine.clone();
        let compressing = |state: JobState| {
            matches!(
                state,
                JobState::Compressing | JobState::Paused | JobState::Finalizing
            )
        };
        let getting_ready =
            |state: JobState| matches!(state, JobState::Preparing | JobState::Prepared);
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
        let room = pace::Room::now(video_on_processor, self.warmth == Warmth::Warm);
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
    fn start_preparing(&mut self, engine: &Arc<Engine>, id: JobId) -> Task<Message> {
        let Some(path) = self.queue.get(id).map(|job| job.path.clone()) else {
            return Task::none();
        };
        let control = JobControl::default();
        self.running.insert(id, control.clone());
        work::prepare_file(engine.clone(), path, control)
            .map(move |report| Message::Job(id, report))
    }

    /// Starts compressing a file that was got ready.
    fn start_compressing(&mut self, engine: &Arc<Engine>, id: JobId) -> Task<Message> {
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

    fn on_report(&mut self, id: JobId, report: Report) -> Task<Message> {
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
                    self.note_problem();
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

    /// Follows Windows between light and dark. The first answer is applied at once, so Gracil
    /// opens in the right colors; later changes fade softly (ADR 0015).
    fn follow_theme(&mut self, mode: Mode, now: Instant) {
        if mode == Mode::None || self.mode == Some(mode) {
            return;
        }
        let first = self.mode.is_none();
        self.mode = Some(mode);
        gracil_windows::set_icon_theme(mode == Mode::Light);
        if first {
            self.theme = style::theme(style::palette(mode, self.accent));
            self.shift = None;
        } else {
            self.shift_to_palette(now);
        }
    }

    /// Picks up a new accent color from Windows, fading to it like a change of theme (ADR 0031).
    fn follow_accent(&mut self, now: Instant) {
        let accent = windows_accent();
        if accent != self.accent {
            self.accent = accent;
            self.shift_to_palette(now);
        }
    }

    /// Fades to the palette for the current mode and accent, starting from what is on screen, so
    /// changing back halfway is still smooth.
    fn shift_to_palette(&mut self, now: Instant) {
        let target = style::palette(self.mode.unwrap_or(Mode::Light), self.accent);
        self.shift = Some((self.theme.palette(), target, now));
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

    /// Pauses or resumes at the person's request (spec §47, §77.3). Pausing with nothing being
    /// worked on does nothing.
    fn pause(&mut self, paused: bool) {
        if self.paused == paused || (paused && self.running.is_empty()) {
            return;
        }
        self.paused = paused;
        self.hold();
    }

    /// Takes in how warm the computer is running (ADR 0043). Too hot holds everything until it
    /// has cooled down; then work carries on by itself.
    fn warmth(&mut self, warmth: Warmth) -> Task<Message> {
        self.warmth = warmth;
        self.hold();
        self.schedule()
    }

    /// Whether everything waits for the computer to cool down (ADR 0043).
    pub fn cooling(&self) -> bool {
        self.warmth == Warmth::Hot
    }

    /// Freezes or thaws every file being worked on, all together, while paused or while the
    /// computer cools down. Nothing new starts while frozen. A failure leaves a file as it was
    /// and is only logged.
    fn hold(&mut self) {
        let held = self.paused || self.cooling();
        if self.held == held {
            return;
        }
        self.held = held;
        let running: Vec<(JobId, JobControl)> = self
            .running
            .iter()
            .map(|(id, control)| (*id, control.clone()))
            .collect();
        for (id, control) in running {
            let result = if held {
                control.pause(gracil_windows::set_suspended)
            } else {
                control.resume()
            };
            if let Err(error) = result {
                warn!(%error, held, "could not pause or resume");
                continue;
            }
            let state = self.queue.get(id).map(|job| job.state);
            match (held, state) {
                (true, Some(JobState::Compressing)) => self.transition(id, JobEvent::Pause),
                (false, Some(JobState::Paused)) => self.transition(id, JobEvent::Resume),
                _ => {}
            }
        }
    }

    fn transition(&mut self, id: JobId, event: JobEvent) {
        if let Err(error) = self.queue.apply(id, event) {
            warn!(%error, "ignored job transition");
        }
    }
}

/// Folders and anything else that is not a file are ignored; images are known by their name, and
/// FFprobe later decides what is video.
fn files(paths: impl IntoIterator<Item = PathBuf>) -> impl Iterator<Item = PathBuf> {
    paths.into_iter().filter(|path| path.is_file())
}

/// The picker shows videos and images by default. Only a convenience: any video FFmpeg reads is
/// accepted.
async fn choose_files(text: &'static Text) -> Vec<PathBuf> {
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

/// Slow at both ends, so the change of theme settles in rather than snapping.
fn ease_in_out(t: f32) -> f32 {
    t * t * (3.0 - 2.0 * t)
}

/// Delivers `message` after `delay`, waited out on a thread of its own so none of Iced's workers
/// is held up.
fn after(delay: Duration, message: Message) -> Task<Message> {
    let (done, waited) = iced::futures::channel::oneshot::channel();
    std::thread::spawn(move || {
        std::thread::sleep(delay);
        let _ = done.send(());
    });
    Task::perform(waited, move |_| message.clone())
}

/// Closes a window without a white flash: Windows animates a closing window, and once its surface
/// is gone that animation shows a blank white frame. Hidden first, the window leaves with its
/// picture intact, and only then is it destroyed.
fn dismiss(id: window::Id) -> Task<Message> {
    window::set_mode::<Message>(id, window::Mode::Hidden).chain(window::close(id))
}

/// The accent color chosen in Windows, or Windows' own blue if it can't be read.
fn windows_accent() -> style::Accent {
    let color = |[r, g, b]: [u8; 3]| iced::Color::from_rgb8(r, g, b);
    gracil_windows::accent().map_or(style::Accent::WINDOWS, |accent| style::Accent {
        on_light: color(accent.on_light),
        on_dark: color(accent.on_dark),
    })
}
