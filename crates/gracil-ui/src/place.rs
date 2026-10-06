//! Gracil's place in Windows without an installer (ADR 0050): starting with Windows and File
//! Explorer's menu, written again each time Gracil starts so a moved or updated folder keeps
//! working.
//!
//! Only a whole Gracil folder, the one the zip unpacks with FFmpeg beside `gracil.exe`, touches
//! Windows. A development build run from `target` leaves the user's Gracil where it is.

use std::fs;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::UNIX_EPOCH;

use gracil_core::toolchain;
use gracil_windows::Places;
use tracing::{info, warn};

const MENU_DLL: &str = "gracil_shell.dll";
/// Where the copy Explorer loads is kept, inside the data folder.
const MENU_FOLDER: &str = "shell";

/// What Gracil's place in Windows should be, as the settings say.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Wanted {
    /// In File Explorer's menu.
    pub in_explorer: bool,
    /// Started with Windows at the next sign-in (ADR 0054).
    pub at_sign_in: bool,
}

/// Settles Gracil's place in Windows on a thread of its own: Explorer's menu and starting with
/// Windows, each on or off as `wanted` says.
pub fn settle(wanted: Wanted) {
    let _ = thread::Builder::new()
        .name("gracil-place".into())
        .spawn(move || {
            let Some(places) = whole_folder() else {
                info!("not a whole Gracil folder; Windows is left as it is");
                return;
            };
            let started = if wanted.at_sign_in {
                gracil_windows::start_with_windows(&places.program)
            } else {
                gracil_windows::stop_starting_with_windows()
            };
            if let Err(error) = started {
                warn!(%error, wanted.at_sign_in, "start with Windows not settled");
            }
            let settled = if wanted.in_explorer {
                add_to_explorer(&places)
            } else {
                remove_from_explorer(&places)
            };
            match settled {
                Ok(()) => info!(wanted.in_explorer, "File Explorer's menu settled"),
                Err(error) => warn!(%error, wanted.in_explorer, "File Explorer's menu not settled"),
            }
        });
}

/// Readies Gracil's folder to be deleted (ADR 0054): takes Gracil out of File Explorer's menu
/// and out of starting with Windows, so deleting the folder leaves nothing, then shows the
/// folder in File Explorer, picked. Done before returning, since Gracil exits right after.
pub fn prepare_to_delete() {
    let Some(places) = whole_folder() else {
        return;
    };
    if let Err(error) = gracil_windows::stop_starting_with_windows() {
        warn!(%error, "start with Windows not removed");
    }
    match remove_from_explorer(&places) {
        Ok(()) => info!("Gracil taken out of Windows"),
        Err(error) => warn!(%error, "File Explorer's menu not removed"),
    }
    release_menu_copies(&places.data.join(MENU_FOLDER));
    if let Some(folder) = places.program.parent() {
        use std::os::windows::process::CommandExt;
        let shown = std::process::Command::new("explorer.exe")
            .raw_arg(format!("/select,\"{}\"", folder.display()))
            .spawn();
        if let Err(error) = shown {
            warn!(%error, "Gracil's folder not shown");
        }
    }
}

/// Clears the menu's DLL copies out of the data folder. A copy Explorer still has loaded cannot
/// be deleted, but it can be moved: it goes to the temporary folder, so it never stops the
/// folder from being deleted, and Windows clears it from there.
fn release_menu_copies(copies: &Path) {
    let Ok(entries) = fs::read_dir(copies) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if fs::remove_file(&path).is_ok() {
            continue;
        }
        let parked = std::env::temp_dir().join(format!(
            "gracil-shell-{:x}-{}",
            std::process::id(),
            entry.file_name().to_string_lossy()
        ));
        if let Err(error) = fs::rename(&path, &parked) {
            warn!(%error, "a copy of the menu stays in the data folder");
        }
    }
    let _ = fs::remove_dir(copies);
}

/// This Gracil's program and data folder, when it runs from a whole Gracil folder.
fn whole_folder() -> Option<Places> {
    let program = std::env::current_exe().ok()?;
    let folder = program.parent()?;
    let whole = folder.join(r"ffmpeg\ffmpeg.exe").is_file() && folder.join(MENU_DLL).is_file();
    whole.then_some(Places {
        program: program.clone(),
        data: toolchain::default_private_dir()?,
    })
}

/// Registers a copy of the menu's DLL kept in the data folder: Explorer holds the DLL it loads
/// until it closes, and the one beside `gracil.exe` must stay free for the next update to replace.
fn add_to_explorer(places: &Places) -> std::io::Result<()> {
    let folder = places
        .program
        .parent()
        .ok_or_else(|| std::io::Error::other("gracil.exe has no folder"))?;
    let shipped = folder.join(MENU_DLL);
    let copies = places.data.join(MENU_FOLDER);
    fs::create_dir_all(&copies)?;
    let copy = copies.join(copy_name(&shipped)?);
    if !copy.is_file() {
        fs::copy(&shipped, &copy)?;
    }
    gracil_windows::add_to_explorer(&copy, places)?;
    tidy_copies(&copies, Some(&copy));
    Ok(())
}

fn remove_from_explorer(places: &Places) -> std::io::Result<()> {
    // Another Gracil folder may hold the menu now; only this one's registration is taken away.
    let ours = gracil_windows::explorer_places().is_none_or(|registered| &registered == places);
    if ours {
        gracil_windows::remove_from_explorer()?;
    }
    tidy_copies(&places.data.join(MENU_FOLDER), None);
    Ok(())
}

/// A name that changes whenever the shipped DLL does, so an update brings a fresh copy.
fn copy_name(shipped: &Path) -> std::io::Result<String> {
    let metadata = fs::metadata(shipped)?;
    let modified = metadata
        .modified()?
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_secs());
    Ok(format!("gracil_shell-{}-{modified}.dll", metadata.len()))
}

/// Deletes the copies other than `keep`. One Explorer still holds can't be deleted; it goes on a
/// later start, once Explorer has let it go.
fn tidy_copies(copies: &Path, keep: Option<&PathBuf>) {
    let Ok(entries) = fs::read_dir(copies) else {
        return;
    };
    for path in entries.flatten().map(|entry| entry.path()) {
        if Some(&path) != keep && path.extension().is_some_and(|ext| ext == "dll") {
            let _ = fs::remove_file(&path);
        }
    }
}
