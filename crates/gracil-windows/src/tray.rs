//! Gracil's icon in the notification area, next to the clock (spec §54, §54.1, ADR 0020).
//!
//! The icon lives on a thread of its own with a hidden window that receives the taskbar's
//! messages: a click shows or hides the mini-window; the right-click menu offers Open, the
//! settings (starting with Windows, File Explorer's menu, the mini-window), Prepare to delete
//! and Exit. The icon follows the taskbar between light and dark, passes on changes to Windows'
//! colors, and comes back if File Explorer restarts. Gracil tells it what to show with
//! [`Tray::show`]; it never polls.

mod picture;

use std::io;
use std::ptr;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Mutex, MutexGuard, OnceLock, mpsc};
use std::thread;

use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::HiDpi::{GetDpiForSystem, GetSystemMetricsForDpi};
use windows_sys::Win32::UI::Shell::{
    NIF_ICON, NIF_MESSAGE, NIF_SHOWTIP, NIF_TIP, NIM_ADD, NIM_DELETE, NIM_MODIFY, NIM_SETVERSION,
    NIN_SELECT, NINF_KEY, NOTIFYICON_VERSION_4, NOTIFYICONDATAW, NOTIFYICONIDENTIFIER,
    Shell_NotifyIconGetRect, Shell_NotifyIconW,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreateIcon, CreatePopupMenu, CreateWindowExW, DefWindowProcW, DestroyIcon,
    DestroyMenu, DispatchMessageW, GetMessageW, HICON, MF_CHECKED, MF_SEPARATOR, MF_STRING, MSG,
    PostMessageW, RegisterClassExW, RegisterWindowMessageW, SM_CXSMICON,
    SPI_SETCLIENTAREAANIMATION, SPI_SETHIGHCONTRAST, SetForegroundWindow, SetMenuDefaultItem,
    TPM_RETURNCMD, TPM_RIGHTBUTTON, TrackPopupMenu, TranslateMessage, WM_APP, WM_CONTEXTMENU,
    WM_NULL, WM_SETTINGCHANGE, WNDCLASSEXW, WS_OVERLAPPED,
};

use crate::wide;

/// What the icon shows. It is driven by Gracil's work, never by a timer (spec §54).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TrayState {
    /// Nothing is being compressed.
    Idle,
    /// A video is compressing; the share done so far, from 0 to 1.
    Processing(f32),
    /// The compression is paused.
    Paused,
    /// Something went wrong that the user hasn't seen yet.
    Error,
}

/// What the user asked for through the icon.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayEvent {
    /// A click on the icon: show or hide the mini-window.
    Toggle,
    /// Open in the icon's menu: show Gracil's window.
    Open,
    /// The menu's Send to setting was clicked, to turn it the other way.
    ExplorerMenu,
    /// The menu's File Explorer main menu setting was clicked, to turn it the other way.
    ExplorerMainMenu,
    /// The menu's File Explorer setting was clicked, to turn it the other way.
    ExplorerMiniWindow,
    /// The menu's start with Windows setting was clicked, to turn it the other way.
    StartWithWindows,
    /// Prepare to delete Gracil, in the icon's menu: out of Windows, then exit.
    PrepareToDelete,
    /// Exit in the icon's menu.
    Exit,
    /// Windows changed how apps should look or move: light or dark, the accent color, a contrast
    /// theme, or animation effects.
    Colors,
}

/// The words of the icon's menu, in the interface's language.
pub struct TrayMenu {
    pub open: String,
    /// The setting for Gracil being in File Explorer's Send to.
    pub explorer_menu: String,
    /// The setting for Gracil being in File Explorer's main menu.
    pub explorer_main_menu: String,
    /// The setting for showing the mini-window when compressing from File Explorer.
    pub explorer_mini_window: String,
    /// The setting for Gracil starting with Windows.
    pub start_with_windows: String,
    /// Taking Gracil out of Windows before its folder is deleted.
    pub prepare_to_delete: String,
    pub exit: String,
}

/// The icon, once it is running. There is one per process.
pub struct Tray {
    window: usize,
}

/// The icon's number for its window; Gracil has only one.
const ICON_ID: u32 = 1;
/// The message the taskbar sends about clicks on the icon.
const ICON_MESSAGE: u32 = WM_APP + 1;
/// The message asking the icon's thread to show the latest state.
const REFRESH: u32 = WM_APP + 2;
/// Menu commands.
const OPEN: usize = 1;
const EXIT: usize = 2;
const EXPLORER_MINI_WINDOW: usize = 3;
const EXPLORER_MENU: usize = 4;
const START_WITH_WINDOWS: usize = 5;
const PREPARE_TO_DELETE: usize = 6;
const EXPLORER_MAIN_MENU: usize = 7;

