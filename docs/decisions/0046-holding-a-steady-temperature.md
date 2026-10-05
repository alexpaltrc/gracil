# 0046 — Holding a steady temperature

**Status:** Accepted, built (2026-10-05).

**Context.** ADR 0045 made Gracil rest more the warmer the graphics card ran, from 70 °C to a
quarter of the work at 80 °C, in breaths of three seconds. The product owner asked for two
changes:

- They worried that working and resting in three-second turns swings the power the card and
  processor draw, like a light switched on and off.
- They asked for a safe temperature to be held ("70 °C is fine, just not more"), instead of the
  work dropping sharply as the card warms and climbing back once it cools, only to reach the same
  temperature again.

On the first point, the swings are small: a game moves the card's power further and more often,
and cards are built for it. Turns of three seconds are long enough for the chip's temperature to
follow them, though, while turns of a second or less are smoothed out by its own heat, as a
dimmer's flicker is. So both points are worth doing.

**Decision.**

1. A breath lasts one second instead of three. A rest shorter than 50 ms is skipped: it would only
   stop and start the work.
2. The temperature is read every breath. The target is 70 °C, or 20 degrees below the card's
   slowdown temperature, whichever is lower.
3. The share of each breath spent working changes gradually: each second, by 0.02 for every
   degree the hottest card is below (up) or above (down) the target. Clamped between a quarter
   and all of it. So it settles at the share that holds the target, working as much as that
   temperature allows.
4. A card more than 10 degrees above its target, a card slowing itself down because of heat, or
   the processor held back for five readings in a row (five seconds, about as long as before)
   still drop the share to a quarter at once. From there it climbs back step by step.
5. Without a card to read, the share comes back up as if well below the target.
6. Everything else in ADR 0045 stands: fewer images while resting at all, never a stop, nothing
   said about it.

**Consequences.** On a warm computer the card stays near 70 °C instead of moving between 70 and
80 °C, and the work goes at a steady pace. A simulated card that would reach 90 °C at full work
settles within 2 degrees of 70 °C, never above 75 °C (tested). Rests are a fraction of a second
and happen up to once a second; nothing on screen shows them. This amends ADR 0045.
