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
2. **Samples stay on the graphics card.** Decoded by NVIDIA's decoder, a sample's frames stay on
   the card for the encoder, as they do in the real encode, and only the reference the encoded
   sample is compared with is copied out (8-bit and 10-bit 4:2:0 sources; others as before). The
   encoded frames and the scores are bit-identical to before; copying every frame out and back in
   cost about a fifth of the processor.
3. **Starting where the last video ended.** Gracil remembers, in memory only and never on disk,
   the level the last video needed for each encoder, size (short side up to 480, 720, 1080, 1440,
   2160, above) and source codec, and measures that level first for the next video like it; the
   search for compressing again (ADR 0033) starts at the level the first search found, or at the
   one the last video compressed again needed. A hint outside the levels searched is brought
   inside them. A hint only changes which levels are measured first: the level chosen is always
   one measured to reach the target, and on a score curve that falls steadily it is the same
   level as without a hint.

**Consequences.**

- Thirty thousand images, each got ready and compressed: keeping the window's counts took about
  0.7 ms per change on the reference computer (some 40 s of the window's time in all, and a
  slower window as the list grew); now about 40 ms in all, the same per change whatever the
  list's length.
- Getting a video ready, measured on the reference computer with synthetic 60 s clips (same
  levels, scores and predicted sizes before and after; wall time, then FFmpeg's processor time):

  | Clip | Before | Samples on the card |
  |---|---|---|
  | 1080p 8-bit | 41.9 s, 206 s | 36.7 s, 164 s |
  | 1080p 10-bit | 25.2 s, 116 s | 21.6 s, 104 s |
  | 2160p 8-bit | 114.8 s, 649 s | 95.3 s, 497 s |

  A second 1080p video like the first measured 2 levels instead of 5 (about 15 s instead of
  37 s), and chose the same level.
