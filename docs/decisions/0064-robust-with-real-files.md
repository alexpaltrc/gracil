# 0064 — Robust with real files: changes, stalls, cloud drives, shutdown

**Status:** Accepted, built (2026-10-14).

*Update (2026-10-14), item 2:* a tool's silence never counts for thirty seconds after the
computer wakes, nor across any long gap between two looks (a sleep Gracil heard of late). A tool
that ended well just as it was taken for stuck counts as finished. Its output is read as bytes,
so a line that isn't valid text can't stop the reading and leave the tool blocked, and Gracil
never waits for a tool to end while holding what pausing and stopping need. A sample or attempt
stopped for silence across a sleep is tried once more, like one that failed.

*Update (2026-10-14), item 5:* only hidden entries are left out (system ones among them), not
those marked system or read-only alone: Windows marks a folder so when it is given its own icon
or look, as Pictures and Videos are, and such a folder is the person's.

*Update (2026-10-14), item 3:* one placer for each drive, made when first needed, each putting its
results in place one after another, so a network share that stalls holds up only its own. The
drive is told from the path's name alone (`C:`, `\\server\share`), asking nothing of it. A placer
that stops unexpectedly still answers that the result couldn't be put in place, so a file is
never left being saved and leaving can always finish.

*Update (2026-10-14), item 7:* a video kept only online is read through, a piece at a time, before
FFprobe opens it, so a long download is never taken for a stuck tool; pause and rest wait between
pieces and Stop stops. A file let go after it got ready but before it compressed (Clear, removing
it, Stop) is given back to the cloud drive too.

**Context.** A robustness review looked at what real computers and real folders do to Gracil
that tests with tidy files don't: files that change while they compress, tools that stop
answering, cloud drives, read-only files, hidden system folders, very long names, old USB drives,
Windows shutting down, a Gracil opened straight from its zip. The gaps found:

- A file edited or replaced while it compressed could be replaced by a compressed copy of the
  old version, losing the edit.
- FFmpeg or FFprobe hanging on a broken file or a stalled network drive kept that file
  "Compressing" forever, and nothing after it started.
- Each result was put in place on a thread of its own, so two choices in a row could land out of
  order, and a thread that couldn't start left the file "Saving" forever. A panic on a work
  thread did the same.
- Clear, removing a file and Undo going away deleted results on the window's own thread, which a
  slow drive could freeze.
- Folders brought in `$RECYCLE.BIN`, `System Volume Information`, macOS's `._` files and other
  hidden or system entries.
- A read-only original failed to be replaced only at the very end, with a vague reason, and a
  folder that takes no new files was found out only after the whole compression.
- A file kept only online by OneDrive (or a similar drive) was downloaded to be compressed and
  then stayed on the computer, taking room the person had freed on purpose.
- The leftovers sweep at start ran before anything could compress, checking each drive again for
  every entry, and waited for the disk on every image's scratch file. Image scratch files were
  never swept at all, as their names didn't match what the sweep looked for.
- Shutting Windows down mid-video could lose the list of unfinished files kept since the last
  save.
- Opened from inside its zip, Gracil ran from a temporary copy without FFmpeg, said its video
  tool was missing, and wrote that temporary copy into Send to and start with Windows.
- A name close to Windows' 255-character limit made the temporary, copy and backup names too
  long. On a drive without hard links (FAT32, exFAT), the fallback move could replace a file that
  appeared meanwhile.
- The log had no size limit; a long day of failures, each with FFmpeg's last lines, could grow it
  without end.
- Images read ahead had no limit in memory, and reading a huge image could fail Gracil itself.

**Decision.**

1. **A file that changes is left alone.** Its size and last-changed time are noted when Gracil
   starts on it and checked again when the result is ready and just before it is put in place.
   If either changed, the result is deleted and the file shows "!": *This file changed while it
   was being compressed, so it was left as it is. Add it again to compress the new version.*
2. **A tool that stops answering is stopped.** FFmpeg and FFprobe that say nothing for five
   minutes (ten for the quality measure above 4K) are stopped, and the next way of compressing
   is tried, as after any failure. Time paused, resting or asleep doesn't count.
3. **One placer.** Results are put in place one after another, in the order chosen, by one worker
   that stays for the whole run. If it can't start, the file says it couldn't be saved, and the
   choice can be made again. Work that stops unexpectedly reports a failure instead of waiting
   forever.
4. **Deleting away from the window.** Clear, removing a file and Undo going away hand unused
   results to one worker that deletes them in the background. On leaving, Gracil waits for it,
   within the three seconds it already allows (ADR 0044). Undo works as before.
5. **Only the person's files.** Looking through a folder leaves out hidden and system entries,
   names starting with `._`, and `$RECYCLE.BIN`, `System Volume Information` and `.Trashes`, in
   any case. A hidden file added by itself is still taken.
6. **Read-only files and folders.** A read-only original is never replaced, and its read-only
   mark is never cleared: Replace says *This file is read-only, so it can't be replaced. Save a
   copy instead*, and the result stays for Save copy. Before compressing, Gracil makes and removes
   an empty temporary file beside the first file of each folder; a folder that takes none fails
   at once with the reason that the compressed file couldn't be saved there.
7. **Cloud drives.** A file kept only online needs room for its download and its result, twice
   its size and the usual spare room besides. Once Gracil is done with it (saved, let go, or
   failed), the cloud drive may free it up again, as "Free up space" does. Only files that were
   online-only are touched; anything else about them stays as it was.
8. **Leftovers.** The sweep of what an earlier run left runs on its own thread while this run
   compresses; this run's temporary files have names of their own (process and start time), so
   it never mistakes them. Each drive is checked once per sweep. Image scratch files are written
   down without waiting for the disk, and are swept now.
9. **Shutting down.** Gracil never holds up Windows shutting down or signing out. When Windows
   says it is ending the session, Gracil keeps the list of unfinished files at once and stops its
   work, within a few seconds.
10. **Inside the zip.** A Gracil running from the temporary folder, or from a folder whose name
    ends in `.zip`, leaves Windows as it is and says: *Gracil is still inside its zip. Extract it
    to a folder first, then open it from there.* A development build is not affected.
11. **Long names.** Temporary, copy and backup names are shortened at the end of the original's
    name, never in the middle of a character, so they always fit in 255 characters.
12. **No hard links.** On drives without hard links, a result is moved only to a name Gracil has
    just claimed for itself, so it can never replace another file.
13. **The log has a daily limit**, about 20 MB. Past it, one line says *log limit reached for
    today*, and only warnings and errors are written, each cut short.
14. **Images in memory.** Images read ahead and waiting their turn hold at most a quarter of the
    memory images may take. Room for an image's bytes is reserved before it is read; one that
    doesn't fit is left as it is, as in ADR 0049.

**Consequences.** Gracil never puts an old version over a file that changed, never sits forever
on a stuck tool, and keeps its places in order. Cloud drives keep the room the person freed.
Folders bring in only what the person put there. Shutting down keeps the list of what was left.
There are two new reasons a "!" can give (a file changed; a read-only file); everything else is
said with words already in use. A stuck tool now costs five minutes before the next way is tried.
The size of the next image to be read isn't known before it is read, so the memory limit for
images waiting can be passed by one image.

This changes in spirit ADRs 0017 (the sweep runs beside the work), 0040 (images read ahead are
limited by memory), 0044 (deleting on leaving waits for the background worker) and 0049 (room for
a download).
