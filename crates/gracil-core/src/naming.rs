//! Names for files produced by "Compress and save a copy".
//!
//! `Vacation.mp4` becomes `Gracil-Vacation.mp4`. If that name is taken, Gracil uses the familiar
//! Windows pattern `Gracil-Vacation (2).mp4`, `Gracil-Vacation (3).mp4`, and so on.
//!
//! Names are built from `OsStr` pieces so the original name is kept byte for byte: no
//! transliteration, no Unicode normalization.
//!
//! Choosing a free name here is only a proposal. Placing the final file must still refuse to
//! overwrite, because another program can create the same name in the meantime.

use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

const PREFIX: &str = "Gracil-";

/// The longest name one file or folder can have on Windows, in UTF-16 units.
pub(crate) const LONGEST_NAME: usize = 255;

/// The `attempt`-th candidate name for a compressed copy of `source`, in the same folder.
/// Attempt 1 is `Gracil-<name>`; attempt `n > 1` adds ` (n)` before the extension. A name that
/// would grow past what Windows allows loses the end of its stem instead.
pub fn save_as_new_candidate(source: &Path, attempt: u32) -> PathBuf {
    let number = if attempt > 1 {
        format!(" ({attempt})")
    } else {
        String::new()
    };
    let mut ending = OsString::from(number);
    if let Some(extension) = source.extension() {
        ending.push(".");
        ending.push(extension);
    }
    let mut name = OsString::from(PREFIX);
    if let Some(stem) = source.file_stem() {
        name.push(stem);
    }
    let mut name = shortened(&name, units(&ending));
    name.push(ending);
    source.with_file_name(name)
}

/// `name`, shortened at the end if need be so that `room` more UTF-16 units still fit in one
/// name (ADR 0064). Never cuts a character in two; everything kept is kept exactly.
pub(crate) fn shortened(name: &OsStr, room: usize) -> OsString {
    let most = LONGEST_NAME.saturating_sub(room);
    #[cfg(windows)]
    {
        use std::os::windows::ffi::{OsStrExt, OsStringExt};
        let mut wide: Vec<u16> = name.encode_wide().collect();
        if wide.len() <= most {
            return name.to_owned();
        }
        wide.truncate(most);
        // The first half of a character outside the basic plane is left out with its pair.
        if wide
            .last()
            .is_some_and(|&unit| (0xD800..0xDC00).contains(&unit))
        {
            wide.pop();
        }
        OsString::from_wide(&wide)
    }
    #[cfg(not(windows))]
    {
        let _ = most;
        name.to_owned()
    }
}

/// How many UTF-16 units `name` takes on Windows.
pub(crate) fn units(name: &OsStr) -> usize {
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        name.encode_wide().count()
    }
    #[cfg(not(windows))]
    {
        name.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn name_of(path: &Path) -> &str {
        path.file_name().and_then(|n| n.to_str()).unwrap()
    }

    #[test]
    fn adds_prefix_in_same_folder() {
        let out = save_as_new_candidate(Path::new(r"C:\Videos\Vacation.mp4"), 1);
        assert_eq!(out, Path::new(r"C:\Videos\Gracil-Vacation.mp4"));
    }

    #[test]
    fn keeps_unicode_and_unusual_characters_exactly() {
        let cases = [
            (
                r"C:\Videos\Mi cumpleaños 🎂.mp4",
                "Gracil-Mi cumpleaños 🎂.mp4",
            ),
            (r"C:\日本語\動画.mp4", "Gracil-動画.mp4"),
            (r"C:\Videos\José\Vacación.mp4", "Gracil-Vacación.mp4"),
            (
                r"C:\V\Tom's (final) [v2].mov",
                "Gracil-Tom's (final) [v2].mov",
            ),
            (r"C:\V\trip.2024.07.mkv", "Gracil-trip.2024.07.mkv"),
            (r"C:\V\UPPER.MP4", "Gracil-UPPER.MP4"),
            (r"C:\V\no_extension", "Gracil-no_extension"),
        ];
        for (source, expected) in cases {
            assert_eq!(
                name_of(&save_as_new_candidate(Path::new(source), 1)),
                expected
            );
        }
    }

    #[test]
    fn decomposed_unicode_is_not_normalized() {
        // "e" followed by a combining acute accent must stay two code points.
        let source = PathBuf::from("C:\\V\\Jose\u{301}.mp4");
        let out = save_as_new_candidate(&source, 1);
        assert_eq!(name_of(&out), "Gracil-Jose\u{301}.mp4");
    }

    #[test]
    fn a_copy_of_a_long_name_still_fits() {
        let stem = "é".repeat(250);
        let source = PathBuf::from(format!(r"C:\V\{stem}.mp4"));
        for attempt in [1, 2, 12] {
            let out = save_as_new_candidate(&source, attempt);
            let name = out.file_name().unwrap();
            assert_eq!(units(name), LONGEST_NAME);
            let name = name.to_str().unwrap();
            assert!(name.starts_with("Gracil-é"), "{name}");
            assert!(name.ends_with(".mp4"), "{name}");
        }
    }

    #[test]
    fn shortening_never_cuts_a_character_in_two() {
        // Each 🎂 takes two UTF-16 units; 255 is odd, so the last one can't fit.
        let name = OsString::from("🎂".repeat(200));
        let short = shortened(&name, 0);
        assert_eq!(short.to_str().unwrap(), "🎂".repeat(127));
    }

    #[test]
    fn numbered_candidates_go_before_extension() {
        let out = save_as_new_candidate(Path::new(r"C:\V\Vacation.mp4"), 3);
        assert_eq!(name_of(&out), "Gracil-Vacation (3).mp4");
    }
}
