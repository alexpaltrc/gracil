# 0038 — A ring without its track

**Status:** Accepted, built (2026-10-04); its look changed again by
[ADR 0076](0076-the-ring-stays.md) (2026-11-16). Asked for by the product owner. Amends the look
of ADR 0014, items 1 and 2; how the dial works does not change.

**Context.** The dial drew a pipe: a faint channel between two thin walls, which the marble rolled
along. A thin line separated pause from stop in the middle. The product owner finds the ring more
elegant, and more Gracil's own, without them.

**Decision.**

1. The track is not drawn: no channel and no walls. Only the marble and the part it has filled
   are seen, as if the marble filled an invisible circle. The filled part keeps the accent color.
2. The line between pause and stop goes. The two parts still light up softly under the pointer,
   which is enough to tell them apart, and they keep their sizes (stop is the bottom quarter).

**Consequences.** Before the marble has moved, the ring is only the marble at twelve o'clock.
`dial.rs` keeps the track's width and the line's position as measures (for the marble's size and
for which part a click lands on); neither is drawn.
