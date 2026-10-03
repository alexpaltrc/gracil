//! Cleaning up temporary files a closed or crashed Gracil left behind (spec §72, ADR 0017).
//!
//! Files are never deleted because their names look like Gracil's. Instead, each running Gracil
//! writes down every temporary file it creates in a ledger of its own, which it keeps open and
//! locked while it runs. At startup, [`sweep`] reads the ledgers nobody holds any more (their
//! Gracil has exited) and deals with the files they list:
//!
//! - an unfinished or unchosen compression (`.part`) is deleted;
//! - an original parked during a replacement (`.bak`) is put back if its place is empty, and
//!   deleted only once the compressed video is in place.
//!
//! Anything that cannot be settled now (the drive is not connected, the file is in use) stays in
//! the ledger for the next start.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Seek, Write};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use tracing::warn;

/// This process's ledger, once [`track`] has opened it.
static CURRENT: OnceLock<Ledger> = OnceLock::new();

/// Starts writing down this process's temporary files in `private_dir`. Call once, before any
/// compression. Without it, compressing still works; leftovers just aren't cleaned up later.
pub fn track(private_dir: &Path) -> io::Result<()> {
    let ledger = Ledger::open(&ledgers_dir(private_dir))?;
    // A second call keeps the first ledger; the new one, empty, is removed when dropped.
    let _ = CURRENT.set(ledger);
    Ok(())
}

/// Notes a temporary compression file this process just created.
pub(crate) fn record_temp(temp: &Path) {
    if let Some(ledger) = CURRENT.get() {
        ledger.write(Entry::Temp(temp.to_owned()));
    }
}

/// Notes that `original` is about to be parked at `backup` while it is replaced.
pub(crate) fn record_backup(backup: &Path, original: &Path) {
    if let Some(ledger) = CURRENT.get() {
        ledger.write(Entry::Backup {
            backup: backup.to_owned(),
            original: original.to_owned(),
        });
    }
}

/// What a [`sweep`] did.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Swept {
    /// Temporary files and parked originals deleted.
    pub removed: usize,
    /// Originals put back in their place.
    pub restored: usize,
    /// Entries left for a later start.
    pub kept: usize,
}

/// Settles the temporary files listed by ledgers whose Gracil is no longer running.
pub fn sweep(private_dir: &Path) -> Swept {
    let mut swept = Swept::default();
    let Ok(ledgers) = fs::read_dir(ledgers_dir(private_dir)) else {
        return swept;
    };
    for ledger in ledgers.flatten() {
        // A ledger that can't be opened is held by a running Gracil; it is not ours to settle.
        if let Ok(file) = open_exclusive(&ledger.path(), false) {
            settle(file, &ledger.path(), &mut swept);
        }
    }
    swept
}

fn ledgers_dir(private_dir: &Path) -> PathBuf {
    private_dir.join("temporary")
}

#[derive(Debug, Clone, PartialEq)]
enum Entry {
    Temp(PathBuf),
    Backup { backup: PathBuf, original: PathBuf },
}

// One entry per line, fields separated by tabs: Windows file names can contain neither.
impl Entry {
    fn to_line(&self) -> Option<String> {
        Some(match self {
            Self::Temp(temp) => format!("part\t{}\n", temp.to_str()?),
            Self::Backup { backup, original } => {
                format!("bak\t{}\t{}\n", backup.to_str()?, original.to_str()?)
            }
        })
    }

    fn parse(line: &str) -> Option<Self> {
        let mut fields = line.split('\t');
        let entry = match (fields.next()?, fields.next()?, fields.next()) {
            ("part", temp, None) => Self::Temp(temp.into()),
            ("bak", backup, Some(original)) => Self::Backup {
                backup: backup.into(),
                original: original.into(),
            },
            _ => return None,
        };
        fields.next().is_none().then_some(entry)
    }

    /// The file this entry is about. Only files named the way Gracil names them are ever
    /// touched, even if a ledger was damaged.
    fn file(&self) -> Option<&Path> {
        let (path, extension) = match self {
            Self::Temp(temp) => (temp, "part"),
            Self::Backup { backup, .. } => (backup, "bak"),
        };
        let name = path.file_name()?.to_str()?;
        let gracil = name.contains(".gracil-") && name.ends_with(&format!(".{extension}"));
        (gracil && path.is_absolute()).then_some(path)
    }

