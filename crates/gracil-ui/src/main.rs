//! Gracil: compress videos without thinking about it.

// Release builds are a windowed app; debug builds keep a console for logs and panics.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod logging;
mod style;
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