/// Everything the icon's window needs, shared with the rest of Gracil.
struct Shared {
    open: Vec<u16>,
    explorer_menu: Vec<u16>,
    explorer_main_menu: Vec<u16>,
    explorer_mini_window: Vec<u16>,
    start_with_windows: Vec<u16>,
    prepare_to_delete: Vec<u16>,
    exit: Vec<u16>,
    on_event: Box<dyn Fn(TrayEvent) + Send + Sync>,
    current: Mutex<Current>,
}

struct Current {
    state: TrayState,
    tip: String,
    /// The icon handle being shown, as an address.
    icon: usize,
    /// Whether the taskbar has the icon now.
    added: bool,
    /// Gracil is leaving: the icon is gone and must not come back.
    gone: bool,
    light_taskbar: bool,
    /// Whether the menu shows Gracil in File Explorer's Send to as on.
    explorer_menu: bool,
    /// Whether the menu shows Gracil in File Explorer's main menu as on.
    explorer_main_menu: bool,
    /// Whether the menu shows the File Explorer setting as on.
    explorer_mini_window: bool,
    /// Whether the menu shows starting with Windows as on.
    start_with_windows: bool,
}

static SHARED: OnceLock<Shared> = OnceLock::new();
/// The message File Explorer broadcasts when the taskbar is (re)created.
static TASKBAR_CREATED: AtomicU32 = AtomicU32::new(0);

impl Tray {
    /// Puts Gracil's icon in the notification area. `on_event` is called, on the icon's own thread,
    /// for each click the user makes.
    pub fn start(
        menu: TrayMenu,
        tip: &str,
        on_event: impl Fn(TrayEvent) + Send + Sync + 'static,
    ) -> io::Result<Self> {
        let shared = Shared {
            open: wide(&menu.open),
            explorer_menu: wide(&menu.explorer_menu),
            explorer_main_menu: wide(&menu.explorer_main_menu),
            explorer_mini_window: wide(&menu.explorer_mini_window),
            start_with_windows: wide(&menu.start_with_windows),
            prepare_to_delete: wide(&menu.prepare_to_delete),
            exit: wide(&menu.exit),
            on_event: Box::new(on_event),
            current: Mutex::new(Current {
                state: TrayState::Idle,
                tip: tip.to_owned(),
                icon: 0,
                added: false,
                gone: false,
                light_taskbar: crate::theme::light_taskbar(),
                explorer_menu: false,
                explorer_main_menu: false,
                explorer_mini_window: false,
                start_with_windows: false,
            }),
        };
        if SHARED.set(shared).is_err() {
            return Err(io::Error::other("the tray icon is already running"));
        }
        let (sent, received) = mpsc::channel();
        thread::Builder::new()
            .name("gracil-tray".into())
            .spawn(move || run(sent))?;
        let window = received
            .recv()
            .map_err(|_| io::Error::other("the tray icon's thread ended"))??;
        Ok(Self { window })
    }

    /// Shows `state`, with `tip` as the text that appears when the pointer rests on the icon.
    pub fn show(&self, state: TrayState, tip: &str) {
        let Some(shared) = SHARED.get() else {
            return;
        };
        {
            let mut current = lock(shared);
            current.state = state;
            tip.clone_into(&mut current.tip);
        }
        // SAFETY: the window lives as long as the process; posting only queues a message.
        #[allow(unsafe_code)]
        unsafe {
            PostMessageW(self.window as HWND, REFRESH, 0, 0)
        };
    }

    /// Whether the menu shows Gracil in File Explorer's Send to as on.
    pub fn check_explorer_menu(&self, on: bool) {
        if let Some(shared) = SHARED.get() {
            lock(shared).explorer_menu = on;
        }
    }

    /// Whether the menu shows Gracil in File Explorer's main menu as on.
    pub fn check_explorer_main_menu(&self, on: bool) {
        if let Some(shared) = SHARED.get() {
            lock(shared).explorer_main_menu = on;
        }
    }

    /// Whether the menu shows starting with Windows as on.
    pub fn check_start_with_windows(&self, on: bool) {
        if let Some(shared) = SHARED.get() {
            lock(shared).start_with_windows = on;
        }
    }

