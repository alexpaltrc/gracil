//! Gracil's place in Windows without an installer (ADR 0050): File Explorer's menus and starting
//! with Windows.
//!
//! File Explorer shows Gracil in two places (ADR 0058):
//!
//! - **Send to**, a shortcut in the user's own Send to folder. It needs no administrator and File
//!   Explorer always shows it, so it is there from the first start.
//! - The **main menu**, plain commands under `HKEY_LOCAL_MACHINE`. Windows 11 leaves out commands
//!   written for one user only, so these are written for the whole computer, which takes an
//!   administrator's permission once; Gracil asks only when the user turns it on.
//!
//! Both start `gracil.exe --explorer` with what was picked; a running Gracil gathers what each one
//! sends and applies **Always**, so their words never change and the permission is never asked
//! again. Starting with Windows is a `RunOnce` value (ADR 0054): Windows deletes it as it runs it,
//! so once Gracil's folder is gone nothing of it stays.

use std::io;
use std::path::{Path, PathBuf};
use std::ptr;

use std::mem::ManuallyDrop;

use windows::Win32::System::Com::{
    CLSCTX_ALL, CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
    CoTaskMemFree, CoUninitialize, IDispatch, IPersistFile, IServiceProvider,
};
use windows::Win32::System::Variant::{
    VARENUM, VARIANT, VARIANT_0, VARIANT_0_0, VARIANT_0_0_0, VT_BSTR, VT_I4, VariantClear,
};
use windows::Win32::UI::Shell::{
    CSIDL_DESKTOP, FOLDERID_SendTo, IShellBrowser, IShellDispatch2, IShellFolderViewDual,
    IShellLinkW, IShellWindows, KF_FLAG_DEFAULT, SHGetKnownFolderPath, SID_STopLevelBrowser,
    SVGIO_BACKGROUND, SWC_DESKTOP, SWFO_NEEDDISPATCH, ShellLink, ShellWindows,
};
use windows::core::{BSTR, HSTRING, Interface};
use windows_sys::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, REG_SZ, RRF_RT_REG_SZ, RegDeleteKeyValueW,
    RegDeleteTreeW, RegGetValueW, RegSetKeyValueW,
};
use windows_sys::Win32::UI::Shell::{SHCNE_ASSOCCHANGED, SHCNF_IDLIST, SHChangeNotify};

use crate::wide;

/// The verb's name in Explorer's keys.
const VERB: &str = "Gracil";
/// The folders' command, with its own commands under it.
const FOLDER_VERB_KEY: &str = r"Software\Classes\Directory\shell\Gracil";
/// Where each file type's command goes, by extension, whatever program opens the type.
const FILE_TYPES_KEY: &str = r"Software\Classes\SystemFileAssociations";
/// Keys of the menus Gracil wrote for the current user before ADR 0058, which File Explorer
/// never showed. They are removed whenever Gracil settles its place.
const FORMER_KEYS: [&str; 6] = [
    r"Software\Classes\*\shell\Gracil",
    r"Software\Classes\*\shell\GracilCompress",
    r"Software\Classes\Directory\shell\GracilCompress",
    FOLDER_VERB_KEY,
    r"Software\Classes\CLSID\{8D7CC31A-14B9-490F-A5B0-3A2B63DAC4EF}",
    r"Software\Classes\CLSID\{A7D6F05F-328B-4A10-A19E-C58E90EB7038}",
];
/// Run once, at the next sign-in, and forgotten by Windows as it runs.
const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\RunOnce";
const RUN_VALUE: &str = "Gracil";
/// Kept beside the folders' command, telling which Gracil wrote the main menu.
const PROGRAM_VALUE: &str = "GracilProgram";
/// The shortcut's name in Send to, which is also what the menu shows.
const SEND_TO_NAME: &str = "Gracil.lnk";

/// Gracil's main menu in File Explorer, in the words the user reads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExplorerMenu {
    /// The `gracil.exe` the commands start.
    pub program: PathBuf,
    /// The file types the menu shows on, lowercase and without the dot.
    pub extensions: Vec<&'static str>,
    /// The one command for picked files.
    pub files: ExplorerCommand,
    /// The folders' entry, which opens onto `folder_commands`.
    pub folders: String,
    pub folder_commands: Vec<ExplorerCommand>,
}

/// One command: its words, and Gracil's flags before the picked path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExplorerCommand {
    pub text: String,
    pub flags: Vec<&'static str>,
}

/// Puts `program` in File Explorer's Send to, starting it with `flags` before what was sent.
pub fn add_to_send_to(program: &Path, flags: &str) -> io::Result<()> {
    let shortcut = send_to()?.join(SEND_TO_NAME);
    let _com = Com::start();
    // SAFETY: COM is started on this thread for as long as `_com` lives; every string is an
    // HSTRING that outlives the call it is passed to.
    #[allow(unsafe_code)]
    unsafe {
        let link: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER)?;
        link.SetPath(&HSTRING::from(program.as_os_str()))?;
        link.SetArguments(&HSTRING::from(flags))?;
        link.SetIconLocation(&HSTRING::from(program.as_os_str()), 0)?;
        if let Some(folder) = program.parent() {
            link.SetWorkingDirectory(&HSTRING::from(folder.as_os_str()))?;
        }
        link.cast::<IPersistFile>()?
            .Save(&HSTRING::from(shortcut.as_os_str()), true)?;
    }
    Ok(())
}

