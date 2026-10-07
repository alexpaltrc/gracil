//! The files left unfinished, kept so that after Gracil closes in the middle of its work (Exit
//! while working, a crash, the computer shutting down) it can offer to carry on with them the next
//! time it opens (ADR 0042). They are kept in `unfinished.txt` in Gracil's data folder, one per
//! line, with the choice File Explorer made for it, if any, before a tab: `replace`, `copy` or
//! nothing. Like the logs, the list never leaves the computer.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Sender};
use std::sync::{Mutex, OnceLock};

use tracing::warn;

use crate::data;
use crate::work::Choice;

const FILE: &str = "unfinished.txt";

/// A file to carry on with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unfinished {
    pub path: PathBuf,
    /// What File Explorer asked to do with the result, if it asked.
    pub choice: Option<Choice>,
}

/// The files left unfinished last time. A missing or damaged file means there are none.
pub fn load() -> Vec<Unfinished> {
    file().map_or_else(Vec::new, |path| read(&path))
}

/// Keeps `list` for next time, away from the window's thread. When several lists are waiting,
/// only the newest is written.
pub fn save(list: Vec<Unfinished>) {
    let generation = GENERATION.fetch_add(1, Ordering::Relaxed) + 1;
    let writer = WRITER.get_or_init(|| {
        let (sender, receiver) = mpsc::channel::<Numbered>();
        std::thread::spawn(move || {
            while let Ok(mut latest) = receiver.recv() {
                while let Ok(newer) = receiver.try_recv() {
                    latest = newer;
                }
                keep(latest.0, &latest.1);
            }
        });
        Mutex::new(sender)
    });
    let sent = writer
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .send((generation, list));
    if sent.is_err() {
        warn!("the list of unfinished files couldn't be kept");
    }
}

/// Keeps `list` for next time before returning, for when Gracil is about to end.
pub fn save_now(list: &[Unfinished]) {
    let generation = GENERATION.fetch_add(1, Ordering::Relaxed) + 1;
    keep(generation, list);
}

/// Numbers each list handed over, so an older one never overwrites a newer one.
static GENERATION: AtomicU64 = AtomicU64::new(0);
/// The number of the list last written.
static WRITTEN: Mutex<u64> = Mutex::new(0);
/// A list to keep, with its number.
type Numbered = (u64, Vec<Unfinished>);
static WRITER: OnceLock<Mutex<Sender<Numbered>>> = OnceLock::new();

fn keep(generation: u64, list: &[Unfinished]) {
    let mut written = WRITTEN
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if generation <= *written {
        return;
    }
    *written = generation;
    let Some(path) = file() else {
        return;
    };
    if let Err(error) = write(&path, list) {
        warn!(%error, "the list of unfinished files couldn't be kept");
    }
}

fn read(path: &Path) -> Vec<Unfinished> {
    let text = fs::read_to_string(path).unwrap_or_default();
    text.lines()
        .filter_map(|line| {
            let (choice, path) = line.split_once('\t')?;
            (!path.is_empty()).then(|| Unfinished {
                path: PathBuf::from(path),
                choice: data::choice_from(choice),
            })
        })
        .collect()
}

/// Writes the list whole or not at all. An empty list leaves no file.
fn write(path: &Path, list: &[Unfinished]) -> io::Result<()> {
    if list.is_empty() {
        return match fs::remove_file(path) {
            Err(error) if error.kind() != io::ErrorKind::NotFound => Err(error),
            _ => Ok(()),
        };
    }
    let mut text = String::new();
    for file in list {
        text.push_str(file.choice.map_or("", data::choice_word));
        text.push('\t');
        text.push_str(&file.path.to_string_lossy());
        text.push('\n');
    }
    data::write_whole(path, &text)
}

fn file() -> Option<PathBuf> {
    data::file(FILE)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    fn fresh_dir(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("gracil-unfinished-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn the_list_comes_back_with_its_choices() {
        let dir = fresh_dir("back");
        let path = dir.join("nested").join(FILE);
        let list = vec![
            Unfinished {
                path: PathBuf::from(r"C:\Videos\a b.mp4"),
                choice: Some(Choice::Replace),
            },
            Unfinished {
                path: PathBuf::from(r"D:\Fotos\ñandú.jpg"),
                choice: None,
            },
            Unfinished {
                path: PathBuf::from(r"\\server\share\c.mov"),
                choice: Some(Choice::SaveCopy),
            },
        ];
        write(&path, &list).unwrap();
        assert_eq!(read(&path), list);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn nothing_left_leaves_no_file() {
        let dir = fresh_dir("empty");
        let path = dir.join(FILE);
        let one = [Unfinished {
            path: PathBuf::from(r"C:\a.mp4"),
            choice: None,
        }];
        write(&path, &one).unwrap();
        write(&path, &[]).unwrap();
        assert!(!path.exists());
        write(&path, &[]).unwrap();
        assert!(read(&path).is_empty());
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn damaged_lines_are_skipped() {
        let dir = fresh_dir("damaged");
        let path = dir.join(FILE);
        fs::write(&path, "garbage\n\t\nsometimes\tC:\\a.mp4\n").unwrap();
        let list = read(&path);
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].choice, None);
        fs::remove_dir_all(dir).unwrap();
    }
}
