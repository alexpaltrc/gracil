# 0002 — FFmpeg runs as a bundled external process

**Status:** Accepted. Where Gracil finds FFmpeg is set by
[0008](0008-ffmpeg-detected-or-provisioned.md); running FFmpeg as an external process still stands.

**Context.** Linking FFmpeg libraries would put codec crashes inside Gracil's process, tie Gracil
to FFmpeg's C API and complicate licensing. Gracil only needs to start, observe, suspend and stop
encodes.

**Decision.** Ship known `ffmpeg.exe` and `ffprobe.exe` builds with Gracil and run them with
structured arguments (`std::process::Command`, never a shell). Read progress from
`-progress pipe:1`.

**Consequences.** An FFmpeg crash fails one job, not the app. Pause works by suspending the
process. FFmpeg can be upgraded by replacing two files plus its provenance notes. The libx265 CPU
encoder requires a GPL FFmpeg build; its licensing obligations are documented with the binaries.