/// Takes Gracil out of Send to.
pub fn remove_from_send_to() -> io::Result<()> {
    match std::fs::remove_file(send_to()?.join(SEND_TO_NAME)) {
        Err(error) if error.kind() != io::ErrorKind::NotFound => Err(error),
        _ => Ok(()),
    }
}

/// The user's Send to folder.
fn send_to() -> io::Result<PathBuf> {
    // SAFETY: Windows allocates the answer, which is copied and then freed here.
    #[allow(unsafe_code)]
    unsafe {
        let found = SHGetKnownFolderPath(&FOLDERID_SendTo, KF_FLAG_DEFAULT, None)?;
        let path = found.to_string().map(PathBuf::from);
        CoTaskMemFree(Some(found.as_ptr().cast()));
        path.map_err(io::Error::other)
    }
}

/// COM, started on this thread for as long as it lives.
struct Com(bool);

impl Com {
    fn start() -> Self {
        // SAFETY: plain call; a thread whose COM is already started gets an error, and then it is
        // not stopped here either.
        #[allow(unsafe_code)]
        let started = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) }.is_ok();
        Self(started)
    }
}

impl Drop for Com {
    fn drop(&mut self) {
        if self.0 {
            // SAFETY: balances the successful start above, on the same thread.
            #[allow(unsafe_code)]
            unsafe {
                CoUninitialize()
            };
        }
    }
}

/// Writes Gracil's main menu for everyone on this computer. Takes an administrator.
pub fn add_main_menu(menu: &ExplorerMenu) -> io::Result<()> {
    let root = HKEY_LOCAL_MACHINE;
    delete_tree(root, FOLDER_VERB_KEY)?;
    let icon = menu.program.to_string_lossy().into_owned();
    let command = |flags: &[&str]| {
        let mut line = format!("\"{}\"", menu.program.display());
        for flag in flags {
            line.push(' ');
            line.push_str(flag);
        }
        line.push_str(" \"%1\"");
        line
    };
    let verb = |key: &str, text: &str| -> io::Result<()> {
        set(root, key, Some("MUIVerb"), text)?;
        set(root, key, Some("Icon"), &icon)?;
        // Every picked file gets the command, however many there are.
        set(root, key, Some("MultiSelectModel"), "Player")
    };
    for extension in &menu.extensions {
        let key = file_verb_key(extension);
        delete_tree(root, &key)?;
        verb(&key, &menu.files.text)?;
        set(
            root,
            &format!(r"{key}\command"),
            None,
            &command(&menu.files.flags),
        )?;
    }
    verb(FOLDER_VERB_KEY, &menu.folders)?;
    set(root, FOLDER_VERB_KEY, Some("SubCommands"), "")?;
    set(
        root,
        FOLDER_VERB_KEY,
        Some(PROGRAM_VALUE),
        &menu.program.to_string_lossy(),
    )?;
    for (number, folder_command) in menu.folder_commands.iter().enumerate() {
        // Numbered, so Explorer keeps them in order.
        let key = format!(r"{FOLDER_VERB_KEY}\shell\{}", number + 1);
        verb(&key, &folder_command.text)?;
        set(
            root,
            &format!(r"{key}\command"),
            None,
            &command(&folder_command.flags),
        )?;
    }
    associations_changed();
    Ok(())
}

/// Takes Gracil's main menu away; `extensions` are the file types it was written for. Takes an
/// administrator.
pub fn remove_main_menu(extensions: &[&str]) -> io::Result<()> {
    delete_tree(HKEY_LOCAL_MACHINE, FOLDER_VERB_KEY)?;
    for extension in extensions {
        delete_tree(HKEY_LOCAL_MACHINE, &file_verb_key(extension))?;
    }
    associations_changed();
    Ok(())
}

/// The `gracil.exe` that wrote the main menu, if Gracil is in it.
pub fn main_menu_program() -> Option<PathBuf> {
    get(HKEY_LOCAL_MACHINE, FOLDER_VERB_KEY, PROGRAM_VALUE).map(PathBuf::from)
}

/// Removes the menus Gracil wrote for the current user before ADR 0058; `extensions` are the file
/// types they were written for.
pub fn remove_former_menus(extensions: &[&str]) -> io::Result<()> {
    for key in FORMER_KEYS {
        delete_tree(HKEY_CURRENT_USER, key)?;
    }
    for extension in extensions {
        delete_tree(HKEY_CURRENT_USER, &file_verb_key(extension))?;
    }
    Ok(())
}

fn file_verb_key(extension: &str) -> String {
    format!(r"{FILE_TYPES_KEY}\.{extension}\shell\{VERB}")
}

