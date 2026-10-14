//! Windows' choices about how apps look and move: light and dark, the accent color, contrast
//! themes and animation effects, read where Windows keeps them.

use std::ptr;

use windows_sys::Win32::Graphics::Gdi::{
    COLOR_GRAYTEXT, COLOR_HIGHLIGHT, COLOR_HIGHLIGHTTEXT, COLOR_WINDOW, COLOR_WINDOWTEXT,
    GetSysColor,
};
use windows_sys::Win32::System::Registry::{
    HKEY_CURRENT_USER, RRF_RT_REG_BINARY, RRF_RT_REG_DWORD, RegGetValueW,
};
use windows_sys::Win32::UI::Accessibility::{HCF_HIGHCONTRASTON, HIGHCONTRASTW};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    SPI_GETCLIENTAREAANIMATION, SPI_GETHIGHCONTRAST, SystemParametersInfoW,
};
use windows_sys::core::BOOL;

use crate::wide;

/// Whether apps should be light, as chosen in Windows' settings, or `None` if it can't be read.
/// Known before any window exists, so a first window opens in the right colors.
pub fn light_apps() -> Option<bool> {
    personalize("AppsUseLightTheme")
}

/// Whether the taskbar is light. Windows 11's taskbar is dark unless the user chose light.
pub(crate) fn light_taskbar() -> bool {
    personalize("SystemUsesLightTheme").unwrap_or(false)
}

/// The user's accent color, in the two shades Windows 11 fills controls with: a darker one for
/// light mode and a lighter one for dark mode, as `[red, green, blue]`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Accent {
    pub on_light: [u8; 3],
    pub on_dark: [u8; 3],
}

/// The accent chosen in Windows' settings, or `None` if it can't be read. Windows keeps it as
/// eight colors, lightest first: three lighter shades, the color itself, three darker shades.
pub fn accent() -> Option<Accent> {
    const LIGHT_2: usize = 1;
    const DARK_1: usize = 4;
    let key = wide(r"Software\Microsoft\Windows\CurrentVersion\Explorer\Accent");
    let value = wide("AccentPalette");
    let mut data = [0u8; 32];
    let mut size = data.len() as u32;
    // SAFETY: the names are NUL-terminated UTF-16 buffers, and `size` tells Windows how much room
    // `data` has; a larger value fails instead of being written.
    #[allow(unsafe_code)]
    let status = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            key.as_ptr(),
            value.as_ptr(),
            RRF_RT_REG_BINARY,
            ptr::null_mut(),
            data.as_mut_ptr().cast(),
            &mut size,
        )
    };
    if status != 0 || size < 32 {
        return None;
    }
    let shade = |index: usize| [data[index * 4], data[index * 4 + 1], data[index * 4 + 2]];
    Some(Accent {
        on_light: shade(DARK_1),
        on_dark: shade(LIGHT_2),
    })
}

fn personalize(name: &str) -> Option<bool> {
    let key = wide(r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize");
    let value = wide(name);
    let mut data: u32 = 0;
    let mut size = size_of::<u32>() as u32;
    // SAFETY: the names are NUL-terminated UTF-16 buffers, and `data` has room for the DWORD that
    // the flags restrict the answer to.
    #[allow(unsafe_code)]
    let status = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            key.as_ptr(),
            value.as_ptr(),
            RRF_RT_REG_DWORD,
            ptr::null_mut(),
            (&raw mut data).cast(),
            &mut size,
        )
    };
    (status == 0).then_some(data != 0)
}

/// Whether Windows' "Animation effects" setting is on. Assumed on if it can't be read, as it is
/// unless someone turned it off.
pub fn animations() -> bool {
    let mut on: BOOL = 1;
    // SAFETY: for this action Windows writes one BOOL to the address given, which is `on`.
    #[allow(unsafe_code)]
    let read =
        unsafe { SystemParametersInfoW(SPI_GETCLIENTAREAANIMATION, 0, (&raw mut on).cast(), 0) };
    read == 0 || on != 0
}

/// The colors of a contrast theme, as `[red, green, blue]`: what Windows asks every app to draw
/// with while one is on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Contrast {
    pub background: [u8; 3],
    pub text: [u8; 3],
    /// Selected and filled controls, and a control under the pointer.
    pub highlight: [u8; 3],
    /// Text on `highlight`.
    pub highlight_text: [u8; 3],
    /// Controls that can't be used now.
    pub disabled: [u8; 3],
}

/// The contrast theme's colors while one is on in Windows' settings, or `None`.
pub fn contrast() -> Option<Contrast> {
    let mut settings = HIGHCONTRASTW {
        cbSize: size_of::<HIGHCONTRASTW>() as u32,
        ..HIGHCONTRASTW::default()
    };
    // SAFETY: for this action Windows fills the HIGHCONTRASTW given, whose size it is told both
    // in `cbSize` and in the second argument.
    #[allow(unsafe_code)]
    let read = unsafe {
        SystemParametersInfoW(
            SPI_GETHIGHCONTRAST,
            settings.cbSize,
            (&raw mut settings).cast(),
            0,
        )
    };
    if read == 0 || settings.dwFlags & HCF_HIGHCONTRASTON == 0 {
        return None;
    }
    let color = |index| {
        // SAFETY: GetSysColor only reads, and every index used here is one of Windows' own.
        #[allow(unsafe_code)]
        let [r, g, b, _] = unsafe { GetSysColor(index) }.to_le_bytes();
        [r, g, b]
    };
    Some(Contrast {
        background: color(COLOR_WINDOW),
        text: color(COLOR_WINDOWTEXT),
        highlight: color(COLOR_HIGHLIGHT),
        highlight_text: color(COLOR_HIGHLIGHTTEXT),
        disabled: color(COLOR_GRAYTEXT),
    })
}
