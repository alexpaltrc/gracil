# 0068 — Images sooner, with the same results

**Status:** Accepted, built (2026-10-15). Asked for by the product owner after a measured review
of where an image's time goes, on the reference computer (Ryzen 5 5600G, 6 cores / 12 threads).
Items 11 and 12 were considered and not done; item 8 waits for NASM on the computer that builds
jpegli.

**Context.** A photo compressed for the first time takes under a second (ADR 0034). The time goes
elsewhere: compressing again, where each SSIMULACRA2 measurement of a 12 MP photo took about 3.6
seconds and of a 48 MP one about 13; and PNGs, where all eight palettes were made before the
first was measured, each picture's every pixel looked up in exoquant's tree, and the quality
measure worked out the original's side again for every palette. None of this may change what
Gracil produces: every result kept still scores at least its target and is within the size worth
keeping, every score stays the same to the bit on any computer, nothing gets heavier, metadata is
kept as before, and every safeguard of ADRs 0040, 0049, 0051, 0063 and 0065 still holds.

**Decision.**

1. **A JPEG compressed again is measured only when it can be kept.** Each distance is encoded
   first and measured only when it, or a looser one it would lead to, is within the size limit:
   when distance 2.0 is too big, 2.5 and 3.0 are encoded before it is measured, and when neither
   is small enough nothing is measured at all and the image is already as small as it gets. Going
   tighter, a result too big ends the search unmeasured (tighter distances only grow). The choice
   is the one measuring everything reached would make: a test checks every combination of sizes
   and scores against the old order, and that nothing is measured that wasn't before.
2. **A PNG's palettes are made when they are to be measured.** The next palette of each kind,
   plain and dithered, is made side by side when it is next to be measured, and kept for the
   second pass. When only one is needed, the next of its kind is made beside it; after one is
   passed over (not smaller than the lossless version, or failed), the rest of its kind are made
   together, as those usually are passed over too. Never more than the eight made before.
3. **A plain palette's nearest colors are looked up once per color.** exoquant gave every pixel
   its nearest palette color through its tree; Gracil now does exoquant's own steps itself and
   looks each distinct color up once, remembered by its four bytes. A test checks the indices are
   exoquant's own.
4. **The original's side of the quality measure is worked out once.** SSIMULACRA2 compares two
   pictures at six scales; the original's planes and their blurs are now worked out the first
   time it is measured against and kept with it, once per background for a picture with
   transparency, and each measurement works out only the other picture's side. To do this the
   `ssimulacra2` crate's code (0.5.1, BSD-2-Clause by the rav1e contributors) now lives in
   `gracil-core` (`images/ssimulacra2`), its license beside it and in the zip's
   `rust-libraries.txt`, which `scripts/licenses.ps1` now also fills from such folders. Its
   unchecked indexing became checked; the crate itself stays only to test against.
5. **Fused multiply-add where the processor has it.** The blur, the SSIM map and the edge map are
   also built for processors with fused multiply-add and AVX2 (x86-64-v3), chosen as Gracil runs
   through the `pulp` crate's safe dispatch (MIT), so `gracil-core` still has no `unsafe`. A fused
   multiply-add is one correctly rounded operation whichever way it is done, and nothing else is
   reordered, so both builds give the same bits; built for any processor, each `mul_add` is a
   call to the C library. The `yuvxyb` color conversion stays as it was. Tests check every score
   is the crate's to the bit, for both builds, on 8-bit, 16-bit, small and transparent pictures.
6. **The blurs of each scale run side by side**, nine for each measurement and six for the
   original, and a product is taken as the blur reads it instead of being written out first.
7. **The libraries' shared threads keep to one tool's share.** oxipng and the quality measure run
   on rayon's shared threads, which were one per processor thread and outside the background. The
   engine sizes them, once as it starts, to the threads one tool may use (ADR 0063), each put in
   the background as it starts like Gracil's own image threads. Windows' limit on the whole job
   (ADR 0049) still holds above it.
8. **jpegli reads JPEGs with SIMD, once NASM is installed.** `scripts/build-jpegli.ps1` builds
   jpegli's copy of libjpeg-turbo with SIMD when it finds NASM on the `PATH` or in its usual
   folders, and as before without it. NASM is not installed on the reference computer and was
   not downloaded, so the shipped `cjpegli.exe` is unchanged and the SIMD build's output has not
   been compared with it yet. Before shipping a SIMD build, its results on a set of photos should
   be checked byte for byte against the current one (libjpeg-turbo's SIMD decoding is meant to
   match its plain C exactly). To get it: install NASM, delete `target\jpegli\build` and run the
   script again.
