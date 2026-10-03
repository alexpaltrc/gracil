//! Putting a finished compression in its final place: beside the original (spec §40) or in its
//! place (§38–§39).

use std::fs::{self, File, FileTimes};
use std::io;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use crate::naming::save_as_new_candidate;
use crate::plan::Container;
use tracing::warn;

/// Moves the validated `temp` file next to `source` as `Gracil-<name>`, never overwriting
/// anything. Returns the final path.
pub fn save_as_new(temp: &Path, source: &Path) -> io::Result<PathBuf> {
    let named_like = source.with_extension(Container::output_extension(source));
    for attempt in 1..=u32::MAX {
        let candidate = save_as_new_candidate(&named_like, attempt);
        if candidate.try_exists()? {
            continue;
        }
        // A hard link fails if the name was taken in the meantime, so nothing is ever replaced.
        match fs::hard_link(temp, &candidate) {
            Ok(()) => {
                fs::remove_file(temp)?;
                return Ok(candidate);
            }
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            // Drives without hard links (FAT32, exFAT): rename, having just checked the name.
            Err(_) => {
                fs::rename(temp, &candidate)?;
                return Ok(candidate);
            }
        }
    }
    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "no free name for the compressed copy",
    ))
}

/// Swaps one file for another in a single step, keeping the original's identity (creation date,
/// permissions, attributes), and parks the original at `backup` while doing so. Provided by the
/// Windows layer; a parameter here so the rules below stay testable.
pub type Swap = fn(original: &Path, replacement: &Path, backup: &Path) -> io::Result<()>;

/// Puts the validated `temp` file in place of `source` (spec §38–§39, §42). Returns the final path.
///
/// The original is never destroyed before the compressed file is in place, and a failure at any
/// step leaves it recoverable: it is either untouched or moved back.
///
/// - Same format: the file is swapped in place and keeps its name and creation date, so it feels
///   like the same video, compressed. Its modified date becomes the moment it was replaced (ADR 0010).
/// - Changed format (`Clip.avi` becomes MP4): `Clip.mp4` is created first, never over an existing
///   file, and only then is `Clip.avi` deleted.
pub fn replace_original(temp: &Path, source: &Path, swap: Swap) -> io::Result<PathBuf> {
    let target = source.with_extension(Container::output_extension(source));
    let times = replaced_times(&fs::metadata(source)?);

    if target == source {
        let backup = temp.with_extension("bak");
        if let Err(error) = swap(source, temp, &backup) {
            if !source.exists() && backup.exists() {
                fs::rename(&backup, source)?;
            }
            return Err(error);
        }
        set_times(source, times);
        tidy(&backup);
        return Ok(target);
    }

    if target.try_exists()? {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "a file with the new name already exists",
        ));
    }
    // A hard link keeps `temp` until the original is gone; drives without hard links rename.
    let linked = match fs::hard_link(temp, &target) {
        Ok(()) => true,
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => return Err(error),
        Err(_) => {
            fs::rename(temp, &target)?;
            false
        }
    };
    if let Err(error) = fs::remove_file(source) {
        // The original stays; undo so the result is still waiting to be placed.
        if linked {
            fs::remove_file(&target)?;
        } else {
            fs::rename(&target, temp)?;
        }
        return Err(error);
    }
    if linked {
        tidy(temp);
    }
    set_times(&target, times);
    Ok(target)
}

// Once the compressed video is in place the replacement has succeeded; the steps below only
// polish it, so their failures are logged rather than reported.

fn set_times(path: &Path, times: FileTimes) {
    let restored = File::options()
        .write(true)
        .open(path)
        .and_then(|file| file.set_times(times.set_modified(SystemTime::now())));
    if let Err(error) = restored {
        warn!(%error, "could not set the dates of the replaced video");
    }
}

fn tidy(leftover: &Path) {
    if let Err(error) = fs::remove_file(leftover) {
        warn!(%error, ?leftover, "could not remove a leftover file");
    }
}

