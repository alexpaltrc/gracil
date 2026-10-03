# 0008 — FFmpeg is detected or provisioned, not bundled

**Status:** Accepted. Supersedes the bundling part of [0002](0002-ffmpeg-as-external-process.md).

**Context.** The product owner wants Gracil to reuse an FFmpeg the user already has and to install
one invisibly when there is none (spec §20.1). This also keeps the package small (a full FFmpeg
build is over 100 MB).

**Decision.**

1. Look for `ffmpeg.exe` on `PATH` (this covers winget, Scoop and Chocolatey installs). Accept it
   only if `ffprobe.exe` is in the same folder and `ffmpeg -encoders` lists `libx265`, the CPU
   encoder every compression can fall back to.
2. Otherwise use Gracil's own copy in `%LOCALAPPDATA%\Gracil\ffmpeg\<build>\` if it exists.
3. Otherwise download the pinned build (see `docs/development/ffmpeg.md`) with Windows' own
   `curl.exe`, verify its SHA-256, extract only `ffmpeg.exe` and `ffprobe.exe` with Windows' own
   `tar.exe`, and move the folder into place in one rename. Both tools ship with Windows 11 and run
   without a console window, so this adds no HTTP or ZIP dependency to Gracil.

Gracil never installs FFmpeg system-wide and never modifies `PATH`. Under MSIX, the private folder
is redirected into the package's own storage and is removed on uninstall.

**Consequences.**

- On a computer with no FFmpeg, the first compression needs an internet connection once. After
  that, Gracil works offline.
- A user-installed FFmpeg can change under Gracil (for example, after an upgrade). Capabilities
  are checked again every time Gracil starts, so a build that no longer qualifies is simply not
  used.
- Gracil does not redistribute FFmpeg, so the GPL obligations of a GPL build fall on its
  publisher, not on Gracil. The pinned build and its license are still documented.
- Microsoft Store policies on downloading executable code must be reviewed before submission. If
  they rule this out, the fallback is to ship the pinned build inside the package.
