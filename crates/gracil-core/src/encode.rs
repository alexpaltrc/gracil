//! Running one FFmpeg encode and reporting its progress.

use std::ffi::OsString;
use std::io::{self, BufRead, BufReader, Read};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread;
use std::time::{Duration, Instant};

use tracing::{debug, warn};

use crate::plan::EncodePlan;
use crate::progress::{Progress, ProgressParser};
use crate::share;
use crate::toolchain::Toolchain;

/// Suspends (`true`) or resumes (`false`) a process by id. Provided by the Windows layer; a
/// parameter here so the core stays free of Windows code.
pub type Suspend = fn(process_id: u32, suspended: bool) -> io::Result<()>;

/// Lets another thread pause, resume or cancel a running compression. Cheap to clone.
#[derive(Debug, Clone, Default)]
pub struct JobControl(Arc<Mutex<Slot>>);

#[derive(Debug, Default)]
struct Slot {
    cancelled: bool,
    /// Set while paused, with the way to resume. A process started meanwhile starts paused.
    paused: Option<Suspend>,
    child: Option<Child>,
}

impl JobControl {
    /// Stops the current FFmpeg process, if any, and every attempt after it. Works while paused.
    pub fn cancel(&self) {
        let mut slot = self.lock();
        slot.cancelled = true;
        if let Some(child) = slot.child.as_mut()
            && let Err(error) = child.kill()
        {
            debug!(%error, "FFmpeg already exited when cancelled");
        }
    }

    pub fn is_cancelled(&self) -> bool {
        self.lock().cancelled
    }

    /// Freezes the running FFmpeg where it is (spec §47). Nothing is lost;
    /// [`Self::resume`] carries on from the same frame.
    pub fn pause(&self, suspend: Suspend) -> io::Result<()> {
        let mut slot = self.lock();
        if slot.cancelled || slot.paused.is_some() {
            return Ok(());
        }
        if let Some(child) = &slot.child {
            suspend(child.id(), true)?;
        }
        slot.paused = Some(suspend);
        Ok(())
    }

    pub fn resume(&self) -> io::Result<()> {
        let mut slot = self.lock();
        let Some(suspend) = slot.paused else {
            return Ok(());
        };
        if let Some(child) = &slot.child {
            suspend(child.id(), false)?;
        }
        slot.paused = None;
        Ok(())
    }

    pub fn is_paused(&self) -> bool {
        self.lock().paused.is_some()
    }

    fn lock(&self) -> MutexGuard<'_, Slot> {
        // The slot holds no invariant a panic could break, so a poisoned lock is still usable.
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EncodeOutcome {
    Finished,
    Cancelled,
    Failed {
        /// The end of FFmpeg's error output, for the log.
        error: String,
        /// Whether any part of the video had been encoded before it failed.
        made_progress: bool,
    },
    /// The tool went silent for longer than its patience while it was meant to be working, and
    /// was stopped (ADR 0064). Treated like a failure of that attempt.
    Stalled,
}

/// Bytes of FFmpeg's error output kept for the log.
const ERROR_TAIL: usize = 4096;

/// How long a tool may go without a word, while it is meant to be working, before it is taken as
/// stuck and stopped (ADR 0064). FFmpeg reports progress about twice a second.
pub(crate) const PATIENCE: Duration = Duration::from_secs(5 * 60);

/// The patience for measuring the quality of a video above 4K, whose samples can go far longer
/// between words on a slow computer.
pub(crate) const PATIENCE_ABOVE_4K: Duration = Duration::from_secs(10 * 60);

/// Runs `plan` to completion, calling `on_progress` after every progress block.
pub fn run(
    toolchain: &Toolchain,
    plan: &EncodePlan,
    control: &JobControl,
    on_progress: &mut dyn FnMut(Progress),
) -> EncodeOutcome {
    let mut parser = ProgressParser::default();
    let mut made_progress = false;
    let (outcome, _) = run_ffmpeg(toolchain, plan.args(), control, &mut |line| {
        if let Some(progress) = parser.feed(line) {
            made_progress |= !progress.position.is_zero();
            on_progress(progress);
        }
    });
    match outcome {
        EncodeOutcome::Failed { error, .. } => EncodeOutcome::Failed {
            error,
            made_progress,
        },
        other => other,
    }
}

/// Runs FFmpeg with `args` under `control`, handing each line of its standard output to
/// `on_line`. Also returns the end of its error output, where FFmpeg writes measurements as well
/// as errors. A failure here never counts as having made progress; [`run`] knows better.
pub(crate) fn run_ffmpeg(
    toolchain: &Toolchain,
    args: Vec<OsString>,
    control: &JobControl,
    on_line: &mut dyn FnMut(&str),
) -> (EncodeOutcome, String) {
    run_program(toolchain.ffmpeg(), args, control, on_line)
}

/// [`run_ffmpeg`] with its own patience instead of [`PATIENCE`].
pub(crate) fn run_ffmpeg_patient(
    toolchain: &Toolchain,
    args: Vec<OsString>,
    control: &JobControl,
    patience: Duration,
    on_line: &mut dyn FnMut(&str),
) -> (EncodeOutcome, String) {
    run_program_patient(toolchain.ffmpeg(), args, control, patience, on_line)
}

/// Runs `command` with `args` under `control`, so another program (jpegli) can be paused, resumed
/// and cancelled exactly like FFmpeg. Same reporting as [`run_ffmpeg`].
pub(crate) fn run_program(
    command: Command,
    args: Vec<OsString>,
    control: &JobControl,
    on_line: &mut dyn FnMut(&str),
) -> (EncodeOutcome, String) {
    run_program_patient(command, args, control, PATIENCE, on_line)
}

/// [`run_program`], stopping the program if it says nothing for `patience` while it is meant to
/// be working. Time paused, resting or asleep doesn't count.
pub(crate) fn run_program_patient(
    mut command: Command,
    args: Vec<OsString>,
    control: &JobControl,
    patience: Duration,
    on_line: &mut dyn FnMut(&str),
) -> (EncodeOutcome, String) {
    let failed = |error: String| {
        let outcome = EncodeOutcome::Failed {
            error,
            made_progress: false,
        };
        (outcome, String::new())
    };

    let mut child = match command
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(child) => child,
        Err(error) => {
            return failed(format!(
                "could not start {:?}: {error}",
                command.get_program()
            ));
        }
    };
    crate::share::tool_started(child.id());
    let (Some(stdout), Some(stderr)) = (child.stdout.take(), child.stderr.take()) else {
        let _ = child.kill();
        let _ = child.wait();
        return failed("FFmpeg pipes unavailable".into());
    };

