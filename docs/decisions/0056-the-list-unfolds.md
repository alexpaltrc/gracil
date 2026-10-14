# 0056 — The list unfolds and folds away

**Status:** Accepted, built (2026-10-06). Asked for by the product owner. Follows up
[0055](0055-natural-first-use.md), which left it for later.

**Context.** **View files** appeared and disappeared from one frame to the next: the ring jumped
to its new size and the list was simply there. Every other moving part of Gracil (the marble, the
pill's stone, the done screen) eases into place. ADR 0055 put this off because animating the
list's height seemed to need its rows measured ahead of time.

**Decision.**

1. Opening **View files** unfolds the list from under the line above it: its space grows from
   nothing to its full height while its rows slide down a few pixels and fade in. The ring makes
   room at the same pace. Closing it (the line again, or **Esc**) does the same backwards.
2. The curve is the pill's: quick at first, settling gently, about a fifth of a second in all.
   The rows settle a little before the space does, so nothing is still moving once it is open.
3. The list measures itself as it is laid out, so nothing is measured ahead of time and only the
   rows in sight are still built (spec §77.3).
4. While it moves, the part not yet shown can't be clicked or hovered, and hints appear only
   once it is fully open.
5. Clearing the whole list goes straight back to the empty window, with nothing left to fold.

**Consequences.**

- A small widget, `reveal.rs`, wraps the list. Like the dial and the pill, it asks for frames
  only while it moves (spec §51).
- No new settings and no new dependencies.

*Update (2026-10-13):* the same quick ease now brings each new view in. When the window's view
changes (the work, the finished view, the question before leaving, files dragged over it), it
fades in from the surface behind it (`appear.rs`). All of Gracil's movements share two speeds,
quick and steady, set in one place (`motion.rs`).
