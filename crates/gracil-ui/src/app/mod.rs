//! Application state and how it reacts to the user, the window and background work.
//!
//! This file holds the state, the messages and where each message goes. The rest of Gracil's
//! behavior is in one file per subject, each adding to `Gracil` on its own:
//!
//! - `list`: files coming onto the list, the choice for each finished one, files leaving it.
//! - `scheduler`: which files are got ready and compressed when, and their progress.
//! - `summary`: what the window and the tray show about the list.
//! - `windows`: the main window, the mini-window and the tray icon.
//! - `look`: light, dark, the accent and the contrast themes.
//! - `keys`: the few keys Gracil answers.
//! - `leaving`: asking before leaving, keeping unfinished files, tidying up.

use std::collections::{HashMap, HashSet};
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
use iced::{Event, Size, Subscription, Task, Theme, event, keyboard, system, window};
use tracing::{info, warn};

use crate::focus::{self, Scope};
use crate::logging;
use crate::motion;
use crate::outside::{self, Outside};
use crate::pace::{self, Breath};
use crate::place;
use crate::request::Request;
use crate::settings::Settings;
use crate::style;
use crate::text::{Language, Text};
use crate::unfinished::{self, Unfinished};
use crate::view::DrawerScroll;
use crate::work::{self, Choice, Failure, Outcome, Report, Sizes};

mod keys;
mod leaving;
mod list;
mod look;
mod scheduler;
mod summary;
mod windows;

use keys::{Key, key_for};
use leaving::Kept;
use list::choose_files;
use look::{windows_accent, windows_contrast};
pub use summary::Finished;
use summary::{Outcomes, Summary, Tally};
use windows::{dismiss, give_icon};