    {
        let mut slot = control.lock();
        if slot.cancelled {
            drop(slot);
            let _ = child.kill();
            let _ = child.wait();
            return (EncodeOutcome::Cancelled, String::new());
        }
        // Paused between attempts: the next one starts paused too.
        if let Some(suspend) = slot.paused
            && let Err(error) = suspend(child.id(), true)
        {
            warn!(%error, "could not pause the next attempt");
        }
        slot.child = Some(child);
    }

    // Anything the tool says, on either output, counts as a sign of life.
    let watch = Arc::new(Watch::new());
    let (finished, done) = mpsc::channel::<()>();
    let watchdog = {
        let (watch, control) = (Arc::clone(&watch), control.clone());
        thread::Builder::new()
            .name("gracil-watch".into())
            .spawn(move || watch_over(&watch, &control, patience, &done))
            .inspect_err(|error| warn!(%error, "could not watch over a tool"))
            .ok()
    };

    // Drained on its own thread so a chatty FFmpeg never blocks on a full pipe.
    let stderr_reader = {
        let watch = Arc::clone(&watch);
        thread::spawn(move || tail(stderr, ERROR_TAIL, &mut || watch.heard()))
    };

    for line in BufReader::new(stdout).lines() {
        let Ok(line) = line else { break };
        watch.heard();
        on_line(&line);
    }

    // Stdout closes when FFmpeg exits or is killed; collect its status.
    let (status, cancelled) = {
        let mut slot = control.lock();
        let status = slot.child.take().map(|mut child| child.wait());
        (status, slot.cancelled)
    };
    let log = stderr_reader.join().unwrap_or_default();
    drop(finished);
    if let Some(watchdog) = watchdog {
        let _ = watchdog.join();
    }

    let outcome = match status {
        _ if cancelled => EncodeOutcome::Cancelled,
        _ if watch.stalled.load(Ordering::Relaxed) => {
            warn!(?patience, error = %log, "a tool stopped answering and was stopped");
            EncodeOutcome::Stalled
        }
        Some(Ok(status)) if status.success() => EncodeOutcome::Finished,
        Some(Ok(status)) => {
            warn!(%status, error = %log, "FFmpeg failed");
            EncodeOutcome::Failed {
                error: if log.is_empty() {
                    format!("FFmpeg exited with {status}")
                } else {
                    log.clone()
                },
                made_progress: false,
            }
        }
        Some(Err(error)) => return failed(format!("waiting for FFmpeg failed: {error}")),
        None => return failed("FFmpeg process lost".into()),
    };
    (outcome, log)
}