    /// Settles this entry. Returns `false` when it has to wait for a later start.
    fn settle(&self, swept: &mut Swept) -> bool {
        let Some(file) = self.file() else {
            return true;
        };
        if !drive_present(file) {
            return false;
        }
        match file.try_exists() {
            Ok(false) => return true,
            Ok(true) => {}
            Err(_) => return false,
        }
        let done = match self {
            Self::Backup { backup, original } => match original.try_exists() {
                // The replacement stopped halfway: the original goes back.
                Ok(false) => fs::rename(backup, original).map(|()| swept.restored += 1),
                // The compressed video is in place; the parked original was due to be removed.
                Ok(true) => fs::remove_file(backup).map(|()| swept.removed += 1),
                Err(_) => return false,
            },
            Self::Temp(temp) => fs::remove_file(temp).map(|()| swept.removed += 1),
        };
        done.inspect_err(|error| warn!(%error, ?file, "could not settle a leftover file"))
            .is_ok()
    }
}

/// Whether the drive or share `path` is on is reachable. A missing file on a drive that is not
/// connected is not gone, just away.
fn drive_present(path: &Path) -> bool {
    path.ancestors()
        .last()
        .is_some_and(|root| root.try_exists().unwrap_or(false))
}

fn settle(mut file: File, path: &Path, swept: &mut Swept) {
    let mut bytes = Vec::new();
    if let Err(error) = file.read_to_end(&mut bytes) {
        warn!(%error, ?path, "could not read a leftover ledger");
        return;
    }
    let text = String::from_utf8_lossy(&bytes);
    // A line without its end was being written when Gracil stopped; it is incomplete.
    let kept: Vec<&str> = text
        .split_inclusive('\n')
        .filter(|line| line.ends_with('\n'))
        .filter(|line| match Entry::parse(line.trim_end_matches('\n')) {
            Some(entry) => !entry.settle(swept),
            None => false,
        })
        .collect();
    swept.kept += kept.len();

    let rewritten = if kept.is_empty() {
        drop(file);
        fs::remove_file(path)
    } else {
        file.set_len(0)
            .and_then(|()| file.rewind())
            .and_then(|()| file.write_all(kept.concat().as_bytes()))
    };
    if let Err(error) = rewritten {
        warn!(%error, ?path, "could not update a leftover ledger");
    }
}

/// A list of the temporary files one Gracil process created.
#[derive(Debug)]
struct Ledger {
    file: Mutex<File>,
}

impl Ledger {
    fn open(dir: &Path) -> io::Result<Self> {
        fs::create_dir_all(dir)?;
        let started = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let path = dir.join(format!("{}-{started:x}.txt", std::process::id()));
        Ok(Self {
            file: Mutex::new(open_exclusive(&path, true)?),
        })
    }

    fn write(&self, entry: Entry) {
        let Some(line) = entry.to_line() else {
            warn!(?entry, "a temporary file's path can't be written down");
            return;
        };
        let mut file = self.file.lock().unwrap_or_else(|p| p.into_inner());
        if let Err(error) = file
            .write_all(line.as_bytes())
            .and_then(|()| file.sync_data())
        {
            warn!(%error, "could not write down a temporary file");
        }
    }
}

