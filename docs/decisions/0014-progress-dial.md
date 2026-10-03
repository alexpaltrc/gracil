# 0014 — Progress is a dial that holds the controls

**Status:** Accepted. Refines the look of spec §31; the interaction model is unchanged.

**Context.** The video in focus showed a thin progress bar with **Pause** and **Cancel** written
under it. The product owner wants a cleaner window with more personality and even fewer words,
and sketched the replacement.

**Decision.**

1. Progress is a ring drawn like a pipe. A marble rolls through it clockwise from twelve o'clock,
   leaving a soft trail behind it; the marble's position is the progress.
2. The middle of the ring holds the controls, as symbols instead of words. Its top three quarters
   are one button: pause (II) while compressing, play (▶) while paused. Its bottom quarter, below
   a thin line, is stop (■), which cancels as before.
3. Changes are animated softly: the marble glides between progress reports, pause morphs into
   play by moving the corners of its bars, and the part under the pointer lights up. The dial
   only asks for new frames while something is still moving (spec §51).
4. While Gracil is getting ready the marble rests at the top and only stop answers.
5. The percentage, or "Paused", stays as one quiet line under the dial.

It is drawn with Iced's canvas (`gracil-ui/src/dial.rs`), so the `canvas` feature is enabled.

**Consequences.**

- The words Pause, Resume and Cancel are gone from the window; the symbols are the universal
  ones from media players.
- The symbols have no hover text yet, so a screen reader cannot name them. To revisit with the
  accessibility work.