9. **The original gets ready while jpegli works.** When a JPEG is compressed again, the original
   is decoded and its side of the measure worked out while jpegli encodes distance 2.0, when twice
   a measurement's memory is free; distances encoded together, such as 2.5 and 3.0 after a 2.0
   too big, run as two jpeglis side by side. A failure decoding the original is still told only
   if something is measured.
10. **Palettes that can't be small enough aren't measured.** When a PNG is compressed again, a
    kind of palette stops once none of its palettes left can weigh little enough, and when none
    can, the image is as small as it gets without measuring. A palette not made yet counts as
    possibly small enough: making one takes longer than measuring it. A test checks the palette
    chosen against the old order, with all eight made, on thousands of made-up sizes and scores.

**Considered and not done.**

- **One palette for the plain and the dithered version of each size** (item 11). exoquant counts
  a picture's colors in a hash table whose order is random for each table, and its palettes depend
  on that order: today the plain and the dithered palette of the same size are two separate draws,
  and the same PNG can give slightly different palettes from one run to the next (seen here: sizes
  a few hundred bytes apart and scores a few hundredths apart). Sharing one would tie the two
  together, which is not the same as before, so it was left. It would save about 1.7 s of one
  thread for each pair on a 12 MP photo.
- **Guessing "compressing again" from a JPEG's quantization tables** (item 12), to encode distance
  1.0 and 2.0 at once. It would save one jpegli run (0.4 s at 12 MP, 1.5 s at 48 MP) only for
  photos already compressed, at the cost of a wasted encode whenever the guess is wrong; left for
  now.

**Consequences.**

- Measured on the reference computer with synthetic pictures (FFmpeg's test sources, at 2 to 48
  MP), from start to finish including about half a second of starting the engine, as a desktop
  left alone; results identical to before (same files, byte for byte, and the same scores, bit for
  bit, wherever a palette's own randomness, above, doesn't play):

  | Picture | Before | Now | Gracil's own processor time |
  |---|---|---|---|
  | 2 MP JPEG, compressed again, kept | 1.9 s | 1.1 s | |
  | 12 MP JPEG, compressed again, kept | 8.9 s | 3.8 s | 9.6 s → 5.2 s |
  | 12 MP WebP, nothing small enough | 4.8 s | 1.9 s | |
  | 48 MP JPEG, compressed again, nothing kept | 34.9 s | 10.3 s | 40.4 s → 13.5 s |
  | 12 MP photo PNG, nothing kept | 60.8 s | 22.5 s | 115.7 s → 46.2 s |
  | 3.7 MP screenshot PNG | 5.9 s | 3.7 s | 16.9 s → 7.3 s |
  | 1.9 MP PNG with transparency | 4.7 s | 2.9 s | 12.4 s → 5.6 s |
  | 3.7 MP gradient PNG | 9.3 s | 8.9 s | |
  | 12 MP JPEG, first compression | 0.8 s | 0.8 s | |

- One SSIMULACRA2 measurement, with the original's side already worked out: about 0.9 s instead
  of 3.6 at 12 MP, about 4 s instead of 13 at 48 MP. Working out the original's side takes about
  as long again, once, mostly while jpegli encodes. In a test with all twelve threads, at 12 MP:
  the crate 3.6 s; now 0.6 s for the original's side and 0.8 s for a measurement (1.0 s built for
  any processor).
- Gracil's peak memory went down in every case measured: 1.8 → 1.4 GB (12 MP JPEG again), 7.2 →
  5.5 GB (48 MP), 2.2 → 1.5 GB (12 MP PNG), 1.2 → 0.6 GB (screenshot). The original's side is kept
  for the whole image, about 48 bytes a pixel (twice for transparency), and a measurement then
  needs about 85 more, against about 150 before; two measured side by side share it.
- While several images compress at once, they share one tool's share of threads for oxipng and
  the quality measure instead of each taking every thread: a little slower each on a busy list,
  never more of the processor. The count is set once, when the engine starts; a smaller share
  allowed later (a battery, the person working) is still enforced by Windows over the whole job.
- Making the original ready while jpegli encodes is work wasted when nothing turns out small enough
  to measure (the 12 MP WebP above: about a second of processor). Two jpeglis at once briefly use
  two threads instead of one.
- The palettes of a PNG are no longer all made: a photo PNG makes two or four instead of eight.
- `gracil-core` now depends on `pulp` (and the few small crates it uses) and `yuvxyb` directly;
  the SSIMULACRA2 code is Gracil's to keep in step with the crate if the crate ever changes its
  scores, which Gracil would then not follow on its own.
- Not tried in the app itself: everything was measured with `examples/compress.rs`. Not tried on a
  processor without AVX2 and fused multiply-add, where the first build runs (the tests run it on
  every computer).
