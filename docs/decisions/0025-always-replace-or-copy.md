# 0025 — "Always": replace or save a copy without being asked

**Status:** Accepted. Requested by the product owner (2026-10-03). Adds a second setting to
spec §66 and lets [0009](0009-choose-after-compressing.md)'s question be answered once, ahead of
time.

**Context.** ADR 0009 asks after every video whether to replace it or save a copy. People who
always answer the same way are asked again and again, and a result finished while only the
mini-window is open waits until someone decides. Spec §66 keeps settings small and forbids
technical ones; this one is about the user's files, not about encoding.

**Decision.**

1. The main window has one more line at its foot: **Always**, followed by a pill with three
   positions: **Replace**, a small ring, and **Save a copy** (Spanish: **Siempre**, **Reemplazar**,
   ○, **Guardar como copia**). It reads as a sentence: "Always replace", "Always save a copy",
   or, in the middle, "replace or save a copy", meaning Gracil asks. The middle was first the
   word **Or** (**O**); the product owner had pictured a circle, so it is a small empty ring
   drawn by the pill itself, like the marble's outline: it needs no translation and reads as
   "neither, ask me".
2. The middle is the default and looks switched off: its stone is barely visible. Picking an end
   slides the stone there over a fraction of a second and fills it with the marble's grey
   (`pill.rs`, which animates itself like the dial and only asks for frames while moving).
3. When a video finishes, File Explorer's command still decides first, since it was chosen for
   those videos. Otherwise an end of the pill decides, exactly as if its button had been clicked.
   The result is placed with the same safe steps (ADR 0009, §39).
4. If placing fails (for example `Clip.mp4` already exists and replacing would overwrite it), the
   result waits with its "!" and both buttons, as if the setting were off for that video.
5. The pill applies to videos that finish after it is changed. Results already waiting keep
   waiting; changing a setting never replaces anyone's files on its own.
6. It is saved as `always=replace|ask|copy` in `settings.txt`. Missing or unknown means ask.
7. The mini-window follows from this: with an end picked it shows where the video went; in the
   middle it offers **Replace** and **Save copy** itself.

**Consequences.**

- One extra line in the main window, always visible, since it is a choice people make once.
  *Changed by [0055](0055-natural-first-use.md):* the line comes up with the first file; the
  empty window shows only where to drop. Hovering **Always** says what it does.
- "Always replace" puts results in place of originals without a click. The swap is still the safe
  one, and a result that is not smaller is never kept (§40.1), so nothing is lost to a bad result.
- Iced's `advanced` feature is enabled for the pill widget; no new dependency.
