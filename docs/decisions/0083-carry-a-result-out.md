# 0083 — Carry a result out of the window

**Status:** Accepted, built (2026-11-23). Decided by the product owner, who picked it among two
ideas for making Gracil feel more magical.

**Context.** Once a file is done, the next thing is often to send it somewhere: a chat, an email,
another folder. That meant opening File Explorer and finding the file. The owner's standard asks
that the real thing moves, not a symbol, with a gesture everyone already knows.

**Decision.**

1. **A file that is done can be dragged out of the window**, from its ring or its row in the
   list, the way a file is dragged out of File Explorer. The ring of a finished list carries
   every file that was put in place. It works in the main window and the mini-window.
2. **Windows leads the drag** (`SHDoDragDrop`), with its own picture of the files, so it looks
   and behaves exactly like dragging from File Explorer, and whatever accepts files accepts them.
3. **Only a copy is offered.** The file stays where Gracil put it, so no drop can lose it.
4. **Only files put in place travel.** A result still waiting for **Replace** or **Save copy** is
   a temporary file, and is not offered.
5. **A press that doesn't move is still a click**: the drag starts only after the pointer moves
   past Windows' drag distance, and a result let go back on its own window is left alone instead
   of being added again.

**Consequences.** Sharing a result takes one gesture and no new button. While Windows leads the
drag, Gracil's window doesn't redraw; the work goes on underneath.
