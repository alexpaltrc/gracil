//! Which files look like videos by their name alone.
//!
//! Only a convenience for places that must decide without opening the file: the file picker's
//! default filter, and File Explorer's menu, which has to answer at once (spec §59). Gracil itself
//! accepts any file FFmpeg reads.

use std::path::Path;

/// Extensions of common video files, lowercase and without the dot.
pub const VIDEO_EXTENSIONS: &[&str] = &[
    "mp4", "m4v", "mov", "mkv", "avi", "wmv", "webm", "mts", "m2ts", "ts", "3gp", "flv", "mpg",
    "mpeg",
];

/// Whether `path` ends in a video extension, in any case.
pub fn looks_like_video(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            VIDEO_EXTENSIONS
                .iter()
                .any(|known| known.eq_ignore_ascii_case(extension))
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn videos_are_known_by_extension_in_any_case() {
        assert!(looks_like_video(Path::new(r"C:\V\Clip.MOV")));
        assert!(looks_like_video(Path::new(r"C:\V\Cumpleaños 🎂.mkv")));
        assert!(!looks_like_video(Path::new(r"C:\V\notes.txt")));
        assert!(!looks_like_video(Path::new(r"C:\V\mp4")));
    }
}
