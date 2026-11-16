//! Design tokens and the few reusable styles every screen is built from (spec §50).
//!
//! The colors are Windows 11's own, with the user's accent color (ADR 0031); type and spacing are
//! still temporary (§78). Screens only ever refer to these names, so restyling Gracil happens
//! here.

use std::sync::RwLock;
use std::time::Duration;

use iced::font::Weight;
use iced::theme::{Mode, Palette};
use iced::widget::{button, container, scrollable, text};
use iced::{Background, Border, Color, Font, Theme};

pub const FONT: Font = Font::with_name("Segoe UI Variable Text");

/// For the few numbers that are the point of a view: the amount saved in the ring.
pub const STRONG: Font = Font {
    weight: Weight::Semibold,
    ..Font::with_name("Segoe UI Variable Display")
};

/// Spacing scale, in logical pixels.
pub mod space {
    pub const XS: f32 = 4.0;
    pub const S: f32 = 8.0;
    pub const M: f32 = 16.0;
}

/// Type scale, in logical pixels.
pub mod size {
    pub const TITLE: f32 = 22.0;
    pub const BODY: f32 = 14.0;
    pub const CAPTION: f32 = 12.0;
}

pub const RADIUS: f32 = 12.0;

/// How long the window takes to follow a change in Windows' colors: light and dark, or the accent.
pub const THEME_SHIFT: Duration = Duration::from_millis(600);

/// The user's accent color, in the shade Windows 11 fills controls with in each mode: a darker one
/// in light mode, a lighter one in dark mode.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Accent {
    pub on_light: Color,
    pub on_dark: Color,
}

impl Accent {
    /// Windows 11's own blue, for when the user's accent can't be read.
    pub const WINDOWS: Self = Self {
        on_light: Color::from_rgb8(0x00, 0x5F, 0xB8),
        on_dark: Color::from_rgb8(0x4C, 0xC2, 0xFF),
    };
}

/// The colors of a contrast theme chosen in Windows. While one is on, Gracil draws with them
/// alone, as Windows asks every app to.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Contrast {
    pub background: Color,
    pub text: Color,
    pub highlight: Color,
    /// Text on `highlight`.
    pub highlight_text: Color,
    /// Controls that can't be used now: Windows' gray text.
    pub disabled: Color,
}

/// The contrast theme's colors while one is on, so the roles beyond the palette follow it too.
static CONTRAST: RwLock<Option<Contrast>> = RwLock::new(None);

/// Says which contrast theme is on, if any, before the window is drawn with `contrast_palette`.
pub fn set_contrast(contrast: Option<Contrast>) {
    if let Ok(mut current) = CONTRAST.write() {
        *current = contrast;
    }
}

/// The contrast theme's colors while one is on.
pub fn contrast() -> Option<Contrast> {
    CONTRAST.read().ok().and_then(|current| *current)
}

/// Whether a contrast theme is on, for the few faint touches that need a solid stand-in.
pub fn contrasting() -> bool {
    contrast().is_some()
}

/// The palette while a contrast theme is on: its background, its text, and its highlight for
/// filled controls. States read in words and marks, not in colors of their own.
pub fn contrast_palette(contrast: Contrast) -> Palette {
    Palette {
        background: contrast.background,
        text: contrast.text,
        primary: contrast.highlight,
        success: contrast.text,
        warning: contrast.text,
        danger: contrast.text,
    }
}

/// Windows 11's palette (ADR 0031): its neutral app backgrounds and text, its status colors, and
/// the user's accent as the primary color.
pub fn palette(mode: Mode, accent: Accent) -> Palette {
    match mode {
        Mode::Dark => Palette {
            background: DARK_BACKGROUND,
            text: Color::WHITE,
            primary: accent.on_dark,
            success: Color::from_rgb8(0x6C, 0xCB, 0x5F),
            warning: Color::from_rgb8(0xFC, 0xE1, 0x00),
            danger: Color::from_rgb8(0xFF, 0x99, 0xA4),
        },
        _ => Palette {
            background: LIGHT_BACKGROUND,
            text: Color::from_rgb8(0x1A, 0x1A, 0x1A),
            primary: accent.on_light,
            success: Color::from_rgb8(0x0F, 0x7B, 0x0F),
            warning: Color::from_rgb8(0x9D, 0x5D, 0x00),
            danger: Color::from_rgb8(0xC4, 0x2B, 0x1C),
        },
    }
}

pub fn theme(palette: Palette) -> Theme {
    Theme::custom("Gracil", palette)
}

/// The roles beyond Iced's palette, Windows 11's own (ADR 0031): text at least 4.5:1 on its
/// background, controls at least 3:1.
#[derive(Debug, Clone, Copy)]
pub struct Tokens {
    /// Secondary text: counts, states, hints, quiet actions.
    pub secondary: Color,
    /// Filled controls and what has been done (the dial's trail): the user's accent color.
    pub accent: Color,
    /// Text on the accent.
    pub on_accent: Color,
    /// Edges of controls and areas.
    pub border: Color,
    /// The solid face of a control that is grabbed or moves, like a slider's thumb.
    pub thumb: Color,
    /// The face of a card, the surface Windows 11 lays content on, and its hairline edge.
    pub card: Color,
    pub card_edge: Color,
}

