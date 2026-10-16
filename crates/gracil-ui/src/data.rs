//! Gracil's own small text files, kept in its data folder (`data\` beside `gracil.exe`): the
//! settings, the files left unfinished and the levels the last videos needed. Like the logs, they
//! never leave the computer.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::work::Choice;

/// Where the data file `name` is kept, if Gracil has a data folder.
pub fn file(name: &str) -> Option<PathBuf> {
    gracil_core::toolchain::default_private_dir().map(|dir| dir.join(name))
}

/// Writes `text` to `path` whole or not at all: to a file beside it first, which then takes its
/// place, so Gracil closing halfway never leaves half a file.
pub fn write_whole(path: &Path, text: &str) -> io::Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let mut fresh = path.as_os_str().to_owned();
    fresh.push(".new");
    fs::write(&fresh, text)?;
    fs::rename(&fresh, path)
}

/// How a choice is written in the data files.
pub fn choice_word(choice: Choice) -> &'static str {
    match choice {
        Choice::Replace => "replace",
        Choice::SaveCopy => "copy",
    }
}

/// The choice written as `word`, if it is one.
pub fn choice_from(word: &str) -> Option<Choice> {
    match word {
        "replace" => Some(Choice::Replace),
        "copy" => Some(Choice::SaveCopy),
        _ => None,
    }
}
