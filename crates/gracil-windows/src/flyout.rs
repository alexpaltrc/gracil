//! Placing the mini-window next to Gracil's tray icon (spec §55, ADR 0021).
//!
//! The taskbar can be on any edge of any screen, so nothing assumes the bottom right: the window
//! goes on the screen that holds the icon, against the edge the taskbar takes, as close to the
//! icon as fits.

use std::io;
use std::ptr;

use windows_sys::Win32::Foundation::{HWND, POINT, RECT};
use windows_sys::Win32::Graphics::Gdi::{
    GetMonitorInfoW, MONITOR_DEFAULTTOPRIMARY, MONITORINFO, MonitorFromPoint, MonitorFromRect,
};
use windows_sys::Win32::UI::HiDpi::GetDpiForWindow;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    GetWindowRect, SWP_NOACTIVATE, SWP_NOSIZE, SWP_NOZORDER, SetWindowPos,
};

use crate::Tray;

/// Space kept between the window and the taskbar or the screen's edges, in logical pixels.
const MARGIN: i32 = 12;

/// A rectangle in physical screen pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Area {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

impl From<RECT> for Area {
    fn from(rect: RECT) -> Self {
        Self {
            left: rect.left,
            top: rect.top,
            right: rect.right,
            bottom: rect.bottom,
        }
    }
}

impl From<Area> for RECT {
    fn from(area: Area) -> Self {
        Self {
            left: area.left,
            top: area.top,
            right: area.right,
            bottom: area.bottom,
        }
    }
}

/// Moves the window given by its raw handle next to the tray icon. Without an icon to go by, it
/// goes to the corner of the main screen where the clock usually is.
pub fn place_near_tray(window: u64, tray: Option<&Tray>) -> io::Result<()> {
    let window = window as HWND;
    let anchor = tray.and_then(Tray::anchor);
    let mut info = MONITORINFO {
        cbSize: size_of::<MONITORINFO>() as u32,
        ..MONITORINFO::default()
    };
    // SAFETY: the rectangle and point are plain values; `info` is a valid MONITORINFO with its size
    // set, which the call fills in.
    #[allow(unsafe_code)]
    let found = unsafe {
        let monitor = match anchor {
            Some(anchor) => MonitorFromRect(&RECT::from(anchor), MONITOR_DEFAULTTOPRIMARY),
            None => MonitorFromPoint(POINT { x: 0, y: 0 }, MONITOR_DEFAULTTOPRIMARY),
        };
        GetMonitorInfoW(monitor, &mut info)
    };
    if found == 0 {
        return Err(io::Error::other("no screen to show the mini-window on"));
    }
    let (screen, work) = (Area::from(info.rcMonitor), Area::from(info.rcWork));

    // First onto that screen, so the window takes the screen's scale and the size that goes with
    // it; then into place.
    move_to(window, work.left, work.top)?;
    let mut bounds = RECT::default();
    // SAFETY: `window` is a live window owned by this process; `bounds` receives its rectangle,
    // and the DPI call only reads.
    #[allow(unsafe_code)]
    let (got, dpi) = unsafe { (GetWindowRect(window, &mut bounds), GetDpiForWindow(window)) };
    if got == 0 {
        return Err(io::Error::last_os_error());
    }
    let bounds = Area::from(bounds);
    let size = (bounds.right - bounds.left, bounds.bottom - bounds.top);
    let margin = MARGIN * i32::try_from(dpi).unwrap_or(96) / 96;
    let (x, y) = position(anchor, screen, work, size, margin);
    move_to(window, x, y)
}

fn move_to(window: HWND, x: i32, y: i32) -> io::Result<()> {
    // SAFETY: `window` is a live window owned by this process; only its position changes.
    #[allow(unsafe_code)]
    let moved = unsafe {
        SetWindowPos(
            window,
            ptr::null_mut(),
            x,
            y,
            0,
            0,
            SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
        )
    };
    if moved == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

/// Where the window's top-left corner goes: against the taskbar's edge of the work area, centred
/// on the icon along that edge, and always wholly inside the work area.
pub(crate) fn position(
    anchor: Option<Area>,
    screen: Area,
    work: Area,
    (width, height): (i32, i32),
    margin: i32,
) -> (i32, i32) {
    let (ax, ay) = match anchor {
        Some(icon) => ((icon.left + icon.right) / 2, (icon.top + icon.bottom) / 2),
        None => (work.right, work.bottom),
    };
    let (x, y) = if work.top > screen.top {
        (ax - width / 2, work.top + margin)
    } else if work.left > screen.left {
        (work.left + margin, ay - height / 2)
    } else if work.right < screen.right {
        (work.right - width - margin, ay - height / 2)
    } else {
        // The taskbar is at the bottom, or hides itself and takes no room.
        (ax - width / 2, work.bottom - height - margin)
    };
    (
        clamp(x, work.left + margin, work.right - width - margin),
        clamp(y, work.top + margin, work.bottom - height - margin),
    )
}

/// Like `Ord::clamp`, but a window larger than the room starts at the low edge instead of
/// panicking.
fn clamp(value: i32, low: i32, high: i32) -> i32 {
    value.min(high).max(low)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SCREEN: Area = Area {
        left: 0,
        top: 0,
        right: 1920,
        bottom: 1080,
    };
    const SIZE: (i32, i32) = (320, 120);

    fn icon_at(x: i32, y: i32) -> Option<Area> {
        Some(Area {
            left: x - 12,
            top: y - 12,
            right: x + 12,
            bottom: y + 12,
        })
    }

    #[test]
    fn bottom_taskbar_puts_it_just_above_the_icon() {
        let work = Area {
            bottom: 1032,
            ..SCREEN
        };
        let (x, y) = position(icon_at(1700, 1056), SCREEN, work, SIZE, 12);
        assert_eq!((x, y), (1700 - 160, 1032 - 120 - 12));
    }

    #[test]
    fn it_never_leaves_the_work_area() {
        let work = Area {
            bottom: 1032,
            ..SCREEN
        };
        // An icon at the far right would push the window off the screen.
        let (x, _) = position(icon_at(1910, 1056), SCREEN, work, SIZE, 12);
        assert_eq!(x, 1920 - 320 - 12);
    }

    #[test]
    fn side_and_top_taskbars_are_respected() {
        let left = Area { left: 48, ..SCREEN };
        assert_eq!(
            position(icon_at(24, 1000), SCREEN, left, SIZE, 12),
            (60, 940)
        );
        let right = Area {
            right: 1872,
            ..SCREEN
        };
        assert_eq!(
            position(icon_at(1896, 500), SCREEN, right, SIZE, 12),
            (1872 - 320 - 12, 440)
        );
        let top = Area { top: 48, ..SCREEN };
        assert_eq!(
            position(icon_at(1700, 24), SCREEN, top, SIZE, 12),
            (1540, 60)
        );
    }

    #[test]
    fn a_second_screen_to_the_left_has_negative_coordinates() {
        let screen = Area {
            left: -1920,
            top: 0,
            right: 0,
            bottom: 1080,
        };
        let work = Area {
            bottom: 1032,
            ..screen
        };
        let (x, y) = position(icon_at(-200, 1056), screen, work, SIZE, 12);
        assert_eq!((x, y), (-360, 900));
    }

    #[test]
    fn without_an_icon_it_takes_the_clock_corner() {
        let work = Area {
            bottom: 1032,
            ..SCREEN
        };
        assert_eq!(
            position(None, SCREEN, work, SIZE, 12),
            (1920 - 320 - 12, 1032 - 120 - 12)
        );
    }
}
