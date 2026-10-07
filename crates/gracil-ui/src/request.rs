//! What Gracil is asked to do, by its own command line or by another invocation through the pipe
//! (spec §52, §62, ADR 0019).
//!
//! - `gracil.exe [videos]`: show the window, with the videos added if any. The user chooses Replace
//!   or Save copy after each one is done.
//! - `gracil.exe --replace videos` and `--save-copy videos`: a choice made in advance.
//!   The videos are compressed in the background, and each result is placed as asked.
//! - `gracil.exe --explorer paths`: what File Explorer's menu and Send to send (ADR 0058). The
//!   **Always** setting decides: set, the files are compressed in the background as it says;
//!   unset, the window opens and asks, as with dropped files.
//! - `--videos` or `--images` with any of these: only that kind, from the files and folders sent. A
//!   folder is looked through, subfolders included (ADR 0035).
//! - `gracil.exe --startup`: what Windows sends when it starts Gracil with the session. Gracil
//!   stays in the tray with no window (ADR 0024).
//!
//! Both arrive the same way, as a list of arguments, so one parser serves both. Anything else is
//! taken as a path and checked like any dropped file.

use std::ffi::{OsStr, OsString};
use std::path::PathBuf;

use gracil_core::media::Kind;

use crate::work::Choice;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    /// `None` shows the window and lets the user choose; otherwise the choice is made already and
    /// the work happens in the background.
    pub choice: Option<Choice>,
    pub paths: Vec<PathBuf>,
    /// Started with Windows: stay in the tray, show nothing.
    pub at_startup: bool,
    /// Only videos, or only images, from what was sent (File Explorer's commands, ADR 0035).
    pub only: Option<Kind>,
    /// From File Explorer, to follow the **Always** setting (ADR 0058).
    pub from_explorer: bool,
}

pub const REPLACE: &str = "--replace";
pub const SAVE_COPY: &str = "--save-copy";
pub const STARTUP: &str = "--startup";
pub const VIDEOS: &str = "--videos";
pub const IMAGES: &str = "--images";
pub const EXPLORER: &str = "--explorer";

impl Request {
    /// Reads a command line without the program's own name.
    pub fn parse(arguments: impl IntoIterator<Item = OsString>) -> Self {
        let mut request = Self {
            choice: None,
            paths: Vec::new(),
            at_startup: false,
            only: None,
            from_explorer: false,
        };
        for argument in arguments {
            if argument == OsStr::new(REPLACE) {
                request.choice = Some(Choice::Replace);
            } else if argument == OsStr::new(SAVE_COPY) {
                request.choice = Some(Choice::SaveCopy);
            } else if argument == OsStr::new(STARTUP) {
                request.at_startup = true;
            } else if argument == OsStr::new(VIDEOS) {
                request.only = Some(Kind::Video);
            } else if argument == OsStr::new(IMAGES) {
                request.only = Some(Kind::Image);
            } else if argument == OsStr::new(EXPLORER) {
                request.from_explorer = true;
            } else {
                // Absolute, so the same video named two ways is still one video.
                let path = PathBuf::from(argument);
                request
                    .paths
                    .push(std::path::absolute(&path).unwrap_or(path));
            }
        }
        request
    }

    /// The request as arguments for the running Gracil, which works from another folder; the
    /// paths are absolute already.
    pub fn to_arguments(&self) -> Vec<OsString> {
        let flag = match self.choice {
            None => None,
            Some(Choice::Replace) => Some(OsString::from(REPLACE)),
            Some(Choice::SaveCopy) => Some(OsString::from(SAVE_COPY)),
        };
        let paths = self.paths.iter().map(|path| path.clone().into_os_string());
        let startup = self.at_startup.then(|| OsString::from(STARTUP));
        let explorer = self.from_explorer.then(|| OsString::from(EXPLORER));
        let only = self.only.map(|kind| {
            OsString::from(match kind {
                Kind::Video => VIDEOS,
                Kind::Image => IMAGES,
            })
        });
        flag.into_iter()
            .chain(startup)
            .chain(explorer)
            .chain(only)
            .chain(paths)
            .collect()
    }

    /// Whether this request needs the main window. A plain launch with nothing to add still does.
    pub fn shows_window(&self) -> bool {
        self.choice.is_none() && !self.at_startup
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn arguments(list: &[&str]) -> Vec<OsString> {
        list.iter().map(OsString::from).collect()
    }

    #[test]
    fn a_plain_launch_shows_the_window() {
        let request = Request::parse(arguments(&[]));
        assert_eq!(request.choice, None);
        assert!(request.paths.is_empty());
        assert!(request.shows_window());
    }

    #[test]
    fn explorer_commands_choose_in_advance() {
        let request = Request::parse(arguments(&["--replace", r"C:\a.mov", r"C:\b.mkv"]));
        assert_eq!(request.choice, Some(Choice::Replace));
        assert_eq!(request.paths.len(), 2);
        assert!(!request.shows_window());

        let request = Request::parse(arguments(&["--save-copy", r"C:\a.mov"]));
        assert_eq!(request.choice, Some(Choice::SaveCopy));
    }

    #[test]
    fn one_kind_only_survives_forwarding() {
        let request = Request::parse(arguments(&["--save-copy", "--images", r"C:\Fotos"]));
        assert_eq!(request.only, Some(Kind::Image));
        assert_eq!(Request::parse(request.to_arguments()), request);
        let request = Request::parse(arguments(&["--videos", r"C:\Fotos"]));
        assert_eq!(request.only, Some(Kind::Video));
        assert!(request.shows_window(), "asking opens the window");
    }

    #[test]
    fn starting_with_windows_shows_nothing() {
        let request = Request::parse(arguments(&["--startup"]));
        assert!(request.at_startup);
        assert!(request.paths.is_empty());
        assert!(!request.shows_window());
        assert!(Request::parse(request.to_arguments()).at_startup);
    }

    #[test]
    fn file_explorer_survives_forwarding() {
        let request = Request::parse(arguments(&["--explorer", "--videos", r"C:\Fotos"]));
        assert!(request.from_explorer);
        assert_eq!(request.choice, None);
        assert_eq!(Request::parse(request.to_arguments()), request);
    }

    #[test]
    fn paths_become_absolute_and_survive_forwarding() {
        let request = Request::parse(arguments(&["--save-copy", "clip.mp4"]));
        let forwarded = Request::parse(request.to_arguments());
        assert_eq!(forwarded.choice, Some(Choice::SaveCopy));
        assert!(forwarded.paths[0].is_absolute());
        assert!(forwarded.paths[0].ends_with("clip.mp4"));
    }
}
