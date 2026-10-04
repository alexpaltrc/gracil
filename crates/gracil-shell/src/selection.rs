//! The menu's words and the plain decisions behind them, kept apart from COM so they can be tested
//! and changed easily.

use std::ffi::OsString;

/// The menu's title for `count` selected videos (spec §58). A single video shows plain "Gracil";
/// change it here to show "Gracil (1)" instead.
pub fn title(count: usize) -> String {
    match count {
        0 | 1 => "Gracil".to_owned(),
        n => format!("Gracil ({n})"),
    }
}

/// What happens to each finished file: the **Always** setting of Gracil's window (ADR 0025,
/// ADR 0035). The menu follows it, so it never offers something the window wouldn't do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Choice {
    Replace,
    SaveCopy,
    /// The circle in the middle: Gracil opens its window and asks after each file.
    Ask,
}

impl Choice {
    /// The setting as Gracil saved it. Anything missing or unreadable asks, which never touches
    /// an original.
    pub fn saved() -> Self {
        gracil_core::toolchain::default_private_dir()
            .and_then(|dir| std::fs::read_to_string(dir.join("settings.txt")).ok())
            .map_or(Self::Ask, |text| Self::from_settings(&text))
    }

    /// Reads the `always=` line of Gracil's `settings.txt` (see `gracil-ui`'s `settings.rs`).
    pub fn from_settings(text: &str) -> Self {
        let value = text.lines().find_map(|line| {
            let (name, value) = line.split_once('=')?;
            (name.trim() == "always").then(|| value.trim())
        });
        match value {
            Some("replace") => Self::Replace,
            Some("copy") => Self::SaveCopy,
            _ => Self::Ask,
        }
    }

    /// Gracil's own command line for it (see `gracil-ui`'s `request.rs`). Asking has none: Gracil
    /// then opens its window as when it is opened with files.
    pub fn flag(self) -> Option<&'static str> {
        match self {
            Self::Replace => Some("--replace"),
            Self::SaveCopy => Some("--save-copy"),
            Self::Ask => None,
        }
    }
}

/// Which files a command takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    All,
    Videos,
    Images,
}

impl Scope {
    pub const ALL: [Self; 3] = [Self::All, Self::Videos, Self::Images];

    pub fn flag(self) -> Option<&'static str> {
        match self {
            Self::All => None,
            Self::Videos => Some("--videos"),
            Self::Images => Some("--images"),
        }
    }
}

/// What was right-clicked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Place {
    /// Files, or files and folders together.
    Files,
    Folder,
    Folders,
}

/// A command's name, in the language Windows is shown in (ADR 0011), for example "Comprimir y
/// reemplazar videos e imágenes dentro de esta carpeta".
pub fn command(spanish: bool, choice: Choice, scope: Scope, place: Place) -> String {
    let (lead, what, place) = if spanish {
        (
            match choice {
                Choice::Replace => "Comprimir y reemplazar",
                Choice::SaveCopy => "Comprimir y guardar como copia",
                Choice::Ask => "Comprimir",
            },
            match scope {
                Scope::All => "videos e imágenes",
                Scope::Videos => "videos",
                Scope::Images => "imágenes",
            },
            match place {
                Place::Files => "",
                Place::Folder => " dentro de esta carpeta",
                Place::Folders => " dentro de estas carpetas",
            },
        )
    } else {
        (
            match choice {
                Choice::Replace => "Compress and replace",
                Choice::SaveCopy => "Compress and save copies of",
                Choice::Ask => "Compress",
            },
            match scope {
                Scope::All => "videos and images",
                Scope::Videos => "videos",
                Scope::Images => "images",
            },
            match place {
                Place::Files => "",
                Place::Folder => " in this folder",
                Place::Folders => " in these folders",
            },
        )
    };
    format!("{lead} {what}{place}")
}

/// Which commands show. Folders may hold both kinds, so they get all three; files get "videos and
/// images" only when both kinds are picked, and otherwise just the one that applies.
pub fn shows(scope: Scope, videos: bool, images: bool, folders: bool) -> bool {
    match scope {
        Scope::All => folders || (videos && images),
        Scope::Videos => folders || videos,
        Scope::Images => folders || images,
    }
}

/// Room for paths on the command line that starts Gracil. Windows allows 32,767 characters; what
/// doesn't fit goes through the pipe once Gracil is up.
pub const COMMAND_LINE_ROOM: usize = 30_000;

/// Splits `paths` into the ones that fit in `room` characters of command line and the rest. The
/// first path always goes first, however long, so Gracil always starts with something to do.
pub fn split_for_command_line(paths: &[OsString], room: usize) -> (&[OsString], &[OsString]) {
    let mut used = 0;
    let mut fits = 0;
    for path in paths {
        // Quotes and a space around each path. The length is counted in bytes, which is never
        // fewer than the characters Windows counts.
        let cost = path.len() + 3;
        if fits > 0 && used + cost > room {
            break;
        }
        used += cost;
        fits += 1;
    }
    paths.split_at(fits)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_title_counts_videos_from_two() {
        assert_eq!(title(1), "Gracil");
        assert_eq!(title(8), "Gracil (8)");
    }

    #[test]
    fn the_commands_follow_the_always_setting() {
        assert_eq!(
            Choice::from_settings("explorer_mini_window=on\nalways=copy\n"),
            Choice::SaveCopy
        );
        assert_eq!(Choice::from_settings("always=replace"), Choice::Replace);
        assert_eq!(Choice::from_settings("always=ask"), Choice::Ask);
        assert_eq!(Choice::from_settings(""), Choice::Ask);
        assert_eq!(
            command(true, Choice::Replace, Scope::All, Place::Folder),
            "Comprimir y reemplazar videos e imágenes dentro de esta carpeta"
        );
        assert_eq!(
            command(true, Choice::SaveCopy, Scope::Images, Place::Files),
            "Comprimir y guardar como copia imágenes"
        );
        assert_eq!(
            command(false, Choice::Ask, Scope::Videos, Place::Folders),
            "Compress videos in these folders"
        );
    }

    #[test]
    fn files_of_one_kind_get_one_command() {
        let shown =
            |videos, images, folders| Scope::ALL.map(|scope| shows(scope, videos, images, folders));
        assert_eq!(shown(true, false, false), [false, true, false]);
        assert_eq!(shown(false, true, false), [false, false, true]);
        assert_eq!(shown(true, true, false), [true, true, true]);
        assert_eq!(shown(false, false, true), [true, true, true]);
    }

    #[test]
    fn long_selections_are_split() {
        let paths: Vec<OsString> = (0..10)
            .map(|n| OsString::from(format!(r"C:\Videos\clip {n}.mp4")))
            .collect();
        let (first, rest) = split_for_command_line(&paths, 60);
        assert_eq!(first.len(), 2);
        assert_eq!(rest.len(), 8);
        let (all, none) = split_for_command_line(&paths, COMMAND_LINE_ROOM);
        assert_eq!((all.len(), none.len()), (10, 0));
    }

    #[test]
    fn one_path_always_goes_first() {
        let paths = [OsString::from("x".repeat(100))];
        let (first, rest) = split_for_command_line(&paths, 10);
        assert_eq!((first.len(), rest.len()), (1, 0));
    }
}
