//! Everything Gracil needs from Windows itself (see docs/architecture/overview.md).
//!
//! This is the only crate allowed to use `unsafe`, and only to call Windows functions.
//! Every call explains why it is sound.

/// Whether this is Gracil Dev: the installed copy for trying changes, made with the `dev`
/// feature, or a development build. It has a name, a place in Windows and icons of its own, so it
/// runs beside the Gracil in daily use and never takes what is theirs (ADR 0082).
pub const DEV: bool = cfg!(any(debug_assertions, feature = "dev"));

/// The name Windows shows: in File Explorer's menus, starting with Windows, the window and tray.
pub const NAME: &str = if DEV { "Gracil Dev" } else { "Gracil" };

mod appear;
mod carry;
mod drawing;
mod flyout;
mod heat;
mod icon;
mod instance;
mod integration;
mod lifetime;
mod numbers;
mod process;
mod quiet;
mod theme;
mod tray;

pub use appear::{park, unpark, wait_for_screen};
pub use carry::{Carried, carry_out};
pub use drawing::draw_with_direct3d;
pub use flyout::place_near_tray;
pub use heat::{Card, GraphicsHeat, Heat, Thermometer};
pub use icon::{set_icon_theme, set_window_icon};
pub use instance::{claim, forward, is_running, serve};
pub use integration::{
    ExplorerCommand, ExplorerMenu, add_main_menu, add_to_send_to, main_menu_program,
    remove_former_menus, remove_from_send_to, remove_main_menu, run_as_administrator,
    start_with_windows, stop_starting_with_windows,
};
pub use lifetime::{
    end_children_with_this_process, quiet_tool, usable_threads, use_efficiency_mode, use_processors,
};
pub use numbers::{NumberSettings, number_settings};
pub use process::set_suspended;
pub use quiet::{
    Power, free_memory, free_space, idle_for, on_sleep, own_memory, person_busy, power,
    saving_energy, total_memory, volume_of, work_in_background, work_quietly,
};
pub use theme::{Accent, Contrast, accent, animations, contrast, light_apps};
pub use tray::{Tray, TrayEvent, TrayMenu, TrayState};

use std::ffi::OsStr;
use std::io;
use std::iter;
use std::os::windows::ffi::OsStrExt;
use std::path::Path;
use std::ptr;

use windows_sys::Win32::Globalization::GetUserDefaultUILanguage;
use windows_sys::Win32::Storage::FileSystem::{
    FILE_ATTRIBUTE_ARCHIVE, FILE_ATTRIBUTE_HIDDEN, FILE_ATTRIBUTE_NOT_CONTENT_INDEXED,
    FILE_ATTRIBUTE_OFFLINE, FILE_ATTRIBUTE_READONLY, FILE_ATTRIBUTE_SYSTEM,
    FILE_ATTRIBUTE_TEMPORARY, FILE_ATTRIBUTE_UNPINNED, GetFileAttributesW, INVALID_FILE_ATTRIBUTES,
    REPLACEFILE_IGNORE_ACL_ERRORS, REPLACEFILE_IGNORE_MERGE_ERRORS, REPLACEFILE_WRITE_THROUGH,
    ReplaceFileW, SetFileAttributesW,
};

