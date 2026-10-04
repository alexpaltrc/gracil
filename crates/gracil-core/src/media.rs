//! Which files are images and which look like videos, by their name alone.
//!
//! Images are known by their extension (ADR 0034): every other file is treated as a video, and
//! Gracil accepts any video FFmpeg reads. The video list is only a convenience for places that must
//! decide without opening the file: the file picker's default filter, and File Explorer's menu,
//! which has to answer at once (spec §59).

use std::path::Path;

/// Extensions of common video files, lowercase and without the dot.
pub const VIDEO_EXTENSIONS: &[&str] = &[
    "mp4", "m4v", "mov", "mkv", "avi", "wmv", "webm", "mts", "m2ts", "ts", "3gp", "flv", "mpg",
    "mpeg",
];

/// Extensions of the images Gracil compresses (ADR 0034), lowercase and without the dot.
pub const IMAGE_EXTENSIONS: &[&str] = &[
    "jpg", "jpeg", "jpe", "jfif", "png", "heic", "heif", "avif", "webp", "tif", "tiff", "bmp",
];

/// Images Gracil does not compress (ADR 0034, item 1): GIF, JPEG XL and camera RAW. They are
/// treated as images, so they get the quiet "not compressed" signal instead of being handed to
/// FFmpeg as videos, but they are left out of the file picker and File Explorer's menu.
const REFUSED_IMAGE_EXTENSIONS: &[&str] = &[
    "gif", "jxl", "dng", "cr2", "cr3", "crw", "nef", "nrw", "arw", "srf", "sr2", "raf", "orf",
    "rw2", "pef", "srw", "rwl", "raw", "3fr", "iiq", "x3f", "erf", "kdc", "mrw", "mos",
];

/// What a file is, as far as Gracil is concerned.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Kind {
    Video,
    Image,
}

/// An image if its extension says so; anything else is given to FFmpeg as a video.
pub fn kind_of(path: &Path) -> Kind {
    if has_extension(path, IMAGE_EXTENSIONS) || is_refused_image(path) {
        Kind::Image
    } else {
        Kind::Video
    }
}

/// Whether `path` is a kind of image Gracil does not compress.
pub fn is_refused_image(path: &Path) -> bool {
    has_extension(path, REFUSED_IMAGE_EXTENSIONS)
}

/// Whether `path` ends in a video extension, in any case.
pub fn looks_like_video(path: &Path) -> bool {
    has_extension(path, VIDEO_EXTENSIONS)
}

/// Whether `path` ends in a video or image extension, in any case.
pub fn looks_like_media(path: &Path) -> bool {
    looks_like_video(path) || has_extension(path, IMAGE_EXTENSIONS)
}

fn has_extension(path: &Path, known: &[&str]) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| known.iter().any(|k| k.eq_ignore_ascii_case(extension)))
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

    #[test]
    fn images_are_known_by_extension_in_any_case() {
        for name in [
            "a.JPG", "b.jpeg", "c.HEIC", "d.png", "e.WebP", "f.tiff", "g.avif",
        ] {
            assert_eq!(kind_of(Path::new(name)), Kind::Image, "{name}");
            assert!(looks_like_media(Path::new(name)), "{name}");
            assert!(!is_refused_image(Path::new(name)), "{name}");
        }
        for name in ["d.mov", "e", "f.txt"] {
            assert_eq!(kind_of(Path::new(name)), Kind::Video, "{name}");
        }
    }

    #[test]
    fn refused_images_are_images_but_not_offered() {
        for name in ["a.gif", "b.JXL", "c.cr2", "d.NEF", "e.dng", "f.arw"] {
            assert_eq!(kind_of(Path::new(name)), Kind::Image, "{name}");
            assert!(is_refused_image(Path::new(name)), "{name}");
            assert!(!looks_like_media(Path::new(name)), "{name}");
        }
    }
}
