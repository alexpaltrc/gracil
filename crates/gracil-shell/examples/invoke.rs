//! Clicks one of the menu's commands the way File Explorer would, without needing the package:
//! loads the built `gracil_shell.dll` (next to `gracil.exe`), asks it for the menu, and invokes
//! the command on the given files.
//!
//! ```text
//! cargo build -p gracil-ui -p gracil-shell
//! cargo run -p gracil-shell --example invoke -- save-copy C:\Videos\a.mp4 C:\Videos\b.mov
//! cargo run -p gracil-shell --example invoke -- replace C:\Videos\Holidays
//! ```

// A development tool that drives COM by hand; every call uses objects created right here.
#![allow(unsafe_code, clippy::unwrap_used, clippy::expect_used)]

use std::ffi::c_void;
use std::path::PathBuf;
use std::ptr;
use std::time::Duration;

use windows::Win32::System::Com::{
    COINIT_APARTMENTTHREADED, CoInitializeEx, CoTaskMemFree, IClassFactory,
};
use windows::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};
use windows::Win32::UI::Shell::Common::ITEMIDLIST;
use windows::Win32::UI::Shell::{
    IExplorerCommand, SHCreateShellItemArrayFromIDLists, SHParseDisplayName,
};
use windows_core::{GUID, HRESULT, HSTRING, Interface, s};

type GetClassObject =
    unsafe extern "system" fn(*const GUID, *const GUID, *mut *mut c_void) -> HRESULT;

fn main() {
    let mut arguments = std::env::args().skip(1);
    let action = arguments.next().expect("replace or save-copy");
    let index = match action.as_str() {
        "replace" => 0,
        "save-copy" => 1,
        other => panic!("unknown command {other}"),
    };
    // A folder stands for everything in it, for selections too long for this command line.
    let paths: Vec<PathBuf> = arguments
        .map(PathBuf::from)
        .flat_map(|path| match std::fs::read_dir(&path) {
            Ok(entries) => entries.map(|entry| entry.unwrap().path()).collect(),
            Err(_) => vec![path],
        })
        .collect();

    let dll = std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("gracil_shell.dll");
    unsafe {
        CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok().unwrap();
        let module = LoadLibraryW(&HSTRING::from(dll.as_path())).expect("built DLL");
        let get: GetClassObject =
            std::mem::transmute(GetProcAddress(module, s!("DllGetClassObject")).unwrap());

        let mut factory = ptr::null_mut();
        get(&gracil_shell::CLSID, &IClassFactory::IID, &mut factory).unwrap();
        let factory = IClassFactory::from_raw(factory);
        let menu: IExplorerCommand = factory.CreateInstance(None).unwrap();

        let lists: Vec<*const ITEMIDLIST> = paths
            .iter()
            .map(|path| {
                let mut list = ptr::null_mut();
                SHParseDisplayName(&HSTRING::from(path.as_path()), None, &mut list, 0, None)
                    .unwrap();
                list.cast_const()
            })
            .collect();
        let items = SHCreateShellItemArrayFromIDLists(&lists).unwrap();
        for list in lists {
            CoTaskMemFree(Some(list.cast::<c_void>()));
        }

        let title = menu.GetTitle(&items).unwrap();
        println!("menu: {}", title.to_string().unwrap());
        CoTaskMemFree(Some(title.0 as *const c_void));
        let mut commands = [None, None];
        menu.EnumSubCommands()
            .unwrap()
            .Next(&mut commands, None)
            .ok()
            .unwrap();
        let command = commands[index].take().unwrap();
        command.Invoke(&items, None).unwrap();
        println!("invoked; waiting for the hand-over");
    }
    // The hand-over happens on a thread of the DLL; give it time before this process ends.
    std::thread::sleep(Duration::from_secs(12));
}
