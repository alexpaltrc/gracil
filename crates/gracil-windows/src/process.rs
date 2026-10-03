//! Pausing a running FFmpeg by suspending its threads (spec §47, ADR 0013).
//!
//! Windows has no documented "suspend process" call. The documented way, which this follows, is to
//! suspend every thread of the process. FFmpeg stays alive with all its work in memory, and
//! resuming lets it carry on exactly where it stopped.

use std::collections::HashSet;
use std::io;
use std::mem;

use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE};
use windows_sys::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, TH32CS_SNAPTHREAD, THREADENTRY32, Thread32First, Thread32Next,
};
use windows_sys::Win32::System::Threading::{
    OpenThread, ResumeThread, SuspendThread, THREAD_SUSPEND_RESUME,
};

/// Suspends (`true`) or resumes (`false`) every thread of the process `process_id`.
///
/// Only pass the id of a process Gracil started and still owns, and pair each suspension with one
/// resumption. A process can be killed while suspended.
pub fn set_suspended(process_id: u32, suspended: bool) -> io::Result<()> {
    if suspended {
        suspend(process_id)
    } else {
        for thread in threads_of(process_id)? {
            // A thread that ended since the snapshot needs nothing.
            if let Some(handle) = open(thread) {
                handle.resume();
            }
        }
        Ok(())
    }
}

fn suspend(process_id: u32) -> io::Result<()> {
    let mut suspended: Vec<Thread> = Vec::new();
    let mut seen = HashSet::new();
    // A thread can start while the others are being suspended; look again until none is new.
    loop {
        let mut found_new = false;
        for thread in threads_of(process_id)? {
            if !seen.insert(thread) {
                continue;
            }
            found_new = true;
            let Some(handle) = open(thread) else { continue };
            if let Err(error) = handle.suspend() {
                // Never leave FFmpeg half-frozen.
                for handle in &suspended {
                    handle.resume();
                }
                return Err(error);
            }
            suspended.push(handle);
        }
        if !found_new {
            return Ok(());
        }
    }
}

/// The ids of the threads currently in `process_id`.
fn threads_of(process_id: u32) -> io::Result<Vec<u32>> {
    // SAFETY: plain call; the result is checked before use and closed by `Owned`.
    #[allow(unsafe_code)]
    let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0) };
    if snapshot == INVALID_HANDLE_VALUE {
        return Err(io::Error::last_os_error());
    }
    let snapshot = Owned(snapshot);

    // SAFETY: THREADENTRY32 is plain data, for which all zeroes is a valid value.
    #[allow(unsafe_code)]
    let mut entry: THREADENTRY32 = unsafe { mem::zeroed() };
    entry.dwSize = mem::size_of::<THREADENTRY32>() as u32;

    let mut threads = Vec::new();
    // SAFETY: the snapshot handle is valid and `entry` is a correctly sized, writable entry.
    #[allow(unsafe_code)]
    let mut more = unsafe { Thread32First(snapshot.0, &mut entry) } != 0;
    while more {
        if entry.th32OwnerProcessID == process_id {
            threads.push(entry.th32ThreadID);
        }
        // SAFETY: as above.
        #[allow(unsafe_code)]
        let next = unsafe { Thread32Next(snapshot.0, &mut entry) };
        more = next != 0;
    }
    Ok(threads)
}

/// A thread opened for suspending and resuming, or `None` if it has ended.
fn open(thread_id: u32) -> Option<Thread> {
    // SAFETY: plain call; a null result means the thread could not be opened.
    #[allow(unsafe_code)]
    let handle = unsafe { OpenThread(THREAD_SUSPEND_RESUME, 0, thread_id) };
    (!handle.is_null()).then(|| Thread(Owned(handle)))
}

struct Thread(Owned);

impl Thread {
    fn suspend(&self) -> io::Result<()> {
        // SAFETY: the handle is open with THREAD_SUSPEND_RESUME access.
        #[allow(unsafe_code)]
        let previous = unsafe { SuspendThread(self.0.0) };
        if previous == u32::MAX {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }

    fn resume(&self) {
        // SAFETY: the handle is open with THREAD_SUSPEND_RESUME access. A failure means the thread
        // is gone, which needs nothing more.
        #[allow(unsafe_code)]
        unsafe {
            ResumeThread(self.0.0);
        }
    }
}

/// A Windows handle closed when dropped.
struct Owned(HANDLE);

impl Drop for Owned {
    fn drop(&mut self) {
        // SAFETY: the handle was returned open by Windows and is closed exactly once, here.
        #[allow(unsafe_code)]
        unsafe {
            CloseHandle(self.0);
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use std::process::{Child, Command, Stdio};
    use std::thread::sleep;
    use std::time::Duration;

    use super::*;

    /// A process that exits on its own after about a second.
    fn short_process() -> Child {
        Command::new("ping")
            .args(["-n", "2", "127.0.0.1"])
            .stdout(Stdio::null())
            .spawn()
            .unwrap()
    }

    #[test]
    fn a_suspended_process_waits_and_then_finishes() {
        let mut child = short_process();
        set_suspended(child.id(), true).unwrap();
        sleep(Duration::from_millis(2_500));
        let still_running = child.try_wait().unwrap().is_none();

        set_suspended(child.id(), false).unwrap();
        let status = child.wait().unwrap();

        assert!(still_running, "it would have finished if it were running");
        assert!(status.success());
    }

    #[test]
    fn a_suspended_process_can_still_be_stopped() {
        let mut child = short_process();
        set_suspended(child.id(), true).unwrap();
        child.kill().unwrap();
        let status = child.wait().unwrap();
        assert!(!status.success());
    }
}
