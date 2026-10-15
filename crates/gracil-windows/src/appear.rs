//! Making a new window appear already painted, never white (spec §2.3's calm, §51).
//!
//! A window shows the moment Windows is told to show it, before its first picture is ready, and
//! until then Windows fills it with white: Iced's windows have no background of their own. Hiding
//! it some other way doesn't help, because Windows throws away what a hidden window draws. So a
//! new window is shown far outside every screen, where it draws its first pictures for real, and
//! is moved into view once they are there.

use std::io;

use windows_sys::Win32::Foundation::RECT;
use windows_sys::Win32::Graphics::Dwm::DwmFlush;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    GetWindowRect, SWP_NOACTIVATE, SWP_NOSIZE, SWP_NOZORDER, SetWindowPos,
};

/// Moves `window`, given as Iced's `window::raw_id` gives it, far outside every screen, keeping
/// its size, and returns where it was.
pub fn park(window: u64) -> io::Result<(i32, i32)> {
    let hwnd = window as usize as *mut core::ffi::c_void;
    let mut rect = RECT {
        left: 0,
        top: 0,
        right: 0,
        bottom: 0,
    };
    // SAFETY: GetWindowRect writes into `rect`, which outlives the call; an invalid handle makes
    // it fail, which is reported.
    #[allow(unsafe_code)]
    if unsafe { GetWindowRect(hwnd, &mut rect) } == 0 {
        return Err(io::Error::last_os_error());
    }
    move_to(window, PARKED, PARKED)?;
    Ok((rect.left, rect.top))
}

/// Waits until Windows has put on screen every frame windows have handed it so far: then a new
/// window's first frame is there, and it can come into view without a white flash. Returns at
/// once with an error where Windows can't be waited for that way.
pub fn wait_for_screen() -> io::Result<()> {
    // SAFETY: takes no arguments; it only waits for the desktop's next composition.
    #[allow(unsafe_code)]
    let result = unsafe { DwmFlush() };
    if result < 0 {
        Err(io::Error::from_raw_os_error(result))
    } else {
        Ok(())
    }
}

/// Puts a parked window back where it was.
pub fn unpark(window: u64, (x, y): (i32, i32)) -> io::Result<()> {
    move_to(window, x, y)
}

/// Far enough from any screen arrangement, but not the -32000 Windows uses for minimized windows.
const PARKED: i32 = -20000;

fn move_to(window: u64, x: i32, y: i32) -> io::Result<()> {
    let hwnd = window as usize as *mut core::ffi::c_void;
    // SAFETY: SetWindowPos only moves the window; an invalid handle makes it fail, reported.
    #[allow(unsafe_code)]
    let moved = unsafe {
        SetWindowPos(
            hwnd,
            std::ptr::null_mut(),
            x,
            y,
            0,
            0,
            SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
        )
    };
    if moved == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}
