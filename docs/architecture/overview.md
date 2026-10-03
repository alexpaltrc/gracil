# Architecture overview

Gracil is one Rust process (`gracil.exe`) that owns a job scheduler and drives external `ffmpeg.exe`
/ `ffprobe.exe` processes, plus a tiny COM DLL loaded by File Explorer that only forwards requests.

```
 File Explorer ──(IExplorerCommand, gracil-shell.dll)──┐
                                                       │ named pipe (paths + action)
 2nd launch of gracil.exe ─────────────────────────────┤
                                                       ▼
 ┌──────────────────────── gracil.exe ─────────────────────────┐
 │ gracil-ui        Iced daemon: main window, mini-window, tray │
 │ gracil-windows   mutex, pipe server, tray, suspend, replace  │
 │ gracil-core      jobs, scheduler, probe, encode, finalize    │
 └───────────────────────────────┬─────────────────────────────┘
                                 │ std::process::Command (no shell)
                      ffmpeg.exe / ffprobe.exe (found on the PC, or installed privately)
```

## Crates

| Crate | Kind | Responsibility | Introduced |
|---|---|---|---|
| `gracil-core` | lib | Job state model, output naming, FFprobe inspection, capability detection, FFmpeg argument construction, progress parsing, scheduler, output validation. No UI dependency. | M0 |
| `gracil-windows` | lib | Everything that needs `windows-rs`: single-instance mutex, named-pipe IPC, process suspend/resume, `ReplaceFileW`, timestamps, tray, taskbar geometry. The only crate (besides the shell DLL) allowed `unsafe`. | M4/M5 |
| `gracil-ui` | bin `gracil` | Iced + WGPU application: main window (Focus + Drawer), mini-window, settings, wiring core ↔ Windows layer. | M2 |
| `gracil-shell` | cdylib | `IExplorerCommand` COM server for the Windows 11 context menu. Depends on nothing heavy; only counts supported files and forwards them. | M7 |

Crates are created when they get real content, not ahead of time.

## Key flows

**Compression.** A job is a lightweight record (path, state, progress) kept in
`gracil_core::queue`. The scheduler runs one job at a time (ADR 0004): probe → plan the streams →
spawn FFmpeg writing to a reserved temporary sibling (`<name>.gracil-<id>.part`) → parse
`-progress pipe:1` → validate the output with FFprobe (HEVC, same duration) → finalize. A result
that is not smaller than the original is discarded (spec §40.1).

**Finding FFmpeg.** At startup, in the background: use an FFmpeg on `PATH` that has `ffprobe.exe`
beside it and the `libx265` encoder; otherwise Gracil's private copy; otherwise download the pinned
build, verify it and install it privately (ADR 0008).

**Choosing how to encode.** Capabilities are detected once per process: `ffmpeg -encoders` must
list `hevc_nvenc`, then a sub-second test encode must succeed. Each job then walks a ladder of
attempts, from fastest to most forgiving: NVENC with CUDA decoding → NVENC with software decoding
→ NVENC keeping only essential streams → `libx265` keeping only essential streams (without NVENC:
`libx265` full, then essential). See [`../development/ffmpeg.md`](../development/ffmpeg.md).

**The window.** `gracil-ui` is an Iced daemon. `app` holds the state and reacts to messages,
`view` draws Focus + Drawer from that state alone, `work` runs the engine and each compression on
their own threads and reports back as messages, and `style` holds every design token. Paths given
on the command line are queued like dropped files. A finished result waits in its temporary file
until the user chooses Replace or Save copy; `work::place` then calls `gracil_core::output` on its
own thread, with `gracil_windows::replace_file` (`ReplaceFileW`) for the swap (ADR 0009).

**Closing and exiting.** Closing a window hides it; the process keeps running in the tray. Only
the tray menu's Exit quits. A process started from Explorer that never showed a window exits by
itself when its work is finished (spec §53, §54.1).

*Until the tray exists (M5), closing the window cancels the running compression, deletes results
nobody chose for, and quits.*

**Single instance.** The first process creates a named mutex scoped to the user session and serves
a named pipe. Any later invocation (new launch, Explorer) sends its request over the pipe and
exits.

## Data on disk

- Settings: `%LOCALAPPDATA%\Gracil\settings.json` (redirected into the package's private folder
  when running as MSIX).
- Logs: `%LOCALAPPDATA%\Gracil\logs\`, one plain-text file per day, the last seven kept.
- No job history is persisted.

See [`../decisions/`](../decisions/) for the reasoning behind the main choices.
