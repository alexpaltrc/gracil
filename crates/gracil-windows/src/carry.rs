//! Carrying files out of a window the way File Explorer does (ADR 0083): Windows leads the drag,
//! with its own picture of the files, and whatever they are let go on receives a copy.

use std::io;
use std::path::PathBuf;

use windows::Win32::Foundation::HWND;
use windows::Win32::System::Com::IDataObject;
use windows::Win32::System::Ole::DROPEFFECT_COPY;
use windows::Win32::UI::Shell::Common::ITEMIDLIST;
use windows::Win32::UI::Shell::{
    BHID_DataObject, ILFree, IShellItemArray, SHCreateShellItemArrayFromIDLists, SHDoDragDrop,
    SHParseDisplayName,
};
use windows::core::HSTRING;
use windows_sys::Win32::Foundation::POINT;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_LBUTTON, VK_RBUTTON};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    GA_ROOT, GetAncestor, GetCursorPos, GetSystemMetrics, SM_SWAPBUTTON, WindowFromPoint,
};

/// Where carried files were let go.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Carried {
    /// Somewhere else, or nowhere: the carrying was called off.
    Away,
    /// On the window they came from, which then hears of them as dropped files to ignore.
    Home,
}

/// Lets the user carry `paths` out of `window` (its handle, as Iced's `window::raw_id` gives it)
/// while the primary mouse button is held, and returns once they are let go. Copying is the only
/// thing offered, so the files themselves always stay where they are. Call it on the window's own
/// thread, as the press that started it is handled.
pub fn carry_out(window: u64, paths: &[PathBuf]) -> io::Result<Carried> {
    if paths.is_empty() || !primary_button_held() {
        return Ok(Carried::Away);
    }
    let hwnd = HWND(window as usize as *mut core::ffi::c_void);
    let items = Items::of(paths)?;
    let pointers: Vec<*const ITEMIDLIST> = items.0.iter().map(|item| item.cast_const()).collect();
    // SAFETY: each pointer is a whole item list made just above, alive until `items` drops.
    #[allow(unsafe_code)]
    let array: IShellItemArray = unsafe { SHCreateShellItemArrayFromIDLists(&pointers) }?;
    // SAFETY: BHID_DataObject is the shell's own id for this handler; no bind context is needed.
    #[allow(unsafe_code)]
    let data: IDataObject = unsafe { array.BindToHandler(None, &BHID_DataObject) }?;
    // SAFETY: the window is this thread's, which Windows needs to lead a drag; with no drop
    // source of ours, Windows uses its own and draws the files' picture itself.
    #[allow(unsafe_code)]
    unsafe { SHDoDragDrop(Some(hwnd), &data, None, DROPEFFECT_COPY) }?;
    Ok(if pointer_over(window) {
        Carried::Home
    } else {
        Carried::Away
    })
}

/// Whether the button that starts a drag is down now: the left one, or the right one when they
/// are swapped.
fn primary_button_held() -> bool {
    // SAFETY: both only read the input state.
    #[allow(unsafe_code)]
    let held = unsafe {
        let key = if GetSystemMetrics(SM_SWAPBUTTON) != 0 {
            VK_RBUTTON
        } else {
            VK_LBUTTON
        };
        GetAsyncKeyState(i32::from(key))
    };
    held < 0
}

/// Whether the pointer is over `window` or anything inside it.
fn pointer_over(window: u64) -> bool {
    let mut at = POINT { x: 0, y: 0 };
    // SAFETY: GetCursorPos writes to the point given; the others only read windows by handle.
    #[allow(unsafe_code)]
    unsafe {
        if GetCursorPos(&mut at) == 0 {
            return false;
        }
        let under = WindowFromPoint(at);
        !under.is_null() && GetAncestor(under, GA_ROOT) as usize as u64 == window
    }
}

/// Item lists for files, freed together.
struct Items(Vec<*mut ITEMIDLIST>);

impl Items {
    fn of(paths: &[PathBuf]) -> io::Result<Self> {
        let mut items = Self(Vec::with_capacity(paths.len()));
        for path in paths {
            let mut item = std::ptr::null_mut();
            // SAFETY: the name is a valid string and `item` receives a list this owns from now.
            #[allow(unsafe_code)]
            unsafe {
                SHParseDisplayName(&HSTRING::from(path.as_os_str()), None, &mut item, 0, None)
            }?;
            items.0.push(item);
        }
        Ok(items)
    }
}

impl Drop for Items {
    fn drop(&mut self) {
        for &item in &self.0 {
            // SAFETY: each list was made by SHParseDisplayName and is freed once.
            #[allow(unsafe_code)]
            unsafe {
                ILFree(Some(item))
            };
        }
    }
}
