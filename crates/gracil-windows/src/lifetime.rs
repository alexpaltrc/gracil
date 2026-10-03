//! Tying the processes Gracil starts to Gracil's own life (ADR 0018).
//!
//! Gracil puts itself in a Windows job object that ends every process in it when the job closes.
//! Windows closes the job when Gracil's process ends, however it ends, so an FFmpeg it started can
//! never keep running, or keep a temporary file in use, after Gracil is gone.

use std::io;
use std::mem;
use std::ptr;

use windows_sys::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
    SetInformationJobObject,
};
use windows_sys::Win32::System::Threading::GetCurrentProcess;

/// Makes every process Gracil starts from now on end when Gracil ends. Call it once, early.
pub fn end_children_with_this_process() -> io::Result<()> {
    // SAFETY: no security attributes and no name; the result is checked before use.
    #[allow(unsafe_code)]
    let job = unsafe { CreateJobObjectW(ptr::null(), ptr::null()) };
    if job.is_null() {
        return Err(io::Error::last_os_error());
    }

    // SAFETY: the structure is plain data, for which all zeroes is a valid value.
    #[allow(unsafe_code)]
    let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = unsafe { mem::zeroed() };
    limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
    // SAFETY: the job handle was just created, and the pointer and size describe `limits`, which
    // outlives the call.
    #[allow(unsafe_code)]
    let set = unsafe {
        SetInformationJobObject(
            job,
            JobObjectExtendedLimitInformation,
            (&raw const limits).cast(),
            mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
        )
    };
    if set == 0 {
        return Err(io::Error::last_os_error());
    }

    // SAFETY: both handles are valid; the current-process pseudo handle needs no closing. The job
    // handle is deliberately never closed: Windows closes it when this process ends, and that is
    // what ends the children.
    #[allow(unsafe_code)]
    let assigned = unsafe { AssignProcessToJobObject(job, GetCurrentProcess()) };
    if assigned == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use std::io::{BufRead, BufReader};
    use std::process::{Command, Stdio};
    use std::thread::sleep;
    use std::time::Duration;

    use super::*;

    const AS_PARENT: &str = "GRACIL_TEST_JOB_PARENT";

    /// Runs this test binary again as a short-lived parent that starts a long child and exits,
    /// then checks the child ended with it.
    #[test]
    fn children_end_with_their_parent() {
        if std::env::var_os(AS_PARENT).is_some() {
            end_children_with_this_process().unwrap();
            let child = Command::new("ping")
                .args(["-n", "60", "127.0.0.1"])
                .stdout(Stdio::null())
                .spawn()
                .unwrap();
            println!("child={}", child.id());
            std::process::exit(0);
        }

        let mut parent = Command::new(std::env::current_exe().unwrap())
            .args(["lifetime::tests::children_end_with_their_parent", "--exact"])
            .args(["--nocapture", "--test-threads=1"])
            .env(AS_PARENT, "1")
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        let output = BufReader::new(parent.stdout.take().unwrap());
        let child = output
            .lines()
            .map_while(Result::ok)
            // The test harness may print its own words before it on the same line.
            .find_map(|line| {
                line.split_once("child=")
                    .map(|(_, id)| id.trim().to_owned())
            })
            .unwrap();
        parent.wait().unwrap();

        let alive = || {
            let list = Command::new("tasklist")
                .args(["/FI", &format!("PID eq {child}"), "/NH"])
                .output()
                .unwrap();
            String::from_utf8_lossy(&list.stdout).contains(&child)
        };
        for _ in 0..50 {
            if !alive() {
                return;
            }
            sleep(Duration::from_millis(100));
        }
        panic!("the child outlived its parent");
    }
}
