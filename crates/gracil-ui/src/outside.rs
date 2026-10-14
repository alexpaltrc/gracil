//! News from outside Iced (the command line, the pipe, the tray, the computer's warmth) reaching
//! the app as messages.
//!
//! Those arrive on threads of their own. They drop what they have into one channel, and a single
//! subscription hands it to the app in order.

use std::sync::{LazyLock, Mutex};
use std::time::Duration;

use gracil_windows::TrayEvent;
use iced::Subscription;
use iced::futures::channel::mpsc::{UnboundedReceiver, UnboundedSender, unbounded};
use iced::futures::stream::{self, BoxStream, StreamExt};

use crate::pace::Breath;
use crate::request::Request;

#[derive(Debug, Clone)]
pub enum Outside {
    Request(Request),
    Tray(TrayEvent),
    /// Gracil rests or works on, as the computer runs warm or cools (ADR 0045).
    Breath(Breath),
    /// Whether File Explorer's main menu starts this Gracil, once Windows has asked (ADR 0058).
    MainMenu(bool),
    /// Windows is shutting down or signing out (ADR 0064): the app keeps the list of unfinished
    /// files and stops its work, then says so through the sender.
    EndSession(std::sync::mpsc::Sender<()>),
}

/// How long the tray waits, as Windows shuts down, for the app to keep what it must. Windows
/// itself waits about five seconds.
const END_SESSION_WAIT: Duration = Duration::from_secs(3);

/// Hands what the tray says to the app. As Windows shuts down, waits for the app to have kept
/// the list of unfinished files, so it isn't lost.
pub fn from_tray(event: TrayEvent) {
    if event == TrayEvent::EndSession {
        let (done, kept) = std::sync::mpsc::channel();
        send(Outside::EndSession(done));
        let _ = kept.recv_timeout(END_SESSION_WAIT);
    } else {
        send(Outside::Tray(event));
    }
}

type Channel = (
    UnboundedSender<Outside>,
    Mutex<Option<UnboundedReceiver<Outside>>>,
);

static CHANNEL: LazyLock<Channel> = LazyLock::new(|| {
    let (sender, receiver) = unbounded();
    (sender, Mutex::new(Some(receiver)))
});

/// Hands `news` to the app. Safe from any thread, before or after the app starts.
pub fn send(news: Outside) {
    // Only fails once the app is gone, when nothing is listening anyway.
    let _ = CHANNEL.0.unbounded_send(news);
}

/// Everything sent, as it arrives.
pub fn subscription() -> Subscription<Outside> {
    Subscription::run(receive)
}

fn receive() -> BoxStream<'static, Outside> {
    let taken = CHANNEL
        .1
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .take();
    match taken {
        Some(receiver) => receiver.boxed(),
        // The subscription runs once; this is never reached in practice.
        None => stream::empty().boxed(),
    }
}
