# 0033 — Every video keeps its resolution; compressing again loses a little

**Status:** Accepted, built (2026-10-03). Decided by the product owner (2026-10-03). Supersedes
item 5 (size) of [0028](0028-quality-standard.md) and the size rule of spec §21, items 4 and 5
(lighter choices, asking) of [0029](0029-lighter-never-heavier.md), and all of
[0032](0032-resolution-pill.md).

**Context.** Gracil's promise is that nobody has to think to compress a video: Gracil knows what to
do. Two decisions had broken that. Already-compressed videos stopped and asked the user to pick
between "keep the size, compress more" and "one size down" (ADR 0029), and the resolution pill
(ADR 0032) added a setting and another question. Lowering the resolution also changed the user's
video in a way they may not have wanted, and 4K always became 1080p.

**Decision.**

1. **The resolution is always kept.** No video is ever scaled, whatever its size: 4K stays 4K,
   480p stays 480p. The frame-rate rule (above 30 becomes 30) is unchanged.
2. **First compression: looks the same.** The search for the level that reaches VMAF 95, and the
   check that the result is predicted at most 80% of the original, are unchanged (ADR 0028, ADR
   0029). This is where Gracil keeps its quality standard: a file's first compression.
3. **Compressing again: loses a little, without asking.** When the 95 level would not be clearly
   smaller, the video was already compressed, and the user is asking for it to be compressed
   again. The search goes on, without stopping or asking, to the highest level reaching VMAF
   **90** (NVENC CQ 24–42, x265 CRF 20–38), at the video's own size, and uses it when it is
   predicted at most 80% of the original. Some quality is lost; that is what the user asked for.
4. **Never heavier.** If even that would not be clearly smaller, nothing is encoded and the video
   keeps the quiet "!" saying it is already about as small as it can get (ADR 0029). The final
   check that the result is smaller still holds.
5. Nothing is offered or asked: no buttons with sizes, no setting. "Always" (ADR 0025) and File
   Explorer's commands apply to these results like any other.

**Consequences.**

- Large videos such as 4K take longer to compress and stay larger than they did at 1080p; their
  quality search also takes longer.
- `scale_cuda` is no longer needed to encode; it is only part of GPU VMAF (ADR 0030).
- A setting such as "make all my larger videos 720p" may come later; it is only an idea and needs
  its own decision.
