# 0032 — "Always": keep the resolution, ask, or reduce

**Status:** Accepted. Requested by the product owner (2026-10-03). Adds a third setting to spec
§66 and amends [0029](0029-lighter-never-heavier.md). Spec §81 forbids exposing advanced settings
silently; this is not an encoder setting but a choice about the picture's size, asked for by the
product owner, and recorded here.

**Context.** Gracil keeps every video's size up to 1080p (spec §21) and only offers one size down
for videos that are already compressed (ADR 0029). Some people would rather have every video
smaller, and some never want a smaller picture. The product owner asked for a pill like the
"Always" one of ADR 0025: **Always (Keep resolution / ○ / Reduce)**.

**Decision.**

1. A second pill sits under the first, with the same look and its own **Always** label:
   **Keep resolution**, a small ring, and **Reduce** (Spanish: **Mantener resolución**, ○,
   **Reducir**). It is saved as `resize=keep|ask|reduce` in `settings.txt`; the ring, **ask**, is
   the default.
2. Above 1080p, videos always become 1080p, whatever the pill says (spec §21).
3. **Keep resolution:** every other video keeps its size. An already-compressed video is still
   offered the lighter version that keeps its size ("keep, compress more"), but never one size
   down; if that version would not be lighter, it stays as it is.
4. **Reduce:** every video becomes one size smaller than it would otherwise leave: 1080p becomes
   720p, 720p becomes 480p. Nothing goes below 480p, and a video already that small keeps its
   size. 4K becomes 1080p, as in item 2, not 720p. The quality search (ADR 0028) runs at the
   reduced size; an already-compressed video gets one lighter choice at that size.
5. **Ask (the ring):** when compressing a video at its size would be worth it, Gracil also
   measures it one size down at the same quality target (it looks the same at that size). If the
   smaller one is predicted lighter, the video waits with **Choose a size** and two buttons with
   their predicted sizes, such as **Keep 1080p · ~120 MB** and **Make it 720p · ~80 MB**, in the
   main window and in the mini-window, exactly like ADR 0029's choices; picking one compresses
   without searching again. Otherwise it compresses at its size without asking. Already-compressed
   videos behave as in ADR 0029.
6. A choice records whether it looks the same (`Lighter::looks_same`), so its button and the
   status say so: no "compress more" and no "!" explaining a loss of sharpness.

**Consequences.**

- With the ring, getting ready takes about twice as long for videos above 480p and up to 1080p,
  since both sizes are measured.
- With the ring, most videos wait for a choice before compressing; the pill's ends remove that
  question, like ADR 0025's ends remove the replace-or-copy question.
- File Explorer's commands follow the pill too; with the ring, the mini-window shows the choice.