    /// Whether the menu shows the File Explorer setting as on.
    pub fn check_explorer_mini_window(&self, on: bool) {
        if let Some(shared) = SHARED.get() {
            lock(shared).explorer_mini_window = on;
        }
    }

    /// Where the icon is on the screen, in physical pixels, when the taskbar shows it.
    pub(crate) fn anchor(&self) -> Option<crate::flyout::Area> {
        let identifier = NOTIFYICONIDENTIFIER {
            cbSize: size_of::<NOTIFYICONIDENTIFIER>() as u32,
            hWnd: self.window as HWND,
            uID: ICON_ID,
            ..NOTIFYICONIDENTIFIER::default()
        };
        let mut place = RECT::default();
        // SAFETY: `identifier` names this process's icon; `place` receives a rectangle.
        #[allow(unsafe_code)]
        let status = unsafe { Shell_NotifyIconGetRect(&identifier, &mut place) };
        (status == 0).then(|| place.into())
    }

    /// Takes the icon away for good. Call it just before Gracil ends, so no stale icon lingers.
    pub fn remove(&self) {
        let Some(shared) = SHARED.get() else {
            return;
        };
        let mut current = lock(shared);
        current.gone = true;
        if current.added {
            let data = icon_data(self.window as HWND);
            // SAFETY: `data` is a valid, fully initialised description of the icon.
            #[allow(unsafe_code)]
            unsafe {
                Shell_NotifyIconW(NIM_DELETE, &data)
            };
            current.added = false;
        }
    }
}

