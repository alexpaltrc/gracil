//! The main window, the mini-window and the tray icon: opening, coming into view without a
//! white flash, and closing.

use super::*;

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

/// The mini-window's size, in logical pixels.
const MINI_SIZE: Size = Size::new(340.0, 108.0);

/// Gives the window Gracil's icon; a window without it still works.
pub(super) fn give_icon(handle: u64) {
    if let Err(error) = gracil_windows::set_window_icon(handle) {
        warn!("could not set the window icon: {error}");
    }
}

/// Closes a window without a white flash: Windows animates a closing window, and once its surface
/// is gone that animation shows a blank white frame. Hidden first, the window leaves with its
/// picture intact, and only then is it destroyed.
pub(super) fn dismiss(id: window::Id) -> Task<Message> {
    window::set_mode::<Message>(id, window::Mode::Hidden).chain(window::close(id))
}

impl Gracil {
    /// Counts a drawn frame of a window waiting out of sight, and shows it once it has drawn
    /// enough.
    pub(super) fn on_frame(&mut self, id: window::Id) -> Task<Message> {
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

    /// Opens the mini-window next to the tray icon (spec §55). It opens hidden, is put in place,
    /// and only then shown, so it never appears somewhere else first.
    pub(super) fn open_mini(&mut self) -> Task<Message> {
        // It opens on whatever went wrong.
        if let Some(id) = self.unseen_problem.take() {
            self.problem_in_mini = Some(id);
        }
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

    pub(super) fn close_mini(&mut self) -> Task<Message> {
        self.problem_in_mini = None;
        match self.mini.take() {
            Some(id) => {
                self.revealing.remove(&id);
                dismiss(id)
            }
            None => Task::none(),
        }
    }

    pub(super) fn place_mini(&self, handle: u64) {
        if let Err(error) = gracil_windows::place_near_tray(handle, self.tray.as_ref()) {
            warn!(%error, "could not place the mini-window");
        }
    }

    /// Whether `window` is the mini-window.
    pub fn is_mini(&self, window: window::Id) -> bool {
        self.mini == Some(window)
    }

    /// Opens the main window, or brings it forward if it is open already.
    pub(super) fn show_main(&mut self) -> Task<Message> {
        // Whatever went wrong is in plain view now.
        self.unseen_problem = None;
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
    pub(super) fn reveal(&mut self, id: window::Id, handle: u64) -> Task<Message> {
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

    pub(super) fn bring_into_view(&mut self, id: window::Id) {
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

    /// Tells the tray icon what to show, when that changed (spec §54).
    pub(super) fn sync_tray(&mut self) {
        let Some(tray) = &self.tray else {
            return;
        };
        let look = self.look();
        if self.tray_shown.as_ref() != Some(&look) {
            tray.show(look.0, &look.1);
            self.tray_shown = Some(look);
        }
    }

    /// How the work goes, as the tray's picture and the words beside it.
    pub(super) fn look(&self) -> (TrayState, String) {
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
            None if self.unseen_problem.is_some() => TrayState::Error,
            None => TrayState::Idle,
        };
        // The tip shows whole percentages, so it follows the work more finely than the picture.
        let tip = match (state, done) {
            (TrayState::Processing(_), Some(done)) => {
                self.text.tray_tip(TrayState::Processing(done))
            }
            _ => self.text.tray_tip(state),
        };
        (state, tip)
    }
}