pub struct Gracil {
    pub queue: Queue,
    pub outcomes: Outcomes,
    pub engine: EngineState,
    /// The window files are being dragged over, if any.
    pub hovering: Option<window::Id>,
    pub drawer_open: bool,
    /// The list was closed and is still folding away (ADR 0056).
    pub drawer_folding: bool,
    /// How far down the list is scrolled and how tall its visible part is, so only the rows in
    /// sight are built (spec §77.3).
    pub drawer_scroll: DrawerScroll,
    /// What the last Clear took off the list, while Undo can still put it back.
    pub cleared: Option<Vec<list::Taken>>,
    /// How many times Clear was pressed, so only the last one's Undo goes away on time.
    clears: u64,
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
    /// The colors of Windows' contrast theme while one is on: then they are all Gracil draws with.
    contrast: Option<style::Contrast>,
    /// A change of colors in progress: where it started, where it goes and when.
    shift: Option<(Palette, Palette, Instant)>,
    /// Files with work in flight, getting ready or compressing, and how to pause or stop each.
    running: HashMap<JobId, JobControl>,
    /// Files got ready and waiting their turn to compress (ADR 0040).
    prepared: HashMap<JobId, work::Prepared>,
    /// When the last video started compressing: the memory free beside it is read only once it
    /// has taken what it needs (ADR 0067).
    video_started: Option<Instant>,
    /// The memory free is to be read again soon, for a video waiting to get ready (ADR 0067).
    room_asked: bool,
    /// Pause holds every file being worked on, and nothing new starts until Resume.
    pub paused: bool,
    /// Whether the computer runs warm, and whether Gracil is resting a moment (ADR 0045).
    breath: Breath,
    /// Every file being worked on is frozen: paused, or resting a moment while the computer is
    /// warm.
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
    /// How many lots of files and folders are still being looked through (ADR 0037): dropped,
    /// chosen, from File Explorer or left unfinished.
    pub gathering: usize,
    /// Files dropped on the window that are still arriving, one at a time.
    dropped: Vec<PathBuf>,
    /// How many times Stop was pressed. Files found by a look started before the last press go
    /// straight to the files offered to carry on with (ADR 0044).
    stops: u64,
    /// Looks through folders still going on from before the last Stop.
    abandoned: usize,
    /// Leaving was asked for; Gracil leaves as soon as nothing is half done.
    exiting: bool,
    /// The icon next to the clock, unless Windows refused it.
    tray: Option<Tray>,
    /// What the tray icon was last told to show, so it is only told about changes.
    tray_shown: Option<(TrayState, String)>,
    /// Exit was chosen while work was in progress; the window asks before stopping it (§54.1).
    pub confirming_exit: bool,
    /// Prepare to delete Gracil was chosen: on leaving, Gracil takes itself out of Windows and
    /// shows its folder (ADR 0054). Keep working cancels it.
    pub preparing_to_delete: bool,
    /// Something went wrong while no window was open, with the file it happened to; the tray icon
    /// says so until a window opens.
    unseen_problem: Option<JobId>,
    /// The file that went wrong unseen, which the mini-window shows first while it is open.
    problem_in_mini: Option<JobId>,
    /// The mini-window next to the tray icon (spec §55), once made; it is kept, hidden, while
    /// closed.
    mini: Option<window::Id>,
    /// Its handle, once Windows has made it.
    mini_handle: Option<u64>,
    /// Whether the mini-window is open.
    mini_shown: bool,
    /// Windows just shown but still out of sight, until they have drawn themselves, so they never
    /// appear blank (white) first: their handle, how many frames they have started and where
    /// they belong.
    revealing: HashMap<window::Id, (u64, u8, (i32, i32))>,
    settings: Settings,
    /// Counts over the whole list, as of the last change.
    summary: Summary,
    /// What each file adds to the counts, so a change counts only the files it touched.
    tally: Tally,
    /// Where this round of work started, kept until everything in it is done.
    round_from: Option<JobId>,
    /// The files left unfinished the last time Gracil closed, offered until they are taken up or
    /// let go (ADR 0042).
    pub offered: Vec<Unfinished>,
    /// What the list of unfinished files looked like when it was last kept, and when.
    kept: Option<(Kept, Instant)>,
    /// The unfinished files as they were when Exit stopped the work, kept as they were.
    leaving_with: Option<Vec<Unfinished>>,
    /// Whether finding out what the computer can do has started. It waits for the first window,
    /// or the first files, so starting Gracil shows its window sooner (ADR 0066).
    engine_asked: bool,
    /// Files kept only online that are being downloaded before they are got ready (ADR 0066).
    downloading: HashSet<JobId>,
    /// What each look still going on was asked to look through, by its number, with the choice
    /// it came with: kept for next time if Gracil closes before it is done (ADR 0066).
    looks: HashMap<u64, (Vec<PathBuf>, Option<Choice>)>,
    /// How many looks were started, to number the next one.
    looks_started: u64,
    /// Stop was pressed in this round of work: the files it set aside mean the list isn't all
    /// done, so the finished view has no check mark (ADR 0066).
    stopped_in_round: bool,
    /// The file that went wrong the finished view's "!" showed last, so pressing it again shows
    /// the next one.
    last_problem: Option<JobId>,
}

#[derive(Debug, Clone)]
pub enum EngineState {
    Starting,
    Ready(Arc<Engine>),
    /// No usable FFmpeg was found: the `ffmpeg` folder beside `gracil.exe` is missing or broken.
    /// Retried whenever the user adds videos.
    Unavailable,
}