/// The icon's thread: creates the hidden window, adds the icon and handles messages until the
/// process ends.
fn run(started: mpsc::Sender<io::Result<usize>>) {
    let window = match create_window() {
        Ok(window) => window,
        Err(error) => {
            let _ = started.send(Err(error));
            return;
        }
    };
    refresh(window);
    let _ = started.send(Ok(window as usize));
    let mut message = MSG::default();
    // SAFETY: `message` is a valid place for each message; the loop ends only if GetMessageW
    // fails or the thread is asked to quit.
    #[allow(unsafe_code)]
    unsafe {
        while GetMessageW(&mut message, ptr::null_mut(), 0, 0) > 0 {
            TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }
}

fn create_window() -> io::Result<HWND> {
    let class = wide("Gracil.Tray");
    // SAFETY: the name is a NUL-terminated UTF-16 buffer that outlives the call.
    #[allow(unsafe_code)]
    let taskbar_created = unsafe { RegisterWindowMessageW(wide("TaskbarCreated").as_ptr()) };
    TASKBAR_CREATED.store(taskbar_created, Ordering::Relaxed);
    // SAFETY: a null name asks for the running executable's module, loaded for the whole process.
    #[allow(unsafe_code)]
    let instance = unsafe { GetModuleHandleW(ptr::null()) };
    let description = WNDCLASSEXW {
        cbSize: size_of::<WNDCLASSEXW>() as u32,
        lpfnWndProc: Some(window_procedure),
        hInstance: instance,
        lpszClassName: class.as_ptr(),
        ..WNDCLASSEXW::default()
    };
    // SAFETY: the description is complete and its strings outlive the call.
    #[allow(unsafe_code)]
    if unsafe { RegisterClassExW(&description) } == 0 {
        return Err(io::Error::last_os_error());
    }
    // An ordinary top-level window that is never shown: unlike a message-only window, it hears
    // the broadcasts about the taskbar and the theme.
    // SAFETY: the class was registered above; the strings outlive the call; no parent, menu or
    // creation data.
    #[allow(unsafe_code)]
    let window = unsafe {
        CreateWindowExW(
            0,
            class.as_ptr(),
            wide("Gracil").as_ptr(),
            WS_OVERLAPPED,
            0,
            0,
            0,
            0,
            ptr::null_mut(),
            ptr::null_mut(),
            instance,
            ptr::null(),
        )
    };
    if window.is_null() {
        return Err(io::Error::last_os_error());
    }
    Ok(window)
}

/// Handles the hidden window's messages, on the icon's thread.
#[allow(unsafe_code)]
unsafe extern "system" fn window_procedure(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match message {
        ICON_MESSAGE => {
            // With version 4 of the icon protocol, the event is in the low word of `lparam` and
            // the click's position in `wparam`.
            let event = (lparam as u32) & 0xFFFF;
            if event == NIN_SELECT || event == NIN_SELECT | NINF_KEY {
                notify(TrayEvent::Toggle);
            } else if event == WM_CONTEXTMENU {
                let x = i32::from((wparam & 0xFFFF) as u16 as i16);
                let y = i32::from(((wparam >> 16) & 0xFFFF) as u16 as i16);
                show_menu(window, x, y);
            }
            0
        }
        REFRESH => {
            refresh(window);
            0
        }
        WM_SETTINGCHANGE if is_look_change(wparam, lparam) => {
            if let Some(shared) = SHARED.get() {
                lock(shared).light_taskbar = crate::theme::light_taskbar();
            }
            refresh(window);
            notify(TrayEvent::Colors);
            0
        }
        _ if message == TASKBAR_CREATED.load(Ordering::Relaxed) && message != 0 => {
            // File Explorer restarted and the taskbar forgot every icon.
            if let Some(shared) = SHARED.get() {
                lock(shared).added = false;
            }
            refresh(window);
            0
        }
        // SAFETY: every other message gets Windows' default handling, with its own arguments.
        _ => unsafe { DefWindowProcW(window, message, wparam, lparam) },
    }
}

fn notify(event: TrayEvent) {
    if let Some(shared) = SHARED.get() {
        (shared.on_event)(event);
    }
}

/// Shows the right-click menu at the click and acts on the choice.
fn show_menu(window: HWND, x: i32, y: i32) {
    let Some(shared) = SHARED.get() else {
        return;
    };
    let (menu_check, main_menu_check, explorer_check, start_check) = {
        let current = lock(shared);
        let check = |on: bool| if on { MF_CHECKED } else { 0 };
        (
            check(current.explorer_menu),
            check(current.explorer_main_menu),
            check(current.explorer_mini_window),
            check(current.start_with_windows),
        )
    };
    // SAFETY: the menu is created, used and destroyed here; the labels are NUL-terminated UTF-16
    // buffers that outlive it. Bringing the hidden window forward first, and posting it an empty
    // message after, is what lets the menu close when the user clicks elsewhere.
    #[allow(unsafe_code)]
    let chosen = unsafe {
        let menu = CreatePopupMenu();
        if menu.is_null() {
            return;
        }
        AppendMenuW(menu, MF_STRING, OPEN, shared.open.as_ptr());
        AppendMenuW(menu, MF_SEPARATOR, 0, ptr::null());
        AppendMenuW(
            menu,
            MF_STRING | start_check,
            START_WITH_WINDOWS,
            shared.start_with_windows.as_ptr(),
        );
        AppendMenuW(
            menu,
            MF_STRING | menu_check,
            EXPLORER_MENU,
            shared.explorer_menu.as_ptr(),
        );
        AppendMenuW(
            menu,
            MF_STRING | main_menu_check,
            EXPLORER_MAIN_MENU,
            shared.explorer_main_menu.as_ptr(),
        );
        AppendMenuW(
            menu,
            MF_STRING | explorer_check,
            EXPLORER_MINI_WINDOW,
            shared.explorer_mini_window.as_ptr(),
        );
        AppendMenuW(menu, MF_SEPARATOR, 0, ptr::null());
        AppendMenuW(
            menu,
            MF_STRING,
            PREPARE_TO_DELETE,
            shared.prepare_to_delete.as_ptr(),
        );
        AppendMenuW(menu, MF_STRING, EXIT, shared.exit.as_ptr());
        SetMenuDefaultItem(menu, OPEN as u32, 0);
        SetForegroundWindow(window);
        let chosen = TrackPopupMenu(
            menu,
            TPM_RETURNCMD | TPM_RIGHTBUTTON,
            x,
            y,
            0,
            window,
            ptr::null(),
        );
        PostMessageW(window, WM_NULL, 0, 0);
        DestroyMenu(menu);
        chosen
    };
    match chosen as usize {
        OPEN => notify(TrayEvent::Open),
        EXPLORER_MENU => notify(TrayEvent::ExplorerMenu),
        EXPLORER_MAIN_MENU => notify(TrayEvent::ExplorerMainMenu),
        EXPLORER_MINI_WINDOW => notify(TrayEvent::ExplorerMiniWindow),
        START_WITH_WINDOWS => notify(TrayEvent::StartWithWindows),
        PREPARE_TO_DELETE => notify(TrayEvent::PrepareToDelete),
        EXIT => notify(TrayEvent::Exit),
        _ => {}
    }
}

/// Draws the latest state and gives it to the taskbar, adding the icon if it isn't there yet.
fn refresh(window: HWND) {
    let Some(shared) = SHARED.get() else {
        return;
    };
    let mut current = lock(shared);
    if current.gone {
        return;
    }
    let Some(icon) = make_icon(current.state, current.light_taskbar) else {
        return;
    };
    let mut data = icon_data(window);
    data.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP | NIF_SHOWTIP;
    data.uCallbackMessage = ICON_MESSAGE;
    data.hIcon = icon;
    for (place, unit) in data
        .szTip
        .iter_mut()
        .zip(current.tip.encode_utf16().take(127))
    {
        *place = unit;
    }
    // SAFETY: `data` is a valid, fully initialised description of the icon, whose handle stays
    // alive until the taskbar has been given the next one.
    #[allow(unsafe_code)]
    let shown = unsafe {
        if current.added {
            Shell_NotifyIconW(NIM_MODIFY, &data) != 0
        } else if Shell_NotifyIconW(NIM_ADD, &data) != 0 {
            data.Anonymous.uVersion = NOTIFYICON_VERSION_4;
            Shell_NotifyIconW(NIM_SETVERSION, &data);
            true
        } else {
            // No taskbar yet; it announces itself when it arrives.
            false
        }
    };
    current.added = shown;
    let previous = std::mem::replace(&mut current.icon, icon as usize);
    if previous != 0 {
        // SAFETY: the previous icon was made by `make_icon` and the taskbar no longer uses it.
        #[allow(unsafe_code)]
        unsafe {
            DestroyIcon(previous as HICON)
        };
    }
}

/// The icon's identity as the taskbar knows it: this window, icon number 1.
fn icon_data(window: HWND) -> NOTIFYICONDATAW {
    NOTIFYICONDATAW {
        cbSize: size_of::<NOTIFYICONDATAW>() as u32,
        hWnd: window,
        uID: ICON_ID,
        ..NOTIFYICONDATAW::default()
    }
}

/// Turns the picture for `state` into an icon at the size the taskbar uses.
fn make_icon(state: TrayState, light_taskbar: bool) -> Option<HICON> {
    // SAFETY: both calls only read system settings.
    #[allow(unsafe_code)]
    let size = unsafe { GetSystemMetricsForDpi(SM_CXSMICON, GetDpiForSystem()) };
    let size = u32::try_from(size).ok().filter(|&size| size > 0)?;
    let pixels = picture::draw(state, light_taskbar, size);
    let colour: Vec<u8> = pixels
        .iter()
        .flat_map(|pixel| pixel.to_le_bytes())
        .collect();
    // The mask marks transparent pixels; each row is padded to a whole number of 16-bit words.
    let row_bytes = size.div_ceil(16) as usize * 2;
    let mut mask = vec![0u8; row_bytes * size as usize];
    for (index, pixel) in pixels.iter().enumerate() {
        if pixel >> 24 == 0 {
            let (row, column) = (index / size as usize, index % size as usize);
            mask[row * row_bytes + column / 8] |= 0x80 >> (column % 8);
        }
    }
    let side = i32::try_from(size).ok()?;
    // SAFETY: both buffers hold exactly `size` rows in the layout the call expects: 32-bit colour
    // pixels and a padded 1-bit mask. They outlive the call, which copies them.
    #[allow(unsafe_code)]
    let icon = unsafe {
        CreateIcon(
            ptr::null_mut(),
            side,
            side,
            1,
            32,
            mask.as_ptr(),
            colour.as_ptr(),
        )
    };
    (!icon.is_null()).then_some(icon)
}

/// Whether a settings broadcast is about how apps look or move: light and dark and the accent
/// color, a contrast theme, or animation effects.
fn is_look_change(wparam: WPARAM, lparam: LPARAM) -> bool {
    if matches!(
        u32::try_from(wparam),
        Ok(SPI_SETHIGHCONTRAST | SPI_SETCLIENTAREAANIMATION)
    ) {
        return true;
    }
    if lparam == 0 {
        return false;
    }
    let expected = wide("ImmersiveColorSet");
    let text = lparam as *const u16;
    // SAFETY: for WM_SETTINGCHANGE, a non-zero `lparam` points to a NUL-terminated UTF-16 string.
    // Reading stops at the first difference or at the expected string's NUL, so it never reads
    // past the end of either.
    #[allow(unsafe_code)]
    expected
        .iter()
        .enumerate()
        .all(|(index, &unit)| unsafe { *text.add(index) } == unit)
}

fn lock(shared: &Shared) -> MutexGuard<'_, Current> {
    shared
        .current
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}
