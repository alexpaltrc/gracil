# 0012 — Every compressed video is an MP4

**Status:** Accepted, built (2026-10-02). Supersedes the container rule of spec §45 ("keep the
source's container when it can hold HEVC").

**Context.** Gracil kept MOV and MKV videos in their own format and turned everything else into
MP4. The product owner wants a single result format, to stay with Gracil's philosophy: one rule
that needs no explaining, and a file that plays everywhere. MP4 with HEVC tagged `hvc1` plays on
Windows, iPhone, Mac, Android, TVs and in browsers.

**Decision.**

1. Every compressed video is an MP4, whatever its source format.
2. The extension is `.mp4`. A source that already ends in `.mp4` keeps its own spelling (`.MP4`
   stays `.MP4`), so replacing it is still an in-place swap. `.m4v` becomes `.mp4`.
3. What MP4 cannot hold is left out, as with every other detail Gracil cannot carry over
   (spec §41, §44): bitmap subtitles (Blu-ray, DVD), fonts and other attachments, and data tracks.
   Styled subtitles (ASS/SSA) become plain text subtitles. Audio MP4 cannot hold (for example
   uncompressed PCM from a camera, or DTS) becomes AAC.
4. iPhone details (`com.apple.*`) are still written, as MP4 `mdta` metadata.
5. When the result will have a new name (`IMG_0042.MOV → IMG_0042.mp4`), the window says so in one
   quiet line under Replace and Save copy, before the user chooses. No dialog, no setting.

**Consequences.**

- Replacing a MOV or MKV creates a file with a new name and removes the original (the
  changed-format path of ADR 0009). Apps that remembered the old file name, such as a video
  editor's project, no longer find it. The quiet line is there so this is never a surprise.
- MKV files lose picture-based subtitles and embedded fonts, and styled subtitles lose their
  styling. These are rare among the videos Gracil is meant for (phone and camera recordings,
  screen captures).
- The plan has one container to reason about; `Container` and the MKV subtitle conversion are
  gone.
