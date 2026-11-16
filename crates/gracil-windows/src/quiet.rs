//! Working without getting in the way (spec §77.3, ADR 0040): Gracil asks Windows for less than
//! other programs get, and looks at whether the person is using the computer and how much memory
//! is free before taking on more at once, whether it runs on a battery, and how much room is left
//! on a drive.

use std::ffi::{OsString, c_void};
use std::io;
use std::mem;
use std::os::windows::ffi::OsStringExt;
use std::path::{Path, PathBuf};
use std::ptr;
use std::sync::OnceLock;
use std::time::Duration;

use windows_sys::Win32::Storage::FileSystem::{GetDiskFreeSpaceExW, GetVolumePathNameW};
use windows_sys::Win32::System::Power::{
    CallNtPowerInformation, DEVICE_NOTIFY_SUBSCRIBE_PARAMETERS, ES_DISPLAY_REQUIRED,
    EXECUTION_STATE, GetSystemPowerStatus, PowerRegisterSuspendResumeNotification,
    SYSTEM_POWER_STATUS, SystemExecutionState,
};
use windows_sys::Win32::System::ProcessStatus::{GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS_EX};

use windows_sys::Win32::System::SystemInformation::{
    GetTickCount, GlobalMemoryStatusEx, MEMORYSTATUSEX,
};
use windows_sys::Win32::System::Threading::{
    BELOW_NORMAL_PRIORITY_CLASS, GetCurrentProcess, GetCurrentThread, SetPriorityClass,
    SetThreadPriority, THREAD_MODE_BACKGROUND_BEGIN, THREAD_PRIORITY_HIGHEST,
};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO};
use windows_sys::Win32::UI::Shell::{
    QUERY_USER_NOTIFICATION_STATE, QUNS_BUSY, QUNS_PRESENTATION_MODE, QUNS_RUNNING_D3D_FULL_SCREEN,
    SHQueryUserNotificationState,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    DEVICE_NOTIFY_CALLBACK, PBT_APMRESUMEAUTOMATIC, PBT_APMRESUMESUSPEND, PBT_APMSUSPEND,
};

/// Lowers this process below other programs, so its work, and the tools it starts, run on what
/// they leave free. The calling thread, the one that draws the window, is raised back to an
/// ordinary program's level, so the window answers as quickly as before.
pub fn work_quietly() {
    // SAFETY: both pseudo-handles stand for this process and thread, need no closing, and the
    // calls only change their scheduling priority.
    #[allow(unsafe_code)]
    unsafe {
        SetPriorityClass(GetCurrentProcess(), BELOW_NORMAL_PRIORITY_CLASS);
        SetThreadPriority(GetCurrentThread(), THREAD_PRIORITY_HIGHEST);
    }
}

/// How long since the person last touched the keyboard or mouse, if Windows says.
pub fn idle_for() -> Option<Duration> {
    let mut info = LASTINPUTINFO {
        cbSize: mem::size_of::<LASTINPUTINFO>() as u32,
        dwTime: 0,
    };
    // SAFETY: `info` is a valid structure with its size filled in, as the call requires.
    #[allow(unsafe_code)]
    let read = unsafe { GetLastInputInfo(&mut info) };
    if read == 0 {
        return None;
    }
    // SAFETY: takes no arguments.
    #[allow(unsafe_code)]
    let now = unsafe { GetTickCount() };
    // Both counters wrap after 49.7 days; the difference is right across the wrap.
    Some(Duration::from_millis(u64::from(
        now.wrapping_sub(info.dwTime),
    )))
}

/// Memory free for programs to use right now, in bytes, if Windows says.
pub fn free_memory() -> Option<u64> {
    memory_status().map(|status| status.ullAvailPhys)
}

/// All the memory in the computer, in bytes, if Windows says.
pub fn total_memory() -> Option<u64> {
    memory_status().map(|status| status.ullTotalPhys)
}

fn memory_status() -> Option<MEMORYSTATUSEX> {
    // SAFETY: a plain structure of integers, valid when zeroed.
    #[allow(unsafe_code)]
    let mut status: MEMORYSTATUSEX = unsafe { mem::zeroed() };
    status.dwLength = mem::size_of::<MEMORYSTATUSEX>() as u32;
    // SAFETY: `status` is a valid structure with its length filled in, as the call requires.
    #[allow(unsafe_code)]
    let read = unsafe { GlobalMemoryStatusEx(&mut status) };
    (read != 0).then_some(status)
}