/// When a running tool was last heard from, and whether it was stopped for going silent.
struct Watch {
    last: Mutex<Instant>,
    stalled: AtomicBool,
}

impl Watch {
    fn new() -> Self {
        Self {
            last: Mutex::new(Instant::now()),
            stalled: AtomicBool::new(false),
        }
    }

    fn heard(&self) {
        *self.last.lock().unwrap_or_else(PoisonError::into_inner) = Instant::now();
    }

    fn quiet_for(&self) -> Duration {
        self.last
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .elapsed()
    }
}

/// Stops the tool under `control` once it has been silent for `patience` while meant to be
/// working, until `done` says it has ended. Paused or resting, and after a sleep, the wait starts
/// again: nothing is expected of a tool that isn't running.
fn watch_over(watch: &Watch, control: &JobControl, patience: Duration, done: &Receiver<()>) {
    let tick = (patience / 10).clamp(Duration::from_millis(10), Duration::from_secs(5));
    let mut sleeps = share::sleeps();
    loop {
        match done.recv_timeout(tick) {
            Err(RecvTimeoutError::Timeout) => {}
            Ok(()) | Err(RecvTimeoutError::Disconnected) => return,
        }
        if control.is_paused() || share::sleeps() != sleeps {
            sleeps = share::sleeps();
            watch.heard();
            continue;
        }
        if watch.quiet_for() < patience {
            continue;
        }
        let mut slot = control.lock();
        if slot.cancelled {
            return;
        }
        // Only a tool still running is stopped; one that just ended is left to report itself.
        if let Some(child) = slot.child.as_mut()
            && child.kill().is_ok()
        {
            watch.stalled.store(true, Ordering::Relaxed);
        }
        return;
    }
}

/// Reads `reader` to the end and returns its last `limit` bytes as text, calling `on_read` each
/// time something arrives.
fn tail(mut reader: impl Read, limit: usize, on_read: &mut dyn FnMut()) -> String {
    let mut kept = Vec::with_capacity(limit);
    let mut buffer = [0; 4096];
    while let Ok(read) = reader.read(&mut buffer) {
        if read == 0 {
            break;
        }
        on_read();
        kept.extend_from_slice(&buffer[..read]);
        if kept.len() > limit {
            kept.drain(..kept.len() - limit);
        }
    }
    String::from_utf8_lossy(&kept).trim().to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tail_keeps_the_end() {
        let input = "a".repeat(10_000) + "the end";
        let kept = tail(input.as_bytes(), 16, &mut || {});
        assert_eq!(kept.len(), 16);
        assert!(kept.ends_with("the end"));
    }

    #[test]
    fn cancel_before_start_is_remembered() {
        let control = JobControl::default();
        assert!(!control.is_cancelled());
        control.cancel();
        assert!(control.is_cancelled());
    }

    fn never_called(_: u32, _: bool) -> io::Result<()> {
        unreachable!("there is no process to suspend")
    }

    #[test]
    fn pausing_between_processes_is_remembered() {
        let control = JobControl::default();
        control.pause(never_called).unwrap();
        assert!(control.is_paused());
        control.resume().unwrap();
        assert!(!control.is_paused());
    }

    #[test]
    fn a_cancelled_job_does_not_pause() {
        let control = JobControl::default();
        control.cancel();
        control.pause(never_called).unwrap();
        assert!(!control.is_paused());
    }

    /// A tool that says nothing at all, for far longer than the patience.
    fn silent_tool() -> Command {
        let mut command = Command::new("powershell");
        command.args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            "Start-Sleep -Seconds 60",
        ]);
        command
    }

    #[test]
    fn a_silent_tool_is_stopped_as_stalled() {
        let control = JobControl::default();
        let started = Instant::now();
        let (outcome, _) = run_program_patient(
            silent_tool(),
            Vec::new(),
            &control,
            Duration::from_secs(2),
            &mut |_| {},
        );
        assert_eq!(outcome, EncodeOutcome::Stalled);
        assert!(started.elapsed() < Duration::from_secs(30));
    }

    fn pretend_suspend(_: u32, _: bool) -> io::Result<()> {
        Ok(())
    }

    #[test]
    fn a_paused_tool_is_never_stalled() {
        let control = JobControl::default();
        control.pause(pretend_suspend).unwrap();
        let stopper = {
            let control = control.clone();
            thread::spawn(move || {
                thread::sleep(Duration::from_secs(3));
                control.cancel();
            })
        };
        let (outcome, _) = run_program_patient(
            silent_tool(),
            Vec::new(),
            &control,
            Duration::from_millis(500),
            &mut |_| {},
        );
        stopper.join().unwrap();
        assert_eq!(outcome, EncodeOutcome::Cancelled);
    }
}
