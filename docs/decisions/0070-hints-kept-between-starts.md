# 0070 — The quality search's hints kept from one start to the next

**Status:** Accepted, built (2026-10-15). Decided by the product owner. Changes item 3 of
[ADR 0065](0065-speed.md), which kept the hints in memory only and never on disk.

**Context.** Each search for a video's level starts from the level the last video of the same
kind needed (ADR 0065, item 3), or from a guess from bits per pixel (ADR 0067). In memory only,
that is lost every time Gracil closes, and Gracil closes often: it starts with Windows, leaves
when background work from File Explorer is done, and is updated by replacing its folder. The
first videos after each start measured more levels than they had to. The hints are a handful of
numbers by kind of video, nothing about any file, so keeping them costs no privacy.

**Decision.**

1. **What is kept**: only the hint table. For each kind (encoder, size step of the short side,
   source codec as FFprobe names it, and whether the search was for looking the same or for
   compressing again, ADR 0033) the level last chosen. No paths, names, sizes or dates of files.
2. **Where**: `hints.txt` in the data folder beside `gracil.exe` (ADR 0050, ADR 0054), and nowhere
   else, so deleting the folder leaves nothing behind and **Prepare to delete Gracil** has nothing
   more to remove. Plain text: a first line `gracil hints 1`, a tab and the identity (item 4),
   then one kind a line, oldest first: encoder, size step, codec, `same` or `again`, level, by
   tabs.
3. **When**: read once, when the engine starts, into the engine's hints. Written whole or not at
   all (a file beside it, then renamed, like the settings and the unfinished list), away from the
   window's thread, only when a level was chosen since: once a video has got ready, when Exit or
   Windows' shutdown stops the work, and on leaving. No timer: nothing wakes up to check them
   while Gracil rests. Without a data folder (a drive Gracil can't write to)
   nothing is read or written, quietly; a write that fails is noted in the log and tried again
   only once something new is chosen.
4. **Only where it still applies.** The first line names what the hints depend on: Gracil's
   version, the size of `ffmpeg.exe` and when it last changed, the encoder in use and the
   settings the card accepts (ADR 0028, ADR 0052). A file written for anything else is not read,
   so a new Gracil, another FFmpeg, or another graphics card or driver that changes the encoder
   or its settings starts afresh. When `ffmpeg.exe` can't be looked at, nothing is kept.
5. **Tolerant.** A missing, empty, half-written or damaged file gives no hints, or those of its
   lines that are whole; a line that can't be read is skipped.
6. **Small.** At most 64 kinds are kept; past that, the kind chosen longest ago is let go.
7. **Still only an order.** As before, a hint only decides which level is measured first: the
   level chosen is always measured on the video's own samples against the same target and size
   rule. A wrong or stale hint only costs time.

**Considered and not done:** asking FFmpeg for its version string (one more process at every
start, while the size and time of change of `ffmpeg.exe` tell the same apart); keeping the
graphics driver's version (a driver update that changes nothing the encoder accepts leaves the
hints as good as before, and a wrong hint only costs time); writing after every video (more
writes to USB drives for no gain).

**Consequences.**

- The first video of each kind after a start gets ready as quickly as later ones did before,
  typically measuring two levels instead of four or five (ADR 0065's measures).
- One more small file in the data folder, a few kilobytes at most.
- Not tried by running Gracil yet: the format, the identity check, damaged files and the cap are
  covered by tests. Writing once a video has got ready and reading it back at the next start were
  seen working; writing on leaving was not.
