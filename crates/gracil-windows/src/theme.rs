//! Windows' light and dark choices, read where Windows keeps them.

use std::ptr;

use windows_sys::Win32::System::Registry::{HKEY_CURRENT_USER, RRF_RT_REG_DWORD, RegGetValueW};

/// Whether apps should be light, as chosen in Windows' settings, or `None` if it can't be read.
/// Known before any window exists, so a first window opens in the right colors.
pub fn light_apps() -> Option<bool> {
    personalize("AppsUseLightTheme")
}

/// Whether the taskbar is light. Windows 11's taskbar is dark unless the user chose light.
pub(crate) fn light_taskbar() -> bool {
    personalize("SystemUsesLightTheme").unwrap_or(false)
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

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(Some(0)).collect()
}