/// Memory Gracil itself holds right now (its private bytes), in bytes, if Windows says.
pub fn own_memory() -> Option<u64> {
    // SAFETY: a plain structure of integers, valid when zeroed.
    #[allow(unsafe_code)]
    let mut counters: PROCESS_MEMORY_COUNTERS_EX = unsafe { mem::zeroed() };
    let size = mem::size_of::<PROCESS_MEMORY_COUNTERS_EX>() as u32;
    counters.cb = size;
    // SAFETY: the pseudo-handle stands for this process; the pointer and size describe
    // `counters`, which outlives the call.
    #[allow(unsafe_code)]
    let read =
        unsafe { GetProcessMemoryInfo(GetCurrentProcess(), (&raw mut counters).cast(), size) };
    (read != 0).then_some(counters.PrivateUsage as u64)
}

/// The person is busy with something that doesn't touch the keyboard or mouse (ADR 0063): a game
/// or a film filling the screen, a presentation, Windows' "do not disturb" for a full-screen app,
/// or a program keeping the screen on, as video players and video calls do.
pub fn person_busy() -> bool {
    let mut state: QUERY_USER_NOTIFICATION_STATE = 0;
    // SAFETY: `state` is a valid place for the answer and outlives the call.
    #[allow(unsafe_code)]
    let read = unsafe { SHQueryUserNotificationState(&mut state) };
    let full_screen = read >= 0
        && matches!(
            state,
            QUNS_BUSY | QUNS_RUNNING_D3D_FULL_SCREEN | QUNS_PRESENTATION_MODE
        );
    full_screen || screen_kept_on()
}

/// Whether some program asks Windows to keep the screen on right now. Gracil itself never does.
fn screen_kept_on() -> bool {
    let mut state: EXECUTION_STATE = 0;
    // SAFETY: this level takes no input and answers with one `EXECUTION_STATE` in `state`, which
    // outlives the call; the sizes match the buffers.
    #[allow(unsafe_code)]
    let status = unsafe {
        CallNtPowerInformation(
            SystemExecutionState,
            ptr::null(),
            0,
            (&raw mut state).cast(),
            mem::size_of::<EXECUTION_STATE>() as u32,
        )
    };
    status == 0 && state & ES_DISPLAY_REQUIRED != 0
}

/// How the computer is powered right now.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Power {
    /// It runs on its battery, or Windows is saving energy (battery saver).
    pub saving_energy: bool,
    /// It runs on its battery, unplugged.
    pub on_battery: bool,
    /// It has a battery at all: a laptop or a tablet, whose cooling is usually modest.
    pub has_battery: bool,
    /// How full the battery is, in percent, when Windows says.
    pub battery_percent: Option<u8>,
}

/// How the computer is powered, as far as Windows says. Without an answer, as a computer that is
/// plugged in with no battery.
pub fn power() -> Power {
    // SAFETY: a plain structure of integers, valid when zeroed.
    #[allow(unsafe_code)]
    let mut status: SYSTEM_POWER_STATUS = unsafe { mem::zeroed() };
    // SAFETY: `status` is a valid structure for the answer.
    #[allow(unsafe_code)]
    let read = unsafe { GetSystemPowerStatus(&mut status) };
    if read == 0 {
        return Power::default();
    }
    // 0 means unplugged; 1 in the flag means energy saver (battery saver) is on. 128 means no
    // battery, and 255 that Windows doesn't know.
    let on_battery = status.ACLineStatus == 0;
    Power {
        saving_energy: on_battery || status.SystemStatusFlag == 1,
        on_battery,
        has_battery: status.BatteryFlag != 128 && status.BatteryFlag != 255,
        battery_percent: (status.BatteryLifePercent <= 100).then_some(status.BatteryLifePercent),
    }
}

/// The computer is running on its battery, or Windows is saving energy, if Windows says.
pub fn saving_energy() -> bool {
    power().saving_energy
}

/// What [`on_sleep`] calls.
static ON_SLEEP: OnceLock<fn(bool)> = OnceLock::new();

