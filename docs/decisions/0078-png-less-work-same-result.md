# 0078 — A PNG takes half the work, for the same result

**Status:** Accepted, built (2026-11-16). Decided by the product owner, who found a small PNG slow
and asked for everything that holds Gracil back to be reviewed. Builds on
[ADR 0034](0034-image-standard.md) and [ADR 0068](0068-images-sooner.md); the standard itself
(item 5 of ADR 0034) is unchanged.

**Context.** A PNG of 800 KB took 35 seconds. Its weight was small and its picture large, between
11 and 17 megapixels, and what a PNG costs goes by its pixels, not its bytes. Measured on this
computer (6 cores, 12 threads), on thirteen PNGs of every kind:

- **The rules that hold Gracil back were not the cause.** The same PNG took 7.8 s with the
  computer in use (half the processor, tools in efficiency mode) and 7.6 s with it idle; five
  12-megapixel photos took 1.2 s either way. They cost little on a short job, as intended, and
  stay as they are (ADR 0049, 0051, 0063).
- **Most of the time went into dithered palettes that could never be kept.** Dithering is the
  slowest step by far (2 to 4 s for each palette of a 3.7-megapixel picture, on one thread), and
  it was done up to four times a picture. Yet dithering only lifts a palette's score where the
  plain one is poor, and then nowhere near enough: plain 12, 31 and 46 became 33, 51 and 58. Where
  the plain palette scores 80 or more, dithering added at most 1.2 points and more often took
  some away. In thirteen pictures it never changed which file was kept.
- A dithered palette too big to keep was passed over without being measured, and the three with
  fewer colors were made at once in case one was smaller.
- The lossless version, the slowest single step (3 to 5 s), was made first, by itself.

**Decision.**

1. **The plain palette with the most colors is made and measured first.** The dithered kind is
   tried only when that one scores within 5 points of what is needed (`DITHER_HELPS`): four
   times the most dithering was seen to add there.
2. **A palette too big to keep is still measured**, then let go: when it scores short, the ones
   with fewer colors would too, and are not made. This is the rule ADR 0068 already follows
   (fewer colors hardly ever look better), applied to every palette rather than only to those
   that could be kept.
3. **A kind's first palette is made by itself**; the one after the next is made beside it only
   once the first has passed.
4. **The lossless version is made while that first palette is made and measured**: neither
   needs the other.

**Measured.** The thirteen PNGs took 72.9 s before and 36.5 s after, each one's result the same
file as before. The largest gains are where the time went: 9.7 s to 2.1 s, 15.2 s to 5.7 s,
9.0 s to 3.3 s. Pictures that reduce well to few colors take what they took.

**Also.** `cargo run --profile timing -p gracil-ui` builds a Gracil as fast as a release that
goes by a development build's name ([ADR 0075](0075-a-development-build-of-its-own.md)), so the
real app can be timed beside the Gracil in daily use. The numbers above come from it and from
the engine's example.

**Left for later.** Making a palette is not the same from one run to the next (the library
that makes it walks the picture's colors in an order that changes), so the same PNG can come out
a few hundred bytes different. It was so before this change. Dithering itself could be made
several times quicker with a method of Gracil's own; that would change the palettes it makes, so
it is a decision for another day.
