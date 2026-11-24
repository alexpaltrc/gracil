//! Gracil's main menu in File Explorer (spec §58, ADR 0035, ADR 0058). Its commands send
//! `--explorer`, and the running Gracil applies **Always** (ADR 0025), so the words stay the same
//! whatever it is set to.

use std::path::PathBuf;

use gracil_core::media::{IMAGE_EXTENSIONS, VIDEO_EXTENSIONS};
use gracil_windows::{ExplorerCommand, ExplorerMenu};

use crate::request::{EXPLORER, IMAGES, VIDEOS};
use crate::text::Text;

/// The menu that starts `program`, in the interface's words.
pub fn menu(program: PathBuf, text: &Text) -> ExplorerMenu {
    let [files, all, videos, images] = text.explorer_commands;
    let command = |words: &str, only: Option<&'static str>| ExplorerCommand {
        text: words.to_owned(),
        flags: [EXPLORER].into_iter().chain(only).collect(),
    };
    ExplorerMenu {
        program,
        extensions: extensions(),
        // Gracil Dev's commands carry its name, so they are never mistaken for the daily Gracil's
        // (ADR 0082).
        files: command(&files.replacen("Gracil", gracil_windows::NAME, 1), None),
        folders: gracil_windows::NAME.to_owned(),
        folder_commands: vec![
            command(all, None),
            command(videos, Some(VIDEOS)),
            command(images, Some(IMAGES)),
        ],
    }
}

/// Every file type the menu shows on: the videos and images Gracil compresses (ADR 0034).
pub fn extensions() -> Vec<&'static str> {
    VIDEO_EXTENSIONS
        .iter()
        .chain(IMAGE_EXTENSIONS)
        .copied()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::text::Language;

    #[test]
    fn every_command_lets_gracil_apply_always() {
        let text = Text::get(Language::Spanish);
        let menu = menu(PathBuf::from(r"C:\Gracil\gracil.exe"), text);
        assert_eq!(
            menu.files.text,
            format!("Comprimir con {}", gracil_windows::NAME)
        );
        assert_eq!(menu.folders, gracil_windows::NAME);
        assert_eq!(menu.files.flags, ["--explorer"]);
        assert_eq!(menu.folder_commands[1].text, "Comprimir solo los videos");
        assert_eq!(menu.folder_commands[1].flags, ["--explorer", "--videos"]);
    }

    #[test]
    fn the_menu_shows_on_videos_and_images_only() {
        let extensions = extensions();
        assert!(extensions.contains(&"mp4"));
        assert!(extensions.contains(&"jpg"));
        assert!(!extensions.contains(&"gif"));
    }
}
