# 0008 — FFmpeg comes with Gracil

**Status:** Accepted. Sets where Gracil finds the FFmpeg of
[0002](0002-ffmpeg-as-external-process.md).

**Changed** by [ADR 0050](0050-portable-and-open-source.md) and
[ADR 0053](0053-ready-for-github.md): Gracil first looked for an FFmpeg on `PATH` and otherwise
downloaded a pinned build into a private folder. FFmpeg now comes in Gracil's zip, and Gracil
never downloads anything.

**Context.** Gracil needs an FFmpeg it can rely on, with nothing for the user to install or set
up (spec §20.1).

**Decision.**

1. Use the `ffmpeg` folder beside `gracil.exe`, where the zip puts the pinned build
   (`packaging/ffmpeg.psd1`, see `docs/development/ffmpeg.md`).
2. Otherwise, while developing, use an `ffmpeg.exe` found on `PATH`.
3. Accept a copy only if `ffprobe.exe` is in the same folder and `ffmpeg -encoders` lists
   `libx265`, the CPU encoder every compression can fall back to.
4. If none qualifies, a quiet line says Gracil is missing part of itself and should be
   downloaded again.

Gracil never installs FFmpeg system-wide, never modifies `PATH` and never uses the internet.

**Consequences.**

- Gracil works offline from the first run.
- The zip is large, since a full FFmpeg build is most of it.
- Gracil redistributes a GPL FFmpeg build, so its license and a pointer to its source ship in
  `licenses\`.
- Capabilities are checked again every time Gracil starts, so a copy that no longer qualifies is
  simply not used.
