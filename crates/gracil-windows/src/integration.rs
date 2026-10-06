//! Gracil's place in Windows without an installer (ADR 0050): File Explorer's classic menu and
//! starting with Windows. Everything is written for the current user only, under
//! `HKEY_CURRENT_USER`, so nothing needs an administrator and nothing reaches other accounts.
//!
//! Gracil writes the keys again each time it starts, so a folder that was moved or updated keeps
//! working. Its start with Windows is a `RunOnce` value (ADR 0054): Windows deletes it as it runs
//! it, so once Gracil's folder is gone nothing of it stays.

use std::io;
use std::path::{Path, PathBuf};
use std::ptr;

use windows_sys::Win32::System::Registry::{
    HKEY_CURRENT_USER, REG_SZ, RRF_RT_REG_SZ, RegDeleteKeyValueW, RegDeleteTreeW, RegGetValueW,
    RegSetKeyValueW,
};
use windows_sys::Win32::UI::Shell::{SHCNE_ASSOCCHANGED, SHCNF_IDLIST, SHChangeNotify};

use crate::wide;

/// The class of Gracil's Explorer menu, `gracil_shell.dll`. The shell crate checks it uses the
/// same value.
pub const EXPLORER_CLSID: &str = "{8D7CC31A-14B9-490F-A5B0-3A2B63DAC4EF}";

/// The verb's name in Explorer's menu keys, for files and for folders.
const VERB_KEYS: [&str; 2] = [
    r"Software\Classes\*\shell\Gracil",
    r"Software\Classes\Directory\shell\Gracil",
];
/// Run once, at the next sign-in, and forgotten by Windows as it runs.
const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\RunOnce";
const RUN_VALUE: &str = "Gracil";
/// Values kept beside the class, telling the menu where its Gracil is.
const PROGRAM_VALUE: &str = "Program";
const DATA_VALUE: &str = "Data";

/// Where the Gracil that registered the menu lives.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Places {
    /// Its `gracil.exe`.
    pub program: PathBuf,
    /// Its data folder, holding `settings.txt`.
    pub data: PathBuf,
}

fn class_key() -> String {
    format!(r"Software\Classes\CLSID\{EXPLORER_CLSID}")
}

/// Puts Gracil in File Explorer's menu: `menu` is the `gracil_shell.dll` to load, and `places`
/// what it starts and reads.
pub fn add_to_explorer(menu: &Path, places: &Places) -> io::Result<()> {
    let class = class_key();
    set(&class, None, "Gracil")?;
    set(
        &class,
        Some(PROGRAM_VALUE),
        &places.program.to_string_lossy(),
    )?;
    set(&class, Some(DATA_VALUE), &places.data.to_string_lossy())?;
    let server = format!(r"{class}\InprocServer32");
    set(&server, None, &menu.to_string_lossy())?;
    set(&server, Some("ThreadingModel"), "Apartment")?;
    for verb in VERB_KEYS {
        set(verb, Some("ExplorerCommandHandler"), EXPLORER_CLSID)?;
        // Every picked file in one call, however many there are.
        set(verb, Some("MultiSelectModel"), "Player")?;
    }
    associations_changed();
    Ok(())
}

/// Takes Gracil out of File Explorer's menu.
pub fn remove_from_explorer() -> io::Result<()> {
    for key in VERB_KEYS
        .into_iter()
        .map(str::to_owned)
        .chain([class_key()])
    {
        delete_tree(&key)?;
    }
    associations_changed();
    Ok(())
}

/// Where the menu's Gracil lives, as it registered itself.
pub fn explorer_places() -> Option<Places> {
    let class = class_key();
    Some(Places {
        program: PathBuf::from(get(&class, PROGRAM_VALUE)?),
        data: PathBuf::from(get(&class, DATA_VALUE)?),
    })
}

/// The `gracil_shell.dll` Explorer loads for the menu, if Gracil is in it.
pub fn explorer_menu() -> Option<PathBuf> {
    get(&format!(r"{}\InprocServer32", class_key()), "").map(PathBuf::from)
}

