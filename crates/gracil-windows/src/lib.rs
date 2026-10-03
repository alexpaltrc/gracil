//! Everything Gracil needs from Windows itself (see docs/architecture/overview.md).
//!
//! This is one of the two crates allowed to use `unsafe`, and only to call Windows functions.
//! Every call explains why it is sound.

use std::ffi::OsStr;
use std::io;
use std::iter;
use std::os::windows::ffi::OsStrExt;
use std::path::Path;
use std::ptr;

use windows_sys::Win32::Storage::FileSystem::{
    REPLACEFILE_IGNORE_ACL_ERRORS, REPLACEFILE_IGNORE_MERGE_ERRORS, REPLACEFILE_WRITE_THROUGH,
    ReplaceFileW,
};

/// Puts `replacement` in place of `original` in one step with `ReplaceFileW` (spec §39).
///
/// The file keeps the original's identity: creation date, attributes and permissions. While
/// swapping, the original is moved to `backup`; the caller deletes it once satisfied, or moves it
/// back if this fails after the original was moved away.
pub fn replace_file(original: &Path, replacement: &Path, backup: &Path) -> io::Result<()> {
    let (original, replacement, backup) = (wide(original), wide(replacement), wide(backup));
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

/// A path as Windows expects it: UTF-16, NUL-terminated, with the long-path prefix when needed.
fn wide(path: &Path) -> Vec<u16> {
    const MAX_PATH: usize = 260;
    const VERBATIM: &str = r"\\?\";
    let text = path.as_os_str();
    let long = text.len() >= MAX_PATH
        && path.is_absolute()
        && !text.to_string_lossy().starts_with(VERBATIM);
    let prefix = OsStr::new(if long { VERBATIM } else { "" });
    prefix
        .encode_wide()
        .chain(text.encode_wide())
        .chain(iter::once(0))
        .collect()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use std::fs;
    use std::path::PathBuf;

    use super::*;

    fn fresh_dir(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("gracil-windows-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

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
        let starts = |path: &Path| String::from_utf16_lossy(&wide(path)).starts_with(r"\\?\");
        assert!(starts(&long));
        assert!(!starts(&short));
    }
}
