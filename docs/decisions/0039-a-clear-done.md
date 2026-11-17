# 0039 — A clear "done"

**Status:** Accepted, built (2026-10-04); changed by ADR 0055 (2026-10-05), below, and by
[ADR 0076](0076-the-ring-stays.md) (2026-11-16): the finished view is one of the ring's faces.
Asked for by the product owner after compressing a folder of more than 15,000 files (spec §77.3).

**Changed** by [ADR 0055](0055-natural-first-use.md) (2026-10-05): the green middle and check mark
appear only when something became smaller; otherwise the big line says why nothing was, and
failed files are counted apart, leading to them.

**Context.** When the last file was finished, the main window simply kept showing that file. After
a long list nothing said that Gracil had finished, nor how the list went as a whole.

**Decision.**

1. The list is *done* when nothing is waiting, being worked on, waiting for a choice (Replace or
   Save copy) or still being looked for in a folder, and at least one file finished. A list where
   every file was stopped is not done; it shows the last file as before.
2. Once done, and unless the user picked a file in **View files**, the main window shows:
   - the ring the marble filled, whole, with its middle filled in Windows' success green and a
     check mark that draws itself;
   - just below, larger than a title, how much room the list saved (*130 MB smaller*), or *All
     done* when nothing came out smaller; it fades in, rising a little;
   - under it, quietly, how the list went: *22 files compressed · 4 were already compressed*, and
     how many couldn't be compressed, when any. Files that couldn't be made smaller count as
     already compressed.
3. The entrance plays once, in about a second, and then asks for no more frames. Adding files
   brings back the work in progress.
4. The mini-window keeps showing the last file.

**Consequences.** `Gracil::finished` works out the summary; `done.rs` draws it with Iced's canvas,
sharing the ring's measures with the dial (`dial::ring`).