#[derive(Debug, Clone)]
pub enum Message {
    EngineStarted(Option<Arc<Engine>>),
    FilesHovered(window::Id),
    FilesHoveredLeft(window::Id),
    FileDropped(PathBuf),
    /// Every file dropped together has arrived.
    DropLanded,
    ChooseFiles,
    FilesChosen(Vec<PathBuf>),
    Job(JobId, Report),
    /// Time to read the memory free again, for a video waiting to get ready beside the one
    /// compressing (ADR 0067).
    RoomAgain,
    Cancel,
    Pause,
    Resume,
    Choose(JobId, Choice),
    ChooseAll(Choice),
    /// The "Always" pill: replace, ask (`None`) or save a copy for every video that finishes.
    Always(Option<Choice>),
    Placed(JobId, Choice, Result<PathBuf, Failure>),
    /// Files found in what File Explorer sent, with the choice it came with, by a look started
    /// after that many presses of Stop, and the look's number.
    Found(u64, u64, Option<Choice>, work::Found),
    /// A file picked in the list is shown large; picking it again goes back to the work.
    Select(JobId),
    /// Back from a file picked in the list to what Gracil is working on.
    ShowCurrent,
    Remove(JobId),
    Clear,
    /// Puts back what Clear took off the list.
    Undo,
    /// Undo for that press of Clear is gone.
    UndoGone(u64),
    ToggleDrawer,
    /// The list finished folding away after being closed.
    DrawerFolded,
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
    /// Time to make the mini-window, hidden, so the first click on the tray icon shows it at once.
    MakeMini,
    /// The mini-window exists, still hidden, and can be put next to the tray icon.
    MiniReady(u64),
    /// The mini-window is on screen; it is put in place again in case the screen's scale
    /// changed its size.
    MiniShown(u64),
    /// Carry on with the files left unfinished last time (ADR 0042).
    ContinueUnfinished,
    /// Let the files left unfinished last time go.
    LetGoUnfinished,
    /// A key that means something to Gracil was pressed in one of its windows.
    Key(window::Id, Key),
    /// The finished view's count of files that couldn't be compressed: the first of them is
    /// shown, or, pressed again, the next one.
    ShowProblem,
    /// The same count in the mini-window: the big window opens on that file.
    OpenProblem,
    /// Time to find out what the computer can do, if nothing asked for it sooner (ADR 0066).
    StartEngine,
    /// The mini-window's "!": the big window opens on that file, with the reason in words.
    Explain(JobId),
    Outside(Outside),
}

impl Gracil {
    /// Starts without a window: the first request (from the command line) decides whether one
    /// is needed (spec §62).
    pub fn boot() -> (Self, Task<Message>) {
        let text = Text::get(Language::detect());
        let settings = Settings::load();
        place::settle(settings.place());
        let menu = TrayMenu {
            open: text.open_gracil.to_owned(),
            explorer_menu: text.explorer_menu.to_owned(),
            explorer_main_menu: text.explorer_main_menu.to_owned(),
            explorer_mini_window: text.explorer_mini_window.to_owned(),
            start_with_windows: text.start_with_windows.to_owned(),
            prepare_to_delete: text.prepare_to_delete.to_owned(),
            exit: text.exit.to_owned(),
        };
        let tray = Tray::start(menu, &text.tray_tip(TrayState::Idle), outside::from_tray)
            .inspect_err(|error| warn!(%error, "no tray icon; closing the window will exit"))
            .ok();
        if let Some(tray) = &tray {
            tray.check_explorer_mini_window(settings.explorer_mini_window);
            tray.check_explorer_menu(settings.explorer_menu);
            tray.check_explorer_main_menu(place::main_menu_is_ours());
            tray.check_start_with_windows(settings.start_with_windows);
        }
        // Read from Windows itself: Iced only learns the theme once a window exists, so its first
        // answer would come too late and the first window would fade from light (ADR 0015).
        let mode =
            gracil_windows::light_apps().map(|light| if light { Mode::Light } else { Mode::Dark });
        let accent = windows_accent();
        let contrast = windows_contrast();
        style::set_contrast(contrast);
        motion::follow_windows();
        gracil_windows::set_icon_theme(mode != Some(Mode::Dark));
        let state = Self {
            queue: Queue::default(),
            outcomes: Outcomes::default(),
            tally: Tally::default(),
            engine: EngineState::Starting,
            hovering: None,
            drawer_open: false,
            drawer_folding: false,
            drawer_scroll: DrawerScroll::default(),
            cleared: None,
            clears: 0,
            selected: None,
            last_placed: None,
            text,
            theme: style::theme(contrast.map_or_else(
                || style::palette(mode.unwrap_or(Mode::Light), accent),
                style::contrast_palette,
            )),
            mode,
            accent,
            contrast,
            shift: None,
            running: HashMap::new(),
            prepared: HashMap::new(),
            video_started: None,
            room_asked: false,
            paused: false,
            breath: Breath::default(),
            held: false,
            main: None,
            window_shown: false,
            requested: false,
            resident: false,
            intents: HashMap::new(),
            gathering: 0,
            dropped: Vec::new(),
            stops: 0,
            abandoned: 0,
            exiting: false,
            tray,
            tray_shown: None,
            confirming_exit: false,
            preparing_to_delete: false,
            unseen_problem: None,
            problem_in_mini: None,
            mini: None,
            mini_handle: None,
            mini_shown: false,
            revealing: HashMap::new(),
            settings,
            summary: Summary::default(),
            round_from: None,
            offered: unfinished::load(),
            kept: None,
            leaving_with: None,
            engine_asked: false,
            downloading: HashSet::new(),
            looks: HashMap::new(),
            looks_started: 0,
            stopped_in_round: false,
            last_problem: None,
        };
        // The engine starts once the first window is in view or the first files come, whichever
        // is first (ADR 0066); a Gracil resting in the tray starts it a while later regardless.
        let tasks = Task::batch([
            system::theme().map(Message::ThemeChanged),
            after(windows::MINI_MADE_AFTER, Message::MakeMini),
            after(ENGINE_WHEN_IDLE, Message::StartEngine),
        ]);
        (state, tasks)
    }

