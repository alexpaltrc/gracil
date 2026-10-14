# 0045 — Breathing instead of cooling down

**Status:** Accepted, built (2026-10-04).

*Update (2026-10-14):* by [ADR 0063](0063-safe-on-any-computer.md), the work also rests, the
same unseen way, while the computer sleeps and for ten seconds after it wakes, and while a
laptop's battery is below 15 % (then with **Low battery · waiting** on each file).

**Context.** ADR 0043 made Gracil stop everything when the computer got too hot, saying
**Letting the computer cool down** until it had. The product owner doesn't want Gracil ever to
reach that point: a program that stops because of heat makes people anxious about their computer.
They asked for a margin kept before high temperatures instead, and left the details to us.

**Decision.**

1. Gracil never stops because of heat, and never says anything about it. The **Letting the
   computer cool down** status is removed.
2. Gracil "breathes": every three seconds it rests a moment and then works. A rest freezes every
   file being worked on, as Pause does, but it is never shown; nothing new starts during it.
3. The share of each breath spent working depends on the hottest NVIDIA card: all of it up to
   70 °C, then less in proportion down to a quarter at 80 °C. A card that starts slowing itself
   down below 90 °C gets both limits earlier: 20 and 10 degrees below its slowdown temperature.
   As the card cools, the rests shrink the same way.
4. A card slowing itself down because of heat, or Windows holding the processor back well below
   its usual speed for two readings in a row, gets the least work, a quarter.
5. While resting at all, fewer images compress at once (as in ADR 0043).
6. The person's own Pause and the rests don't mix: only Pause shows as paused, and a file is
   never frozen twice.
7. The processor's temperature still can't be read (ADR 0043). Its margin comes from never using
   all of it (ADR 0040) and from fewer images while the card is warm.

**Consequences.** Long lists on computers with poor cooling go a little slower instead of
stopping. On a well-cooled computer the card stays below 70 °C, and nothing changes at all.
The rests are short enough that progress keeps moving on screen. This amends ADR 0043.
