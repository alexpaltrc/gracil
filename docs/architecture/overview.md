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
                      ffmpeg.exe / ffprobe.exe (bundled)
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

**Compression.** A job is a lightweight record (path, size, probe summary, state, progress, output
mode). The scheduler runs one job at a time: probe → choose backend (cached capabilities) → spawn
FFmpeg writing to a temporary sibling file → parse `-progress pipe:1` → validate the output with
FFprobe → finalize (replace or place new file without overwriting).

**Backend selection.** Capabilities are detected once per process and cached: `ffmpeg -encoders`
must list `hevc_nvenc`, then a sub-second test encode of a synthetic source must succeed. If it does,
NVENC is used; otherwise `libx265`. CUDA decoding is attempted first on NVENC jobs; if FFmpeg fails
in a way attributable to hardware decoding, the same job is restarted once with software decoding.

**Closing and exiting.** Closing a window hides it; the process keeps running in the tray. Only
the tray menu's Exit quits. A process started from Explorer that never showed a window exits by
itself when its work is finished (spec §53, §54.1).

**Single instance.** The first process creates a named mutex scoped to the user session and serves
a named pipe. Any later invocation (new launch, Explorer) sends its request over the pipe and
exits.

## Data on disk

- Settings: `%LOCALAPPDATA%\Gracil\settings.json` (redirected into the package's private folder
  when running as MSIX).
- Logs: `%LOCALAPPDATA%\Gracil\logs\`, daily rotation, a small number of files kept.
- No job history is persisted.

See [`../decisions/`](../decisions/) for the reasoning behind the main choices.
