# 0052 — Compressing on Intel and AMD graphics

**Status:** Accepted, built (2026-10-05). Not yet tried on Intel or AMD hardware; the test encode
keeps Gracil on the processor wherever it fails.

*Update (2026-10-14):* by [ADR 0065](0065-speed.md), item 4 below is no longer so: where a
one-frame test passes, Quick Sync and AMF first try decoding on their own card, keeping the
frames there; processor decoding stays as the next attempt. Also untried on that hardware.

**Context.** Only NVIDIA cards compressed videos (ADR 0028). Most laptops have no NVIDIA card,
but nearly all have Intel graphics with Quick Sync, or AMD graphics with AMF, both able to encode
HEVC many times faster than x265 and far cooler. The bundled FFmpeg (ADR 0050) has `hevc_qsv` and
`hevc_amf`.

**Decision.**

1. **Order.** NVENC, then Quick Sync, then AMF, then x265. The first card encoder whose test
   encode succeeds is used. NVENC stays first: it has the most quality features and the best
   tested settings.
2. **Tiers, as for NVENC.** `NvencLevel` becomes `HardwareLevel` (Full, Basic, Legacy), with
   settings per encoder, each confirmed by a three-frame test encode fed the way Gracil feeds it
   (the same pixel format and a quality level):
   - Quick Sync: Full is 10-bit, `veryslow`, extended rate control with a 40-frame look-ahead and
     adaptive frame types; Basic is 10-bit `veryslow`; Legacy is 8-bit `slow`.
   - AMF: Full and Basic are 10-bit, `transcoding` usage, `quality` preset; Legacy is 8-bit.
3. **Quality levels.** Quick Sync uses its constant quality (`global_quality`, ICQ), searched over
   20–36 (20–40 for already-compressed videos), starting at 24. AMF has no constant-quality mode
   for HEVC without a bitrate, so it uses a constant quantizer (`rc cqp`, the same `qp_i` and
   `qp_p`), searched like NVENC's CQ over 24–38 (24–42), starting at 27. The quality search
   (ADR 0028) measures the result either way, so the level that looks the same is found.
4. **Decoding stays on the processor.** Frames are decoded by FFmpeg on the processor and handed
   to the card, as NVENC already does when CUDA decoding fails. Card decoding for Intel and AMD
   can come later.
5. **Falling back.** If a video fails on the card, it is tried with fewer streams, then on the
   processor with x265, as with NVENC.

**Consequences.** Laptops with Intel or AMD graphics compress much faster and cooler. The settings
couldn't be tried on Intel or AMD hardware here: a card or driver that refuses them makes Gracil
fall back a tier, then to the processor, so nothing breaks, but the ranges may need tuning once
logs from such computers are seen.
