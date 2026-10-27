//! Putting a finished compression in its final place: beside the original (spec §40) or in its
//! place (§38–§39).

use std::ffi::OsStr;
use std::fs::{self, File, FileTimes};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};
use std::time::{Duration, Instant, SystemTime};

use crate::leftovers;
use crate::naming::save_as_new_candidate;
use tracing::warn;

/// Moves the validated `temp` file next to `source` as `Gracil-<name>.<extension>`, never
/// overwriting anything. Returns the final path.
pub fn save_as_new(temp: &Path, source: &Path, extension: &OsStr) -> io::Result<PathBuf> {
    let named_like = source.with_extension(extension);
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
            // Drives without hard links (FAT32, exFAT): a move that also never replaces.
            Err(_) => match move_to_free_name(temp, &candidate) {
                Ok(()) => return Ok(candidate),
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error),
            },
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

/// Puts the validated `temp` file in place of `source` (spec §38–§39, §42). Returns the final
/// path.
///
/// The original is never destroyed before the compressed file is in place, and a failure at any
/// step leaves it recoverable: it is either untouched or moved back.
///
/// - Same format: the file is swapped in place and keeps its name and creation date, so it feels
///   like the same video, compressed. Its modified date becomes the moment it was replaced
///   (ADR 0010).
/// - Changed format (`Clip.mov` becomes MP4): `Clip.mp4` is created first, never over an existing
///   file, and only then is `Clip.mov` deleted.
pub fn replace_original(
    temp: &Path,
    source: &Path,
    extension: &OsStr,
    swap: Swap,
) -> io::Result<PathBuf> {
    let target = source.with_extension(extension);
    let times = replaced_times(&fs::metadata(source)?);

    if target == source {
        let backup = temp.with_extension("bak");
        leftovers::record_backup(&backup, source);
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
            move_to_free_name(temp, &target)?;
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

/// Moves `temp` to `target` on a drive without hard links (FAT32, exFAT), failing with
/// `AlreadyExists` instead of replacing a file of that name (ADR 0064). The name is claimed first
/// with an empty file only this call can create; the move then replaces only that claim.
fn move_to_free_name(temp: &Path, target: &Path) -> io::Result<()> {
    File::options().write(true).create_new(true).open(target)?;
    if let Err(error) = fs::rename(temp, target) {
        // The claim is still empty and still ours; it goes, and the name is free again.
        if fs::metadata(target).is_ok_and(|metadata| metadata.len() == 0) {
            let _ = fs::remove_file(target);
        }
        return Err(error);
    }
    Ok(())
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

/// Leftovers another program still had open when they were due to go, still being retried.
static STUBBORN: Mutex<Vec<PathBuf>> = Mutex::new(Vec::new());

/// How long to wait before each new try at a stubborn leftover: about a minute in all.
const RETRY_DELAYS: [Duration; 9] = [
    Duration::from_millis(100),
    Duration::from_millis(250),
    Duration::from_millis(500),
    Duration::from_secs(1),
    Duration::from_secs(2),
    Duration::from_secs(4),
    Duration::from_secs(8),
    Duration::from_secs(15),
    Duration::from_secs(30),
];

/// Removes a file Gracil no longer needs. A moment after a video is replaced, other programs often
/// open the file that just appeared (antivirus, search indexing, File Explorer's thumbnails), so
/// the first try may fail; it is tried again in the background, and whatever is still left then
/// is settled the next time Gracil starts (`leftovers::sweep`).
fn tidy(leftover: &Path) {
    if remove(leftover) {
        return;
    }
    warn!(?leftover, "a leftover file is in use; trying again shortly");
    stubborn().push(leftover.to_owned());
    let leftover = leftover.to_owned();
    std::thread::spawn(move || {
        for delay in RETRY_DELAYS {
            std::thread::sleep(delay);
            if retry(&leftover) {
                return;
            }
        }
        warn!(
            ?leftover,
            "could not remove a leftover file; it is removed at the next start"
        );
    });
}

/// Tries the stubborn leftovers until none is left or `within` has passed. Call before Gracil
/// quits, so it doesn't leave behind a file it was about to remove.
pub fn finish_tidying(within: Duration) {
    let deadline = Instant::now() + within;
    loop {
        let left: Vec<PathBuf> = stubborn().clone();
        if left.is_empty() || Instant::now() >= deadline {
            return;
        }
        for leftover in &left {
            retry(leftover);
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

/// One more try at a stubborn leftover; `true` once it is gone, whoever removed it.
fn retry(leftover: &Path) -> bool {
    let gone = remove(leftover);
    if gone {
        stubborn().retain(|path| path != leftover);
    }
    gone
}

/// Removes `leftover`; `true` if it is gone.
fn remove(leftover: &Path) -> bool {
    match fs::remove_file(leftover) {
        Ok(()) => true,
        Err(error) => error.kind() == io::ErrorKind::NotFound,
    }
}

fn stubborn() -> MutexGuard<'static, Vec<PathBuf>> {
    STUBBORN
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// The original's creation date, so the video keeps its place. The modified date is set to the
/// moment of replacing, by [`set_times`].
fn replaced_times(metadata: &fs::Metadata) -> FileTimes {
    let times = FileTimes::new();
    #[cfg(windows)]
    if let Ok(created) = metadata.created() {
        use std::os::windows::fs::FileTimesExt;
        return times.set_created(created);
    }
    // Only Windows sets a file's creation date; elsewhere, such as the cloud's Linux checks
    // (ADR 0072), the original's is not needed.
    #[cfg(not(windows))]
    let _ = metadata;
    times
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan::output_extension;
    use crate::testing::fresh_dir;

    #[test]
    fn places_copy_beside_source_without_overwriting() {
        let dir = fresh_dir("copy");
        let source = dir.join("Clip.AVI");
        fs::write(&source, b"original").unwrap();
        fs::write(dir.join("Gracil-Clip.mp4"), b"someone else's").unwrap();
        let temp = dir.join("Clip.AVI.gracil-1.part");
        fs::write(&temp, b"compressed").unwrap();

        let placed = save_as_new(&temp, &source, &output_extension(&source)).unwrap();
        let placed_content = fs::read(&placed).unwrap();
        let other = fs::read(dir.join("Gracil-Clip.mp4")).unwrap();
        let temp_left = temp.exists();
        fs::remove_dir_all(&dir).unwrap();

        assert_eq!(placed, dir.join("Gracil-Clip (2).mp4"));
        assert_eq!(placed_content, b"compressed");
        assert_eq!(other, b"someone else's");
        assert!(!temp_left);
    }

    #[test]
    fn moving_without_hard_links_never_replaces_a_file() {
        let dir = fresh_dir("no-links");
        let temp = dir.join("Clip.mp4.gracil-1.part");
        let taken = dir.join("Gracil-Clip.mp4");
        let free = dir.join("Gracil-Clip (2).mp4");
        fs::write(&temp, b"compressed").unwrap();
        fs::write(&taken, b"someone else's").unwrap();

        let onto_taken = move_to_free_name(&temp, &taken);
        let onto_free = move_to_free_name(&temp, &free);
        let contents = (fs::read(&taken).unwrap(), fs::read(&free).unwrap());
        let temp_left = temp.exists();
        fs::remove_dir_all(&dir).unwrap();

        assert_eq!(
            onto_taken.map_err(|error| error.kind()),
            Err(io::ErrorKind::AlreadyExists)
        );
        assert!(onto_free.is_ok());
        assert_eq!(
            contents,
            (b"someone else's".to_vec(), b"compressed".to_vec())
        );
        assert!(!temp_left);
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

    /// The parked original, held open by "another program" after [`swap_then_scanned`].
    #[cfg(windows)]
    static HELD: Mutex<Option<File>> = Mutex::new(None);

    /// Swaps, then opens the parked original without letting anyone delete it, as an antivirus or
    /// File Explorer may do with a file that just appeared.
    #[cfg(windows)]
    fn swap_then_scanned(original: &Path, replacement: &Path, backup: &Path) -> io::Result<()> {
        use std::os::windows::fs::OpenOptionsExt;
        const FILE_SHARE_READ: u32 = 1;
        rename_swap(original, replacement, backup)?;
        let held = File::options()
            .read(true)
            .share_mode(FILE_SHARE_READ)
            .open(backup)?;
        *HELD.lock().unwrap() = Some(held);
        Ok(())
    }

    #[cfg(windows)]
    #[test]
    fn a_parked_original_in_use_is_removed_once_released() {
        let dir = fresh_dir("in-use");
        let source = dir.join("Clip.mp4");
        let temp = dir.join("Clip.mp4.gracil-1.part");
        fs::write(&source, b"original").unwrap();
        fs::write(&temp, b"compressed").unwrap();

        replace_original(
            &temp,
            &source,
            &output_extension(&source),
            swap_then_scanned,
        )
        .unwrap();
        let kept_while_held = temp.with_extension("bak").exists();
        HELD.lock().unwrap().take();
        finish_tidying(Duration::from_secs(5));
        let entries = fs::read_dir(&dir).unwrap().count();
        fs::remove_dir_all(&dir).unwrap();

        assert!(kept_while_held);
        assert_eq!(entries, 1, "the parked original is gone once released");
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
        let placed =
            replace_original(&temp, &source, &output_extension(&source), rename_swap).unwrap();
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

        let result = replace_original(&temp, &source, &output_extension(&source), failing_swap);
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
        let source = dir.join("IMG_0042.MOV");
        let temp = dir.join("IMG_0042.MOV.gracil-1.part");
        fs::write(&source, b"original").unwrap();
        fs::write(&temp, b"compressed").unwrap();

        let placed =
            replace_original(&temp, &source, &output_extension(&source), rename_swap).unwrap();
        let content = fs::read(&placed).unwrap();
        let entries = fs::read_dir(&dir).unwrap().count();
        fs::remove_dir_all(&dir).unwrap();

        assert_eq!(placed, dir.join("IMG_0042.mp4"));
        assert_eq!(content, b"compressed");
        assert_eq!(entries, 1, "only the compressed video remains");
    }

    #[test]
    fn changed_format_never_overwrites_another_file() {
        let dir = fresh_dir("taken");
        let source = dir.join("IMG_0042.MOV");
        let other = dir.join("IMG_0042.mp4");
        let temp = dir.join("IMG_0042.MOV.gracil-1.part");
        fs::write(&source, b"original").unwrap();
        fs::write(&other, b"someone else's").unwrap();
        fs::write(&temp, b"compressed").unwrap();

        let result = replace_original(&temp, &source, &output_extension(&source), rename_swap);
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
