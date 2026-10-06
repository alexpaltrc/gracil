//! Prints File Explorer's classic right-click menu (**Show more options**) for the given files,
//! built the way File Explorer builds it, with Gracil's entry as registered for the current user.
//! For checking that the menu shows up without clicking around in File Explorer.
//!
//! ```text
//! cargo run -p gracil-shell --example menu -- C:\Videos\a.mp4
//! ```

// A development tool that drives COM by hand; every call uses objects created right here.
#![allow(unsafe_code, clippy::unwrap_used, clippy::expect_used)]

use std::path::PathBuf;
use std::ptr;

use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::System::Com::{COINIT_APARTMENTTHREADED, CoInitializeEx};
use windows::Win32::UI::Shell::Common::ITEMIDLIST;
use windows::Win32::UI::Shell::{
    CMF_EXTENDEDVERBS, CMF_NORMAL, IContextMenu, IContextMenu3, IShellFolder, SHBindToParent,
    SHParseDisplayName,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreatePopupMenu, GetMenuItemCount, GetMenuItemInfoW, HMENU, MENUITEMINFOW, MFT_SEPARATOR,
    MIIM_FTYPE, MIIM_STRING, MIIM_SUBMENU, WM_INITMENUPOPUP,
};
use windows_core::{HSTRING, Interface};

fn main() {
    let paths: Vec<PathBuf> = std::env::args().skip(1).map(PathBuf::from).collect();
    let first = paths.first().expect("one or more files of the same folder");
    let extended = std::env::var_os("GRACIL_SHIFT").is_some();
    unsafe {
        CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok().unwrap();
        let mut list = ptr::null_mut();
        SHParseDisplayName(&HSTRING::from(first.as_path()), None, &mut list, 0, None).unwrap();
        let mut child: *mut ITEMIDLIST = ptr::null_mut();
        let folder: IShellFolder = SHBindToParent(list, Some(&mut child)).unwrap();

        let mut children = vec![child.cast_const()];
        for path in &paths[1..] {
            let mut other = ptr::null_mut();
            folder
                .ParseDisplayName(
                    HWND::default(),
                    None,
                    &HSTRING::from(path.file_name().unwrap()),
                    None,
                    &mut other,
                    ptr::null_mut(),
                )
                .unwrap();
            children.push(other.cast_const());
        }
        let menu: IContextMenu = folder
            .GetUIObjectOf(HWND::default(), &children, None)
            .unwrap();

        let popup = CreatePopupMenu().unwrap();
        let flags = if extended {
            CMF_NORMAL | CMF_EXTENDEDVERBS
        } else {
            CMF_NORMAL
        };
        menu.QueryContextMenu(popup, 0, 1, 0x7FFF, flags)
            .ok()
            .unwrap();
        print(&menu, popup, 0);
    }
}

/// Prints every entry of `popup`, opening submenus the way File Explorer does.
unsafe fn print(menu: &IContextMenu, popup: HMENU, depth: usize) {
    let count = unsafe { GetMenuItemCount(Some(popup)) };
    for index in 0..count.max(0) as u32 {
        let mut text = [0u16; 256];
        let mut info = MENUITEMINFOW {
            cbSize: size_of::<MENUITEMINFOW>() as u32,
            fMask: MIIM_STRING | MIIM_SUBMENU | MIIM_FTYPE,
            dwTypeData: windows_core::PWSTR(text.as_mut_ptr()),
            cch: text.len() as u32,
            ..Default::default()
        };
        if unsafe { GetMenuItemInfoW(popup, index, true, &mut info) }.is_err() {
            continue;
        }
        let indent = "  ".repeat(depth);
        if info.fType.0 & MFT_SEPARATOR.0 != 0 {
            println!("{indent}----");
            continue;
        }
        let name = String::from_utf16_lossy(&text[..info.cch as usize]);
        println!("{indent}{name}");
        if !info.hSubMenu.is_invalid() {
            if let Ok(three) = menu.cast::<IContextMenu3>() {
                let _ = unsafe {
                    three.HandleMenuMsg2(
                        WM_INITMENUPOPUP,
                        WPARAM(info.hSubMenu.0 as usize),
                        LPARAM(index as isize),
                        None,
                    )
                };
            }
            unsafe { print(menu, info.hSubMenu, depth + 1) };
        }
    }
}