    /// Starts finding FFmpeg and what the computer can do, unless that has started already.
    pub(super) fn start_engine(&mut self) -> Task<Message> {
        if std::mem::replace(&mut self.engine_asked, true) {
            return Task::none();
        }
        work::start_engine().map(Message::EngineStarted)
    }

    /// The same words as the tray's tip, so the taskbar and Alt+Tab say how the work goes too.
    pub fn title(&self, _window: window::Id) -> String {
        self.look().1
    }

    pub fn subscription(&self) -> Subscription<Message> {
        Subscription::batch([
            event::listen_with(|event, status, window| match event {
                Event::Window(window::Event::FileHovered(_)) => Some(Message::FilesHovered(window)),
                Event::Window(window::Event::FilesHoveredLeft) => {
                    Some(Message::FilesHoveredLeft(window))
                }
                Event::Window(window::Event::FileDropped(path)) => Some(Message::FileDropped(path)),
                Event::Window(window::Event::CloseRequested) => {
                    Some(Message::CloseRequested(window))
                }
                Event::Window(window::Event::Rescaled(_)) => Some(Message::Rescaled(window)),
                // Only keys no widget took.
                Event::Keyboard(keyboard::Event::KeyPressed { key, modifiers, .. })
                    if status == event::Status::Ignored =>
                {
                    key_for(&key, modifiers).map(|key| Message::Key(window, key))
                }
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
            Message::Job(_, Report::Progress(_) | Report::Downloading(_))
                | Message::ThemeFrame(_)
                | Message::Frame(_)
                | Message::DrawerScrolled(_)
        );
        let task = self.handle(message);
        if recount {
            self.summary = self.summarize();
            self.round_from = self.summary.round_from;
            // New work after Stop is a new round: what Stop set aside no longer counts against it.
            if self.summary.left > 0 {
                self.stopped_in_round = false;
            }
        }
        // Only files still being got ready can be downloading.
        if !self.downloading.is_empty() {
            let running = &self.running;
            self.downloading.retain(|id| running.contains_key(id));
        }
        self.keep_unfinished();
        // The work finished while Exit was being asked about: nothing is left to stop, so the
        // question goes away.
        if self.confirming_exit && self.nothing_to_lose() {
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
            // Windows hands over a drop one file at a time: they are gathered, and looked through
            // together once all have arrived.
            Message::FileDropped(path) => {
                self.hovering = None;
                self.dropped.push(path);
                if self.dropped.len() == 1 {
                    Task::done(Message::DropLanded)
                } else {
                    Task::none()
                }
            }
            Message::DropLanded => {
                let paths = std::mem::take(&mut self.dropped);
                self.look_through(paths, None, None)
            }
            Message::Key(window, key) => self.on_key(window, key),
            Message::ShowProblem => self.show_problem(),
            Message::OpenProblem => {
                let shown = self.show_problem();
                Task::batch([self.show_main(), shown])
            }
            Message::StartEngine => self.start_engine(),
            Message::ChooseFiles => Task::perform(choose_files(self.text), Message::FilesChosen),
            Message::FilesChosen(paths) => self.look_through(paths, None, None),
            Message::Job(id, Report::Downloading(downloading)) => {
                if downloading && self.running.contains_key(&id) {
                    self.downloading.insert(id);
                } else {
                    self.downloading.remove(&id);
                }
                Task::none()
            }
            Message::Job(id, report) => self.on_report(id, report),
            Message::Cancel => {
                self.stop();
                self.stopped_in_round = true;
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
                self.save_settings();
                Task::none()
            }
            Message::ChooseAll(choice) => {
                let waiting: Vec<JobId> = self
                    .queue
                    .jobs()
                    .iter()
                    .map(|job| job.id)
                    .filter(|&id| self.is_waiting(id))
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
            Message::Found(stops, look, choice, found) => self.on_found(stops, look, choice, found),
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
            Message::Clear => self.clear(),
            Message::Undo => self.undo_clear(),
            Message::UndoGone(clears) => {
                if clears == self.clears {
                    self.let_go_cleared();
                }
                Task::none()
            }
            Message::ToggleDrawer => {
                if self.drawer_open {
                    // Closing the list puts away what was picked in it, too.
                    self.selected = None;
                    self.close_drawer();
                    Task::none()
                } else {
                    self.drawer_open = true;
                    // A list opened again is where it was left.
                    self.drawer_scroll.restore()
                }
            }
            Message::DrawerFolded => {
                self.drawer_folding = false;
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
                give_icon(handle);
                self.reveal(id, handle)
            }
            Message::WindowReady(handle) => {
                give_icon(handle);
                Task::none()
            }
            Message::Frame(id) => self.on_frame(id),
            Message::Drawn(id) => self.bring_into_view(id),
            Message::Reveal(id) => {
                if let Some((_, frames, _)) = self.revealing.get(&id) {
                    warn!(frames, "a new window took too long to draw; shown anyway");
                }
                self.bring_into_view(id)
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
                    self.preparing_to_delete = false;
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
                self.preparing_to_delete = false;
                Task::none()
            }
            Message::OpenMain => self.show_main(),
            Message::Explain(id) => {
                self.selected = Some(id);
                self.show_main()
            }
            Message::CloseMini => self.close_mini(),
            // Without the tray icon there is nothing to click. A Gracil resting in the tray with
            // no window makes it only once the pointer comes to the icon or files come (ADR 0066).
            Message::MakeMini if self.tray.is_some() && !self.resting_in_tray() => self.make_mini(),
            Message::MakeMini => Task::none(),
            Message::RoomAgain => {
                self.room_asked = false;
                self.schedule()
            }
            Message::Outside(Outside::Tray(TrayEvent::Pointed)) => self.make_mini(),
            Message::MiniReady(handle) => self.on_mini_ready(handle),
            Message::MiniShown(handle) => {
                if self.mini_shown && !self.mini.is_some_and(|id| self.revealing.contains_key(&id))
                {
                    self.place_mini(handle);
                }
                Task::none()
            }
            Message::Outside(Outside::Request(request)) => self.on_request(request),
            Message::Outside(Outside::Breath(breath)) => self.breathe(breath),
            Message::Outside(Outside::Tray(TrayEvent::Toggle)) => {
                if self.mini_shown {
                    self.close_mini()
                } else {
                    self.open_mini()
                }
            }
            Message::Outside(Outside::Tray(TrayEvent::Open)) => self.show_main(),
            Message::Outside(Outside::Tray(TrayEvent::ExplorerMenu)) => {
                self.toggle_explorer_menu();
                Task::none()
            }
            Message::Outside(Outside::Tray(TrayEvent::ExplorerMainMenu)) => {
                // Windows asks for permission; the menu shows the outcome once it is known.
                let on = !place::main_menu_is_ours();
                place::set_main_menu(on, |ours| outside::send(Outside::MainMenu(ours)));
                Task::none()
            }
            Message::Outside(Outside::MainMenu(ours)) => {
                if let Some(tray) = &self.tray {
                    tray.check_explorer_main_menu(ours);
                }
                Task::none()
            }
            Message::Outside(Outside::Tray(TrayEvent::ExplorerMiniWindow)) => {
                self.toggle_explorer_mini_window();
                Task::none()
            }
            Message::Outside(Outside::Tray(TrayEvent::StartWithWindows)) => {
                self.toggle_start_with_windows();
                Task::none()
            }
            Message::Outside(Outside::Tray(TrayEvent::PrepareToDelete)) => {
                self.preparing_to_delete = true;
                self.ask_to_exit()
            }
            Message::Outside(Outside::Tray(TrayEvent::Exit)) => self.ask_to_exit(),
            Message::Outside(Outside::Tray(TrayEvent::Colors)) => {
                self.follow_windows_look(Instant::now());
                Task::none()
            }
            // Windows shuts down or signs out: the list of unfinished files is kept and the work
            // stops, at once and without asking (ADR 0064). Never held up.
            Message::Outside(Outside::EndSession(done)) => {
                info!("Windows is shutting down");
                self.exit_now();
                let _ = done.send(());
                Task::none()
            }
            Message::Outside(Outside::Tray(TrayEvent::EndSession)) => Task::none(),
        }
    }

    /// Whether Gracil was started with Windows and has shown nothing yet: then nothing is made
    /// until it is needed, so a Gracil waiting in the tray all day costs as little as it can.
    pub(super) fn resting_in_tray(&self) -> bool {
        self.resident && !self.window_shown && !self.mini_shown && self.mini.is_none()
    }

    fn toggle_explorer_menu(&mut self) {
        self.settings.explorer_menu = !self.settings.explorer_menu;
        if let Some(tray) = &self.tray {
            tray.check_explorer_menu(self.settings.explorer_menu);
        }
        self.save_settings();
        place::settle(self.settings.place());
    }

    fn toggle_start_with_windows(&mut self) {
        self.settings.start_with_windows = !self.settings.start_with_windows;
        if let Some(tray) = &self.tray {
            tray.check_start_with_windows(self.settings.start_with_windows);
        }
        self.save_settings();
        place::settle(self.settings.place());
    }

    fn toggle_explorer_mini_window(&mut self) {
        self.settings.explorer_mini_window = !self.settings.explorer_mini_window;
        if let Some(tray) = &self.tray {
            tray.check_explorer_mini_window(self.settings.explorer_mini_window);
        }
        self.save_settings();
    }

    fn save_settings(&self) {
        if let Err(error) = self.settings.save() {
            warn!(%error, "could not save the settings");
        }
    }
}

/// How long after starting a Gracil that no window or file has asked for it finds out what the
/// computer can do anyway, so the first file later starts at once (ADR 0066).
const ENGINE_WHEN_IDLE: Duration = Duration::from_secs(60);

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
