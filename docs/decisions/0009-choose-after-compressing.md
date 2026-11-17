# 0009 — In the main window, the user chooses what to do with a result after compressing

**Status:** Accepted, built (2026-10-02). Amended by
[0010](0010-replacing-updates-modified-date.md); [0025](0025-always-replace-or-copy.md) lets its
question be answered ahead of time.

**Context.** The specification defines two output intentions, replace and save a copy (§37), but
not when the main window asks for one. File Explorer chooses up front, with two commands (§57–§58).
The product owner wants the save actions to turn off, with a quiet "!", when a result is not
smaller (§40.1), which implies the actions are offered once the result is known.

**Decision.**

1. Videos added to the main window are compressed straight away. Nothing is asked first.
2. A finished result is kept as its temporary file, already validated, and the window offers two
   quiet actions beside how much smaller it is: **Replace** and **Save copy**. With several results
   waiting, the drawer offers both for all of them at once.
3. A result that is not smaller is discarded at once; its actions are shown turned off, with a
   "!" that explains why (§40.1).
4. A result the user never chose for (removed from the list, cleared, or left when Gracil exits)
   is deleted. The original is untouched in every case.
5. Replacing keeps the file's name and creation date, so it still reads as the same video; its
   modified date becomes the moment of replacing ([0010](0010-replacing-updates-modified-date.md)).
   `ReplaceFileW` swaps the files in one step through a backup that is deleted only
   after the swap succeeded, and is moved back if it failed (§39).
6. When the format changes (`Clip.avi` becomes MP4), replacing creates `Clip.mp4` without
   overwriting anything and only then deletes `Clip.avi`. If `Clip.mp4` already exists, nothing is
   replaced and the "!" says why; saving a copy still works.

**Consequences.**

- The user sees the saving before committing to it, and never loses an original to a result that
  didn't help.
- Finished results take disk space beside their originals until the user chooses. This is
  bounded by the videos the user added and cleaned up at exit.
- Explorer's commands (M7) skip the question, since the user already chose there.
