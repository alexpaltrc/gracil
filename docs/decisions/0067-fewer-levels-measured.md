# 0067 — Fewer levels measured: getting a video ready sooner, with the same result

**Status:** Accepted, built (2026-10-14). Decided by the product owner after a measured review of
where getting a video ready spends its time on the reference computer (Ryzen 5 5600G, 6 cores /
12 threads, with an RTX 5060). Not tried on Intel or AMD graphics, nor with x265 doing the search.

**Context.** ADR 0065 found that, on that computer, the quality measure on the processor was the
bottleneck. With its samples kept on the graphics card and measured together, that is no longer
so at 1080p: a 1080p level's four samples take about 6.4 s, and the card's encoder is busy for
nearly all of it, as it is for the whole compression afterwards. Making each measurement cheaper
on the processor no longer shortens the wait; measuring fewer levels does. Nothing about what
Gracil produces may change: the level chosen is always one measured at or above its target
(VMAF 95, or 90 when compressing again, ADR 0033) and predicted at most 80 % of the original
(ADR 0029), exactly as before; the resolution is kept; one video compresses at a time; and every
safeguard of ADRs 0049, 0051, 0063 and 0064 still holds.

**Decision.**

1. **Stepping by what the score lost.** Past a level that reaches the target, the next level is
   as far as the score it has to spare allows at the points per level it lost since the nearest
   measured level below it that also reaches the target, rounded down, when that level is at most
   three below. Otherwise, as before, at about a point per level. Scores fall faster as levels
   rise: measured further apart, the fall looks gentler than it is, and the step would overshoot.
2. **Compressing again gives up too.** The search for compressing again stops, as the first one
   does, as soon as a level below its threshold is still too big, and when a level measured
   earlier already is: every level reaching the threshold is bigger still. The video is then
   already as small as it gets.
3. **Telling first whether anything can be small enough.** The samples are encoded at the top
   level for compressing again (NVENC and AMF 42, Quick Sync 40, x265 38), the smallest file
   there is, without measuring their quality: the same arguments up to the encoder's output, so
   the same frames (checked byte for byte on the CUDA and processor paths). If even that is
   predicted above 80 % of the original, nothing is encoded and the video is already as small as
   it gets. This happens once per video: first, for a source spending under 0.02 bits per pixel
   of each frame; otherwise when a level measured is too big and the top level, estimated from
   it on a log scale, is at least 85 % of the most that is worth it. A wrong guess costs that
   one encode; a sample that fails leaves it unknown, and the search goes on as before.
4. **Too big: straight to where it may stop being so.** When the highest level reaching the
   target is too big, the next level measured is the highest one whose size, estimated on a log
   scale from the sizes measured (or about a ninth smaller per level when only one is known), is
   still too big. If it misses the target while too big, the search gives up (items 2 and the
   rule of ADR 0029); if it reaches it, the search goes on above it. Only the order of measuring
   depends on the estimate.
5. **Reading memory again.** The next video gets ready beside one compressing only with memory
   to spare (ADR 0063). The memory free is now read once the compression has run for five
   seconds, when it holds what it needs, and again every five seconds while a video waits and
   nothing is getting ready, so memory freed later lets it start then and not only when the
   compression ends. Pause, rest, a low battery and exiting still start nothing.
6. **A first guess for the first video of its kind.** Without a hint (ADR 0065), the first level
   measured is guessed from the bits the source spends on each pixel of each frame: two below the
   top of the levels searched for the leanest sources (0.03 or less), one lower for each doubling.
   Only the order of measuring depends on it.
7. **Every thread handed out.** Samples measured together share the threads one may use for the
   quality measure as evenly as they go: six for four samples is two, two, one and one, not one
   each. Never more than one alone would use, as before.

Hints are still kept in memory only (ADR 0065); keeping them from one run to the next waits on the
product owner.

**Considered and not done:**