/// Windows 11's app backgrounds (Mica's fallback colors).
const LIGHT_BACKGROUND: Color = Color::from_rgb8(0xF3, 0xF3, 0xF3);
const DARK_BACKGROUND: Color = Color::from_rgb8(0x20, 0x20, 0x20);

/// The light and dark values of the roles that don't depend on the accent.
struct Neutrals {
    secondary: Color,
    border: Color,
    thumb: Color,
    card: Color,
    card_edge: Color,
}

const LIGHT: Neutrals = Neutrals {
    secondary: Color::from_rgb8(0x5C, 0x5C, 0x5C),
    border: Color::from_rgb8(0x86, 0x86, 0x86),
    thumb: Color::WHITE,
    card: Color::from_rgb8(0xFB, 0xFB, 0xFB),
    card_edge: Color::from_rgb8(0xE5, 0xE5, 0xE5),
};

const DARK: Neutrals = Neutrals {
    secondary: Color::from_rgb8(0xCC, 0xCC, 0xCC),
    border: Color::from_rgb8(0x9A, 0x9A, 0x9A),
    thumb: Color::from_rgb8(0x45, 0x45, 0x45),
    card: Color::from_rgb8(0x2B, 0x2B, 0x2B),
    card_edge: Color::from_rgb8(0x1D, 0x1D, 0x1D),
};

/// The roles for what `theme` shows, including partway through a change between light and dark:
/// how far its background has gone from light grey to dark grey says how far to mix them.
pub fn tokens(theme: &Theme) -> Tokens {
    let palette = theme.palette();
    let on_accent = readable_on(palette.primary);
    if contrasting() {
        return Tokens {
            secondary: palette.text,
            accent: palette.primary,
            on_accent,
            border: palette.text,
            thumb: palette.background,
            card: palette.background,
            card_edge: palette.text,
        };
    }
    let t = ((palette.background.r - LIGHT_BACKGROUND.r)
        / (DARK_BACKGROUND.r - LIGHT_BACKGROUND.r))
        .clamp(0.0, 1.0);
    let mix = |a, b| mix(a, b, t);
    Tokens {
        secondary: mix(LIGHT.secondary, DARK.secondary),
        accent: palette.primary,
        on_accent,
        border: mix(LIGHT.border, DARK.border),
        thumb: mix(LIGHT.thumb, DARK.thumb),
        card: mix(LIGHT.card, DARK.card),
        card_edge: mix(LIGHT.card_edge, DARK.card_edge),
    }
}

/// White or black, whichever reads better on `color`: white on the darker shade Windows uses in
/// light mode and black on the lighter one in dark mode, as Windows does, but also right for an
/// accent pale enough that white would fade into it.
fn readable_on(color: Color) -> Color {
    // Contrast is (lighter + 0.05) / (darker + 0.05) in relative luminance (WCAG 2), so white and
    // black read equally well where luminance is 0.179.
    if luminance(color) > 0.179 {
        Color::BLACK
    } else {
        Color::WHITE
    }
}

