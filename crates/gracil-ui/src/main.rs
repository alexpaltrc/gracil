//! Gracil: compress videos without thinking about it.

// Always a windowed app, never a console window, even in development builds, so what is tested
// is what users see. Logs and panics go to the log files instead (see `logging`).
#![windows_subsystem = "windows"]

mod app;
mod data;
mod dial;
mod done;
mod explorer;
mod focus;
mod logging;
mod motion;
mod numbers;
mod outside;
mod pace;
mod pill;
mod place;
mod request;
mod reveal;
mod settings;
mod style;
mod text;
mod unfinished;
mod view;
mod work;

use app::Gracil;
use outside::Outside;
use request::Request;
use tracing::{info, warn};

fn main() -> iced::Result {
    // First, while this is the only thread (see `draw_with_direct3d`).
    gracil_windows::draw_with_direct3d();
    logging::init();

    // Started as an administrator to write or remove File Explorer's main menu (ADR 0058): that
    // is all, with no window and no claim on the running Gracil.
    let mut arguments = std::env::args_os().skip(1);
    if let (Some(flag), None) = (arguments.next(), arguments.next()) {
        for (asked, on) in [
            (place::ADD_MAIN_MENU, true),
            (place::REMOVE_MAIN_MENU, false),
        ] {
            if flag == asked {
                let written = place::write_main_menu(on);
                logging::finish();
                std::process::exit(if written { 0 } else { 1 });
            }
        }
    }

    let request = Request::parse(std::env::args_os().skip(1));

    // One Gracil per session: a later launch hands its request over and leaves (ADR 0019).
    match gracil_windows::claim() {
        Ok(true) => {
            let serving = gracil_windows::serve(|arguments| {
                outside::send(Outside::Request(Request::parse(arguments)));
            });
            if let Err(error) = serving {
                warn!(%error, "later launches can't reach this Gracil");
            }
        }
        Ok(false) => match gracil_windows::forward(&request.to_arguments()) {
            Ok(()) => return Ok(()),
            // The running Gracil doesn't answer; do the work here rather than lose it.
            Err(error) => warn!(%error, "the running Gracil didn't answer; working alone"),
        },
        Err(error) => warn!(%error, "could not check for a running Gracil"),
    }

    info!(version = gracil_core::VERSION, "Gracil started");
    // FFmpeg must never outlive Gracil, even if Gracil is killed (ADR 0018).
    if let Err(error) = gracil_windows::end_children_with_this_process() {
        warn!(%error, "FFmpeg may outlive a sudden close");
    }
    // Compressing runs below other programs; the window, drawn on this thread, does not (ADR 0040).
    gracil_windows::work_quietly();
    // Too hot, and everything waits for the computer to cool down (ADR 0043).
    pace::watch(|breath| outside::send(Outside::Breath(breath)));
    outside::send(Outside::Request(request));

    // Iced reads Windows' list of fonts the first time it draws text, which after a fresh start
    // of the computer can take most of a second. Read it now, alongside the rest of starting up,
    // so the first window is ready to draw sooner.
    std::thread::spawn(|| {
        let started = std::time::Instant::now();
        let _ = iced::advanced::graphics::text::font_system();
        info!(took = ?started.elapsed(), "fonts ready");
    });

    let result = iced::daemon(Gracil::boot, Gracil::update, view::view)
        .title(Gracil::title)
        .theme(Gracil::theme)
        .subscription(Gracil::subscription)
        .default_font(style::FONT)
        .run();
    logging::finish();
    result
}
