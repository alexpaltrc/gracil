//! Gracil's settings (spec §66, ADR 0025, ADR 0050, ADR 0054): whether Gracil is in File
//! Explorer's menu, whether it starts with Windows, whether the mini-window appears when
//! compressing from File Explorer, and whether finished videos are always replaced or always
//! saved as copies instead of asking. They are kept in Gracil's data
//! folder (`data\settings.txt` beside `gracil.exe`) as `name=value` lines, so a later setting
//! only adds a line.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::data;
use crate::place::Wanted;
use crate::work::Choice;

const FILE: &str = "settings.txt";
const EXPLORER_MINI_WINDOW: &str = "explorer_mini_window";
const ALWAYS: &str = "always";
const EXPLORER_MENU: &str = "explorer_menu";
const START_WITH_WINDOWS: &str = "start_with_windows";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Settings {
    /// Gracil in File Explorer's menu (ADR 0050). On unless turned off.
    pub explorer_menu: bool,
    /// Gracil starts with Windows (ADR 0054). On unless turned off.
    pub start_with_windows: bool,
    /// Show the mini-window when compressing from File Explorer (spec §56). On unless turned off.
    pub explorer_mini_window: bool,
    /// What to do with every finished video without asking (ADR 0025). `None`, the default,
    /// asks after each one.
    pub always: Option<Choice>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            explorer_menu: true,
            start_with_windows: true,
            explorer_mini_window: true,
            always: None,
        }
    }
}

impl Settings {
    /// The saved settings. Anything missing or unreadable keeps its default, so a damaged file
    /// never stops Gracil.
    pub fn load() -> Self {
        file().map_or_else(Self::default, |path| Self::read(&path))
    }

    /// Gracil's place in Windows, as these settings want it.
    pub fn place(&self) -> Wanted {
        Wanted {
            in_explorer: self.explorer_menu,
            at_sign_in: self.start_with_windows,
        }
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
                Some((ALWAYS, "ask")) => settings.always = None,
                Some((ALWAYS, word)) if data::choice_from(word).is_some() => {
                    settings.always = data::choice_from(word);
                }
                Some((EXPLORER_MENU, "on")) => settings.explorer_menu = true,
                Some((EXPLORER_MENU, "off")) => settings.explorer_menu = false,
                Some((START_WITH_WINDOWS, "on")) => settings.start_with_windows = true,
                Some((START_WITH_WINDOWS, "off")) => settings.start_with_windows = false,
                _ => {}
            }
        }
        settings
    }

    /// Writes the settings whole or not at all, so closing halfway never loses them.
    fn write(&self, path: &Path) -> io::Result<()> {
        let switch = |on: bool| if on { "on" } else { "off" };
        let always = self.always.map_or("ask", data::choice_word);
        data::write_whole(
            path,
            &format!(
                "{EXPLORER_MENU}={}\n{START_WITH_WINDOWS}={}\n{EXPLORER_MINI_WINDOW}={}\n\
                 {ALWAYS}={always}\n",
                switch(self.explorer_menu),
                switch(self.start_with_windows),
                switch(self.explorer_mini_window),
            ),
        )
    }
}

fn file() -> Option<PathBuf> {
    data::file(FILE)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use gracil_core::testing::fresh_dir;

    use super::*;

    #[test]
    fn defaults_add_the_menu_show_the_mini_window_and_ask() {
        let dir = fresh_dir("default");
        let settings = Settings::read(&dir.join(FILE));
        assert!(settings.explorer_menu);
        assert!(settings.start_with_windows);
        assert!(settings.explorer_mini_window);
        assert_eq!(settings.always, None);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn saved_settings_come_back() {
        let dir = fresh_dir("saved");
        let path = dir.join("nested").join(FILE);
        for always in [Some(Choice::Replace), Some(Choice::SaveCopy), None] {
            let saved = Settings {
                explorer_menu: always.is_some(),
                start_with_windows: always.is_none(),
                explorer_mini_window: false,
                always,
            };
            saved.write(&path).unwrap();
            assert_eq!(Settings::read(&path), saved);
        }
        Settings::default().write(&path).unwrap();
        assert_eq!(Settings::read(&path), Settings::default());
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn unknown_lines_are_ignored() {
        let dir = fresh_dir("unknown");
        let path = dir.join(FILE);
        fs::write(
            &path,
            "something=else\nexplorer_mini_window = off\nalways=sometimes\ngarbage",
        )
        .unwrap();
        let settings = Settings::read(&path);
        assert!(!settings.explorer_mini_window);
        assert_eq!(settings.always, None);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_settings_file_from_before_always_still_reads() {
        let dir = fresh_dir("older");
        let path = dir.join(FILE);
        fs::write(&path, "explorer_mini_window=on\n").unwrap();
        assert_eq!(Settings::read(&path), Settings::default());
        fs::remove_dir_all(dir).unwrap();
    }
}
