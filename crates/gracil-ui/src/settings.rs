//! Gracil's one setting (spec §66): whether the mini-window appears when compressing from File
//! Explorer. It is kept in `%LOCALAPPDATA%\Gracil\settings.txt` as `name=value` lines, so a later
//! setting only adds a line.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

const FILE: &str = "settings.txt";
const EXPLORER_MINI_WINDOW: &str = "explorer_mini_window";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Settings {
    /// Show the mini-window when compressing from File Explorer (spec §56). On unless turned off.
    pub explorer_mini_window: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            explorer_mini_window: true,
        }
    }
}

impl Settings {
    /// The saved settings. Anything missing or unreadable keeps its default, so a damaged file
    /// never stops Gracil.
    pub fn load() -> Self {
        file().map_or_else(Self::default, |path| Self::read(&path))
    }

    /// Keeps the settings for next time.
    pub fn save(&self) -> io::Result<()> {
        let path = file().ok_or_else(|| io::Error::other("no folder for Gracil's settings"))?;
        self.write(&path)
    }

    fn read(path: &Path) -> Self {
        let text = fs::read_to_string(path).unwrap_or_default();
        let mut settings = Self::default();
        for line in text.lines() {
            match line
                .split_once('=')
                .map(|(name, value)| (name.trim(), value.trim()))
            {
                Some((EXPLORER_MINI_WINDOW, "on")) => settings.explorer_mini_window = true,
                Some((EXPLORER_MINI_WINDOW, "off")) => settings.explorer_mini_window = false,
                _ => {}
            }
        }
        settings
    }

    fn write(&self, path: &Path) -> io::Result<()> {
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)?;
        }
        let value = if self.explorer_mini_window {
            "on"
        } else {
            "off"
        };
        fs::write(path, format!("{EXPLORER_MINI_WINDOW}={value}\n"))
    }
}

fn file() -> Option<PathBuf> {
    gracil_core::toolchain::default_private_dir().map(|dir| dir.join(FILE))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    fn fresh_dir(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("gracil-settings-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn the_mini_window_is_on_until_turned_off() {
        let dir = fresh_dir("default");
        assert!(Settings::read(&dir.join(FILE)).explorer_mini_window);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_saved_setting_comes_back() {
        let dir = fresh_dir("saved");
        let path = dir.join("nested").join(FILE);
        let off = Settings {
            explorer_mini_window: false,
        };
        off.write(&path).unwrap();
        assert_eq!(Settings::read(&path), off);
        Settings::default().write(&path).unwrap();
        assert_eq!(Settings::read(&path), Settings::default());
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn unknown_lines_are_ignored() {
        let dir = fresh_dir("unknown");
        let path = dir.join(FILE);
        fs::write(&path, "something=else\nexplorer_mini_window = off\ngarbage").unwrap();
        assert!(!Settings::read(&path).explorer_mini_window);
        fs::remove_dir_all(dir).unwrap();
    }
}
