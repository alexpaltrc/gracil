//! Design tokens and the few reusable styles every screen is built from (spec §50).
//!
//! The colors are the Piedra identity (ADR 0015); type and spacing are still temporary (§78).
//! Screens only ever refer to these names, so restyling Gracil happens here.

use std::time::Duration;

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

/// How long the window takes to follow a change between Windows' light and dark themes.
pub const THEME_SHIFT: Duration = Duration::from_millis(600);

/// The Piedra palette (ADR 0015): mineral greys and ivory. The marble is the same stone grey in
/// both themes; the support, used for text, flips between graphite and pale stone.
pub fn palette(mode: Mode) -> Palette {
    const MARBLE: Color = Color::from_rgb8(0x8C, 0x88, 0x7E);
    match mode {
        Mode::Dark => Palette {
            background: Color::from_rgb8(0x25, 0x25, 0x22),
            text: Color::from_rgb8(0xDA, 0xD8, 0xCF),
            primary: MARBLE,
            success: Color::from_rgb8(0x5C, 0xC9, 0x8A),
            warning: Color::from_rgb8(0xF0, 0xB4, 0x4C),
            danger: Color::from_rgb8(0xF2, 0x6D, 0x6D),
        },
        _ => Palette {
            background: Color::from_rgb8(0xF3, 0xF1, 0xEB),
            text: Color::from_rgb8(0x4D, 0x4B, 0x46),
            primary: MARBLE,
            success: Color::from_rgb8(0x2E, 0x9E, 0x62),
            warning: Color::from_rgb8(0xC2, 0x80, 0x10),
            danger: Color::from_rgb8(0xD0, 0x3F, 0x3F),
        },
    }
}

pub fn theme(palette: Palette) -> Theme {
    Theme::custom("Gracil", palette)
}

/// A palette part of the way from one theme to the other, for the shift between them.
pub fn blend(from: Palette, to: Palette, t: f32) -> Palette {
    let mix = |a: Color, b: Color| Color {
        r: a.r + (b.r - a.r) * t,
        g: a.g + (b.g - a.g) * t,
        b: a.b + (b.b - a.b) * t,
        a: a.a + (b.a - a.a) * t,
    };
    Palette {
        background: mix(from.background, to.background),
        text: mix(from.text, to.text),
        primary: mix(from.primary, to.primary),
        success: mix(from.success, to.success),
        warning: mix(from.warning, to.warning),
        danger: mix(from.danger, to.danger),
    }
}

/// Secondary text: file counts, states, hints.
pub fn muted(theme: &Theme) -> text::Style {
    text::Style {
        color: Some(theme.palette().text.scale_alpha(0.75)),
    }
}

/// The small "!" that marks something needing attention (spec §2.3).
pub fn notice(theme: &Theme) -> text::Style {
    text::Style {
        color: Some(theme.palette().warning),
    }
}

/// A text-only button that comes forward on hover. Used for every quiet action.
pub fn quiet_button(theme: &Theme, status: button::Status) -> button::Style {
    let palette = theme.palette();
    let text_color = match status {
        button::Status::Hovered | button::Status::Pressed => palette.text,
        button::Status::Disabled => palette.text.scale_alpha(0.3),
        button::Status::Active => palette.text.scale_alpha(0.65),
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
            (palette.text.scale_alpha(0.13), palette.text)
        }
        button::Status::Disabled => (
            palette.text.scale_alpha(0.04),
            palette.text.scale_alpha(0.3),
        ),
        button::Status::Active => (palette.text.scale_alpha(0.07), palette.text),
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
        (palette.primary.scale_alpha(0.12), palette.primary)
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
