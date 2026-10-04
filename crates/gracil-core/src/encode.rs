//! Running one FFmpeg encode and reporting its progress.

use std::ffi::OsString;
use std::io::{self, BufRead, BufReader, Read};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread;

use tracing::{debug, warn};

use crate::plan::EncodePlan;
use crate::progress::{Progress, ProgressParser};
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

    /// Freezes the running FFmpeg where it is (spec §47). Nothing is lost; [`Self::resume`] carries
    /// on from the same frame.
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
}

/// Bytes of FFmpeg's error output kept for the log.
const ERROR_TAIL: usize = 4096;

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

/// Runs `command` with `args` under `control`, so another program (jpegli) can be paused, resumed
/// and cancelled exactly like FFmpeg. Same reporting as [`run_ffmpeg`].
pub(crate) fn run_program(
    mut command: Command,
    args: Vec<OsString>,
    control: &JobControl,
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

    // Drained on its own thread so a chatty FFmpeg never blocks on a full pipe.
    let stderr_reader = thread::spawn(move || tail(stderr, ERROR_TAIL));

    for line in BufReader::new(stdout).lines() {
        let Ok(line) = line else { break };
        on_line(&line);
    }

    // Stdout closes when FFmpeg exits or is killed; collect its status.
    let (status, cancelled) = {
        let mut slot = control.lock();
        let status = slot.child.take().map(|mut child| child.wait());
        (status, slot.cancelled)
    };
    let log = stderr_reader.join().unwrap_or_default();

    let outcome = match status {
        _ if cancelled => EncodeOutcome::Cancelled,
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

/// Reads `reader` to the end and returns its last `limit` bytes as text.
fn tail(mut reader: impl Read, limit: usize) -> String {
    let mut kept = Vec::with_capacity(limit);
    let mut buffer = [0; 4096];
    while let Ok(read) = reader.read(&mut buffer) {
        if read == 0 {
            break;
        }
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
        let kept = tail(input.as_bytes(), 16);
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
}