/// The original's creation date, so the video keeps its place. The modified date is set to the
/// moment of replacing, by [`set_times`].
fn replaced_times(metadata: &fs::Metadata) -> FileTimes {
    let mut times = FileTimes::new();
    #[cfg(windows)]
    if let Ok(created) = metadata.created() {
        use std::os::windows::fs::FileTimesExt;
        times = times.set_created(created);
    }
    times
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn places_copy_beside_source_without_overwriting() {
        let dir = fresh_dir("copy");
        let source = dir.join("Clip.AVI");
        fs::write(&source, b"original").unwrap();
        fs::write(dir.join("Gracil-Clip.mp4"), b"someone else's").unwrap();
        let temp = dir.join("Clip.AVI.gracil-1.part");
        fs::write(&temp, b"compressed").unwrap();

        let placed = save_as_new(&temp, &source).unwrap();
        let placed_content = fs::read(&placed).unwrap();
        let other = fs::read(dir.join("Gracil-Clip.mp4")).unwrap();
        let temp_left = temp.exists();
        fs::remove_dir_all(&dir).unwrap();

        assert_eq!(placed, dir.join("Gracil-Clip (2).mp4"));
        assert_eq!(placed_content, b"compressed");
        assert_eq!(other, b"someone else's");
        assert!(!temp_left);
    }

    fn fresh_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("gracil-output-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// Stands in for the Windows swap: parks the original at `backup`, moves the replacement in.
    fn rename_swap(original: &Path, replacement: &Path, backup: &Path) -> io::Result<()> {
        fs::rename(original, backup)?;
        fs::rename(replacement, original)
    }

    /// Fails after parking the original, the worst moment for a swap to fail.
    fn failing_swap(original: &Path, _replacement: &Path, backup: &Path) -> io::Result<()> {
        fs::rename(original, backup)?;
        Err(io::Error::other("power cut"))
    }

    fn an_hour_ago() -> std::time::SystemTime {
        std::time::SystemTime::now() - std::time::Duration::from_secs(3600)
    }

    #[test]
    fn replacing_keeps_the_name_and_records_when_it_changed() {
        let dir = fresh_dir("same");
        let source = dir.join("Vacación 🎂.MP4");
        let temp = dir.join("Vacación 🎂.MP4.gracil-1.part");
        fs::write(&source, b"original").unwrap();
        let recorded = an_hour_ago();
        File::options()
            .write(true)
            .open(&source)
            .unwrap()
            .set_modified(recorded)
            .unwrap();
        fs::write(&temp, b"compressed").unwrap();

        let replacing = std::time::SystemTime::now();
        let placed = replace_original(&temp, &source, rename_swap).unwrap();
        let content = fs::read(&placed).unwrap();
        let modified = fs::metadata(&placed).unwrap().modified().unwrap();
        let entries = fs::read_dir(&dir).unwrap().count();
        fs::remove_dir_all(&dir).unwrap();

        assert_eq!(placed, source);
        assert_eq!(content, b"compressed");
        assert!(
            modified >= replacing,
            "the modified date is when it was replaced"
        );
        assert_ne!(modified, recorded);
        assert_eq!(entries, 1, "no temporary or backup file is left");
    }

    #[test]
    fn failed_swap_puts_the_original_back() {
        let dir = fresh_dir("failed");
        let source = dir.join("Clip.mp4");
        let temp = dir.join("Clip.mp4.gracil-1.part");
        fs::write(&source, b"original").unwrap();
        fs::write(&temp, b"compressed").unwrap();

        let result = replace_original(&temp, &source, failing_swap);
        let original = fs::read(&source).unwrap();
        let temp_kept = temp.exists();
        fs::remove_dir_all(&dir).unwrap();

        assert!(result.is_err());
        assert_eq!(original, b"original");
        assert!(temp_kept, "the result can still be saved another way");
    }

    #[test]
    fn changed_format_creates_new_name_then_removes_original() {
        let dir = fresh_dir("format");
        let source = dir.join("Clip.avi");
        let temp = dir.join("Clip.avi.gracil-1.part");
        fs::write(&source, b"original").unwrap();
        fs::write(&temp, b"compressed").unwrap();

        let placed = replace_original(&temp, &source, rename_swap).unwrap();
        let content = fs::read(&placed).unwrap();
        let entries = fs::read_dir(&dir).unwrap().count();
        fs::remove_dir_all(&dir).unwrap();

        assert_eq!(placed, dir.join("Clip.mp4"));
        assert_eq!(content, b"compressed");
        assert_eq!(entries, 1, "only the compressed video remains");
    }

    #[test]
    fn changed_format_never_overwrites_another_file() {
        let dir = fresh_dir("taken");
        let source = dir.join("Clip.avi");
        let other = dir.join("Clip.mp4");
        let temp = dir.join("Clip.avi.gracil-1.part");
        fs::write(&source, b"original").unwrap();
        fs::write(&other, b"someone else's").unwrap();
        fs::write(&temp, b"compressed").unwrap();

        let result = replace_original(&temp, &source, rename_swap);
        let contents = (
            fs::read(&source).unwrap(),
            fs::read(&other).unwrap(),
            fs::read(&temp).unwrap(),
        );
        fs::remove_dir_all(&dir).unwrap();

        assert!(result.is_err());
        assert_eq!(
            contents,
            (
                b"original".to_vec(),
                b"someone else's".to_vec(),
                b"compressed".to_vec()
            )
        );
    }
}