/// Asks File Explorer to start `program` with `arguments` as an administrator, which Windows first
/// asks the user about. Returns once asked, without waiting: whether it was done shows in what
/// the program changes.
///
/// File Explorer starts it rather than Gracil itself: Gracil runs in a job (ADR 0018), and Windows
/// denies starting an administrator's program from one (ADR 0058). File Explorer is where the
/// user's own "Run as administrator" comes from, so it is also the permission they know.
pub fn run_as_administrator(program: &Path, arguments: &str) -> io::Result<()> {
    const SHOW_NORMAL: i32 = 1;
    let _com = Com::start();
    // SAFETY: plain COM calls on interfaces Windows hands back, each checked before use; the
    // VARIANTs are built and cleared by `Variant`, and outlive the calls they are passed to.
    #[allow(unsafe_code)]
    unsafe {
        // The desktop's own File Explorer window, and from it the Shell's automation.
        let windows: IShellWindows = CoCreateInstance(&ShellWindows, None, CLSCTX_ALL)?;
        let mut window = 0;
        let desktop = windows.FindWindowSW(
            &Variant::number(CSIDL_DESKTOP as i32).0,
            &Variant::empty().0,
            SWC_DESKTOP,
            &mut window,
            SWFO_NEEDDISPATCH,
        )?;
        let browser: IShellBrowser = desktop
            .cast::<IServiceProvider>()?
            .QueryService(&SID_STopLevelBrowser)?;
        let view: IDispatch = browser
            .QueryActiveShellView()?
            .GetItemObject(SVGIO_BACKGROUND)?;
        let shell: IShellDispatch2 = view.cast::<IShellFolderViewDual>()?.Application()?.cast()?;
        shell.ShellExecute(
            &BSTR::from_wide(&HSTRING::from(program.as_os_str())),
            &Variant::text(arguments).0,
            &Variant::empty().0,
            &Variant::text("runas").0,
            &Variant::number(SHOW_NORMAL).0,
        )?;
    }
    Ok(())
}

/// A VARIANT that frees what it holds.
struct Variant(VARIANT);

impl Variant {
    fn empty() -> Self {
        Self(VARIANT::default())
    }

    fn text(text: &str) -> Self {
        Self::of(
            VT_BSTR,
            VARIANT_0_0_0 {
                bstrVal: ManuallyDrop::new(BSTR::from(text)),
            },
        )
    }

    fn number(number: i32) -> Self {
        Self::of(VT_I4, VARIANT_0_0_0 { lVal: number })
    }

    /// A value with the type that tells which of its fields is set.
    fn of(kind: VARENUM, value: VARIANT_0_0_0) -> Self {
        Self(VARIANT {
            Anonymous: VARIANT_0 {
                Anonymous: ManuallyDrop::new(VARIANT_0_0 {
                    vt: kind,
                    wReserved1: 0,
                    wReserved2: 0,
                    wReserved3: 0,
                    Anonymous: value,
                }),
            },
        })
    }
}

impl Drop for Variant {
    fn drop(&mut self) {
        // SAFETY: the VARIANT was built by `Variant` and is cleared once.
        #[allow(unsafe_code)]
        let _ = unsafe { VariantClear(&mut self.0) };
    }
}

/// Starts `program` with Windows at the next sign-in, in the tray (ADR 0024, ADR 0054).
pub fn start_with_windows(program: &Path) -> io::Result<()> {
    let command = format!("\"{}\" --startup", program.display());
    set(HKEY_CURRENT_USER, RUN_KEY, Some(RUN_VALUE), &command)
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
fn set(root: HKEY, key: &str, name: Option<&str>, value: &str) -> io::Result<()> {
    let key = wide(key);
    let name = name.map(wide);
    let data = wide(value);
    let bytes = u32::try_from(data.len() * 2).map_err(io::Error::other)?;
    // SAFETY: the key, name and data are NUL-terminated UTF-16 buffers that outlive the call, and
    // `bytes` is the data's length in bytes, terminator included, as REG_SZ requires.
    #[allow(unsafe_code)]
    let status = unsafe {
        RegSetKeyValueW(
            root,
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
fn get(root: HKEY, key: &str, name: &str) -> Option<String> {
    let key = wide(key);
    let name = wide(name);
    let mut buffer = vec![0u16; 2048];
    let mut bytes = u32::try_from(buffer.len() * 2).ok()?;
    // SAFETY: the names are NUL-terminated UTF-16 buffers; `buffer` has room for `bytes` bytes,
    // and the flag restricts the answer to text, which Windows terminates.
    #[allow(unsafe_code)]
    let status = unsafe {
        RegGetValueW(
            root,
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

fn delete_tree(root: HKEY, key: &str) -> io::Result<()> {
    const ERROR_FILE_NOT_FOUND: u32 = 2;
    let key = wide(key);
    // SAFETY: the key name is a NUL-terminated UTF-16 buffer that outlives the call.
    #[allow(unsafe_code)]
    let status = unsafe { RegDeleteTreeW(root, key.as_ptr()) };
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
