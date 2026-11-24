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

/// How long after starting the mini-window is made, hidden, for the first click on the tray icon:
/// late enough not to slow a main window opening at start.
pub(super) const MINI_MADE_AFTER: Duration = Duration::from_secs(2);

/// Brings a new window into view once its first frame is on screen (ADR 0066). By the time this
/// is asked, Iced has drawn that frame and handed it to Windows, and waiting for Windows to put
/// what it was handed on screen is enough; it is waited for twice, to be safe. Where that can't be
/// waited for, a short moment is, as before. Whichever comes first of this and the third frame
/// brings the window into view.
fn composed(id: window::Id) -> Task<Message> {
    let (done, waited) = iced::futures::channel::oneshot::channel();
    std::thread::spawn(move || {
        let flushed = (0..2).try_for_each(|_| gracil_windows::wait_for_screen());
        if let Err(error) = flushed {
            warn!(%error, "could not wait for the screen; waiting a moment instead");
            std::thread::sleep(SETTLE_AFTER_FIRST_FRAME);
        }
        let _ = done.send(());
    });
    Task::perform(waited, move |_| Message::Drawn(id))
}

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
            1 => composed(id),
            FRAMES_BEFORE_REVEAL.. => self.bring_into_view(id),
            _ => Task::none(),
        }
    }

    /// Opens the mini-window next to the tray icon (spec §55). It is shown out of sight, and put
    /// in place only once it has drawn itself, so it never appears white or somewhere else first.
    pub(super) fn open_mini(&mut self) -> Task<Message> {
        // It opens on whatever went wrong.
        if let Some(id) = self.unseen_problem.take() {
            self.problem_in_mini = Some(id);
        }
        if self.mini_shown {
            return self.mini.map_or_else(Task::none, window::gain_focus);
        }
        self.mini_shown = true;
        match (self.mini, self.mini_handle) {
            // Kept from last time: only shown again.
            (Some(id), Some(handle)) => self.show_mini(id, handle),
            // Still being made: shown as soon as it is (`MiniReady`).
            (Some(_), None) => Task::none(),
            (None, _) => self.make_mini(),
        }
    }

    /// Makes the mini-window, hidden. Making a window takes about half a second, and showing one
    /// a few hundredths, so it is made once and kept, hidden, between openings: a moment after
    /// Gracil starts (`MINI_MADE_AFTER`), or at the first click if that comes sooner. A Gracil
    /// resting in the tray since Windows started waits for the pointer or for files (ADR 0066).
    pub(super) fn make_mini(&mut self) -> Task<Message> {
        if self.mini.is_some() {
            return Task::none();
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

    /// The mini-window exists: it is shown if it was asked for meanwhile, or else waits hidden.
    pub(super) fn on_mini_ready(&mut self, handle: u64) -> Task<Message> {
        self.mini_handle = Some(handle);
        match self.mini {
            Some(id) if self.mini_shown => self.show_mini(id, handle),
            _ => Task::none(),
        }
    }

    fn show_mini(&mut self, id: window::Id, handle: u64) -> Task<Message> {
        self.place_mini(handle);
        self.reveal(id, handle)
            .chain(Task::done(Message::MiniShown(handle)))
    }

    /// Hides the mini-window; it is kept for the next time.
    pub(super) fn close_mini(&mut self) -> Task<Message> {
        self.problem_in_mini = None;
        if !std::mem::take(&mut self.mini_shown) {
            return Task::none();
        }
        match self.mini {
            Some(id) => {
                self.revealing.remove(&id);
                window::set_mode(id, window::Mode::Hidden)
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

    /// Whether the mini-window is open, rather than kept hidden or not made yet.
    pub fn mini_is_open(&self) -> bool {
        self.mini_shown
    }

    /// Opens the main window, or brings it forward if it is open already.
    pub(super) fn show_main(&mut self) -> Task<Message> {
        // Whatever went wrong is in plain view now.
        self.unseen_problem = None;
        if let Some(id) = self.main {
            return Task::batch([window::minimize(id, false), window::gain_focus(id)]);
        }
        let (id, open) = window::open(window::Settings {
            size: crate::view::WINDOW,
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
                return Task::batch([show, self.in_view(id)]);
            }
        };
        self.revealing.insert(id, (handle, 0, home));
        Task::batch([show, after(REVEAL_AT_THE_LATEST, Message::Reveal(id))])
    }

    pub(super) fn bring_into_view(&mut self, id: window::Id) -> Task<Message> {
        let Some((handle, _, home)) = self.revealing.remove(&id) else {
            return Task::none();
        };
        if self.mini == Some(id) {
            // Placed again: the screen's scale may have changed its size meanwhile.
            self.place_mini(handle);
        } else if let Err(error) = gracil_windows::unpark(handle, home) {
            warn!(%error, "could not bring a new window into view");
        }
        self.in_view(id)
    }

    /// A window is in view. Once the main window is, Gracil finds out what the computer can do,
    /// if nothing asked for it sooner (ADR 0066): until then, that work would only slow the
    /// window's first moment.
    fn in_view(&mut self, id: window::Id) -> Task<Message> {
        if self.main == Some(id) {
            self.start_engine()
        } else {
            Task::none()
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
        // It is also what screen readers read, so every quiet state has words of its own
        // (ADR 0066).
        let working = self.working();
        let getting_ready = working.is_some_and(|job| job.state == JobState::Preparing);
        let downloading = working.is_some_and(|job| self.downloading.contains(&job.id));
        let low_battery = self.breath.battery != pace::Battery::Fine
            && self.left() > 0
            && working.is_none_or(|job| self.waiting_for_battery(job.state));
        let text = self.text;
        let tip = match (state, done) {
            (TrayState::Error | TrayState::Paused, _) => text.tray_tip(state),
            _ if low_battery => text.tray_low_battery.to_owned(),
            // Nothing has moved yet: it says so, as the window does, rather than "0%".
            (TrayState::Processing(_), Some(done)) if downloading && done <= 0.0 => {
                text.tray_downloading.to_owned()
            }
            (TrayState::Processing(_), Some(done)) if getting_ready && done <= 0.0 => {
                text.tray_getting_ready.to_owned()
            }
            // With about how long is left, once that is known well enough (ADR 0069).
            (TrayState::Processing(_), Some(done)) => text.tray_compressing(done, self.time_left()),
            _ if self.waiting_count() > 0 => text.tray_waiting(self.waiting_count()),
            // Files are on the list, and what the computer can do isn't known yet.
            _ if self.left() > 0 && matches!(self.engine, EngineState::Starting) => {
                text.tray_getting_ready.to_owned()
            }
            _ => match self.finished() {
                Some(finished) => text.tray_done(&finished),
                None => text.tray_tip(state),
            },
        };
        // Gracil Dev says so, in its window's title and beside its tray icon, since it runs next
        // to the Gracil in daily use (ADR 0082).
        let tip = if gracil_windows::DEV {
            tip.replacen("Gracil", gracil_windows::NAME, 1)
        } else {
            tip
        };
        (state, tip)
    }
}
