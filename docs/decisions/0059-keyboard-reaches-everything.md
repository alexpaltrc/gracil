# 0059 — The keyboard reaches every control

**Status:** Accepted, built (2026-10-07). From the 1.0 review. Extends item 10 of
[0055](0055-natural-first-use.md).

**Context.** ADR 0055 gave Gracil a few keys (Esc, Space, Ctrl+O), "what the pointer can do, a
little faster". Everything else needed a pointer: **Replace** and **Save copy**, **Continue**, the
list and its rows, **Clear**, the **Always** pill, the mini-window's buttons. Someone using only a
keyboard could not finish a single file. Iced 0.14's buttons can't take keyboard focus; only text
fields can.

**Decision.**

1. **Tab and Shift+Tab** move a ring through the controls of the window they are pressed in, in
   reading order, and wrap round. The first Tab rings the first control. Nothing holds the ring
   until Tab is pressed.
2. **Enter or Space** presses the ringed control, exactly as a click would; a control that can't
   be clicked now does nothing. With nothing ringed, Space still pauses and resumes (ADR 0055).
3. **The Always pill** takes the ring as one control; the arrow keys (and Home, End) move its
   stone, as with Windows' own option groups.
4. **The ring** is Windows' own focus line: two pixels in the text's color, around the control.
   It is there for the keyboard only: a click puts it away, as in Windows.
5. Built in Gracil itself (`focus.rs`): a button is wrapped so it can hold the ring, and Enter
   clicks the button it wraps. Each window's controls are kept apart, so Tab in the mini-window
   never moves the ring in the big one.

**Consequences.**

- Every control a pointer can reach, the keyboard can reach too, except the dial's **Stop**
  square; Space still pauses and resumes the work.
- In a long list, Tab reaches only the rows in sight and a few beyond, the ones that are built
  (spec §77.3); the mouse wheel shows the rest.
- When Iced's buttons learn to take focus, `focus.rs` can give way to them.
