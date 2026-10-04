//! How much of the processor one piece of work may use (spec §77.3, ADR 0040): never all of it,
//! so the computer stays quiet and other programs stay quick. The tools Gracil starts also run
//! below other programs, so they only take what is left free.

use std::thread;

/// Processor threads in this computer.
pub fn processor_threads() -> usize {
    thread::available_parallelism().map_or(4, usize::from)
}

/// Threads one tool, such as the video encoder or the quality measure, may use: half the
/// processor, at least one.
pub fn threads_for_one() -> usize {
    (processor_threads() / 2).max(1)
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
}
