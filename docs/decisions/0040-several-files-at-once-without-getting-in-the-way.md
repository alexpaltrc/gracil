# 0040 — Several files at once, without getting in the way

**Status:** Accepted, built (2026-10-04). Decided by the product owner after compressing a folder
of more than 15,000 files (spec §77.3). Replaces the one-file-at-a-time rule of the queue (spec
§33) and the "images go first" order of ADR 0034, item 13.

*Update (2026-10-14):* by [ADR 0063](0063-safe-on-any-computer.md), the computer is idle only if
nobody needs it (no full-screen program, presentation or screen kept on) and its warmth can be
read; the next video gets ready beside one compressing only with 4 GB free and never on 8 GB of
memory or less; a battery below 30 % starts nothing new.

*Update (2026-10-14):* by [ADR 0065](0065-speed.md), a desktop computer (no battery) in use,
plugged in, not saving energy and not warm, compresses a sixth of the processor's threads in
images at once instead of an eighth.

**Context.** Gracil compressed one file at a time, and each file's two stages followed each other:
first the short samples that choose a video's compression (mostly on the processor), then the
compression itself (mostly on the graphics card). While one ran, the other sat idle, and a long
list of images, each needing a few seconds, took far longer than it had to. At the same time the
owner was clear that Gracil must never make the computer feel slow or make its fans roar: "high
performance, yes; scaring the user, never", even with 15,000 files.

**Decision.**

1. **Two stages for every file.** Every file is first *got ready*, then *compressed*. For a video,
   getting ready is reading it and finding its compression from short samples; for an image, it is
   reading the file and telling its size. A file that is ready waits for its turn in a new
   *Prepared* state, which can still be stopped or taken off the list. This is built once, for
   every kind of file.
2. **One video at a time.** At most one video compresses, and at most one more gets ready while it
   does, so the next one starts as soon as the current one ends. Two videos are never compressed at
   once: one already keeps the graphics card's encoder (or the processor) fully busy.
3. **Images beside the video.** Images are compressed several at a time alongside the video, each
   kind in the order it was added, so images no longer go before waiting videos. How many:
   - an eighth of the processor's threads while the person is using the computer, a quarter once
     nobody has touched the keyboard or mouse for a minute;
   - one fewer while the video compresses on the processor rather than the graphics card;
   - never more than half of the free memory, counted from each image's size in pixels (48 bytes
     per pixel, a large phone photo when the size can't be told); one image always goes ahead;
   - always at least one.
   As many images get ready as may compress, so reading from a slow drive overlaps the work.
4. **Below the person's programs.** Gracil and every tool it starts run below normal priority, so
   whatever the person is using comes first. The window's own thread is raised back to normal, so
   it still answers at once.
5. **Never the whole processor.** The video encoder on the processor, the quality measure and the
   palette search for PNG each use at most half of the processor's threads.
6. **Pause and stop act on everything together.** Pause pauses every file being worked on and
   starts nothing new until resumed; the ring stays on screen while paused, so resume is always
   at hand. Stop cancels every file being worked on; the files still waiting carry on. Exiting
   says how many files would stop.
7. **What the window shows.** The file shown large is the video being compressed, or with only
   images, the first image being compressed. The circle for the whole list (ADR 0034, item 13)
   was built the same day; the tray follows it.

**Consequences.**

- A list of videos no longer pauses between them for the samples, and images no longer wait
  behind a video, nor a video behind images.
- Gracil uses more of the computer when it is free and backs off as soon as the person returns,
  without anything to set.
- With many small images the file shown large can change quickly, until the whole-list circle is
  built.

*Amended by ADR 0044:* Stop now stops the whole list; the files waiting are offered with
**Continue** instead of carrying on.
