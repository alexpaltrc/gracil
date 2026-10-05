//! News from outside Iced (the command line, the pipe, the tray, the computer's warmth) reaching
//! the app as messages.
//!
//! Those arrive on threads of their own. They drop what they have into one channel, and a single
//! subscription hands it to the app in order.

use std::sync::{LazyLock, Mutex};

use gracil_windows::TrayEvent;
use iced::Subscription;
use iced::futures::channel::mpsc::{UnboundedReceiver, UnboundedSender, unbounded};
use iced::futures::stream::{self, BoxStream, StreamExt};

use crate::pace::Warmth;
use crate::request::Request;

#[derive(Debug, Clone)]
pub enum Outside {
    Request(Request),
    Tray(TrayEvent),
    /// How warm the computer is running changed (ADR 0043).
    Warmth(Warmth),
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
