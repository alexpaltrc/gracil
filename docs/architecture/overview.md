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
| `gracil-shell` | cdylib | `gracil_shell.dll`, the `IExplorerCommand` COM server for the Windows 11 context menu, built with the `windows` crate's `#[implement]` (ADR 0022). Depends only on `gracil-core` (the video extension list) and `gracil-windows`; counts videos and forwards them. | M7 |

Crates are created when they get real content, not ahead of time.

## Key flows

**Compression.** A job is a lightweight record (path, state, progress) kept in
`gracil_core::queue`. The scheduler runs one job at a time (ADR 0004): probe → plan the streams →
spawn FFmpeg writing to a reserved temporary sibling (`<name>.gracil-<id>.part`) → parse
`-progress pipe:1` (Pause suspends FFmpeg's threads through `gracil_windows::set_suspended`,
ADR 0013) → validate the output with FFprobe (HEVC, same duration) → finalize. A result
that is not smaller than the original is discarded (spec §40.1).

**Leftovers.** Every `.part` and parked `.bak` a process creates is written down in its own
ledger in `%LOCALAPPDATA%\Gracil\temporary\`, held open exclusively while the process runs. At
startup, before the engine starts, `gracil_core::leftovers::sweep` settles the ledgers no running
process holds: temporary files are deleted, a parked original goes back if its place is empty,
and anything on an absent drive or in use waits for the next start (ADR 0017). Gracil puts itself
in a kill-on-close job object at startup, so FFmpeg never outlives it (ADR 0018).

**Finding FFmpeg.** At startup, in the background: use an FFmpeg on `PATH` that has `ffprobe.exe`
beside it and the `libx265` encoder; otherwise Gracil's private copy; otherwise download the pinned
build, verify it and install it privately (ADR 0008).

**Choosing how to encode.** Capabilities are detected once per process: `ffmpeg -encoders` must
list `hevc_nvenc`, then a sub-second test encode must succeed. Each job then walks a ladder of
attempts, from fastest to most forgiving: NVENC with CUDA decoding → NVENC with software decoding
→ NVENC keeping only essential streams → `libx265` keeping only essential streams (without NVENC:
`libx265` full, then essential). See [`../development/ffmpeg.md`](../development/ffmpeg.md).

**The window.** `gracil-ui` is an Iced daemon. `app` holds the state and reacts to messages,
`view` draws Focus + Drawer from that state alone (`dial` draws the progress ring and its
controls, ADR 0014), `work` runs the engine and each compression on
their own threads and reports back as messages, `style` holds every design token, and `text` every
word shown, in English and Spanish (ADR 0011). Paths given on the command line or forwarded
by a later launch are queued like dropped files. A finished result waits in its temporary file until the user chooses Replace or
Save copy; `work::place` then calls `gracil_core::output` on its own thread, with `gracil_windows::replace_file` (`ReplaceFileW`) for the swap (ADR 0009).

**Closing and exiting.** Closing a window hides it; the process keeps running in the tray. Only
the tray menu's Exit quits. A process started from Explorer that never showed a window exits by
itself when its work is finished (spec §53, §54.1); one started with Windows stays (ADR 0024).

**Tray.** `gracil_windows::Tray` owns the notification icon on a thread of its own, with a hidden
window for its messages, and draws each state (Idle, Processing, Paused, Error) itself
(`tray/picture.rs`, ADR 0020). Menu choices arrive through `outside` like pipe requests. After
every update the app derives the tray state from the queue and refreshes the icon only when the
picture or tooltip would change. Exit with work running or waiting opens the window on a
confirmation; with nothing running it cancels nothing and leaves at once. If the tray can't be
created, closing the window behaves as Exit.

**Mini-window.** A click on the tray icon opens a second Iced window (`view::mini`), undecorated,
always on top and off the taskbar (ADR 0021). It opens hidden; `gracil_windows::place_near_tray`
puts it against the taskbar's edge on the icon's screen, and only then is it shown. The app keeps
its `window::Id` apart from the main window's, so closing it never counts as closing Gracil, and
drag-and-drop tracks which window files hover over. Its one setting lives in
`settings.rs` (`%LOCALAPPDATA%\Gracil\settings.txt`) and is toggled from the tray menu.

**Single instance.** The first process creates the named mutex `Local\Gracil.SingleInstance`
and serves the pipe `\\.\pipe\Gracil.<session>` (`gracil_windows::claim`, `serve`). Any later
invocation (a new launch, "Open with", Explorer) sends its command line over the pipe
(`gracil_windows::forward`) and exits. `gracil-ui`'s `request` parses the command line and pipe
messages alike, and `outside` turns them, from any thread, into messages for the app. Gracil
starts with no window; a request without a choice opens it, while `--replace` and `--save-copy`
work in the background. A Gracil whose window never opened ends itself when that work is done
(ADR 0019).

**File Explorer.** `gracil_shell.dll` gives Explorer one `IExplorerCommand`, titled "Gracil" or
"Gracil (N)" by the number of selected files with a video extension
(`gracil_core::media::looks_like_video`) and hidden when there are none, with **Compress and
replace** and **Compress and save a copy** under it. A click returns at once; a thread of the DLL
forwards the paths over the pipe when Gracil is running, or starts Gracil with as many paths as
fit on its command line and forwards the rest (ADR 0022). Packaged, it starts Gracil through the
package's `gracil.exe` alias, so Gracil keeps the package's identity (ADR 0023); otherwise it
starts the `gracil.exe` beside the DLL.

**Starting with Windows.** The package's hidden `Startup` application runs `gracil.exe --startup`
at sign-in (`windows.startupTask`). That request opens nothing, and marks the app `resident`:
it stays in the tray for the session instead of leaving when background work is done
(ADR 0024).

## Data on disk

- Settings: `%LOCALAPPDATA%\Gracil\settings.txt`, one `name=value` per line.
- When Gracil runs from its package, what it writes under `%LOCALAPPDATA%` lands in the package's
  private folder and is removed on uninstall (ADR 0023).
- Logs: `%LOCALAPPDATA%\Gracil\logs\`, one plain-text file per day, the last seven kept.
- No job history is persisted.

See [`../decisions/`](../decisions/) for the reasoning behind the main choices.