/// Opens a ledger so that no other process can open it while it is held.
fn open_exclusive(path: &Path, create: bool) -> io::Result<File> {
    let mut options = OpenOptions::new();
    options.read(true).write(true);
    if create {
        options.create_new(true);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options.share_mode(0);
    }
    options.open(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fresh_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "gracil-leftovers-{name}-{}-{:x}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn ledger(private: &Path) -> Ledger {
        Ledger::open(&ledgers_dir(private)).unwrap()
    }

    fn ledger_count(private: &Path) -> usize {
        fs::read_dir(ledgers_dir(private)).map_or(0, |d| d.count())
    }

    #[test]
    fn leftovers_of_a_closed_gracil_are_removed() {
        let dir = fresh_dir("closed");
        let temp = dir.join("Vacación 🎂.mp4.gracil-1a.part");
        let placed = dir.join("Clip.mp4.gracil-1b.part");
        fs::write(&temp, b"half a video").unwrap();
        let ledger = ledger(&dir);
        ledger.write(Entry::Temp(temp.clone()));
        ledger.write(Entry::Temp(placed));
        drop(ledger);

        let swept = sweep(&dir);
        let ledgers = ledger_count(&dir);
        let temp_left = temp.exists();
        fs::remove_dir_all(&dir).unwrap();

        assert_eq!(
            swept,
            Swept {
                removed: 1,
                restored: 0,
                kept: 0
            }
        );
        assert!(!temp_left);
        assert_eq!(ledgers, 0, "a settled ledger is removed");
    }

    #[test]
    fn a_running_gracil_is_left_alone() {
        let dir = fresh_dir("running");
        let temp = dir.join("Clip.mp4.gracil-2a.part");
        fs::write(&temp, b"being written").unwrap();
        let ledger = ledger(&dir);
        ledger.write(Entry::Temp(temp.clone()));

        let swept = sweep(&dir);
        let temp_left = temp.exists();
        drop(ledger);
        fs::remove_dir_all(&dir).unwrap();

        assert_eq!(swept, Swept::default());
        assert!(temp_left);
    }

    #[test]
    fn a_parked_original_goes_back_when_its_place_is_empty() {
        let dir = fresh_dir("restore");
        let original = dir.join("Clip.mp4");
        let backup = dir.join("Clip.mp4.gracil-3a.bak");
        fs::write(&backup, b"the original").unwrap();
        let ledger = ledger(&dir);
        ledger.write(Entry::Backup {
            backup: backup.clone(),
            original: original.clone(),
        });
        drop(ledger);

        let swept = sweep(&dir);
        let restored = fs::read(&original).unwrap();
        let backup_left = backup.exists();
        fs::remove_dir_all(&dir).unwrap();

        assert_eq!(swept.restored, 1);
        assert_eq!(restored, b"the original");
        assert!(!backup_left);
    }

    #[test]
    fn a_parked_original_is_removed_once_the_compressed_video_is_in_place() {
        let dir = fresh_dir("replaced");
        let original = dir.join("Clip.mp4");
        let backup = dir.join("Clip.mp4.gracil-4a.bak");
        fs::write(&original, b"compressed").unwrap();
        fs::write(&backup, b"the original").unwrap();
        let ledger = ledger(&dir);
        ledger.write(Entry::Backup {
            backup: backup.clone(),
            original: original.clone(),
        });
        drop(ledger);

        let swept = sweep(&dir);
        let kept = fs::read(&original).unwrap();
        let backup_left = backup.exists();
        fs::remove_dir_all(&dir).unwrap();

        assert_eq!(swept.removed, 1);
        assert_eq!(kept, b"compressed");
        assert!(!backup_left);
    }

    #[test]
    fn only_files_named_by_gracil_are_touched() {
        let dir = fresh_dir("names");
        let user_file = dir.join("Clip.mp4");
        fs::write(&user_file, b"mine").unwrap();
        let ledgers = ledgers_dir(&dir);
        fs::create_dir_all(&ledgers).unwrap();
        let line = format!(
            "part\t{}\nbak\t{}\t{}\n",
            user_file.display(),
            user_file.display(),
            dir.join("x").display()
        );
        fs::write(ledgers.join("1-1.txt"), line).unwrap();

        sweep(&dir);
        let user_file_left = user_file.exists();
        fs::remove_dir_all(&dir).unwrap();

        assert!(user_file_left);
    }

    #[test]
    fn an_unfinished_line_is_ignored() {
        let dir = fresh_dir("cut");
        let temp = dir.join("Clip.mp4.gracil-5a.part");
        fs::write(&temp, b"x").unwrap();
        let ledgers = ledgers_dir(&dir);
        fs::create_dir_all(&ledgers).unwrap();
        // Cut short: the path ends early, and could name another file.
        fs::write(ledgers.join("1-1.txt"), format!("part\t{}", temp.display())).unwrap();

        sweep(&dir);
        let temp_left = temp.exists();
        fs::remove_dir_all(&dir).unwrap();

        assert!(temp_left);
    }

    #[test]
    fn files_on_a_drive_that_is_away_wait_for_later() {
        let dir = fresh_dir("away");
        let away = PathBuf::from(
            r"\\?\Volume{00000000-0000-0000-0000-000000000000}\Clip.mp4.gracil-6a.part",
        );
        let ledger = ledger(&dir);
        ledger.write(Entry::Temp(away));
        drop(ledger);

        let swept = sweep(&dir);
        let ledgers = ledger_count(&dir);
        fs::remove_dir_all(&dir).unwrap();

        assert_eq!(swept.kept, 1);
        assert_eq!(ledgers, 1, "the ledger stays for the next start");
    }

    #[test]
    fn entries_read_back_as_written() {
        let entries = [
            Entry::Temp(r"C:\日本語\動画.mp4.gracil-1.part".into()),
            Entry::Backup {
                backup: r"C:\Videos\José.mp4.gracil-1.bak".into(),
                original: r"C:\Videos\José.mp4".into(),
            },
        ];
        for entry in entries {
            let line = entry.to_line().unwrap();
            assert_eq!(Entry::parse(line.trim_end_matches('\n')), Some(entry));
        }
    }
}
