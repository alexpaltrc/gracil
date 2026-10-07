# 0060 — Reasons in sight, and the middle of Always in words

**Status:** Accepted, built (2026-10-07). From the 1.0 review. Changes item 1 of
[0025](0025-always-replace-or-copy.md) and the "hover to explain" of spec §2.3.

**Context.** When a file couldn't be compressed, or was already as small as it gets, a small "!"
sat beside its status and the reason only appeared while the pointer rested on it. Someone using
the keyboard, a touch screen or a screen reader never saw it, and nobody guesses that a "!" hides
words. The **Always** line had the same trouble: its middle choice was a small circle, and only
pointing at **Always** said that the circle means "ask each time".

**Decision.**

1. **The file shown large says why in words**, under its status, with nothing to point at. The
   "!" stays beside the status as the mark that something is worth reading.
2. **The mini-window, too small for a sentence**, keeps the "!" and makes it a button: clicking
   it (or Enter, once Tab rings it) opens the big window on that file, where the reason is
   written out. Pointing at it still shows the reason.
3. **The list** keeps the reason behind its "!" on hover: picking the row shows the file large,
   with the reason in words (item 1).
4. **The middle of Always says Ask** (Preguntar): **Replace · Ask · Save copy**. Its stone stays
   the quiet outline it was, so the setting still looks off there. Pointing at **Always** says
   "What to do with each finished file".

**Consequences.**

- Every reason Gracil gives can be read without a pointer.
- The **Always** line is a little wider; it still fits the smallest window in both languages.
- Spec §2.3 keeps its rule (no dialogs); only "hovering" stops being the one way in.
