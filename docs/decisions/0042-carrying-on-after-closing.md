# 0042 — Carrying on after closing

**Status:** Accepted, built (2026-10-04).

**Context.** A long list, such as 15,078 files, takes many hours. If Gracil closes before it is
done (the user chooses **Exit**, Gracil stops suddenly, or the computer is turned off), everything
not yet finished was simply forgotten, and the user had to work out which files were left and add
them again. The product owner asked for Gracil to pick up where it was, with a button.

**Decision.**

1. Gracil keeps the list of files not yet finished in `unfinished.txt`, in its own private folder
   beside the logs, one file per line with File Explorer's choice (**Replace**, **Save copy** or
   none) before a tab. It never leaves the computer.
2. A file counts as unfinished until its result is saved or it ends for good (failed, kept as it
   is, removed). A finished video still waiting for the user's choice counts as unfinished,
   because its result is deleted when Gracil closes.
3. The list is written away from the window's thread, whole or not at all (to a file beside it
   that then takes its place), at most every 30 seconds while the list changes, at once when
   nothing is left, and before returning when Gracil exits. Only the newest list is written. An
   empty list leaves no file.
4. When Gracil opens and the list is not empty, the window shows a quiet line, such as **12 files
   were left unfinished**, with **Continue**, which adds them back to the list as if they had
   just been dropped (with their File Explorer choice), and **×**, which lets them go. No dialog
   (§2.3).
5. Files that were being compressed start again from their beginning. After a sudden stop, the
   last few files finished in up to 30 seconds may be done again; their originals and results are
   untouched, so this only costs time.
6. The Exit question now says that waiting files are kept for next time instead of saying they
   won't be compressed.

**Consequences.** Closing Gracil in the middle of a long list no longer loses track of it. A file
moved or deleted in the meantime is handled like any other missing file when it is added back.
The list is only paths and choices, never any content.
