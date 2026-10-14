//! Tying the processes Gracil starts to Gracil's own life (ADR 0018), and keeping them within
//! limits Windows itself enforces (ADR 0049).
//!
//! Gracil puts itself in a Windows job object that ends every process in it when the job closes.
//! Windows closes the job when Gracil's process ends, however it ends, so an FFmpeg it started can
//! never keep running, or keep a temporary file in use, after Gracil is gone.
//!
//! The same job holds Gracil and everything it starts to some of the processor's threads, and
//! keeps all of them together from taking most of the computer's memory. The tools in it can also
//! be put in Windows' efficiency mode (EcoQoS), as Task Manager does, and their memory is the
//! first Windows gives back when others need it (ADR 0063).

use std::io;
use std::mem;
use std::ptr;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};

use windows_sys::Win32::Foundation::CloseHandle;
use windows_sys::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_AFFINITY,
    JOB_OBJECT_LIMIT_JOB_MEMORY, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    JOBOBJECT_BASIC_PROCESS_ID_LIST, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
    JobObjectBasicProcessIdList, JobObjectExtendedLimitInformation, QueryInformationJobObject,
    SetInformationJobObject,
};
use windows_sys::Win32::System::Threading::{
    GetCurrentProcess, GetCurrentProcessId, GetProcessAffinityMask, MEMORY_PRIORITY_INFORMATION,
    MEMORY_PRIORITY_LOW, OpenProcess, PROCESS_POWER_THROTTLING_CURRENT_VERSION,
    PROCESS_POWER_THROTTLING_EXECUTION_SPEED, PROCESS_POWER_THROTTLING_STATE,
    PROCESS_SET_INFORMATION, ProcessMemoryPriority, ProcessPowerThrottling, SetProcessInformation,
};

/// The share of the computer's memory Gracil and every tool it runs may hold together, in
/// hundredths (ADR 0063). Past it, a process is refused more memory, and its file fails quietly,
/// instead of pushing the whole computer into the swap file.
const MEMORY_PERCENT: u64 = 65;

/// Whether the tools should run in efficiency mode, for those started from now on.
static EFFICIENT: AtomicBool = AtomicBool::new(false);

