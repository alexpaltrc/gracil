//! Tying the processes Gracil starts to Gracil's own life (ADR 0018), and keeping them within
//! limits Windows itself enforces (ADR 0049).
//!
//! Gracil puts itself in a Windows job object that ends every process in it when the job closes.
//! Windows closes the job when Gracil's process ends, however it ends, so an FFmpeg it started can
//! never keep running, or keep a temporary file in use, after Gracil is gone.
//!
//! The same job holds Gracil and everything it starts to some of the processor's threads, and
//! keeps any one of them from taking most of the computer's memory.

use std::io;
use std::mem;
use std::ptr;
use std::sync::OnceLock;

use windows_sys::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_AFFINITY,
    JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE, JOB_OBJECT_LIMIT_PROCESS_MEMORY,
    JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
    SetInformationJobObject,
};
use windows_sys::Win32::System::Threading::{GetCurrentProcess, GetProcessAffinityMask};

/// The share of the computer's memory any one process in the job may hold. Past it, the process
/// is refused more memory instead of pushing the whole computer into the swap file.
const MEMORY_SHARE: u64 = 4;
const MEMORY_SHARE_OF: u64 = 3;

/// Gracil's job, once made.
struct Job {
    handle: usize,
    /// The processor threads Gracil was allowed when it started.
    threads: usize,
    /// The most memory one process may hold, if the computer's memory is known.
    memory: Option<usize>,
}

static JOB: OnceLock<Job> = OnceLock::new();

/// Makes every process Gracil starts from now on end when Gracil ends. Call it once, early.
pub fn end_children_with_this_process() -> io::Result<()> {
    // SAFETY: no security attributes and no name; the result is checked before use.
    #[allow(unsafe_code)]
    let handle = unsafe { CreateJobObjectW(ptr::null(), ptr::null()) };
    if handle.is_null() {
        return Err(io::Error::last_os_error());
    }
    let (mut process, mut system) = (0, 0);
    // SAFETY: the pseudo-handle stands for this process; both answers go in local integers.
    #[allow(unsafe_code)]
    let read = unsafe { GetProcessAffinityMask(GetCurrentProcess(), &mut process, &mut system) };
    let job = Job {
        handle: handle as usize,
        threads: if read != 0 { process } else { 0 },
        memory: crate::total_memory()
            .and_then(|total| usize::try_from(total / MEMORY_SHARE * MEMORY_SHARE_OF).ok()),
    };
    limit(&job, None)?;

    // SAFETY: both handles are valid; the current-process pseudo handle needs no closing. The job
    // handle is deliberately never closed: Windows closes it when this process ends, and that is
    // what ends the children.
    #[allow(unsafe_code)]
    let assigned = unsafe { AssignProcessToJobObject(handle, GetCurrentProcess()) };
    if assigned == 0 {
        return Err(io::Error::last_os_error());
    }
    let _ = JOB.set(job);
    Ok(())
}

/// Lets Gracil and everything it starts run on `count` of the processor's threads at most, the
/// last ones (on processors with efficient cores, those are usually them; the first are the
/// ones Windows and other programs lean on most). Takes effect at once, also for tools already
/// running. Does nothing before [`end_children_with_this_process`].
pub fn use_processors(count: usize) -> io::Result<()> {
    let Some(job) = JOB.get() else {
        return Ok(());
    };
    limit(job, Some(last_threads(job.threads, count)))
}

/// The last `count` threads of `threads`, a set of processor threads as Windows writes it (one
/// bit each). At least one; all of them when `count` is as many or more.
fn last_threads(threads: usize, count: usize) -> usize {
    let mut chosen = 0;
    let mut left = count.max(1);
    for bit in (0..usize::BITS).rev() {
        let thread = 1 << bit;
        if left > 0 && threads & thread != 0 {
            chosen |= thread;
            left -= 1;
        }
    }
    chosen
}

fn limit(job: &Job, threads: Option<usize>) -> io::Result<()> {
    // SAFETY: the structure is plain data, for which all zeroes is a valid value.
    #[allow(unsafe_code)]
    let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = unsafe { mem::zeroed() };
    limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
    if let Some(memory) = job.memory {
        limits.BasicLimitInformation.LimitFlags |= JOB_OBJECT_LIMIT_PROCESS_MEMORY;
        limits.ProcessMemoryLimit = memory;
    }
    if let Some(threads) = threads.filter(|&threads| threads != 0) {
        limits.BasicLimitInformation.LimitFlags |= JOB_OBJECT_LIMIT_AFFINITY;
        limits.BasicLimitInformation.Affinity = threads;
    }
    // SAFETY: the job handle is open for the life of the process, and the pointer and size
    // describe `limits`, which outlives the call.
    #[allow(unsafe_code)]
    let set = unsafe {
        SetInformationJobObject(
            job.handle as *mut core::ffi::c_void,
            JobObjectExtendedLimitInformation,
            (&raw const limits).cast(),
            mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
        )
    };
    if set == 0 {
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

    #[test]
    fn the_last_threads_are_chosen() {
        assert_eq!(last_threads(0b1111_1111, 4), 0b1111_0000);
        assert_eq!(last_threads(0b1111_1111, 0), 0b1000_0000);
        assert_eq!(last_threads(0b1111, 9), 0b1111);
        // Only threads Gracil may use.
        assert_eq!(last_threads(0b1010_1010, 2), 0b1010_0000);
    }

    #[test]
    fn gracil_keeps_to_the_threads_it_is_given() {
        end_children_with_this_process().unwrap();
        use_processors(1).unwrap();
        let (mut process, mut system) = (0, 0);
        // SAFETY: as in the code under test.
        #[allow(unsafe_code)]
        let read =
            unsafe { GetProcessAffinityMask(GetCurrentProcess(), &mut process, &mut system) };
        assert_ne!(read, 0);
        assert_eq!(process.count_ones(), 1);
        use_processors(usize::MAX).unwrap();
    }

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