/// Calls `changed` with `true` as the computer goes to sleep or hibernates, and with `false` once
/// it has woken up again. Call it once; it lasts as long as Gracil runs. It is called on a thread
/// of Windows', and must return at once.
pub fn on_sleep(changed: fn(bool)) -> io::Result<()> {
    if ON_SLEEP.set(changed).is_err() {
        return Ok(());
    }
    // Kept for as long as Gracil runs, as Windows requires while it may call.
    let parameters = Box::leak(Box::new(DEVICE_NOTIFY_SUBSCRIBE_PARAMETERS {
        Callback: Some(sleep_changed),
        Context: ptr::null_mut(),
    }));
    let mut registration = ptr::null_mut();
    // SAFETY: with `DEVICE_NOTIFY_CALLBACK`, the recipient is the parameters, which are leaked so
    // they outlive every call; the registration handle goes in a local and is never closed.
    #[allow(unsafe_code)]
    let error = unsafe {
        PowerRegisterSuspendResumeNotification(
            DEVICE_NOTIFY_CALLBACK,
            (&raw mut *parameters).cast(),
            &mut registration,
        )
    };
    if error != 0 {
        return Err(io::Error::from_raw_os_error(error as i32));
    }
    Ok(())
}

#[allow(unsafe_code)]
unsafe extern "system" fn sleep_changed(
    _context: *const c_void,
    kind: u32,
    _setting: *const c_void,
) -> u32 {
    if let Some(changed) = ON_SLEEP.get() {
        match kind {
            PBT_APMSUSPEND => changed(true),
            PBT_APMRESUMEAUTOMATIC | PBT_APMRESUMESUSPEND => changed(false),
            _ => {}
        }
    }
    0
}

/// Lowers the calling thread to Windows' background mode: its processor time, its reading and
/// writing of files and its memory come after everything else's. For threads doing Gracil's own
/// heavy work, such as images; never the window's. It lasts as long as the thread.
pub fn work_in_background() {
    // SAFETY: the pseudo-handle stands for the calling thread, as this mode requires; the call
    // only changes its scheduling.
    #[allow(unsafe_code)]
    unsafe {
        SetThreadPriority(GetCurrentThread(), THREAD_MODE_BACKGROUND_BEGIN);
    }
}

/// The drive (its root, or the folder it is mounted at) that holds `path`, if Windows says.
pub fn volume_of(path: &Path) -> Option<PathBuf> {
    let wide = crate::wide_path(path);
    let mut volume = vec![0_u16; wide.len().max(MAX_VOLUME_PATH)];
    // SAFETY: the path is NUL-terminated and outlives the call; the answer buffer is writable and
    // as long as the length given.
    #[allow(unsafe_code)]
    let read =
        unsafe { GetVolumePathNameW(wide.as_ptr(), volume.as_mut_ptr(), volume.len() as u32) };
    if read == 0 {
        return None;
    }
    let end = volume.iter().position(|&c| c == 0).unwrap_or(volume.len());
    Some(PathBuf::from(OsString::from_wide(&volume[..end])))
}

/// Room for the name of a drive's root or mount folder, in UTF-16 units.
const MAX_VOLUME_PATH: usize = 1024;

/// Space free on the drive that holds `folder`, in bytes, if Windows says.
pub fn free_space(folder: &Path) -> Option<u64> {
    let wide = crate::wide_path(folder);
    let mut free = 0_u64;
    // SAFETY: the path is a NUL-terminated UTF-16 buffer that outlives the call; the answer goes
    // in `free`, and the other two answers aren't wanted.
    #[allow(unsafe_code)]
    let read =
        unsafe { GetDiskFreeSpaceExW(wide.as_ptr(), &mut free, ptr::null_mut(), ptr::null_mut()) };
    (read != 0).then_some(free)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_answers() {
        assert!(idle_for().is_some());
        assert!(free_memory().is_some_and(|free| free > 0));
        assert!(total_memory() >= free_memory());
        assert!(own_memory().is_some_and(|own| own > 0));
        assert!(free_space(&std::env::temp_dir()).is_some_and(|free| free > 0));
        assert!(free_space(Path::new(r"Q:\no\such\folder")).is_none());
        let power = power();
        assert!(power.battery_percent.is_none_or(|percent| percent <= 100));
        assert!(!power.on_battery || power.saving_energy);
    }

    #[test]
    fn drives_are_found() {
        let temp = std::env::temp_dir();
        let volume = volume_of(&temp).unwrap_or_default();
        assert!(temp.starts_with(&volume), "{volume:?}");
        assert_eq!(volume_of(&temp), volume_of(&temp.join("no such file")));
    }

    #[test]
    fn a_background_thread_still_runs() {
        let done = std::thread::spawn(|| {
            work_in_background();
            (0..1000_u64).sum::<u64>()
        })
        .join();
        assert_eq!(done.ok(), Some(499_500));
    }
}
