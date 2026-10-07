//! Gracil's place in Windows without an installer (ADR 0050): starting with Windows and File
//! Explorer's Send to, written again each time Gracil starts so a moved or updated folder keeps
//! working, and File Explorer's main menu, which takes an administrator's permission and so is
//! only written when the user turns it on (ADR 0058).
//!
//! Only a whole Gracil folder, the one the zip unpacks with FFmpeg beside `gracil.exe`, touches
//! Windows. A development build run from `target` leaves the user's Gracil where it is.

use std::fs;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant};

use gracil_core::toolchain;
use tracing::{info, warn};

use crate::explorer;
use crate::request::EXPLORER;
use crate::text::{Language, Text};

/// Where the DLL copies of the menu Gracil had before ADR 0058 were kept, in the data folder.
const FORMER_MENU_FOLDER: &str = "shell";
/// What Gracil is started with, as an administrator, to write or remove the main menu.
pub const ADD_MAIN_MENU: &str = "--add-main-menu";
pub const REMOVE_MAIN_MENU: &str = "--remove-main-menu";

/// What Gracil's place in Windows should be, as the settings say.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Wanted {
    /// In File Explorer's Send to.
    pub in_explorer: bool,
    /// Started with Windows at the next sign-in (ADR 0054).
    pub at_sign_in: bool,
}

/// Settles Gracil's place in Windows on a thread of its own: Send to and starting with Windows,
/// each on or off as `wanted` says.
pub fn settle(wanted: Wanted) {
    let _ = thread::Builder::new()
        .name("gracil-place".into())
        .spawn(move || {
            let Some(program) = whole_folder() else {
                info!("not a whole Gracil folder; Windows is left as it is");
                return;
            };
            let started = if wanted.at_sign_in {
                gracil_windows::start_with_windows(&program)
            } else {
                gracil_windows::stop_starting_with_windows()
            };
            if let Err(error) = started {
                warn!(%error, wanted.at_sign_in, "start with Windows not settled");
            }
            let settled = if wanted.in_explorer {
                gracil_windows::add_to_send_to(&program, EXPLORER)
            } else {
                gracil_windows::remove_from_send_to()
            };
            match settled {
                Ok(()) => info!(wanted.in_explorer, "Send to settled"),
                Err(error) => warn!(%error, wanted.in_explorer, "Send to not settled"),
            }
            remove_former_menus();
        });
}

/// Whether File Explorer's main menu starts this Gracil.
pub fn main_menu_is_ours() -> bool {
    whole_folder().is_some_and(|program| gracil_windows::main_menu_program() == Some(program))
}

/// Turns File Explorer's main menu on or off for this Gracil, on a thread of its own. Windows
/// first asks the user for an administrator's permission; `done` gets whether the main menu now
/// starts this Gracil, whatever the answer was. File Explorer starts the administrator's Gracil
/// (ADR 0058), so the menu itself is watched for the change, for as long as asking may take.
pub fn set_main_menu(on: bool, done: impl FnOnce(bool) + Send + 'static) {
    let _ = thread::Builder::new()
        .name("gracil-main-menu".into())
        .spawn(move || {
            if let Some(program) = whole_folder() {
                let flag = if on { ADD_MAIN_MENU } else { REMOVE_MAIN_MENU };
                match gracil_windows::run_as_administrator(&program, flag) {
                    Ok(()) => {
                        let changed = wait_for_main_menu(on);
                        info!(on, changed, "File Explorer's main menu");
                    }
                    Err(error) => warn!(%error, on, "File Explorer's main menu not settled"),
                }
            }
            done(main_menu_is_ours());
        });
}

/// Waits until the main menu is `on`, or off, or the user has had long enough to answer.
/// Whether it changed.
fn wait_for_main_menu(on: bool) -> bool {
    const LONGEST: Duration = Duration::from_secs(120);
    const EVERY: Duration = Duration::from_millis(500);
    let asked = Instant::now();
    while asked.elapsed() < LONGEST {
        if main_menu_is_ours() == on {
            return true;
        }
        thread::sleep(EVERY);
    }
    false
}

/// What Gracil does when started with [`ADD_MAIN_MENU`] or [`REMOVE_MAIN_MENU`], as an
/// administrator. `true` when it went well.
pub fn write_main_menu(on: bool) -> bool {
    let Ok(program) = std::env::current_exe() else {
        return false;
    };
    let written = if on {
        let menu = explorer::menu(program, Text::get(Language::detect()));
        gracil_windows::add_main_menu(&menu)
    } else {
        gracil_windows::remove_main_menu(&explorer::extensions())
    };
    written
        .inspect_err(|error| warn!(%error, on, "File Explorer's main menu not written"))
        .is_ok()
}

/// Readies Gracil's folder to be deleted (ADR 0054): takes Gracil out of File Explorer and out
/// of starting with Windows, so deleting the folder leaves nothing, then shows the folder in File
/// Explorer, picked. Done before returning, since Gracil exits right after.
pub fn prepare_to_delete() {
    let Some(program) = whole_folder() else {
        return;
    };
    if let Err(error) = gracil_windows::stop_starting_with_windows() {
        warn!(%error, "start with Windows not removed");
    }
    if let Err(error) = gracil_windows::remove_from_send_to() {
        warn!(%error, "Send to not removed");
    }
    if main_menu_is_ours() {
        // Windows asks for permission once more; saying no leaves the menu, which then simply
        // finds no Gracil.
        if let Err(error) = gracil_windows::run_as_administrator(&program, REMOVE_MAIN_MENU) {
            warn!(%error, "File Explorer's main menu not removed");
        }
    }
    remove_former_menus();
    info!("Gracil taken out of Windows");
    if let Some(folder) = program.parent() {
        use std::os::windows::process::CommandExt;
        let shown = std::process::Command::new("explorer.exe")
            .raw_arg(format!("/select,\"{}\"", folder.display()))
            .spawn();
        if let Err(error) = shown {
            warn!(%error, "Gracil's folder not shown");
        }
    }
}

/// Clears what the menus Gracil had before ADR 0058 left: keys for the current user only, which
/// File Explorer never showed, and the DLL copies kept in the data folder.
fn remove_former_menus() {
    if let Err(error) = gracil_windows::remove_former_menus(&explorer::extensions()) {
        warn!(%error, "the former menu's keys stay");
    }
    if let Some(copies) = toolchain::default_private_dir().map(|data| data.join(FORMER_MENU_FOLDER))
    {
        remove_dll_copies(&copies);
    }
}

fn remove_dll_copies(copies: &Path) {
    let Ok(entries) = fs::read_dir(copies) else {
        return;
    };
    for path in entries.flatten().map(|entry| entry.path()) {
        if path.extension().is_some_and(|ext| ext == "dll") {
            let _ = fs::remove_file(&path);
        }
    }
    let _ = fs::remove_dir(copies);
}

/// This Gracil's `gracil.exe`, when it runs from a whole Gracil folder.
fn whole_folder() -> Option<PathBuf> {
    let program = std::env::current_exe().ok()?;
    let whole = program.parent()?.join(r"ffmpeg\ffmpeg.exe").is_file();
    whole.then_some(program)
}
