//! Which files are images and which look like videos, by their name alone.
//!
//! Images are known by their extension (ADR 0034): every other file is treated as a video, and
//! Gracil accepts any video FFmpeg reads. The video list is only a convenience for places that must
//! decide without opening the file: the file picker's default filter, and File Explorer's menu,
//! which has to answer at once (spec §59).

use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// Extensions of common video files, lowercase and without the dot.
pub const VIDEO_EXTENSIONS: &[&str] = &[
    "mp4", "m4v", "mov", "mkv", "avi", "wmv", "webm", "mts", "m2ts", "ts", "3gp", "flv", "mpg",
    "mpeg",
];

/// The ways a JPEG file's extension is spelled, lowercase and without the dot.
pub const JPEG_EXTENSIONS: &[&str] = &["jpg", "jpeg", "jpe", "jfif"];

/// Extensions of the images Gracil compresses (ADR 0034), lowercase and without the dot. It
/// starts with [`JPEG_EXTENSIONS`].
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
/// included, for the videos and images in it (of the `only` kind, when given): each folder's own
/// files in name order, then its subfolders in name order. Links to other folders are not
/// followed, so a folder can never lead back to itself.
pub fn gather(paths: impl IntoIterator<Item = PathBuf>, only: Option<Kind>) -> Vec<PathBuf> {
    let mut found = Vec::new();
    gather_each(paths, only, &mut |batch| {
        found.extend(batch.into_iter().map(|(path, _)| path));
    });
    found
}

/// [`gather`], handing over what it finds folder by folder as soon as each folder is read, so
/// work can start before a large or slow folder (a network or cloud drive) is looked through
/// (ADR 0037). Only names are read: no file is opened, so nothing has to be downloaded. Each file
/// comes with its size in bytes, as the folder's listing gives it (0 when it can't be told), which
/// weighs how long it will take (ADR 0069).
pub fn gather_each(
    paths: impl IntoIterator<Item = PathBuf>,
    only: Option<Kind>,
    found: &mut dyn FnMut(Vec<(PathBuf, u64)>),
) {
    let wanted = |path: &Path| only.is_none_or(|kind| kind_of(path) == kind);
    let mut loose = Vec::new();
    for path in paths {
        let Ok(metadata) = std::fs::metadata(&path) else {
            continue;
        };
        if metadata.is_dir() {
            if !loose.is_empty() {
                found(std::mem::take(&mut loose));
            }
            walk(&path, &|path| looks_like_media(path) && wanted(path), found);
        } else if metadata.is_file() && (only.is_none() || looks_like_media(&path) && wanted(&path))
        {
            loose.push((path, metadata.len()));
        }
    }
    if !loose.is_empty() {
        found(loose);
    }
}

fn walk(dir: &Path, wanted: &dyn Fn(&Path) -> bool, found: &mut dyn FnMut(Vec<(PathBuf, u64)>)) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut entries: Vec<_> = entries.flatten().collect();
    entries.sort_by_key(std::fs::DirEntry::file_name);
    let mut files = Vec::new();
    let mut folders = Vec::new();
    for entry in entries {
        // What kind of entry it is comes with the folder's listing; nothing more is asked.
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        if not_the_persons(&entry) {
            continue;
        }
        let path = entry.path();
        if kind.is_dir() {
            folders.push(path);
        } else if kind.is_file() && wanted(&path) {
            // On Windows its size comes with the listing too.
            let bytes = entry.metadata().map_or(0, |metadata| metadata.len());
            files.push((path, bytes));
        }
    }
    if !files.is_empty() {
        found(files);
    }
    for folder in folders {
        walk(&folder, wanted, found);
    }
}

/// Folders Windows and other systems keep for themselves on a drive: the recycle bin, restore
/// points, and a Mac's trash.
const SYSTEM_FOLDERS: [&str; 3] = ["$RECYCLE.BIN", "System Volume Information", ".Trashes"];

