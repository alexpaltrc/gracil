//! Design tokens and the few reusable styles every screen is built from (spec §50).
//!
//! The values are temporary (§78): the palette, type and spacing will be refined, but screens only
//! ever refer to these names, so restyling Gracil happens here.

use iced::theme::{Mode, Palette};
use iced::widget::{button, container, text};
use iced::{Background, Border, Color, Font, Theme};

pub const FONT: Font = Font::with_name("Segoe UI Variable Text");

/// Spacing scale, in logical pixels.
pub mod space {
    pub const XS: f32 = 4.0;
    pub const S: f32 = 8.0;
    pub const M: f32 = 16.0;
    pub const L: f32 = 24.0;
}

/// Type scale, in logical pixels.
pub mod size {
    pub const TITLE: f32 = 22.0;
    pub const BODY: f32 = 14.0;
    pub const CAPTION: f32 = 12.0;
}

pub const RADIUS: f32 = 12.0;

pub fn theme(mode: Mode) -> Theme {
    match mode {
        Mode::Dark => Theme::custom(
            "Gracil Dark",
            Palette {
                background: Color::from_rgb8(0x1C, 0x1C, 0x1E),
                text: Color::from_rgb8(0xF2, 0xF2, 0xF4),
                primary: Color::from_rgb8(0x8E, 0x8C, 0xF8),
                success: Color::from_rgb8(0x5C, 0xC9, 0x8A),
                warning: Color::from_rgb8(0xF0, 0xB4, 0x4C),
                danger: Color::from_rgb8(0xF2, 0x6D, 0x6D),
            },
        ),
        _ => Theme::custom(
            "Gracil Light",
            Palette {
                background: Color::from_rgb8(0xFA, 0xFA, 0xFB),
                text: Color::from_rgb8(0x1C, 0x1C, 0x1E),
                primary: Color::from_rgb8(0x56, 0x52, 0xD9),
                success: Color::from_rgb8(0x2E, 0x9E, 0x62),
                warning: Color::from_rgb8(0xC2, 0x80, 0x10),
                danger: Color::from_rgb8(0xD0, 0x3F, 0x3F),
            },
        ),
    }
}

/// Secondary text: file counts, states, hints.
pub fn muted(theme: &Theme) -> text::Style {
    text::Style {
        color: Some(theme.palette().text.scale_alpha(0.55)),
    }
}

/// The small "!" that marks something needing attention (spec §2.3).
pub fn notice(theme: &Theme) -> text::Style {
    text::Style {
        color: Some(theme.palette().warning),
    }
}

/// A text-only button that brightens on hover. Used for every quiet action.
pub fn quiet_button(theme: &Theme, status: button::Status) -> button::Style {
    let palette = theme.palette();
    let text_color = match status {
        button::Status::Hovered | button::Status::Pressed => palette.primary,
        button::Status::Disabled => palette.text.scale_alpha(0.3),
        button::Status::Active => palette.text.scale_alpha(0.6),
    };
    button::Style {
        background: None,
        text_color,
        border: Border::default().rounded(RADIUS),
        ..button::Style::default()
    }
}

/// The one kind of button with a surface: what to do with a finished result. Still soft, so it
/// reads as an offer rather than a demand.
pub fn action_button(theme: &Theme, status: button::Status) -> button::Style {
    let palette = theme.palette();
    let (fill, text_color) = match status {
        button::Status::Hovered | button::Status::Pressed => {
            (palette.primary.scale_alpha(0.18), palette.primary)
        }
        button::Status::Disabled => (
            palette.text.scale_alpha(0.04),
            palette.text.scale_alpha(0.3),
        ),
        button::Status::Active => (palette.primary.scale_alpha(0.1), palette.primary),
    };
    button::Style {
        background: Some(Background::Color(fill)),
        text_color,
        border: Border::default().rounded(RADIUS),
        ..button::Style::default()
    }
}

/// The drop area, which lights up while files are dragged over the window.
pub fn drop_zone(theme: &Theme, hovered: bool) -> container::Style {
    let palette = theme.palette();
    let (fill, edge) = if hovered {
        (
            palette.primary.scale_alpha(0.08),
            palette.primary.scale_alpha(0.6),
        )
    } else {
        (Color::TRANSPARENT, palette.text.scale_alpha(0.12))
    };
    container::Style {
        background: Some(Background::Color(fill)),
        border: Border::default().rounded(RADIUS).width(1.0).color(edge),
        ..container::Style::default()
    }
}

/// The file drawer: a soft surface that sits below the focus without competing with it.
pub fn drawer(theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(theme.palette().text.scale_alpha(0.04))),
        border: Border::default().rounded(RADIUS),
        ..container::Style::default()
    }
}

pub fn tooltip(theme: &Theme) -> container::Style {
    let palette = theme.palette();
    container::Style {
        background: Some(Background::Color(palette.background)),
        text_color: Some(palette.text),
        border: Border::default()
            .rounded(RADIUS / 2.0)
            .width(1.0)
            .color(palette.text.scale_alpha(0.12)),
        ..container::Style::default()
    }
}
