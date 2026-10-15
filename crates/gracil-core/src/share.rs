//! How much of the computer one piece of work may use (spec §77.3, ADR 0040, ADR 0063): never all
//! of the processor, so the computer stays quiet and other programs stay quick. The tools Gracil
//! starts also run below other programs, so they only take what is left free.
//!
//! The engine knows nothing of Windows. Whoever runs it tells it, through [`set_host`], how to
//! quiet the tools and threads it starts and how much memory is free, and, through
//! [`allow_threads`], how many processor threads Gracil may use right now.

use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Mutex, Once, OnceLock, PoisonError};
use std::thread;
use std::time::{Duration, Instant};

/// What the program running the engine does for it. Each is optional in effect: without a host,
/// nothing is quieted and free memory is unknown.
#[derive(Debug, Clone, Copy)]
pub struct Host {
    /// Called with the process id of each tool just started (FFmpeg, jpegli), to quiet it.
    pub tool_started: fn(u32),
    /// Called on each thread the engine starts for its own heavy work, from that thread.
    pub worker_started: fn(),
    /// The memory free right now, in bytes, if known.
    pub free_memory: fn() -> Option<u64>,
}

static HOST: OnceLock<Host> = OnceLock::new();

/// Tells the engine what its host does for it. Call it once, early; later calls are ignored.
pub fn set_host(host: Host) {
    let _ = HOST.set(host);
}

/// Quiets a tool the engine has just started.
pub(crate) fn tool_started(id: u32) {
    if let Some(host) = HOST.get() {
        (host.tool_started)(id);
    }
}

/// Quiets the calling thread, one the engine started for its own heavy work.
pub(crate) fn worker_started() {
    if let Some(host) = HOST.get() {
        (host.worker_started)();
    }
}

/// The memory free right now, in bytes, if the host says.
pub fn free_memory() -> Option<u64> {
    HOST.get().and_then(|host| (host.free_memory)())
}

/// The processor threads Gracil may use right now, as last told; 0 until told.
static ALLOWED: AtomicUsize = AtomicUsize::new(0);

/// Tells the engine Gracil may use `count` processor threads right now. Tools started from now on
/// are sized for it.
pub fn allow_threads(count: usize) {
    ALLOWED.store(count, Ordering::Relaxed);
}

/// Processor threads in this computer. Rust's own count on Windows is of the processors in one
/// group, whatever this process is allowed, so it is only the most there could be.
pub fn processor_threads() -> usize {
    thread::available_parallelism().map_or(4, usize::from)
}

/// Threads one tool, such as the video encoder or the quality measure, may use: half the
/// processor, at least one, and never more than Gracil is allowed right now.
pub fn threads_for_one() -> usize {
    one_of(processor_threads(), ALLOWED.load(Ordering::Relaxed))
}

/// Sizes the threads the libraries built into Gracil share (oxipng's, and the quality measure's
/// for images) to what one tool may use, each quieted as it starts, as Gracil's own threads are
/// (ADR 0068). Without it they would take one thread for every processor thread, outside the
/// background. Done once, when the engine starts; later calls do nothing.
pub(crate) fn start_shared_threads() {
    static STARTED: Once = Once::new();
    STARTED.call_once(|| {
        let built = rayon::ThreadPoolBuilder::new()
            .num_threads(threads_for_one())
            .thread_name(|n| format!("gracil-shared-{n}"))
            .start_handler(|_| worker_started())
            .build_global();
        if let Err(error) = built {
            tracing::warn!(%error, "the shared threads were already started");
        }
    });
}

fn one_of(all: usize, allowed: usize) -> usize {
    let half = (all / 2).max(1);
    if allowed == 0 {
        half
    } else {
        half.min(allowed)
    }
}

/// Whether the tools must go easy right now: in Windows' efficiency mode, which includes running
/// on a battery (ADR 0051, ADR 0063). Taken as yes until told otherwise.
static EASY: AtomicBool = AtomicBool::new(true);

/// Tells the engine whether its tools run in efficiency mode, or on a battery, right now. While
/// they do, the samples of a quality level are measured one at a time (ADR 0065).
pub fn go_easy(easy: bool) {
    EASY.store(easy, Ordering::Relaxed);
}

/// Whether the tools must go easy right now (see [`go_easy`]).
pub(crate) fn easy() -> bool {
    EASY.load(Ordering::Relaxed)
}

/// How many times the computer has gone to sleep since Gracil started.
static SLEEPS: AtomicU64 = AtomicU64::new(0);

/// Tells the engine the computer is going to sleep. A tool that fails across it is given its try
/// again, once.
pub fn note_sleep() {
    SLEEPS.fetch_add(1, Ordering::Relaxed);
}

/// How many times the computer has gone to sleep, to compare before and after a step.
pub(crate) fn sleeps() -> u64 {
    SLEEPS.load(Ordering::Relaxed)
}

/// When the computer last woke up, if it has slept since Gracil started.
static WOKE: Mutex<Option<Instant>> = Mutex::new(None);

/// Tells the engine the computer has just woken up. For a while after it, a tool's silence is not
/// taken for being stuck.
pub fn note_wake() {
    *WOKE.lock().unwrap_or_else(PoisonError::into_inner) = Some(Instant::now());
}

/// Whether the computer woke up less than `within` ago.
pub(crate) fn woke_within(within: Duration) -> bool {
    WOKE.lock()
        .unwrap_or_else(PoisonError::into_inner)
        .is_some_and(|woke| woke.elapsed() < within)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_tool_never_takes_the_whole_processor() {
        let all = processor_threads();
        assert!(threads_for_one() >= 1);
        assert!(all == 1 || threads_for_one() < all);
    }

    #[test]
    fn one_tool_keeps_to_what_gracil_is_allowed() {
        assert_eq!(one_of(16, 0), 8);
        assert_eq!(one_of(16, 4), 4);
        assert_eq!(one_of(16, 12), 8);
        assert_eq!(one_of(1, 0), 1);
        assert_eq!(one_of(2, 1), 1);
    }
}
