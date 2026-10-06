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
ledger in the data folder's `temporary\`, held open exclusively while the process runs. At
startup, before the engine starts, `gracil_core::leftovers::sweep` settles the ledgers no running
process holds: temporary files are deleted, a parked original goes back if its place is empty,
and anything on an absent drive or in use waits for the next start (ADR 0017). Gracil puts itself
in a kill-on-close job object at startup, so FFmpeg never outlives it (ADR 0018).

**Finding FFmpeg.** At startup, in the background: the `ffmpeg` folder beside `gracil.exe`, or,
while developing, an FFmpeg on `PATH`; either needs `ffprobe.exe` beside it and the `libx265`
encoder. Nothing is ever downloaded (ADR 0050, ADR 0053).

**Choosing how to encode.** Capabilities are detected once per process: `ffmpeg -encoders` must
list a card encoder (`hevc_nvenc`, then `hevc_qsv`, then `hevc_amf`), then a sub-second test encode
must succeed; the first encoder that passes is used, with its richest settings tier that passes
(ADR 0052). Before encoding, `gracil_core::search` measures short samples with VMAF to
pick the video's quality level and predict its size (ADR 0028, ADR 0029); a video already
compressed too hard to shrink that way is searched again at a slightly lower quality target (ADR
0033). `gracil_core::standard` decides frame rate (to 30) and audio conversion; the picture size
is never changed. Each job then walks a ladder of attempts, from fastest to most forgiving: NVENC with
CUDA decoding → NVENC with software decoding → NVENC keeping only essential streams → `libx265`
keeping only essential streams (Quick Sync and AMF: the card with software decoding, full then
essential, then `libx265` essential; without a card: `libx265` full, then essential). See [`../development/ffmpeg.md`](../development/ffmpeg.md).

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
drag-and-drop tracks which window files hover over. Its setting lives in
`settings.rs` (`settings.txt` in the data folder) and is toggled from the tray menu. A
finished result can be decided in it as in the main window.

**Opening without white.** Iced's windows have no class background, so Windows shows a new one
white until its first WGPU frame is presented, and cloaking doesn't help (presents to a cloaked
window are discarded). Both windows therefore open hidden and are shown parked at -20000
(`gracil_windows::park`). `listen_raw` counts their `RedrawRequested` events; after three frames,
50 ms after the first one (a window with nothing moving draws only one), or 3 s at the latest
(logged when it happens), the main window returns to where Windows put it and the mini-window is
placed by the tray (`App::reveal`, `App::bring_into_view`). The system font list, which Iced
reads on its first text, is loaded on a thread at start-up so a cold first frame is quicker.

**Always.** The main window's foot holds a three-way pill (`pill.rs`, a small custom widget that
slides its stone itself) for `settings.always`: replace, ask, or save a copy. When a job finishes,
`on_report` places it with File Explorer's choice for that job if there is one, else with
`settings.always`; in the middle nothing happens until the user chooses (ADR 0025).

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
fit on its command line and forwards the rest (ADR 0022). It starts the `gracil.exe` recorded in its
registration (ADR 0050), or the one beside the DLL.

**Starting with Windows.** The current user's `Run` key starts `gracil.exe --startup` at sign-in.
That request opens nothing, and marks the app `resident`: it stays in the tray for the session
instead of leaving when background work is done (ADR 0024).

**Its place in Windows** (`place.rs`, ADR 0050). Gracil is a portable folder with no installer. On
every start, off the UI thread, a whole Gracil folder (FFmpeg and `gracil_shell.dll` beside
`gracil.exe`) writes the `Run` value again and registers File Explorer's classic menu for the
current user (`gracil_windows::add_to_explorer`): the `IExplorerCommand` class, pointing at a copy
of the DLL in `data\shell\` so the shipped one is never held, with the program and data folder
recorded beside it for the DLL to find. With the menu turned off in the tray, the registration is
removed if it points at this folder. A development build in `target` leaves Windows alone.

## Data on disk

All in `data\` beside `gracil.exe`, or `%LOCALAPPDATA%\Gracil` when that can't be written
(`toolchain::default_private_dir`):

- Settings: `settings.txt`, one `name=value` per line.
- Logs: `logs\`, one plain-text file per day, the last seven kept.
- Unfinished files: `unfinished.txt`; leftovers' ledgers: `temporary\`; the menu's DLL: `shell\`.
- No job history is persisted.

See [`../decisions/`](../decisions/) for the reasoning behind the main choices.
