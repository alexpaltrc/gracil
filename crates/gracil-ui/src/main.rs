//! Gracil: compress videos without thinking about it.

// Always a windowed app, never a console window, even in development builds, so what is tested
// is what users see. Logs and panics go to the log files instead (see `logging`).
#![windows_subsystem = "windows"]

mod app;
mod logging;
mod style;
mod text;
mod view;
mod work;

use app::Gracil;

fn main() -> iced::Result {
    let _logs = logging::init();
    tracing::info!(version = gracil_core::VERSION, "Gracil started");

    iced::daemon(Gracil::boot, Gracil::update, view::view)
        .title(Gracil::title)
        .theme(Gracil::theme)
        .subscription(Gracil::subscription)
        .default_font(style::FONT)
        .run()
}