/// Gracil's job, once made.
struct Job {
    handle: usize,
    /// The processor threads Gracil was allowed when it started.
    threads: usize,
    /// The most memory the whole job may hold, if the computer's memory is known.
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
            .and_then(|total| usize::try_from(total / 100 * MEMORY_PERCENT).ok()),
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

/// How many processor threads Gracil was allowed when it started, as Windows counts them for this
/// process, once [`end_children_with_this_process`] has run. Unlike Rust's own count, it keeps to
/// the threads the process may really use.
pub fn usable_threads() -> Option<usize> {
    JOB.get()
        .map(|job| job.threads.count_ones() as usize)
        .filter(|&count| count > 0)
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

/// Puts every tool Gracil runs (FFmpeg, jpegli) in Windows' efficiency mode, or lets Windows decide
/// again: in it, Windows runs them on its efficient cores and at lower clock speeds, cooler and
/// quieter but slower. Gracil's own process, which draws the window, is left alone. Applies to
/// the tools running now, and to those [`quiet_tool`] is told of later; call it only when the
/// answer changes. Does nothing before [`end_children_with_this_process`].
pub fn use_efficiency_mode(on: bool) -> io::Result<()> {
    EFFICIENT.store(on, Ordering::Relaxed);
    let Some(job) = JOB.get() else {
        return Ok(());
    };
    // SAFETY: takes no arguments.
    #[allow(unsafe_code)]
    let own = unsafe { GetCurrentProcessId() };
    for id in processes_in(job)?.into_iter().filter(|&id| id != own) {
        quiet(id, on);
    }
    Ok(())
}

/// Quiets a tool Gracil has just started, by its process id: efficiency mode as last asked of
/// [`use_efficiency_mode`], and memory Windows gives back first when others need it.
pub fn quiet_tool(id: u32) {
    quiet(id, EFFICIENT.load(Ordering::Relaxed));
}

/// Sets one process's efficiency mode, and lowers its memory priority. A process that has just
/// ended is skipped.
fn quiet(id: u32, efficient: bool) {
    let state = PROCESS_POWER_THROTTLING_STATE {
        Version: PROCESS_POWER_THROTTLING_CURRENT_VERSION,
        ControlMask: if efficient {
            PROCESS_POWER_THROTTLING_EXECUTION_SPEED
        } else {
            0
        },
        StateMask: if efficient {
            PROCESS_POWER_THROTTLING_EXECUTION_SPEED
        } else {
            0
        },
    };
    let memory = MEMORY_PRIORITY_INFORMATION {
        MemoryPriority: MEMORY_PRIORITY_LOW,
    };
    // SAFETY: asks only for the right to change the process's information; the handle is checked
    // before use and closed right after. Each pointer and size describe a local structure that
    // outlives its call, of the kind its information class takes (both documented classes of
    // `SetProcessInformation`).
    #[allow(unsafe_code)]
    unsafe {
        let process = OpenProcess(PROCESS_SET_INFORMATION, 0, id);
        if process.is_null() {
            return;
        }
        SetProcessInformation(
            process,
            ProcessPowerThrottling,
            (&raw const state).cast(),
            mem::size_of::<PROCESS_POWER_THROTTLING_STATE>() as u32,
        );
        SetProcessInformation(
            process,
            ProcessMemoryPriority,
            (&raw const memory).cast(),
            mem::size_of::<MEMORY_PRIORITY_INFORMATION>() as u32,
        );
        CloseHandle(process);
    }
}

/// The processes in the job right now, Gracil's own included.
fn processes_in(job: &Job) -> io::Result<Vec<u32>> {
    // Room for far more processes than Gracil ever runs at once. `usize` keeps the list aligned;
    // its two counts fill the first one.
    const ROOM: usize = 256;
    let mut list = vec![0_usize; 1 + ROOM];
    let size = mem::size_of_val(list.as_slice()) as u32;
    // SAFETY: the job handle is open for the life of the process; the buffer is aligned for the
    // list, `size` bytes long and outlives the call.
    #[allow(unsafe_code)]
    let read = unsafe {
        QueryInformationJobObject(
            job.handle as *mut core::ffi::c_void,
            JobObjectBasicProcessIdList,
            list.as_mut_ptr().cast(),
            size,
            ptr::null_mut(),
        )
    };
    if read == 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: Windows filled the header at the start of the aligned buffer.
    #[allow(unsafe_code)]
    let listed = unsafe {
        (*list.as_ptr().cast::<JOBOBJECT_BASIC_PROCESS_ID_LIST>()).NumberOfProcessIdsInList
    };
    Ok(list[1..]
        .iter()
        .take(listed as usize)
        .filter_map(|&id| u32::try_from(id).ok())
        .collect())
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
        limits.BasicLimitInformation.LimitFlags |= JOB_OBJECT_LIMIT_JOB_MEMORY;
        limits.JobMemoryLimit = memory;
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

    #[test]
    fn tools_can_be_put_in_efficiency_mode() {
        end_children_with_this_process().unwrap();
        let mut child = Command::new("ping")
            .args(["-n", "5", "127.0.0.1"])
            .stdout(Stdio::null())
            .spawn()
            .unwrap();
        assert!(
            processes_in(JOB.get().unwrap())
                .unwrap()
                .contains(&child.id())
        );
        use_efficiency_mode(true).unwrap();
        quiet_tool(child.id());
        use_efficiency_mode(false).unwrap();
        // One that has ended is skipped.
        quiet_tool(u32::MAX - 3);
        child.kill().unwrap();
        child.wait().unwrap();
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
