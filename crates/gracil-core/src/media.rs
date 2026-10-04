//! Which files are images and which look like videos, by their name alone.
//!
//! Images are known by their extension (ADR 0034): every other file is treated as a video, and
//! Gracil accepts any video FFmpeg reads. The video list is only a convenience for places that must
//! decide without opening the file: the file picker's default filter, and File Explorer's menu,
//! which has to answer at once (spec §59).

use std::path::{Path, PathBuf};

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

/// The files to compress from what File Explorer sent (ADR 0035). A file given by itself is kept
/// whatever its name, unless `only` asks for one kind. A folder is looked through, subfolders
/// included, for the videos and images in it (of the `only` kind, when given), in name order.
/// Links to other folders are not followed, so a folder can never lead back to itself.
pub fn gather(paths: impl IntoIterator<Item = PathBuf>, only: Option<Kind>) -> Vec<PathBuf> {
    let wanted = |path: &Path| only.is_none_or(|kind| kind_of(path) == kind);
    let mut found = Vec::new();
    for path in paths {
        if path.is_dir() {
            walk(
                &path,
                &|path| looks_like_media(path) && wanted(path),
                &mut found,
            );
        } else if path.is_file() && (only.is_none() || looks_like_media(&path) && wanted(&path)) {
            found.push(path);
        }
    }
    found
}

fn walk(dir: &Path, wanted: &dyn Fn(&Path) -> bool, found: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut entries: Vec<_> = entries.flatten().collect();
    entries.sort_by_key(std::fs::DirEntry::file_name);
    for entry in entries {
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        let path = entry.path();
        if kind.is_dir() {
            walk(&path, wanted, found);
        } else if kind.is_file() && wanted(&path) {
            found.push(path);
        }
    }
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

    #[test]
    #[allow(clippy::unwrap_used)]
    fn folders_are_looked_through_with_their_subfolders() {
        let root = std::env::temp_dir().join(format!("gracil-gather-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let deep = root.join("trip").join("day 1");
        std::fs::create_dir_all(&deep).unwrap();
        for name in ["b.mp4", "a.jpg", "notes.txt", "anim.gif"] {
            std::fs::write(root.join(name), b"x").unwrap();
        }
        std::fs::write(deep.join("c.MOV"), b"x").unwrap();
        std::fs::write(deep.join("d.heic"), b"x").unwrap();
        let names = |found: Vec<PathBuf>| -> Vec<String> {
            found
                .iter()
                .map(|path| path.file_name().unwrap().to_string_lossy().into_owned())
                .collect()
        };
        let all = names(gather([root.clone()], None));
        assert_eq!(all, ["a.jpg", "b.mp4", "c.MOV", "d.heic"]);
        assert_eq!(
            names(gather([root.clone()], Some(Kind::Video))),
            ["b.mp4", "c.MOV"]
        );
        assert_eq!(
            names(gather([root.clone()], Some(Kind::Image))),
            ["a.jpg", "d.heic"]
        );
        // A file given by itself is taken whatever its name, as when it is dropped.
        assert_eq!(names(gather([root.join("notes.txt")], None)), ["notes.txt"]);
        assert!(gather([root.join("notes.txt")], Some(Kind::Video)).is_empty());
        std::fs::remove_dir_all(root).unwrap();
    }
}
