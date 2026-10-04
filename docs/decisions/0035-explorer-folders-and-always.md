# 0035 — File Explorer: folders, one kind at a time, and the Always setting

**Status:** Accepted, built (2026-10-04). Decided by the product owner. Amends spec §58 and the
two fixed commands of ADR 0022.

**Context.** File Explorer's menu offered **Compress and replace** and **Compress and save a
copy** for selected files only. Folders were ignored, so compressing a whole album meant opening
it and selecting everything. The two commands also ignored the **Always** setting of Gracil's
window (ADR 0025), so the menu and the window could say different things.

**Decision.**

1. **Folders.** Right-clicking a folder shows **Gracil** with three commands, for the videos and
   images inside it, subfolders included:
   - *Compress and replace videos and images in this folder*
   - *Compress and replace videos in this folder*
   - *Compress and replace images in this folder*

   With several folders it says "in these folders". Folders are looked through by Gracil, not by
   the menu, so Explorer never waits (spec §59, §60). Only known video and image extensions are
   taken from a folder; links to other folders are not followed; files go in name order.
2. **The words follow the Always setting.** "Compress and replace" when it is on Replace,
   "Compress and save copies of" (Spanish: "Comprimir y guardar como copia") on Save a copy, and
   just "Compress" on the circle in the middle. That last one opens Gracil's window, which asks
   after each file, exactly as when files are opened with Gracil. The menu reads the setting each
   time it opens, so it always matches the window.
3. **Files get the same commands.** Picking only videos shows one command (*… videos*); only
   images, one (*… images*); both, all three, so either kind can be left out. The title still
   counts the files, **Gracil (N)**; with a folder it is plain **Gracil**.
4. Gracil's command line gains `--videos` and `--images`; no choice flag means ask.

**Consequences.**

- The two fixed commands are gone; what the menu does is always what the window would do.
- A very large folder takes a moment to look through before its files join the list.
- A folder may hold copies Gracil saved earlier (`Gracil-…`); they are compressed again like any
  other file. Never heavier still applies to each.
