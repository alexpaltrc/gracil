# 0076 — The ring stays, and the window lays itself out around it

**Status:** Accepted, built (2026-11-16). Decided by the product owner, who asked for the main
window and the mini-window to be reviewed and made over where they fell short. Keeps Focus +
Drawer (spec §31, §79). Changes the look decided in [ADR 0038](0038-a-ring-without-its-track.md),
[ADR 0039](0039-a-clear-done.md) and [ADR 0056](0056-the-list-unfolds.md).

**Context.** Focus + Drawer held up; how it was drawn did not, in five places.

- The focus was a box with a strong outline, like a wireframe, with the rest of the window
  floating under it.
- The ring showed while a file compressed, gave way to plain words once it was done, and came
  back for the finished list: the one thing that should tie the window together kept leaving.
  How much smaller a file came out, the point of the whole view, was its smallest line.
- **View files** was the last words of a line of status, so it didn't look pressable, and a long
  line broke in two with **View files** alone on the second.
- The list showed three files however tall the window was, and opening it squeezed the focus:
  the ring shrank to a dot and the finished view's check mark was cut off.
- Everything eased at one pace towards where it was going, which is smooth but weightless.

**Decision.**

1. **The ring is one drawing through everything that happens to a file.** A marble fills it as
   the file compresses; when the file is done it closes and its middle says how much smaller the
   file came out (`−35%`); when the whole list is done its middle turns green and the check mark
   draws itself. One face leaves before the next comes in, and the ring never starts again. What
   it stands for is on one line above it (the file's name, how many files, or how much room the
   list saved), and how it goes or what to do next is below it. Both are held to their heights,
   so the ring doesn't move while the words around it change.
2. **Two sizes of focus.** *Large*: the ring in the middle of the focus. *Small*: the ring
   beside the words, as in the mini-window. The window picks the one that fits: large while the
   list is closed and the window is tall enough, small otherwise. The window opens 440 by 480,
   which is what the large one needs.
3. **An open list takes the room.** It takes all of the window but the small focus, grows with
   the window, and looks the same however many files it holds. Its scroll bar is a thin line.
4. **The focus is a card**, with the face and hairline edge Windows 11 gives the surfaces
   content lies on, and so is the list. While files are dragged over the window the focus lights
   up in the accent, softly. The mini-window is a card by itself: nothing is drawn around what
   it shows.
5. **The line under the focus is always one line**: how the list goes on its left, and **View
   files** and **+** on its right, each a button of its own. When everything doesn't fit, the
   least needed part gives way first: the room freed, then *looking for more*, then how many
   wait for a choice, then how long is left. The words are measured, not guessed.
6. **Movement has weight** (`motion.rs`). What follows the work or the pointer still eases
   towards it. What answers a choice rides a spring: a new view rises a little into place as it
   fades in, the list unfolds, the pill's stone gathers speed, stretches and settles, and the
   check mark's green lands a little past its size and comes back while the ring swells under
   it for a moment. Windows' **Animation effects** off still means nothing moves.
7. **A name that changes is said in the plainest way**: `IMG_0042.MOV → .mp4`, instead of both
   names cut short.

**Consequences.** The finished view's check mark can't be cut off, and the line under the focus
can't break in two: both were faults of the layout this replaces. The way back to the work
(spec §32.1) is under the picked file in the small focus too. `done.rs` is gone: the finished
view is one of the ring's faces. The window's layout is worked out from the window's size,
which the app now follows.
