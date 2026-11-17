# 0065 — Speed: getting ready sooner, and a window that keeps up with any list

**Status:** Accepted, built (2026-10-14); item 7 not tried on Intel or AMD hardware; item 3 changed
by [ADR 0070](0070-hints-kept-between-starts.md) (2026-10-15). Decided by the product owner after a
measured review of where time goes on a strong graphics card with an older processor (RTX 5060
with a Ryzen 5 5600G, 6 cores / 12 threads).

*Update (2026-10-14), items 4 and 7:* a graphics card's test encode or decode that takes longer
than 20 seconds, as with a driver that hangs, is stopped and counts as the card not being able
to, so starting never waits on it for good.

*Update (2026-10-14), by [ADR 0067](0067-fewer-levels-measured.md):* with items 2 and 6 in place,
at 1080p on the reference computer the graphics card's encoder, not the quality measure on the
processor, is what limits getting ready (and the compression itself); measuring fewer levels is
what shortens it now.

*Update (2026-10-15), item 3, by [ADR 0070](0070-hints-kept-between-starts.md):* the hints are no
longer kept in memory only: they are written to `hints.txt` in the data folder and read back at
the next start, with what they depend on (Gracil's version, FFmpeg, the encoder and its settings),
so hints for anything else are ignored. They still only decide what is measured first.

**Context.** On that computer the graphics card compresses a video in a fraction of the time the
processor needs to measure it: getting a video ready (ADR 0028, ADR 0030) is most of the wait. The
window also went through the whole list after every change, which with tens of thousands of files
cost more than the work it described. None of this may change what Gracil produces: the quality
target, the size rules and the resolution stay, one video compresses at a time, nothing is asked,
and every safeguard of ADRs 0049, 0051, 0063 and 0064 still holds.

**Decision.**

1. **The list counts as it goes** (spec §77.3). The list keeps how many files of each kind are in
   each state, and where the first waiting and the first ready file of each kind may be, so the
   scheduler's questions never go through it. The window's summary is kept up to date from the
   files that changed alone: each file's share of every count is remembered, and a change takes
   off its old share and adds its new one. Clear and Stop take files off in one pass. In debug
   builds, every change to a list of up to 2,048 files is checked against a full count.
2. **Samples stay on the graphics card.** Decoded by NVIDIA's decoder, a sample's frames stay on
   the card for the encoder, as they do in the real encode, and only the reference the encoded
   sample is compared with is copied out (8-bit and 10-bit 4:2:0 sources; others as before). The
   encoded frames and the scores are bit-identical to before; copying every frame out and back in
   cost about a fifth of the processor.
3. **Starting where the last video ended.** Gracil remembers, in memory only and never on disk,
   the level the last video needed for each encoder, size (short side up to 480, 720, 1080, 1440,
   2160, above) and source codec, and measures that level first for the next video like it; the
   search for compressing again (ADR 0033) starts at the level the first search found, or at the
   one the last video compressed again needed. A hint outside the levels searched is brought
   inside them. A hint only changes which levels are measured first: the level chosen is always
   one measured to reach the target, and on a score curve that falls steadily it is the same
   level as without a hint.
4. **Finding out what the computer can do, all at once.** At start, FFmpeg's listings and each
   graphics card encoder's test encodes run at the same time, each card's settings still tried
   best first; the first card in the usual order (ADR 0052) whose test passed is used. Nothing
   is kept from one start to the next.
5. **A few more images on a desktop in use.** While the person uses the computer, a desktop
   (no battery), plugged in, not saving energy and not warm, compresses a sixth of the
   processor's threads in images at once instead of an eighth (ADR 0040): two instead of one on
   twelve threads. Laptops are unchanged, and so is every other limit: Gracil's share of the
   processor (ADR 0049), efficiency mode (ADR 0051), memory, heat and battery (ADR 0063).
6. **A level's samples together.** Up to four samples of one level are measured at once, each
   with its share of the threads the quality measure may use, so all together use no more than
   one did. Only when nothing asks Gracil to go easy: never with x265, above 1440p, with less than
   4 GB of memory free (2 GB for each sample), on a battery or in efficiency mode, which Gracil's
   tools are in whenever the computer is in use. Pause, stop, rest and the watch for a tool that
   stops answering act on every process. A sample that fails together is measured again alone;
   one whose encoder couldn't be opened (the card had no room for another) makes the samples go
   one at a time for as long as Gracil runs, without being taken for a decoding failure. The
   scores are added up in the samples' order, exactly as before.
7. **Intel and AMD graphics decode too** (changes ADR 0052, item 4). Where a one-frame test
   passes at start (an H.264 frame decoded on the card, kept there and encoded with the chosen
   settings), the first attempt for Quick Sync and AMF decodes on the card: Quick Sync's own
   decoding, its frames changed to 10-bit on the card for the encoder; Direct3D 11 for AMF, the
   frames taken as they are, since FFmpeg's Direct3D 11 scaler failed to start here. Decoding on
   the processor stays as the next attempt, and the quality measure's samples decode the same way,
   copying out only the reference, as in item 2. Without such a card the test fails at once, as
   on the NVIDIA computer this was built on, and nothing changes. **Not tried on Intel or AMD
   hardware**: the arguments are covered by tests, and logs from such computers should be looked
   at before relying on it.

**Considered and not done**, measured on the reference computer:

- **Cutting each sample once into a lossless copy** (FFV1) for sources with key frames far apart,
  so later levels don't decode from the last key frame again. The copy scores bit-identically,
  but it is slower: decoding the FFV1 copy on the processor took longer than decoding the source
  again on the graphics card, even 18 s into a 20 s gap between key frames (2.6 s against 2.4 s
  a sample), and making it took 2.3 s and 115 MB of disk a 1080p sample. Decoded on the processor,
  it saved 0.2 s a sample in the middle of such a gap while using more of the processor.
- **Decoding the encoded sample on the graphics card**, with the encoder in one FFmpeg and the
  measure in another, joined by a pipe (raw HEVC, frame sizes on the encoder's error output,
  frames paired by number). Bit-identical scores and sizes, and a fifth less processor, but each
  sample took 15–20 % longer (2.3 s against 1.9 s): two processes and two CUDA start-ups, and
  the source decoded twice, cost more time than the processor saved, as the measure keeps to the
  threads it is given either way.

**Consequences.**

- Thirty thousand images, each got ready and compressed: keeping the window's counts took about
  0.7 ms per change on the reference computer (some 40 s of the window's time in all, and a
  slower window as the list grew); now about 40 ms in all, the same per change whatever the
  list's length.
- Getting a video ready, measured on the reference computer with synthetic 60 s clips (same
  levels, scores and predicted sizes before and after; wall time, then FFmpeg's processor time):

  | Clip | Before | Samples on the card |
  |---|---|---|
  | 1080p 8-bit | 41.9 s, 206 s | 36.7 s, 164 s |
  | 1080p 10-bit | 25.2 s, 116 s | 21.6 s, 104 s |
  | 2160p 8-bit | 114.8 s, 649 s | 95.3 s, 497 s |

  A second 1080p video like the first measured 2 levels instead of 5 (about 15 s instead of
  37 s), and chose the same level.

  Samples together, on a desktop left alone (same scores, same levels): 1080p 8-bit 37.2 s, 162 s
  of processor → 34.6 s, 160 s; 1080p 10-bit 22.4 s, 103 s → 19.4 s, 96 s. The quality measure
  on the processor stays the bottleneck, as ADR 0030 found; above 1440p the samples still go one
  at a time.

  Everything together, first video of its kind, desktop left alone (same levels, scores and
  predicted sizes as before): 1080p 8-bit 41.9 s → 34.4 s and 206 s → 157 s of processor; 1080p
  10-bit 25.2 s → 18.7 s and 116 s → 96 s; 2160p 114.8 s → 95.2 s and 649 s → 495 s.
- Starting the engine (finding FFmpeg and what the computer can do) went from about 525 ms to
  about 485 ms on the reference computer. Cards that are listed but come after the one used are
  now tested too, at the same time; their tests fail or pass quietly.
