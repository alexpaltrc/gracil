# 0034 — Gracil's image standard

**Status:** Accepted, built (2026-10-04). Formats, the engine's source, the PNG rules and how
images share the list, the order and the dial were decided by the product owner (2026-10-03);
the numbers for JPEG were left to engineering, "always looking for a golden standard", and were
chosen from the measurements below. Settles open decisions 2–5 of spec §77.1. *Amended the same
day, before any of it was built:* `cjpegli` now reads JPEG directly (item 9), and item 13 was
added. *Amended when built (2026-10-04):* images are read inside Gracil and by FFmpeg, not by
Windows (item 9); every descriptive PNG chunk is kept (item 5); palettes are measured from the
most colors down (item 5); the file types are listed (item 1). *Superseded in part by ADR 0040 (2026-10-04):*
images no longer go before waiting videos; they compress beside the video being compressed
(item 13).

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
   animated PNG, animated WebP; a future plan), camera RAW, JPEG XL. Those are left exactly
   as they are, with the quiet "!" and the reason.

   | Opened | Saved as |
   |---|---|
   | JPEG (`.jpg`, `.jpeg`, `.jpe`, `.jfif`) | JPEG, same name |
   | PNG (`.png`) | PNG, same name |
   | HEIC, HEIF, AVIF, WebP, TIFF, BMP | `.jpg`, or `.png` with transparency |
   | GIF, animated PNG or WebP, JPEG XL, camera RAW | not compressed: the quiet "!" |

   An image is known by its extension; what it holds is then read from its first bytes, so a HEIC
   photo named `.jpg` is still read as HEIC. A picture stored as true HDR also gets the
   "!" (item 11).
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
5. **PNG, first compression:** lossless optimization (oxipng, preset 4; every descriptive
   chunk — color profile, gamma, text, dates, EXIF — is kept, so nothing the original says about
   itself is lost) always; plus a reduced palette (256, 128, 64 or 32 colors, with and without
   dithering) only when its SSIMULACRA2 is at least **90**. The smallest qualifying result is
   kept, when it is at most 80% of the original. Fewer colors hardly ever look better, so each
   kind of palette (plain, dithered) is measured from 256 colors down and stops at the first that
   falls short: usually two measurements instead of eight. The palettes are made side by side,
   and the two kinds measured side by side, within about 1 GB of memory.
6. **PNG, compressing again:** the same palettes, at SSIMULACRA2 **80** or more, automatically.
   Otherwise the quiet "!".
7. **Never heavier.** Every result is checked against the original's size before it replaces
   anything (spec §40.1).
8. **One scale for every image.** Every score is SSIMULACRA2 against the decoded original, so
   JPEG, HEIC, WebP and PNG are judged the same way. Gracil measures in its own process with the
   `ssimulacra2` Rust crate (BSD-2).
9. **Engine.**
   - A JPEG goes to `cjpegli` as it is: jpegli reads it itself through libjpeg-turbo (BSD-3,
     IJG and zlib licenses), the copy jpegli's own repository includes. On three 12 MP photos this
     was about five times quicker than decoding to a PNG first (0.4 s against 2–2.5 s) and scored
     the same or better against the original.
   - Every other format is read inside Gracil: WebP, TIFF and BMP by the `image` Rust crate
     (MIT/Apache-2.0), HEIC, HEIF and AVIF by FFmpeg, which Gracil already runs hidden for video
     and which reads the tile grid of an iPhone photo already turned upright. FFmpeg drops a
     HEIC's EXIF data and color profile, so Gracil reads those from the file itself. The picture
     goes to the encoder as a PPM file in Gracil's temporary space: jpegli reads only JPEG, PNG
     and the simple uncompressed formats, and this one extra step is lossless. Windows' own image
     reader (WIC) was set aside: it needs the Store's HEIF extension for HEIC, and nothing it
     offered was missing otherwise. Nothing opens a window: no image viewer, no console.
   - JPEG: `cjpegli`, Google's jpegli, built by Gracil from the official repository
     (`github.com/google/jpegli`, BSD-3) at a pinned commit with `scripts/build-jpegli.ps1`, and
     shipped inside the MSIX (about 1.7 MB). jpegli is no longer part of libjxl; nothing from
     libjxl or JPEG XL is used. Run as a separate, hidden program, like FFmpeg. `cjpegli` opens
     files with the C library's narrow functions, so the build script gives it a manifest that
     makes its code page UTF-8: names in any language, emoji included, work.
   - PNG: `oxipng` (MIT) and `exoquant` (MIT), linked into Gracil. GPL quantizers such as
     libimagequant/pngquant are never used: Gracil is proprietary.
   - Credits and license texts for jpegli and what is built into it (libjpeg-turbo and the IJG
     notice, Highway, libpng, zlib, skcms, Little CMS) ship next to `cjpegli.exe` as
     `cjpegli-licenses.txt`; with those of oxipng, exoquant, `image` and `ssimulacra2` they go in
     the About screen and the Store listing, as for FFmpeg.