/// Puts `replacement` in place of `original` in one step with `ReplaceFileW` (spec §39).
///
/// The file keeps the original's identity: creation date, attributes and permissions. While
/// swapping, the original is moved to `backup`; the caller deletes it once satisfied, or moves it
/// back if this fails after the original was moved away.
pub fn replace_file(original: &Path, replacement: &Path, backup: &Path) -> io::Result<()> {
    let (original, replacement, backup) = (
        wide_path(original),
        wide_path(replacement),
        wide_path(backup),
    );
    let flags =
        REPLACEFILE_WRITE_THROUGH | REPLACEFILE_IGNORE_MERGE_ERRORS | REPLACEFILE_IGNORE_ACL_ERRORS;
    // SAFETY: the three names are NUL-terminated UTF-16 buffers that outlive the call, and the
    // two reserved pointers are null as the documentation requires.
    #[allow(unsafe_code)]
    let replaced = unsafe {
        ReplaceFileW(
            original.as_ptr(),
            replacement.as_ptr(),
            backup.as_ptr(),
            flags,
            ptr::null(),
            ptr::null(),
        )
    };
    if replaced == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

/// Lets a cloud drive (OneDrive and the like) free up `path` again, as "Free up space" does, once
/// Gracil is done with a file that was kept only online before (ADR 0064). Best effort: nothing
/// else about the file changes, and its read-only mark is never touched.
pub fn unpin(path: &Path) -> io::Result<()> {
    // The attributes `SetFileAttributesW` accepts, besides the two pin marks.
    const SETTABLE: u32 = FILE_ATTRIBUTE_READONLY
        | FILE_ATTRIBUTE_HIDDEN
        | FILE_ATTRIBUTE_SYSTEM
        | FILE_ATTRIBUTE_ARCHIVE
        | FILE_ATTRIBUTE_TEMPORARY
        | FILE_ATTRIBUTE_OFFLINE
        | FILE_ATTRIBUTE_NOT_CONTENT_INDEXED;
    let wide = wide_path(path);
    // SAFETY: the name is a NUL-terminated UTF-16 buffer that outlives the call.
    #[allow(unsafe_code)]
    let now = unsafe { GetFileAttributesW(wide.as_ptr()) };
    if now == INVALID_FILE_ATTRIBUTES {
        return Err(io::Error::last_os_error());
    }
    let wanted = (now & SETTABLE) | FILE_ATTRIBUTE_UNPINNED;
    if wanted == now {
        return Ok(());
    }
    // SAFETY: as above; the attributes are ones the call accepts.
    #[allow(unsafe_code)]
    let set = unsafe { SetFileAttributesW(wide.as_ptr(), wanted) };
    if set == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

/// Whether the user's Windows display language is Spanish (any region). Gracil's interface follows
/// it; everything else is shown in English (ADR 0011).
pub fn display_language_is_spanish() -> bool {
    const LANG_SPANISH: u16 = 0x0A;
    const PRIMARY_LANGUAGE_MASK: u16 = 0x3FF;
    // SAFETY: takes no arguments and only reads the current user's settings.
    #[allow(unsafe_code)]
    let language = unsafe { GetUserDefaultUILanguage() };
    language & PRIMARY_LANGUAGE_MASK == LANG_SPANISH
}

/// `text` as Windows expects it: UTF-16, NUL-terminated.
fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(iter::once(0)).collect()
}

/// A path as Windows expects it: UTF-16, NUL-terminated, with the long-path prefix when needed:
/// `\\?\C:\…` on a drive, `\\?\UNC\server\share\…` on a network share.
fn wide_path(path: &Path) -> Vec<u16> {
    const MAX_PATH: usize = 260;
    const VERBATIM: &str = r"\\?\";
    const SHARE: &str = r"\\";
    let text = path.as_os_str();
    let lossy = text.to_string_lossy();
    let long = text.len() >= MAX_PATH && path.is_absolute() && !lossy.starts_with(VERBATIM);
    let (prefix, skip) = match (long, lossy.starts_with(SHARE)) {
        (false, _) => ("", 0),
        (true, false) => (VERBATIM, 0),
        (true, true) => (r"\\?\UNC\", SHARE.len()),
    };
    OsStr::new(prefix)
        .encode_wide()
        .chain(text.encode_wide().skip(skip))
        .chain(iter::once(0))
        .collect()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use std::fs;
    use std::path::PathBuf;

    use gracil_core::testing::fresh_dir;

    use super::*;

    #[test]
    fn replaces_contents_and_keeps_backup() {
        let dir = fresh_dir("replace");
        let original = dir.join("Cumpleaños 🎂.mp4");
        let replacement = dir.join("Cumpleaños 🎂.mp4.gracil-1.part");
        let backup = dir.join("Cumpleaños 🎂.mp4.gracil-1.bak");
        fs::write(&original, b"original").unwrap();
        fs::write(&replacement, b"compressed").unwrap();

        replace_file(&original, &replacement, &backup).unwrap();
        let now = fs::read(&original).unwrap();
        let kept = fs::read(&backup).unwrap();
        let replacement_left = replacement.exists();
        fs::remove_dir_all(&dir).unwrap();

        assert_eq!(now, b"compressed");
        assert_eq!(kept, b"original");
        assert!(!replacement_left);
    }

    #[test]
    fn missing_replacement_leaves_original_alone() {
        let dir = fresh_dir("missing");
        let original = dir.join("Clip.mp4");
        fs::write(&original, b"original").unwrap();

        let result = replace_file(&original, &dir.join("gone.part"), &dir.join("Clip.bak"));
        let now = fs::read(&original).unwrap();
        fs::remove_dir_all(&dir).unwrap();

        assert!(result.is_err());
        assert_eq!(now, b"original");
    }

    #[test]
    fn long_paths_get_the_verbatim_prefix() {
        let long = PathBuf::from(format!(r"C:\{}\Clip.mp4", "a".repeat(300)));
        let short = PathBuf::from(r"C:\Videos\Clip.mp4");
        let starts = |path: &Path| String::from_utf16_lossy(&wide_path(path)).starts_with(r"\\?\");
        assert!(starts(&long));
        assert!(!starts(&short));
        let share = PathBuf::from(format!(r"\\nas\videos\{}\Clip.mp4", "a".repeat(300)));
        let wide = String::from_utf16_lossy(&wide_path(&share));
        assert!(wide.starts_with(r"\\?\UNC\nas\videos\"));
    }
}
