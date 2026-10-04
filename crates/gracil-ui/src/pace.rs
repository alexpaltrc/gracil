//! How much Gracil takes on at once (spec §77.3, ADR 0040): fast, but never alarming. One video
//! compresses while the next one gets ready; images compress beside it, more of them while the
//! computer is otherwise idle and fewer while the person is using it, and fewer when they are
//! large, so memory never runs short.

use std::time::Duration;

use gracil_core::share;

/// Without a touch of the keyboard or mouse for this long, the computer counts as idle.
const IDLE_AFTER: Duration = Duration::from_secs(60);

/// Memory an image needs while it is compressed, per pixel: the decoded picture, the copies the
/// quality measure works on and the encoder's own.
const BYTES_PER_PIXEL: u64 = 48;

/// When an image's size can't be told before decoding it, it is taken to be this large, as big
/// as a phone's largest photos.
const UNKNOWN_PIXELS: u64 = 24_000_000;

/// What the computer is doing right now.
#[derive(Debug, Clone, Copy)]
pub struct Room {
    /// Processor threads in the computer.
    pub threads: usize,
    /// The person isn't using the computer.
    pub idle: bool,
    /// A video is being compressed on the processor rather than the graphics card.
    pub video_on_processor: bool,
    /// Memory free right now, when Windows says.
    pub free_memory: Option<u64>,
}

impl Room {
    /// Looks at the computer now.
    pub fn now(video_on_processor: bool) -> Self {
        Self {
            threads: share::processor_threads(),
            idle: gracil_windows::idle_for().is_some_and(|idle| idle >= IDLE_AFTER),
            video_on_processor,
            free_memory: gracil_windows::free_memory(),
        }
    }

    /// How many images may compress at once: an eighth of the processor while the person is
    /// using the computer, a quarter while it is idle, one fewer while a video takes the
    /// processor, and always at least one. Each image's tools use several threads of their own.
    pub fn images_at_once(self) -> usize {
        let share = if self.idle {
            self.threads / 4
        } else {
            self.threads / 8
        };
        share
            .saturating_sub(usize::from(self.video_on_processor))
            .max(1)
    }

    /// Memory images may take on top of what is already in use: half of what is free, so the
    /// person's own programs always keep room.
    pub fn memory_for_images(self) -> u64 {
        self.free_memory.map_or(u64::MAX, |free| free / 2)
    }
}

/// Memory an image of `pixels` needs while it is compressed.
pub fn image_memory(pixels: Option<u64>) -> u64 {
    pixels.unwrap_or(UNKNOWN_PIXELS) * BYTES_PER_PIXEL
}

#[cfg(test)]
mod tests {
    use super::*;

    fn room(threads: usize, idle: bool, video_on_processor: bool) -> Room {
        Room {
            threads,
            idle,
            video_on_processor,
            free_memory: None,
        }
    }

    #[test]
    fn more_images_while_the_computer_is_idle() {
        assert_eq!(room(16, false, false).images_at_once(), 2);
        assert_eq!(room(16, true, false).images_at_once(), 4);
        assert_eq!(room(32, true, false).images_at_once(), 8);
    }

    #[test]
    fn a_video_on_the_processor_leaves_less_room() {
        assert_eq!(room(16, true, true).images_at_once(), 3);
    }

    #[test]
    fn always_at_least_one() {
        assert_eq!(room(2, false, true).images_at_once(), 1);
        assert_eq!(room(1, true, false).images_at_once(), 1);
    }

    #[test]
    fn large_images_need_more_memory() {
        assert!(image_memory(Some(48_000_000)) > image_memory(Some(12_000_000)));
        assert_eq!(image_memory(None), image_memory(Some(UNKNOWN_PIXELS)));
        let free = Room {
            free_memory: Some(8 << 30),
            ..room(8, true, false)
        };
        assert_eq!(free.memory_for_images(), 4 << 30);
    }
}
