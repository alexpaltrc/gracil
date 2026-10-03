# 0028 — Gracil's quality standard: each video finds its own level

**Status:** Accepted. Decided by the product owner (2026-10-03). Supersedes the fixed settings of
spec §21 (preset P6, CQ 27 for every video); the encoder choice of §22 and the attempt ladder are
unchanged.

**Context.** One fixed setting treats every video the same. A calm phone clip could be much
smaller at the same visible quality, while a grainy or detailed one needs more room. Real results
showed a 1.2 GB video shrinking well (−61%) and a 350 MB one barely (−22%). The product owner asked
for a standard of Gracil's own: as small as possible while looking the same, plus two size rules.
AV1 was considered and rejected: too many phones, TVs and older PCs still cannot play it.

**Decision.**

1. **Codec.** HEVC stays, always 10-bit (Main10), tagged `hvc1`. 10-bit costs nothing in
   compatibility for HEVC players and avoids banding in skies and gradients.
2. **NVENC settings**, the best set the GPU and driver accept, chosen once per process by a
   three-frame test encode of each set in order:
   - *Full:* `-preset p7 -tune hq -rc vbr -b:v 0 -profile main10 -highbitdepth 1 -spatial-aq 1
     -temporal-aq 1 -rc-lookahead 32 -bf 4 -b_ref_mode middle -multipass fullres -tf_level 4`.
   - *Basic:* `-preset p7 -tune hq -rc vbr -b:v 0 -profile main10 -highbitdepth 1 -spatial-aq 1
     -rc-lookahead 32 -multipass fullres`.
   - *Legacy:* the old reference, `-preset p6 -tune hq -rc vbr -b:v 0`, 8-bit.
3. **x265**: `-preset medium`, 10-bit when the FFmpeg build's x265 supports it.
4. **Each video finds its own level.** Before encoding, Gracil encodes short samples spread
   through the video (one to four, four seconds each, one per 15 seconds of video) at candidate
   levels and compares each with the source using VMAF, with the same size and frame-rate
   changes applied to both. It keeps the highest level (smallest file) whose average score is at
   least **95**, searching CQ 24–38 for NVENC and CRF 20–34 for x265. Content that does not reach
   95 even at the bottom of the range gets the bottom.
   - Encoding and scoring run in one FFmpeg process per sample (a loopback decoder, `-dec`), so
     nothing is written to disk.
   - The search is skipped and the old fixed level is used (CQ 27, CRF 26) when the video is
     shorter than 6 seconds or of unknown length, when FFmpeg has no `libvmaf`, when the search
     fails, and with x265 for videos under a minute (its samples are slow to encode).
   - While it runs the video shows "Getting ready"; Stop works as usual.
5. **Size.** A video whose short side is above 1080 pixels is scaled down so its short side is
   1080 (4K becomes 1080p, a vertical 4K video becomes 1080×1920). Anything at or below stays
   as it is: 480p stays 480p. Aspect ratio is kept and dimensions stay even.
6. **Frame rate.** Above 30 frames per second becomes 30 (29.97 for 59.94/119.88 sources). At or
   below 30 nothing changes: 24.4 stays 24.4. A source counts as above 30 when its average rate
   exceeds 30.5, so 29.97 and slightly irregular 30 fps phone videos are untouched.
7. **Heavy audio** is converted to AAC (64 kbps per channel, 128–512): lossless audio (PCM, FLAC,
   ALAC) always, and any other track above twice that bitrate. Everything else is copied, as
   before.

**Consequences.**

- Easy videos get much smaller; hard ones keep their look instead of falling apart. The level and
  the score are written to the log for each video.
- Each video spends a few seconds to a minute getting ready before encoding starts.
- The size and frame-rate rules change what a user gets back; how-gracil-works.md says so plainly.
  Slow-motion videos recorded at high frame rates lose their extra frames.
- VMAF rewards some things AQ does not, so the search may pick a slightly lower level with AQ on;
  the visible quality in flat areas is the reason it stays.
- Image compression (spec §77.1) follows this philosophy later: a measured target, not a fixed
  setting.
