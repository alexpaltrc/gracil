//! Working without getting in the way (spec §77.3, ADR 0040): Gracil asks Windows for less than
//! other programs get, and looks at whether the person is using the computer and how much memory
//! is free before taking on more at once, whether it runs on a battery, and how much room is left
//! on a drive.

use std::mem;
use std::path::Path;
use std::ptr;
use std::time::Duration;

use windows_sys::Win32::Storage::FileSystem::GetDiskFreeSpaceExW;
use windows_sys::Win32::System::Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS};
use windows_sys::Win32::System::ProcessStatus::{GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS_EX};

use windows_sys::Win32::System::SystemInformation::{
    GetTickCount, GlobalMemoryStatusEx, MEMORYSTATUSEX,
};
use windows_sys::Win32::System::Threading::{
    BELOW_NORMAL_PRIORITY_CLASS, GetCurrentProcess, GetCurrentThread, SetPriorityClass,
    SetThreadPriority, THREAD_PRIORITY_HIGHEST,
};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO};

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

/// The computer is running on its battery, or Windows is saving energy, if Windows says.
pub fn saving_energy() -> bool {
    // SAFETY: a plain structure of integers, valid when zeroed.
    #[allow(unsafe_code)]
    let mut status: SYSTEM_POWER_STATUS = unsafe { mem::zeroed() };
    // SAFETY: `status` is a valid structure for the answer.
    #[allow(unsafe_code)]
    let read = unsafe { GetSystemPowerStatus(&mut status) };
    // 0 means unplugged; 1 in the flag means energy saver (battery saver) is on.
    read != 0 && (status.ACLineStatus == 0 || status.SystemStatusFlag == 1)
}

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
        assert!(free_space(Path::new(r"Q:\no\such\folder")).is_none() || cfg!(any()));
    }
}
