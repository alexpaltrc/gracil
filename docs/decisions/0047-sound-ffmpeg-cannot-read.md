# 0047 — Sound FFmpeg can't read

**Status:** Accepted, built (2026-10-05).

**Context.** iPhones from the iPhone 16 on record two sound tracks in each video: the usual AAC
one, and Apple's spatial audio (APAC, `apple_apac`). FFmpeg recognizes APAC but can't decode it,
and copying it into an MP4 fails too, so every such video failed: 171 of them in one of the
product owner's lists.

**Decision.**

1. When Gracil starts, it asks FFmpeg which codecs it can decode (`ffmpeg -codecs`, the `D`
   flag). If FFmpeg doesn't answer, nothing changes.
2. An audio stream FFmpeg can't decode is left out of the compressed video when another audio
   stream it can decode remains.
3. A video whose only sound can't be decoded is not changed: it fails as before rather than come
   out silent.
4. When a future FFmpeg can decode APAC, the stream is treated like any other sound, with no
   change to Gracil.

**Consequences.** Videos from recent iPhones compress, keeping the stereo sound every player
uses. The spatial version is lost from the compressed copy; the original keeps it if the person
chooses **Save copy**.
