# 0041 — The window keeps up with long work

**Status:** Accepted, built (2026-10-04). Item 1 narrowed by ADR 0066: only Direct3D 12 is built
in, so `WGPU_BACKEND` can no longer pick another way to draw, and no particular graphics card is
asked for.

**Context.** After about five hours compressing a list of 15,078 files, Gracil's window stopped
answering: pause and stop did nothing and **View files** never opened, while the tray marble kept
moving and the work itself went on normally underneath (about 3,300 images and 350 videos were
finished in that time). Sampling the window's thread showed it busy drawing again and again
without the window ever showing a new frame, even after being resized. Iced 0.14, when Windows
won't hand it a frame to draw into (a "timeout" or other error), only writes a log line, which
went nowhere, and tries again on the next frame, forever. WGPU picks Vulkan first on most
graphics cards, and Vulkan on Windows is known to stop handing back frames for windows that are
minimized or behind others while the graphics card is busy, which it was for hours with video
encoding. Separately, the footer showed how many files were on the list, so it never went down,
and every message and frame went through the whole list several times (finding a file, counting
what is waiting, adding up the room saved), which with thousands of files is real work on the
same thread that draws.

**Decision.**

1. The window is drawn with Direct3D 12, Windows' own way for the graphics card to draw, which
   does not stall like that. `WGPU_BACKEND`, when set, still decides, so other choices can be
   tried without a new build. Gracil's windows are opaque, so Direct3D's lack of see-through
   windows costs nothing.
2. The warnings and errors of the drawing libraries, which report through `log` rather than
   `tracing`, go into Gracil's log. The same message is written at most once a minute, with how
   many times it came again in between, so a failure that repeats every frame can't flood it.
3. Everything the window and tray show about the whole list (how many are left and waiting,
   the room saved, which file is shown, whether the list is done) is worked out in one pass after
   each change and kept, not worked out again for every frame. Progress, frames and scrolling,
   by far the most frequent messages, change none of it and skip that pass. A file is found by
   its number by halving the list, which stays sorted because numbers are given in order.
4. The footer says how many files are left, counting down to none as they finish ("12 files
   left"), instead of how many are on the list.

**Consequences.** If the window ever stops drawing again, the log says why. The window's thread
does a fixed, small amount of work per frame however long the list grows. If Direct3D 12 turns
out to have its own trouble on some computers, the log will show it and the choice can be revisited
here.
