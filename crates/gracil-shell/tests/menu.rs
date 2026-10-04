//! The menu as File Explorer sees it: through COM, with real files selected. Invoking a command is
//! left out, since it would start Gracil.

// Talking to COM is `unsafe` throughout; every call here uses objects created in the same test.
#![allow(unsafe_code, clippy::unwrap_used)]

use std::ffi::c_void;
use std::fs;
use std::path::{Path, PathBuf};
use std::ptr;

use gracil_shell::{CLSID, DllCanUnloadNow, DllGetClassObject};
use windows::Win32::Foundation::S_OK;
use windows::Win32::System::Com::{
    COINIT_APARTMENTTHREADED, CoInitializeEx, CoTaskMemFree, IClassFactory,
};
use windows::Win32::UI::Shell::Common::ITEMIDLIST;
use windows::Win32::UI::Shell::{
    ECF_HASSUBCOMMANDS, ECS_ENABLED, ECS_HIDDEN, IExplorerCommand, IShellItemArray,
    SHCreateShellItemArrayFromIDLists, SHParseDisplayName,
};
use windows_core::{HSTRING, Interface, PWSTR};

fn fresh_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("gracil-shell-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn selection(paths: &[PathBuf]) -> IShellItemArray {
    let lists: Vec<*const ITEMIDLIST> = paths
        .iter()
        .map(|path| {
            let mut list = ptr::null_mut();
            unsafe {
                SHParseDisplayName(&HSTRING::from(path.as_path()), None, &mut list, 0, None)
                    .unwrap();
            }
            list.cast_const()
        })
        .collect();
    let items = unsafe { SHCreateShellItemArrayFromIDLists(&lists) }.unwrap();
    for list in lists {
        unsafe { CoTaskMemFree(Some(list.cast::<c_void>())) };
    }
    items
}

fn menu() -> IExplorerCommand {
    unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        let mut factory: *mut c_void = ptr::null_mut();
        assert_eq!(
            DllGetClassObject(&CLSID, &IClassFactory::IID, &mut factory),
            S_OK
        );
        let factory = IClassFactory::from_raw(factory);
        factory.CreateInstance(None).unwrap()
    }
}

fn text(value: PWSTR) -> String {
    let text = unsafe { value.to_string() }.unwrap();
    unsafe { CoTaskMemFree(Some(value.0 as *const c_void)) };
    text
}

fn touch(dir: &Path, name: &str) -> PathBuf {
    let path = dir.join(name);
    fs::write(&path, b"").unwrap();
    path
}

#[test]
fn the_menu_counts_files_and_offers_the_commands_that_apply() {
    let dir = fresh_dir("count");
    let videos = [
        touch(&dir, "Cumpleaños 🎂.mp4"),
        touch(&dir, "IMG_0042.MOV"),
    ];
    let notes = touch(&dir, "notes.txt");
    let menu = menu();

    let mixed = selection(&[videos[0].clone(), notes.clone(), videos[1].clone()]);
    let one = selection(&videos[..1]);
    let none = selection(std::slice::from_ref(&notes));
    unsafe {
        assert_eq!(text(menu.GetTitle(&mixed).unwrap()), "Gracil (2)");
        assert_eq!(text(menu.GetTitle(&one).unwrap()), "Gracil");
        assert_eq!(menu.GetState(&mixed, false).unwrap(), ECS_ENABLED.0 as u32);
        assert_eq!(menu.GetState(&none, false).unwrap(), ECS_HIDDEN.0 as u32);
        assert_eq!(menu.GetFlags().unwrap(), ECF_HASSUBCOMMANDS.0 as u32);

        let commands = menu.EnumSubCommands().unwrap();
        let mut found = [None, None, None];
        let mut fetched = 0;
        let _ = commands.Next(&mut found, Some(&mut fetched));
        assert_eq!(fetched, 3);
        let commands: Vec<IExplorerCommand> = found.into_iter().map(Option::unwrap).collect();

        // Videos only: just the videos command shows. Its words follow the Always setting.
        let states: Vec<u32> = commands
            .iter()
            .map(|command| command.GetState(&mixed, false).unwrap())
            .collect();
        let (shown, hidden) = (ECS_ENABLED.0 as u32, ECS_HIDDEN.0 as u32);
        assert_eq!(states, [hidden, shown, hidden]);
        let title = text(commands[1].GetTitle(&mixed).unwrap());
        assert!(
            title.starts_with("Compr") && title.ends_with("videos"),
            "{title}"
        );

        // A folder: plain "Gracil", all three commands, "in this folder".
        let folder = selection(std::slice::from_ref(&dir));
        assert_eq!(text(menu.GetTitle(&folder).unwrap()), "Gracil");
        assert_eq!(menu.GetState(&folder, false).unwrap(), shown);
        for command in &commands {
            assert_eq!(command.GetState(&folder, false).unwrap(), shown);
        }
        let title = text(commands[0].GetTitle(&folder).unwrap());
        assert!(
            title.ends_with("videos and images in this folder")
                || title.ends_with("videos e imágenes dentro de esta carpeta"),
            "{title}"
        );
    }
    drop(menu);
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn the_dll_stays_loaded_while_anything_of_it_lives() {
    let menu = menu();
    assert_ne!(DllCanUnloadNow(), S_OK);
    drop(menu);
}
