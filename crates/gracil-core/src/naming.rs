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

use std::ffi::OsString;
use std::path::{Path, PathBuf};

const PREFIX: &str = "Gracil-";

/// The `attempt`-th candidate name for a compressed copy of `source`, in the same folder.
/// Attempt 1 is `Gracil-<name>`; attempt `n > 1` adds ` (n)` before the extension.
pub fn save_as_new_candidate(source: &Path, attempt: u32) -> PathBuf {
    let mut name = OsString::from(PREFIX);
    if let Some(stem) = source.file_stem() {
        name.push(stem);
    }
    if attempt > 1 {
        name.push(format!(" ({attempt})"));
    }
    if let Some(extension) = source.extension() {
        name.push(".");
        name.push(extension);
    }
    source.with_file_name(name)
}

/// The first candidate for which `exists` returns false, or `None` if every candidate is taken.
pub fn first_free_save_as_new_path(
    source: &Path,
    mut exists: impl FnMut(&Path) -> bool,
) -> Option<PathBuf> {
    (1..=u32::MAX)
        .map(|attempt| save_as_new_candidate(source, attempt))
        .find(|candidate| !exists(candidate))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

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
    fn numbered_candidates_go_before_extension() {
        let out = save_as_new_candidate(Path::new(r"C:\V\Vacation.mp4"), 3);
        assert_eq!(name_of(&out), "Gracil-Vacation (3).mp4");
    }

    #[test]
    fn first_free_skips_taken_names() {
        let source = Path::new(r"C:\V\Vacation.mp4");
        let taken: HashSet<PathBuf> = [
            PathBuf::from(r"C:\V\Gracil-Vacation.mp4"),
            PathBuf::from(r"C:\V\Gracil-Vacation (2).mp4"),
        ]
        .into();
        let out = first_free_save_as_new_path(source, |p| taken.contains(p));
        assert_eq!(out, Some(PathBuf::from(r"C:\V\Gracil-Vacation (3).mp4")));
    }

    #[test]
    fn first_free_uses_plain_name_when_available() {
        let source = Path::new(r"C:\V\Vacation.mp4");
        let out = first_free_save_as_new_path(source, |_| false);
        assert_eq!(out, Some(PathBuf::from(r"C:\V\Gracil-Vacation.mp4")));
    }

    #[test]
    fn first_free_against_real_directory() {
        let dir = std::env::temp_dir().join(format!("gracil-naming-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let source = dir.join("Mi cumpleaños 🎂.mp4");
        std::fs::write(dir.join("Gracil-Mi cumpleaños 🎂.mp4"), b"").unwrap();

        let out = first_free_save_as_new_path(&source, Path::exists);

        std::fs::remove_dir_all(&dir).unwrap();
        assert_eq!(out, Some(dir.join("Gracil-Mi cumpleaños 🎂 (2).mp4")));
    }
}
