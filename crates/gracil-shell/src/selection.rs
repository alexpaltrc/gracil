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

/// What the two commands are called, in the language Windows is shown in (ADR 0011).
pub struct Words {
    pub replace: &'static str,
    pub save_new: &'static str,
}

impl Words {
    pub fn get() -> Self {
        if gracil_windows::display_language_is_spanish() {
            Self {
                replace: "Comprimir y reemplazar",
                save_new: "Comprimir y guardar como nuevo",
            }
        } else {
            Self {
                replace: "Compress and replace",
                save_new: "Compress and save as new",
            }
        }
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
