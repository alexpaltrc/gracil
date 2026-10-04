# 0034 — Gracil's image standard

**Status:** Accepted, not built. Formats, the engine's source and the PNG rules were decided by the
product owner (2026-10-03); the numbers for JPEG were left to engineering, "always looking for a
golden standard", and were chosen from the measurements below. Settles open decisions 2, 3 and 4 of
spec §77.1; decision 5 (how images share the list, the dial and the tray) stays open and must be
settled before the interface is built.

**Context.** Images follow the video standard (ADR 0028, 0029, 0033): a file's first compression
looks the same as the original; when that would not be clearly smaller, the file was already
compressed and the user is asking for it to be compressed again, so Gracil goes further, loses a
little, and never asks; never heavier; resolution always kept. Images need their own measure of
"looks the same": VMAF is for moving pictures. **SSIMULACRA2** (by the authors of JPEG XL and
jpegli, BSD license) is the image counterpart: 90 means no difference even when flipping between
the two images, about 80 means no difference at normal viewing, 70 is "high quality".

**Decision.**

1. **Formats.** A PNG stays a PNG. Every other image becomes a JPEG and keeps its name. An image
   with transparency that is not a PNG becomes a PNG. Not accepted: animated images (GIF,
   animated PNG, animated WebP; a future plan), camera RAW, JPEG XL. Accepted ones are left exactly
   as they are, with the quiet "!" and the reason.
2. **Resolution is always kept**, as for video (ADR 0033). No image is ever scaled.
3. **JPEG, first compression:** jpegli at butteraugli distance **1.0**, jpegli's own "visually
   lossless" point, fixed. Kept when at most **80%** of the original (ADR 0029). There is no
   search: on 26 photos, distance 1.0 scored 83.8–86.3 every time, so measuring each photo would
   only add time.
4. **JPEG, compressing again** (distance 1.0 would not be clearly smaller): Gracil tries looser
   distances **1.5, 2.0, 2.5 and 3.0** and keeps the loosest whose SSIMULACRA2 is at least **80**
   and whose size is at most 80% of the original. It starts at 2.0 and moves one step looser or
   tighter, so at most three are measured. If none qualifies, nothing is written and the image gets
   the quiet "!" (already about as small as it can get).
5. **PNG, first compression:** lossless optimization (oxipng, preset 4, safe chunk stripping)
   always; plus a reduced palette (256, 128, 64 or 32 colors, with and without dithering) only
   when its SSIMULACRA2 is at least **90**. The smallest qualifying result is kept, when it is at
   most 80% of the original.
6. **PNG, compressing again:** the same palettes, at SSIMULACRA2 **80** or more, automatically.
   Otherwise the quiet "!".
7. **Never heavier.** Every result is checked against the original's size before it replaces
   anything (spec §40.1).
8. **One scale for every image.** Every score is SSIMULACRA2 against the decoded original, so
   JPEG, HEIC, WebP and PNG are judged the same way. Gracil measures in its own process with the
   `ssimulacra2` Rust crate (BSD-2).
9. **Engine.**
   - Reading: Windows itself (WIC), with FFmpeg as the fallback (for example HEIC without the
     Store's HEIF extension: FFmpeg reads its tile grid). The decoded image goes to the encoder as
     a PNG or PPM file in Gracil's temporary space.
   - JPEG: `cjpegli`, Google's jpegli, built by Gracil from the official repository
     (`github.com/google/jpegli`, BSD-3) at a pinned commit with `scripts/build-jpegli.ps1`, and
     shipped inside the MSIX (about 1.3 MB). jpegli is no longer part of libjxl; nothing from
     libjxl or JPEG XL is used. Run as a separate program, like FFmpeg.
   - PNG: `oxipng` (MIT) and `exoquant` (MIT), linked into Gracil. GPL quantizers such as
     libimagequant/pngquant are never used: Gracil is proprietary.
   - Credits and license texts for jpegli, its bundled libpng/zlib, oxipng and exoquant go in the
     About screen and the Store listing, as for FFmpeg.
10. **Metadata** (spec §41, best effort, never a reason to reject): the date taken, camera data
    and location (EXIF), the orientation and the color profile are carried over; Gracil writes them
    into the result itself. A Display P3 photo stays Display P3 through its profile.
11. **HDR.** A photo with an HDR gain map (recent iPhones and Android phones) becomes its normal
    picture: the result looks as the photo does on most screens, without the extra brightness on
    an HDR screen. A picture stored as true HDR (PQ or HLG) is left as it is, with the quiet "!",
    for now.
12. **Very large images** have no limit. They take longer: for a 48 MP photo, jpegli takes about 2
    seconds and each SSIMULACRA2 measurement about 10, so compressing again can take about half a
    minute.

**Measurements** (the product owner's test set; sizes as a share of the original; scores are
SSIMULACRA2 against the decoded original):

| JPEG, first compression (26 photos) | Size | Score |
|---|---|---|
| distance 0.5 | 66–99% | 86.5–89 |
| **distance 1.0** | **41–70%** | **83.8–86.3** |
| distance 2.0 | 26–42% | 71–82 (48 MP photos lowest) |

| Compressing again (a distance 1.0 result) | 1.5 | 2.0 | 2.5 |
|---|---|---|---|
| Photo 1 (48 MP) | 59%, 77.7 | 44%, 74.4 | — |
| Photo 2 | 84%, 87.5 | **63%, 82.3** | 59%, 79.0 |
| Photo 5 | 87%, 86.9 | **62%, 82.3** | — |
| Photo 12 | **76%, 84.1** | 55%, 78.9 | — |
| Photo 20 | 88%, 86.0 | **63%, 81.2** | — |

Distance 1.0 on its own result gives about 100%: not worth it. Photo 1 gets the "!": no looser
distance reaches 80.

| HEIC | 1.0 | 1.5 | 2.0 |
|---|---|---|---|
| 1 (2316×3088) | **72%, 85.9** | 54%, 82.9 | 45%, 79.1 |
| 2 (3024×4032) | 95%, 85.1 (not worth it) | **73%, 81.7** | 61%, 78.2 |

| PNG | Lossless | Best palette |
|---|---|---|
| 1 | 64% | 32 colors: 21.6%, 90.85 (first compression) |
| 2 | 70% | 256 colors: 31%, 82 (compressing again) |
| 3 | 76% | none reaches 80 (best 77.6): lossless kept |
| 4 | 59% | 256 colors: 18.7%, 83.4 (compressing again) |

Palettes without dithering were usually both smaller and better scored. Each PNG took 0.4–4.6 s.

**Consequences.**

- A phone photo is usually compressed once at distance 1.0; an iPhone HEIC that is already small
  goes straight to the "compressing again" path, as HEIC 2 does.
- Building `cjpegli` needs Visual Studio Build Tools with CMake and Ninja (already part of the
  development setup); it is not rebuilt on every package.
- The PNG and SSIMULACRA2 crates add to Gracil's own binary; jpegli stays a separate program.
- These thresholds come from one person's test set, as ADR 0029's did; they are revisited when
  more images are measured.
