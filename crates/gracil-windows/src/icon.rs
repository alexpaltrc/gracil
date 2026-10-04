//! The window's icon, taken from the ones linked into gracil.exe (ADR 0016, ADR 0031).
//!
//! Windows picks the closest picture in the icon file for each size, so the small icon in the
//! title bar uses the version drawn for 16 px instead of a shrunken large one. The icon is the
//! opposite of Windows' theme: the dark tile in light mode, the light tile in dark mode.

use std::io;
use std::ptr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, MutexGuard};

use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::HiDpi::{GetDpiForWindow, GetSystemMetricsForDpi};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    DestroyIcon, HICON, ICON_BIG, ICON_SMALL, IMAGE_ICON, LR_DEFAULTCOLOR, LoadImageW, SM_CXICON,
    SM_CXSMICON, SendMessageW, WM_SETICON,
};

/// The icons' numbers in gracil.rc: the dark tile, for light mode, and the light tile.
const DARK_TILE: u16 = 1;
const LIGHT_TILE: u16 = 2;

/// Whether Windows is in dark mode, so windows get the light tile.
static DARK_MODE: AtomicBool = AtomicBool::new(false);

/// The icons Gracil gave each window, as addresses, so they can be released when replaced.
static GIVEN: Mutex<Vec<(u64, [usize; 2])>> = Mutex::new(Vec::new());

/// Follows Windows' theme: every window Gracil gave an icon gets the one for `light`, and later
/// windows too. Windows that have closed since are forgotten, and their icons released.
pub fn set_icon_theme(light: bool) {
    if DARK_MODE.swap(!light, Ordering::Relaxed) == !light {
        return;
    }
    let windows: Vec<u64> = given().iter().map(|(window, _)| *window).collect();
    for window in windows {
        if set_window_icon(window).is_err() {
            let mut given = given();
            if let Some(index) = given.iter().position(|(handle, _)| *handle == window) {
                given.swap_remove(index).1.into_iter().for_each(release);
            }
        }
    }
}

/// Gives the window Gracil's icon at the sizes its current scale needs. Call it again when the
/// window's scale changes. `window` is the window's handle, as Iced's `window::raw_id` gives it.
pub fn set_window_icon(window: u64) -> io::Result<()> {
    let hwnd = window as usize as *mut core::ffi::c_void;
    // SAFETY: GetDpiForWindow only reads; an invalid handle makes it return 0, handled below.
    #[allow(unsafe_code)]
    let dpi = unsafe { GetDpiForWindow(hwnd) };
    if dpi == 0 {
        return Err(io::Error::other("not a window"));
    }

    let small = load(SM_CXSMICON, dpi)?;
    let big = match load(SM_CXICON, dpi) {
        Ok(big) => big,
        Err(error) => {
            release(small as usize);
            return Err(error);
        }
    };
    for (kind, icon) in [(ICON_SMALL, small), (ICON_BIG, big)] {
        // SAFETY: the window handle was checked above and the icon was just loaded. The window
        // keeps using the icon, so it is released only once replaced (below).
        #[allow(unsafe_code)]
        unsafe {
            SendMessageW(hwnd, WM_SETICON, kind as usize, icon as isize)
        };
    }

    let mut given = given();
    let icons = [small as usize, big as usize];
    match given.iter_mut().find(|(handle, _)| *handle == window) {
        Some((_, previous)) => std::mem::replace(previous, icons)
            .into_iter()
            .for_each(release),
        None => given.push((window, icons)),
    }
    Ok(())
}

fn load(metric: i32, dpi: u32) -> io::Result<HICON> {
    let id = if DARK_MODE.load(Ordering::Relaxed) {
        LIGHT_TILE
    } else {
        DARK_TILE
    };
    // SAFETY: only reads system metrics.
    #[allow(unsafe_code)]
    let size = unsafe { GetSystemMetricsForDpi(metric, dpi) };
    // SAFETY: a null name asks for the running executable, which stays loaded for the whole
    // process. The icon's name is a resource number in the form MAKEINTRESOURCE produces.
    #[allow(unsafe_code)]
    let icon = unsafe {
        let module = GetModuleHandleW(ptr::null());
        LoadImageW(
            module,
            usize::from(id) as *const u16,
            IMAGE_ICON,
            size,
            size,
            LR_DEFAULTCOLOR,
        )
    };
    if icon.is_null() {
        return Err(io::Error::last_os_error());
    }
    Ok(icon)
}

fn given() -> MutexGuard<'static, Vec<(u64, [usize; 2])>> {
    GIVEN
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn release(icon: usize) {
    // SAFETY: only icons this module loaded are released, once, after the window stopped using
    // them.
    #[allow(unsafe_code)]
    unsafe {
        DestroyIcon(icon as HICON)
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn something_that_is_not_a_window_is_refused() {
        assert!(set_window_icon(0).is_err());
    }
}