/// How light a color looks, from 0 (black) to 1 (white), as WCAG 2 measures it.
fn luminance(color: Color) -> f32 {
    let linear = |c: f32| {
        if c <= 0.040_45 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * linear(color.r) + 0.7152 * linear(color.g) + 0.0722 * linear(color.b)
}

/// A color part of the way from `from` to `to`.
fn mix(from: Color, to: Color, t: f32) -> Color {
    Color {
        r: from.r + (to.r - from.r) * t,
        g: from.g + (to.g - from.g) * t,
        b: from.b + (to.b - from.b) * t,
        a: from.a + (to.a - from.a) * t,
    }
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
        color: Some(tokens(theme).secondary),
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
    // In a contrast theme every text is one color, so the pointer shows as Windows' own
    // highlight, and what can't be used now in its gray text.
    if let Some(contrast) = contrast() {
        let (background, text_color) = match status {
            button::Status::Hovered | button::Status::Pressed => (
                Some(Background::Color(contrast.highlight)),
                contrast.highlight_text,
            ),
            button::Status::Disabled => (None, contrast.disabled),
            button::Status::Active => (None, contrast.text),
        };
        return button::Style {
            background,
            text_color,
            border: Border::default().rounded(RADIUS),
            ..button::Style::default()
        };
    }
    let secondary = tokens(theme).secondary;
    let text_color = match status {
        button::Status::Hovered | button::Status::Pressed => palette.text,
        button::Status::Disabled => secondary.scale_alpha(0.45),
        button::Status::Active => secondary,
    };
    button::Style {
        background: None,
        text_color,
        border: Border::default().rounded(RADIUS),
        ..button::Style::default()
    }
}

/// The file picked in the list, shown large: a quiet button on a faint surface, so it is clear
/// which one is showing and that picking it again puts it away.
pub fn picked_row(theme: &Theme, status: button::Status) -> button::Style {
    let palette = theme.palette();
    // A faint surface doesn't show in a contrast theme: an edge in the text's color does.
    if contrasting() {
        let style = quiet_button(theme, status);
        return button::Style {
            border: style.border.width(1.0).color(palette.text),
            ..style
        };
    }
    button::Style {
        background: Some(Background::Color(palette.text.scale_alpha(0.07))),
        text_color: palette.text,
        ..quiet_button(theme, status)
    }
}

/// The one kind of button with a surface: what to do with a finished result. Filled with the
/// accent, and a little see-through under the pointer, as Windows' own accent buttons are.
pub fn action_button(theme: &Theme, status: button::Status) -> button::Style {
    let palette = theme.palette();
    let tokens = tokens(theme);
    let (fill, text_color) = match status {
        button::Status::Hovered => (tokens.accent.scale_alpha(0.9), tokens.on_accent),
        button::Status::Pressed => (
            tokens.accent.scale_alpha(0.8),
            tokens.on_accent.scale_alpha(0.8),
        ),
        button::Status::Disabled => match contrast() {
            Some(contrast) => (Color::TRANSPARENT, contrast.disabled),
            None => (
                palette.text.scale_alpha(0.06),
                tokens.secondary.scale_alpha(0.6),
            ),
        },
        button::Status::Active => (tokens.accent, tokens.on_accent),
    };
    button::Style {
        background: Some(Background::Color(fill)),
        text_color,
        border: Border::default().rounded(RADIUS),
        ..button::Style::default()
    }
}

/// How a surface looks as it lights up, from 0 (at rest) to 1: its face and its edge.
pub type Surface = fn(&Theme, f32) -> (Color, Border);

/// A card: the face and hairline edge Windows 11 gives the surfaces content lies on. It turns to
/// the accent while files are dragged over it. In a contrast theme it is the window's own color
/// with an edge in the text's, as Windows asks.
pub fn card(theme: &Theme, lit: f32) -> (Color, Border) {
    let palette = theme.palette();
    let tokens = tokens(theme);
    (
        mix(tokens.card, palette.primary, LIT * lit),
        Border::default().rounded(RADIUS).width(1.0).color(mix(
            tokens.card_edge,
            palette.primary,
            lit,
        )),
    )
}

/// The mini-window, which is a card by itself: nothing but the window, until files are dragged
/// over it. Its corners are the ones Windows 11 gives a window.
pub fn bare(theme: &Theme, lit: f32) -> (Color, Border) {
    let palette = theme.palette();
    (
        mix(palette.background, palette.primary, LIT * lit),
        Border::default()
            .rounded(WINDOW_RADIUS)
            .width(1.0)
            .color(palette.primary.scale_alpha(lit)),
    )
}

/// How much of the accent a surface takes on while files are dragged over it.
const LIT: f32 = 0.14;

/// The corners Windows 11 gives a window.
const WINDOW_RADIUS: f32 = 8.0;

/// The list of files: a card of its own under the focus.
pub fn drawer(theme: &Theme) -> container::Style {
    let (face, border) = card(theme, 0.0);
    container::Style {
        background: Some(Background::Color(face)),
        border,
        ..container::Style::default()
    }
}

/// How wide the list's scroll bar is, in logical pixels.
pub const SCROLL_BAR: f32 = 4.0;

/// The list's scroll bar: a thin line with no rail, as Windows 11's own are until the pointer
/// comes, and stronger under the pointer or while dragged.
pub fn scroll_bar(theme: &Theme, status: scrollable::Status) -> scrollable::Style {
    let strength = match status {
        _ if contrasting() => 1.0,
        scrollable::Status::Hovered {
            is_vertical_scrollbar_hovered: true,
            ..
        }
        | scrollable::Status::Dragged { .. } => 0.5,
        _ => 0.25,
    };
    let rail = scrollable::Rail {
        background: None,
        border: Border::default(),
        scroller: scrollable::Scroller {
            background: Background::Color(theme.palette().text.scale_alpha(strength)),
            border: Border::default().rounded(SCROLL_BAR / 2.0),
        },
    };
    scrollable::Style {
        vertical_rail: rail,
        horizontal_rail: rail,
        ..scrollable::default(theme, status)
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
            .color(tokens(theme).border),
        ..container::Style::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_on_the_accent_reads_like_windows_and_stays_readable_on_pale_accents() {
        assert_eq!(readable_on(Accent::WINDOWS.on_light), Color::WHITE);
        assert_eq!(readable_on(Accent::WINDOWS.on_dark), Color::BLACK);
        // A yellow accent's darker shade is still too light for white text.
        assert_eq!(
            readable_on(Color::from_rgb8(0xC2, 0x9F, 0x00)),
            Color::BLACK
        );
    }
}
