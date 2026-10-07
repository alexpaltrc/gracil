# 0057 — Always stays in sight, the empty window included

**Status:** Accepted, built (2026-10-06). Asked for by the product owner. Reverses item 2 of
[0055](0055-natural-first-use.md) as far as the **Always** pill goes; back to what
[0025](0025-always-replace-or-copy.md) first decided.

**Superseded in part** by [ADR 0058](0058-explorer-send-to-and-main-menu.md) (2026-10-07): File
Explorer's words no longer follow **Always**; the running Gracil applies it to what File Explorer
sends. The first reason below no longer holds, but **Always** still decides what File Explorer's
commands do, so the decision stands.

**Context.** ADR 0055 hid the **Always** pill until the first file came, so the empty window
showed only where to drop. In use this turned out to hide a setting people need before there are
files:

- File Explorer's menu follows **Always** (ADR 0035): its commands say *Compress and replace* or
  *Compress and save copies of* when it is set, and just *Compress* otherwise. To change what the
  menu does, the pill had to be reachable without first adding a file.
- Before dropping a large folder, the moment to decide between replacing and saving copies is
  before it starts, not after the first files are already done.

**Decision.**

1. The **Always** line is always at the foot of the main window, the empty window included.
   Hovering the word **Always** still says what it does.
2. The rest of 0055's empty window stays: the list's line comes up with the first file.

**Consequences.**

- One quiet line under the drop area in the empty window. It is a choice made once, so it reads
  as part of the window rather than as something to do.
- No new settings, no new dependencies.