10. **Metadata** (spec §41, best effort, never a reason to reject): the date taken, camera data
    and location (EXIF), descriptions and captions (XMP, IPTC), the orientation and the color
    profile are carried over. jpegli keeps the color profile but not EXIF, XMP or IPTC, so Gracil
    copies them into the result itself. An image that Gracil turns upright itself (every format
    but JPEG) has its EXIF orientation set to "upright", so viewers do not turn it a second time.
    The orientation must survive: jpegli keeps a photo's pixels as stored, so without it a photo taken upright would
    show sideways. A Display P3 photo stays Display P3 through its profile.

11. **HDR.** A photo with an HDR gain map (recent iPhones and Android phones) becomes its normal
    picture: the result looks as the photo does on most screens, without the extra brightness on
    an HDR screen. A picture stored as true HDR (PQ or HLG) is left as it is, with the quiet "!",
    for now.
12. **Very large images** have no limit. They take longer: for a 48 MP photo, jpegli takes about 2
    seconds and each SSIMULACRA2 measurement about 10, so compressing again can take about half a
    minute.
13. **Images and videos together** (spec §77.1, decision 5):
    - **One list** for every file, images and videos alike: nothing separate to learn.
    - **Images go first.** They take seconds and videos take minutes, so images added while videos
      wait are compressed before the waiting videos; the video being compressed is not
      interrupted. Among images, and among videos, the order is the order they were added.
    - **The dial.** With one file, image or video, it works as it does today: the marble goes
      round once for that file. With more than one, the whole circle is the whole list: it is
      split into equal parts, one per file, and the marble crosses one part per file. Each time a
      file is done, the marble leaves a mark where its part ends: a hollow marble, only its
      outline. The marks show at a glance how many are done and how many are left. Equal parts,
      rather than parts sized by how long each file takes, keep the marks evenly spaced and the
      count honest: a quick image and a long video are each one file. When the parts would be too
      small for the marks to be told apart, the marks are left out and only the circle shows.
      The list here means the files done since Gracil last started working plus the ones still
      waiting; adding files while it works makes the parts smaller. When everything is done, the
      next file starts a fresh circle. The percentage below stays that of the file being
      compressed.
    - **The tray** marble's ring follows the dial: the whole list with more than one file.
    - Making the dial look more refined, modern and elegant is a planned design pass of its own.

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

Palettes without dithering were usually both smaller and better scored. Each PNG took 0.4–4.6 s
in the evaluation; as built, 1.8–7.5 s (the lossless version of a 3.5 MP screenshot alone takes
about 2 s).

**Consequences.**

- A phone photo is usually compressed once at distance 1.0; an iPhone HEIC that is already small
  goes straight to the "compressing again" path, as HEIC 2 does.
- Building `cjpegli` needs Visual Studio Build Tools with CMake and Ninja (already part of the
  development setup); it is not rebuilt on every package.
- The PNG and SSIMULACRA2 crates add to Gracil's own binary; jpegli stays a separate program.
- These thresholds come from one person's test set, as ADR 0029's did; they are revisited when
  more images are measured.
