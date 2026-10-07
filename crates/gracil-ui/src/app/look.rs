//! Gracil's colors: light or dark, the Windows accent, and the contrast themes (ADR 0031,
//! ADR 0061).

use super::*;

/// Slow at both ends, so the change of theme settles in rather than snapping.
fn ease_in_out(t: f32) -> f32 {
    t * t * (3.0 - 2.0 * t)
}

/// The colors of the contrast theme on in Windows, if one is.
pub(super) fn windows_contrast() -> Option<style::Contrast> {
    let color = |[r, g, b]: [u8; 3]| iced::Color::from_rgb8(r, g, b);
    gracil_windows::contrast().map(|contrast| style::Contrast {
        background: color(contrast.background),
        text: color(contrast.text),
        highlight: color(contrast.highlight),
    })
}

/// The accent color chosen in Windows, or Windows' own blue if it can't be read.
pub(super) fn windows_accent() -> style::Accent {
    let color = |[r, g, b]: [u8; 3]| iced::Color::from_rgb8(r, g, b);
    gracil_windows::accent().map_or(style::Accent::WINDOWS, |accent| style::Accent {
        on_light: color(accent.on_light),
        on_dark: color(accent.on_dark),
    })
}

impl Gracil {
    pub fn theme(&self, _window: window::Id) -> Theme {
        self.theme.clone()
    }

    /// Follows Windows between light and dark. The first answer is applied at once, so Gracil
    /// opens in the right colors; later changes fade softly (ADR 0015).
    pub(super) fn follow_theme(&mut self, mode: Mode, now: Instant) {
        if mode == Mode::None || self.mode == Some(mode) {
            return;
        }
        let first = self.mode.is_none();
        self.mode = Some(mode);
        gracil_windows::set_icon_theme(mode == Mode::Light);
        if first {
            self.theme = style::theme(self.palette());
            self.shift = None;
        } else {
            self.shift_to_palette(now);
        }
    }

    /// Picks up what changed in Windows' settings for how apps look and move: a new accent color
    /// fades in like a change of theme (ADR 0031); a contrast theme coming or going changes the
    /// colors at once, as Windows does.
    pub(super) fn follow_windows_look(&mut self, now: Instant) {
        motion::follow_windows();
        let contrast = windows_contrast();
        if contrast != self.contrast {
            self.contrast = contrast;
            style::set_contrasting(contrast.is_some());
            self.theme = style::theme(self.palette());
            self.shift = None;
        }
        let accent = windows_accent();
        if accent != self.accent {
            self.accent = accent;
            self.shift_to_palette(now);
        }
    }

    /// The colors for Windows' current choices: its contrast theme while one is on, or else light
    /// or dark with the accent color.
    pub(super) fn palette(&self) -> Palette {
        self.contrast.map_or_else(
            || style::palette(self.mode.unwrap_or(Mode::Light), self.accent),
            style::contrast_palette,
        )
    }

    /// Fades to the palette for Windows' current choices, starting from what is on screen, so
    /// changing back halfway is still smooth. At once if Windows' animation effects are off.
    pub(super) fn shift_to_palette(&mut self, now: Instant) {
        let target = self.palette();
        if motion::still() {
            self.theme = style::theme(target);
            self.shift = None;
        } else {
            self.shift = Some((self.theme.palette(), target, now));
        }
    }

    pub(super) fn shift_theme(&mut self, now: Instant) {
        let Some((from, to, start)) = self.shift else {
            return;
        };
        let progress = (now.saturating_duration_since(start).as_secs_f32()
            / style::THEME_SHIFT.as_secs_f32())
        .min(1.0);
        self.theme = style::theme(style::blend(from, to, ease_in_out(progress)));
        if progress >= 1.0 {
            self.shift = None;
        }
    }
}
