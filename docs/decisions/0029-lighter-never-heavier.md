# 0029 — Lighter, never heavier: predicted sizes and already-compressed videos

**Status:** Accepted. Decided by the product owner (2026-10-03). Amends ADR 0028: removes adaptive
quantization and the temporal filter from the NVENC tiers, and adds a size check to the quality
search.

**Context.** With ADR 0028's settings, three real videos (an H.264 web download and two earlier
Gracil results, 0.03–0.05 bits per pixel) came out 18–79% *larger* than their sources, after four
to eight minutes of encoding each, before being discarded. Two causes:

- Spatial adaptive quantization made NVENC files 10–45% larger at the same or similar VMAF in
  controlled tests; temporal AQ and the temporal filter changed nothing measurable.
- A source that is already compressed hard has no room left at "looks the same as the original"
  (VMAF 95). The user still wants it smaller, and accepts some loss to get a real reduction, well
  beyond 10%.

The product owner's rule: Gracil exists to make videos lighter, never heavier. The user should
not wait for a whole encode to learn that a video couldn't shrink.

**Decision.**

1. **No adaptive quantization.** *Full* is `-preset p7 -tune hq -rc vbr -b:v 0 -profile main10
   -highbitdepth 1 -rc-lookahead 32 -bf 4 -b_ref_mode middle -multipass fullres`; *Basic* is the
   same without `-bf` and `-b_ref_mode`. *Legacy* is unchanged.
2. **Predicted size.** Each search sample also reports the size of every encoded frame
   (`-stats_enc_post`), at no extra cost. The prediction for the whole video is the samples'
   average frame size without their first frame, times the video's frame count, plus their
   average first (key) frame times one key frame per 250 frames (the encoders' default spacing).
   Everything Gracil does not re-encode is assumed to stay the size it is in the source. Measured
   on synthetic and real videos, the prediction is within −1% to +14% of the result, nearly always
   above it. (Counting each sample's size directly overestimates by 200–500%: its opening key frame
   dominates four seconds.)
3. **Normal compression** happens when the level found by the search (VMAF 95) is predicted to
   give at most **80%** of the original's size. The search stops as soon as a level that misses
   95 is already predicted above 80%: every level that would reach 95 is bigger still.
4. **An already-compressed video** is one where that fails. Nothing is encoded. Gracil measures up
   to two lighter choices and offers them, each with its predicted size (the search reuses every
   level already measured, and steps past a level that reaches its target by the score it has
   to spare):
   - **Keep the size, compress more:** the highest level whose VMAF is at least **90** at the
     video's own size, searching NVENC CQ 24–42 and x265 CRF 20–38.
   - **One size down:** the short side goes to the next step of 1080 → 720 → 480 strictly below
     the video's own (a 1080p video becomes 720p, a 720p one 480p; never below 480), at the
     highest level whose VMAF is at least **90** against the source brought to that same size.
     Videos at 480p or smaller don't get this choice.

   A choice is offered only when it is predicted to be at most 80% of the original. If neither
   is, the video shows a quiet "!" saying it is already about as small as it can get.
5. **Asking.** The choices appear inline where the video is shown, in the main window and in the
   mini-window. "Always" (ADR 0025) and File Explorer's replace/copy choice don't answer them;
   they still decide what happens once the chosen compression finishes. When the choice is
   needed and no Gracil window is open, the mini-window opens so the decision is quick (unless
   the user turned File Explorer's mini-window off: then the tray icon asks for attention).
   Picking one puts the video back in line, and it compresses with that size and level, without
   searching again.
6. **The final check stays.** A result that is not smaller than the original is still discarded
   (spec §40.1); it should now be rare.

**Thresholds** come from the product owner's three test videos, measured without AQ:

| Video | VMAF 95 | Kept size, VMAF ≥ 90 | One size down, VMAF ≥ 90 |
|---|---|---|---|
| H.264 web download, 1080p | 88% of the original | ~64% | ~40% (720p) |
| Earlier Gracil result, 1080p | 87% | ~44% | ~27% (720p) |
| Earlier Gracil result, 720p | 66%: compressed normally | — | — |

**Consequences.**

- No more long encodes that end in "couldn't make it smaller" when the samples can tell first.
- An already-compressed video spends longer getting ready (up to about a minute and a half for a
  long Full HD video: each measured level costs about 2.5 s per sample, half encoding on the GPU,
  half VMAF on the CPU). The FFmpeg build has no GPU VMAF (`libvmaf_cuda`). ADR 0030 makes the measuring
  quicker.
- Quality below "looks the same" is only ever the user's explicit choice, never automatic.
- The prediction relies on the encoders' default key-frame spacing; changing `-g` means revisiting
  it.