/// Whether an entry inside a folder being looked through belongs to the system rather than the
/// person (ADR 0064): hidden files and folders (system ones among them), the folders in
/// [`SYSTEM_FOLDERS`], and the `._` files a Mac leaves beside every file it copies. A folder marked
/// system or read-only alone is the person's: Windows marks folders so when they are given their
/// own icon or look, as Pictures and Videos are. A file dropped or chosen by itself is never looked
/// at this way.
fn not_the_persons(entry: &std::fs::DirEntry) -> bool {
    let name = entry.file_name();
    let name = name.to_string_lossy();
    if name.starts_with("._")
        || SYSTEM_FOLDERS
            .iter()
            .any(|folder| name.eq_ignore_ascii_case(folder))
    {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const HIDDEN: u32 = 0x2;
        // Comes with the folder's listing too.
        if let Ok(metadata) = entry.metadata()
            && metadata.file_attributes() & HIDDEN != 0
        {
            return true;
        }
    }
    false
}

/// The extension a result in a format known by `same` gets: the source's own spelling when it
/// already is one of them (so `Clip.MP4` stays `Clip.MP4` and replaces the original in place
/// rather than being renamed), otherwise `default`.
pub fn keep_spelling(source: &Path, same: &[&str], default: &str) -> OsString {
    match source.extension() {
        Some(ext) if same.iter().any(|s| ext.eq_ignore_ascii_case(s)) => ext.to_owned(),
        _ => default.into(),
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
    fn every_jpeg_spelling_is_an_image() {
        assert_eq!(IMAGE_EXTENSIONS[..JPEG_EXTENSIONS.len()], *JPEG_EXTENSIONS);
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
        let mut batches = Vec::new();
        gather_each([root.clone()], None, &mut |batch| {
            assert!(
                batch.iter().all(|&(_, bytes)| bytes == 1),
                "with their sizes"
            );
            batches.push(names(batch.into_iter().map(|(path, _)| path).collect()))
        });
        assert_eq!(
            batches,
            [vec!["a.jpg", "b.mp4"], vec!["c.MOV", "d.heic"]],
            "folder by folder"
        );
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

    #[cfg(windows)]
    #[test]
    fn what_belongs_to_the_system_is_left_out_of_folders() {
        let root = crate::testing::fresh_dir("system-files");
        for folder in [
            "$Recycle.Bin",
            "System Volume Information",
            ".Trashes",
            "Trip",
        ] {
            let folder = root.join(folder);
            std::fs::create_dir_all(&folder).unwrap();
            std::fs::write(folder.join("inside.mp4"), b"x").unwrap();
        }
        for name in ["._Clip.mp4", "Clip.mp4", "hidden.mp4", "hidden system.jpg"] {
            std::fs::write(root.join(name), b"x").unwrap();
        }
        for folder in ["Hidden folder", "Customized", "Read-only"] {
            let folder = root.join(folder);
            std::fs::create_dir_all(&folder).unwrap();
            std::fs::write(folder.join("inside.mp4"), b"x").unwrap();
        }
        // A folder given its own icon or look is marked system or read-only, and stays the
        // person's.
        for (flags, path) in [
            (&["+h"][..], root.join("hidden.mp4")),
            (&["+h", "+s"], root.join("hidden system.jpg")),
            (&["+h"], root.join("Hidden folder")),
            (&["+s"], root.join("Customized")),
            (&["+r"], root.join("Read-only")),
        ] {
            let status = std::process::Command::new("attrib")
                .args(flags)
                .arg(&path)
                .status()
                .unwrap();
            assert!(status.success());
        }

        let found: Vec<PathBuf> = gather([root.clone()], None);
        let names: Vec<String> = found
            .iter()
            .map(|path| {
                path.strip_prefix(&root)
                    .unwrap()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();
        // Given by itself, a hidden file is still taken.
        let given = gather([root.join("hidden.mp4")], None);
        for path in [
            root.join("hidden system.jpg"),
            root.join("Customized"),
            root.join("Read-only"),
        ] {
            let _ = std::process::Command::new("attrib")
                .args(["-s", "-h", "-r"])
                .arg(path)
                .status();
        }
        std::fs::remove_dir_all(&root).unwrap();

        assert_eq!(
            names,
            [
                "Clip.mp4",
                r"Customized\inside.mp4",
                r"Read-only\inside.mp4",
                r"Trip\inside.mp4"
            ]
        );
        assert_eq!(given.len(), 1);
    }
}
