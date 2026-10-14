# 0065 — Speed: getting ready sooner, and a window that keeps up with any list

**Status:** Accepted, being built (2026-10-14). Decided by the product owner after a measured
review of where time goes on a strong graphics card with an older processor (RTX 5060 with a
Ryzen 5 5600G, 6 cores / 12 threads).

**Context.** On that computer the graphics card compresses a video in a fraction of the time the
processor needs to measure it: getting a video ready (ADR 0028, ADR 0030) is most of the wait. The
window also went through the whole list after every change, which with tens of thousands of files
cost more than the work it described. None of this may change what Gracil produces: the quality
target, the size rules and the resolution stay, one video compresses at a time, nothing is asked,
and every safeguard of ADRs 0049, 0051, 0063 and 0064 still holds.

**Decision.**

1. **The list counts as it goes** (spec §77.3). The list keeps how many files of each kind are in
   each state, and where the first waiting and the first ready file of each kind may be, so the
   scheduler's questions never go through it. The window's summary is kept up to date from the
   files that changed alone: each file's share of every count is remembered, and a change takes
   off its old share and adds its new one. Clear and Stop take files off in one pass. In debug
   builds, every change to a list of up to 2,048 files is checked against a full count.

**Consequences.**

- Thirty thousand images, each got ready and compressed: keeping the window's counts took about
  0.7 ms per change on the reference computer (some 40 s of the window's time in all, and a
  slower window as the list grew); now about 40 ms in all, the same per change whatever the
  list's length.