/// Starts `program` with Windows at the next sign-in, in the tray (ADR 0024, ADR 0054).
pub fn start_with_windows(program: &Path) -> io::Result<()> {
    let command = format!("\"{}\" --startup", program.display());
    set(RUN_KEY, Some(RUN_VALUE), &command)
}

/// Takes Gracil out of starting with Windows.
pub fn stop_starting_with_windows() -> io::Result<()> {
    const ERROR_FILE_NOT_FOUND: u32 = 2;
    let key = wide(RUN_KEY);
    let name = wide(RUN_VALUE);
    // SAFETY: the key and value names are NUL-terminated UTF-16 buffers that outlive the call.
    #[allow(unsafe_code)]
    let status = unsafe { RegDeleteKeyValueW(HKEY_CURRENT_USER, key.as_ptr(), name.as_ptr()) };
    if status == ERROR_FILE_NOT_FOUND {
        return Ok(());
    }
    result(status)
}

/// Sets a text value, creating the key as needed. `None` is the key's default value.
fn set(key: &str, name: Option<&str>, value: &str) -> io::Result<()> {
    let key = wide(key);
    let name = name.map(wide);
    let data = wide(value);
    let bytes = u32::try_from(data.len() * 2).map_err(io::Error::other)?;
    // SAFETY: the key, name and data are NUL-terminated UTF-16 buffers that outlive the call, and
    // `bytes` is the data's length in bytes, terminator included, as REG_SZ requires.
    #[allow(unsafe_code)]
    let status = unsafe {
        RegSetKeyValueW(
            HKEY_CURRENT_USER,
            key.as_ptr(),
            name.as_ref().map_or(ptr::null(), |name| name.as_ptr()),
            REG_SZ,
            data.as_ptr().cast(),
            bytes,
        )
    };
    result(status)
}

/// A text value; `""` reads the key's default value.
fn get(key: &str, name: &str) -> Option<String> {
    let key = wide(key);
    let name = wide(name);
    let mut buffer = vec![0u16; 2048];
    let mut bytes = u32::try_from(buffer.len() * 2).ok()?;
    // SAFETY: the names are NUL-terminated UTF-16 buffers; `buffer` has room for `bytes` bytes,
    // and the flag restricts the answer to text, which Windows terminates.
    #[allow(unsafe_code)]
    let status = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            key.as_ptr(),
            name.as_ptr(),
            RRF_RT_REG_SZ,
            ptr::null_mut(),
            buffer.as_mut_ptr().cast(),
            &mut bytes,
        )
    };
    if status != 0 {
        return None;
    }
    let length = buffer.iter().position(|&unit| unit == 0)?;
    Some(String::from_utf16_lossy(&buffer[..length])).filter(|text| !text.is_empty())
}

fn delete_tree(key: &str) -> io::Result<()> {
    const ERROR_FILE_NOT_FOUND: u32 = 2;
    let key = wide(key);
    // SAFETY: the key name is a NUL-terminated UTF-16 buffer that outlives the call.
    #[allow(unsafe_code)]
    let status = unsafe { RegDeleteTreeW(HKEY_CURRENT_USER, key.as_ptr()) };
    if status == ERROR_FILE_NOT_FOUND {
        return Ok(());
    }
    result(status)
}

/// Tells File Explorer its menus changed, so it shows the change without a restart.
fn associations_changed() {
    // SAFETY: this event takes no items; both pointers are null as documented.
    #[allow(unsafe_code)]
    unsafe {
        SHChangeNotify(
            SHCNE_ASSOCCHANGED as i32,
            SHCNF_IDLIST,
            ptr::null(),
            ptr::null(),
        )
    };
}

fn result(status: u32) -> io::Result<()> {
    if status == 0 {
        Ok(())
    } else {
        Err(io::Error::from_raw_os_error(status as i32))
    }
}