- **Setting x265's frames at once** (`frame-threads`), the same in the search and the compression,
  so a level is encoded exactly as measured. x265 picks it from the threads it is given (`pools`),
  which follow the computer's state (ADR 0049): 2 with 6 threads, 1 with 3. But setting it isn't
  enough: with the same `frame-threads=2`, 20 s of 1080p at CRF 26 came out 12,448,515 bytes
  with 6 threads and 12,463,743 with 3, so the output follows the threads given in other ways too.
  And with 3 threads it was 0.15 % heavier than x265's own choice (12,445,586). Making the two
  encodes identical would mean fixing `pools`, which is how the processor share is kept; not done.
- **Stepping back from a level that misses by the score it lacks**, instead of halving what is
  left: it measured more levels on some of the test curves.

**Consequences.** Measured on the reference computer with synthetic 60 s clips (moving test
pattern with grain), each in a fresh run with no hints, samples together on a desktop left alone,
NVENC. Levels measured (size-only encodes in brackets), time getting ready (from the engine ready
to the level chosen), and from the engine ready to the result in place. Every clip chose the same
level and came out the same size as before.

| Clip | Bits per pixel | Levels | Getting ready | In all | Result |
|---|---|---|---|---|---|
| Camera-like H.264, 79 Mbps | 1.27 | 4 → 4 | 33.7 → 28.8 s | 48.6 → 43.4 s | CQ 34, 4.1 % |
| Camera-like H.264, 8 Mbps | 0.13 | 5 → 2 | 32.3 → 12.5 s | 46.4 → 26.6 s | CQ 34, 39.9 % |
| AV1, 4.8 Mbps | 0.08 | 5 → 4 | 31.5 → 23.3 s | 45.6 → 37.4 s | CQ 34, 56.9 % |
| Gracil's own result, again | 0.05 | 6 → 3 | 41.4 → 18.3 s | 55.7 → 32.6 s | CQ 36, 72.2 % |
| Gracil's result of that, again | 0.04 | 7 → 4 | 44.5 → 24.8 s | 58.8 → 39.1 s | CQ 42 again, 47.8 % |
| HEVC, 2.2 Mbps | 0.03 | 7 → 4 | 67.3 → 24.2 s | 81.7 → 51.1 s | CQ 39 again, 67.8 % |
| 720p H.264, 1 Mbps | 0.04 | 7 → 4 | 24.4 → 17.9 s | 31.2 → 24.7 s | CQ 40 again, 68.6 % |
| HEVC (x265 CRF 34) | 0.03 | 8 → 3 | 47.9 → 18.8 s | 70.1 → 33.1 s | CQ 42 again, 58.2 % |
| H.264, 1.5 Mbps | 0.02 | 7 → 5 (1) | 42.4 → 37.5 s | 56.5 → 51.8 s | CQ 41 again, 78.1 % |
| HEVC (x265 CRF 38) | 0.02 | 7 → 0 (1) | 43.8 → 4.4 s | — | as small as it gets |

At 1080p a measured level takes about 6.4 s and a size-only encode of the same samples about
4.4 s: the encoder is most of both. The 1.5 Mbps H.264 clip shows the cost of a wrong guess: its
top level, estimated within reach of too big, was encoded alone and turned out small enough.
The AV1 clip's first guess (CQ 35) missed by 0.4 points, and halving what was left cost a level;
the synthetic clips all need CQ 33–37 whatever their bits per pixel, so the guess could not be
calibrated much further on them. Real videos should be checked in the log.

Two parts were not measured on a real run of Gracil: the memory read again (item 5), covered by
building and the scheduler's existing paths, and Quick Sync, AMF and x265 searches, which take
the same decisions over their own ranges.

On the test curves (17 score offsets × 4 steepnesses × 2 targets × 2 ranges × 5 hints) the step
alone measures 4,802 levels against 4,881, never more in any case. With sizes too (17 offsets ×
3 steepnesses × 3 size slopes × 6 original sizes), the whole choice measures 2,226 levels plus 452
size-only encodes against 3,609 levels before, and chooses the same as the rule says in every
case and every start.
