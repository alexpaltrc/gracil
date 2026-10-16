# Architecture overview

Gracil is one Rust process (`gracil.exe`) that owns a job scheduler and drives external `ffmpeg.exe`
/ `ffprobe.exe` and `cjpegli.exe` processes. File Explorer reaches it through plain commands that
start `gracil.exe`; no Gracil code runs inside `explorer.exe` (ADR 0058).

```
 File Explorer: Send to, main menu ──(--explorer)──────┐
                                                       │ named pipe (paths + action)
 2nd launch of gracil.exe ─────────────────────────────┤
                                                       ▼
 ┌──────────────────────── gracil.exe ─────────────────────────┐
 │ gracil-ui        Iced daemon: windows, tray, scheduler, pace │
 │ gracil-windows   mutex, pipe server, tray, suspend, replace  │
 │ gracil-core      jobs, probe, search, encode, images, finish │
 └───────────────────────────────┬─────────────────────────────┘
                                 │ std::process::Command (no shell)
          ffmpeg.exe / ffprobe.exe / cjpegli.exe (in the zip beside gracil.exe)
```

## Crates

| Crate | Kind | Responsibility | Introduced |
|---|---|---|---|
| `gracil-core` | lib | Job state model and the list (`queue`), output naming, FFprobe inspection, capability detection, the quality search, FFmpeg argument construction, progress parsing, image compression, output validation, leftovers. No UI dependency. Its `testing` module, behind the `test-support` feature, gives every crate's tests their temporary folders. | M0 |
| `gracil-windows` | lib | Everything that needs Windows itself (`windows-sys`): single-instance mutex, named-pipe IPC, process suspend/resume, `ReplaceFileW`, the tray, taskbar geometry, colors and accessibility settings, temperature, idleness, power and processor limits, File Explorer's Send to shortcut and main menu (the `windows` crate for their few COM calls). The only crate allowed `unsafe`. | M4/M5 |
| `gracil-ui` | bin `gracil` | Iced + WGPU application: main window (Focus + Drawer), mini-window, settings, the scheduler and its pace, wiring core ↔ Windows layer. | M2 |

Crates are created when they get real content, not ahead of time.

## Key flows

**Compression.** A job is a lightweight record (path, state, progress) kept in
`gracil_core::queue`. The scheduler, in `gracil-ui` (`app/scheduler.rs`, ADR 0040), compresses one
video at a time while the next one is got ready, and images beside it. A video: probe → measure
its quality level (below) → plan the streams → spawn FFmpeg writing to a reserved temporary
sibling (`<name>.gracil-<id>.part`) → parse `-progress pipe:1` (Pause suspends FFmpeg's threads
through `gracil_windows::set_suspended`, ADR 0013) → validate the output with FFprobe (HEVC, same
duration) → finalize. An image goes through `gracil_core::images`: a PNG stays a PNG, the rest
becomes a JPEG written by jpegli, scored against the original (ADR 0034). A result that is not
smaller than the original is discarded (spec §40.1).

**Pace.** `pace.rs` decides how much runs at once (ADR 0040): images at once from the processor's
threads, whether the person is using the computer and the memory free. A thread reads the
computer once a second, a *breath* (ADR 0045, ADR 0046): its warmth, whether it is idle and
whether it saves energy. While warm, fewer images run and each breath starts with a short rest,
longer the warmer it is (ADR 0043). Windows itself holds Gracil to part of the processor, less
while the computer is in use or on battery (ADR 0049). The scheduler tells `pace::working` what
is going on (`Activity`), so the computer is read every second only while files are worked on,
the battery alone while files wait, and nothing otherwise, and only the card of the encoder in use
is watched (ADR 0063). `pace::sleeping`, called from Windows' suspend/resume notification, rests
the work around sleep and lets `gracil_core::share::sleeps` retry the step sleep cut short. The
core reaches Windows only through `share::set_host` (tools started, image threads, free memory),
so it stays free of Windows code.

**Leftovers.** Every `.part` and parked `.bak` a process creates is written down in its own
ledger in the data folder's `temporary\`, held open exclusively while the process runs. At
startup, before the engine starts, `gracil_core::leftovers::sweep` settles the ledgers no running
process holds: temporary files are deleted, a parked original goes back if its place is empty,
and anything on an absent drive or in use waits for the next start (ADR 0017). Gracil puts itself
in a kill-on-close job object at startup, so FFmpeg never outlives it (ADR 0018); each tool also
joins a job nested inside it that holds the tools alone to their memory limit (ADR 0063).

**Finding FFmpeg.** At startup, in the background: the `ffmpeg` folder beside `gracil.exe`, or,
while developing, an FFmpeg on `PATH`; either needs `ffprobe.exe` beside it and the `libx265`
encoder. Nothing is ever downloaded (ADR 0050, ADR 0053).

**Choosing how to encode.** Capabilities are detected once per process: `ffmpeg -encoders` must list
a card encoder (`hevc_nvenc`, then `hevc_qsv`, then `hevc_amf`), then a sub-second test encode must
succeed; the first encoder that passes is used, with its richest settings tier that passes (ADR
0052). Before encoding, `gracil_core::search` measures short samples with VMAF to pick the video's
quality level and predict its size (ADR 0028, ADR 0029); a video already compressed too hard to
shrink that way is searched again at a slightly lower quality target (ADR 0033). The search
measures as few levels as it can: it steps by how fast the score falls, jumps where the size
may stop being too big, and tells first, from one encode without the measure, whether anything
can be small enough (ADR 0067).
`gracil_core::standard` decides frame rate (to 30) and audio conversion; the picture size is never
changed. Each job then walks a ladder of attempts, from fastest to most forgiving: NVENC with CUDA
decoding → NVENC with software decoding → NVENC keeping only essential streams → `libx265` keeping
only essential streams (Quick Sync and AMF: the card decoding too where its one-frame test passed,
ADR 0065, then the card with software decoding, full then essential, then `libx265` essential;
without a card: `libx265` full, then essential). See
[`../development/ffmpeg.md`](../development/ffmpeg.md).

**The window.** `gracil-ui` is an Iced daemon. `app` holds the state and reacts to messages; it is a
folder, with one file per subject (the list, the scheduler, what is shown about the list, the
windows, the colors, the keys, leaving). `view` draws Focus + Drawer from that state alone (`dial`
draws the progress ring and its controls, ADR 0014; `done` the finished screen), `work` runs the
engine and each compression on their own threads and reports back as messages, `style` holds every
design token, and `text` every word shown, in English and Spanish (ADR 0011). `motion` eases every
animation and stills them all when Windows' **Animation effects** are off; `style` follows Windows'
accent and contrast themes (ADR 0031, ADR 0061). `data` writes Gracil's small text files whole or
not at all. Paths given on the command line or forwarded by a later launch are queued like dropped
files. A finished result waits in its temporary file until the user chooses Replace or Save copy;
`work::place` then calls `gracil_core::output` on its own thread, with
`gracil_windows::replace_file` (`ReplaceFileW`) for the swap (ADR 0009).

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
always on top and off the taskbar (ADR 0021). Making a window takes about half a second, showing
one a few hundredths, so it is made once, hidden, two seconds after start (or at the first click),
and closing only hides it; while hidden it draws nothing. `gracil_windows::place_near_tray` puts it
against the taskbar's edge on the icon's screen each time it is shown. The app keeps
its `window::Id` apart from the main window's, so closing it never counts as closing Gracil, and
drag-and-drop tracks which window files hover over. Its setting lives in
`settings.rs` (`settings.txt` in the data folder) and is toggled from the tray menu. A
finished result can be decided in it as in the main window.

**Opening without white.** Iced's windows have no class background, so Windows shows a new one white
until its first WGPU frame is presented, and cloaking doesn't help (presents to a cloaked window are
discarded). Both windows therefore open hidden and are shown parked at -20000
(`gracil_windows::park`). `listen_raw` counts their `RedrawRequested` events; after three frames, 50
ms after the first one (a window with nothing moving draws only one), or 3 s at the latest (logged
when it happens), the main window returns to where Windows put it and the mini-window is placed by
the tray (`Gracil::reveal`, `Gracil::bring_into_view`, in `app/windows.rs`). The system font list,
which Iced reads on its first text, is loaded on a thread at start-up so a cold first frame is
quicker.

**Always.** The main window's foot holds a three-way pill (`pill.rs`, a small custom widget that
slides its stone itself) for `settings.always`: replace, ask, or save a copy. When a job finishes,
`on_report` places it with the choice made in advance for that job (`--replace`, `--save-copy`)
if there is one, else with `settings.always`; in the middle nothing happens until the user
chooses (ADR 0025). An `--explorer` request takes `settings.always` as its choice when it arrives
(ADR 0058).

**Single instance.** The first process creates the named mutex `Local\Gracil.SingleInstance`
and serves the pipe `\\.\pipe\Gracil.<session>` (`gracil_windows::claim`, `serve`). Any later
invocation (a new launch, "Open with", Explorer) sends its command line over the pipe
(`gracil_windows::forward`) and exits. `gracil-ui`'s `request` parses the command line and pipe
messages alike, and `outside` turns them, from any thread, into messages for the app. Gracil
starts with no window; a request without a choice opens it, while `--replace` and `--save-copy`
work in the background, and so does `--explorer` when **Always** is set. A Gracil whose window
never opened ends itself when that work is done (ADR 0019).

**File Explorer** (ADR 0058). Both of Gracil's places in File Explorer start
`gracil.exe --explorer` with what was picked, which forwards it over the pipe like any later
launch. **Send to** is a shortcut, `Gracil.lnk`, in the user's own Send to folder
(`gracil_windows::add_to_send_to`). The **main menu** is plain commands for the whole computer
under `HKEY_LOCAL_MACHINE\SOFTWARE\Classes` (`gracil_windows::add_main_menu`, words from
`explorer.rs`): `SystemFileAssociations\.<ext>\shell\Gracil` for every video and image type, with
`MultiSelectModel` `Player`, and `Directory\shell\Gracil` with three numbered subcommands (all,
`--videos`, `--images`, ADR 0035). Per-user (`HKCU`) verbs are not used: File Explorer on
Windows 11 ignored them, though in-process verb listings showed them. The running Gracil applies
**Always** to an `--explorer` request (ADR 0025); in the middle, the window opens and asks.

**Starting with Windows.** The current user's `RunOnce` key starts `gracil.exe --startup` at the
next sign-in; Windows deletes the value as it runs it, and every start writes it again while
`start_with_windows` is on, so a deleted folder leaves nothing behind (ADR 0054).
That request opens nothing, and marks the app `resident`: it stays in the tray for the session
instead of leaving when background work is done (ADR 0024).

**Its place in Windows** (`place.rs`, ADR 0050). Gracil is a portable folder with no installer. On
every start, off the UI thread, a whole Gracil folder (FFmpeg beside `gracil.exe`) writes the
`RunOnce` value and the Send to shortcut again, each as its tray setting says, and removes what
the per-user menu of ADR 0050 left (its `HKCU` keys, both COM classes, the DLL copies in
`data\shell\`). The main menu is never written at start-up: it needs an administrator. Its tray
item starts `gracil.exe --add-main-menu` (or `--remove-main-menu`) as an administrator, which
writes or removes it with no window and exits; a `GracilProgram` value beside the folder command
tells which `gracil.exe` wrote it, so the tray ticks it only for that one. Gracil runs in a job
object (ADR 0018), so it asks File Explorer, through the desktop's Shell automation, to start the
administrator's Gracil (`gracil_windows::run_as_administrator`), then watches the menu for up to
two minutes. A development build in `target` leaves Windows alone.

## Data on disk

All in `data\` beside `gracil.exe`, and nowhere else; when that can't be written there is none
and nothing is saved (`toolchain::default_private_dir`, ADR 0054):

- Settings: `settings.txt`, one `name=value` per line.
- Logs: `logs\`, one plain-text file per day, the last seven kept.
- Unfinished files: `unfinished.txt`; leftovers' ledgers: `temporary\`. A `shell\` folder left
  by the former menu DLL is removed on start (ADR 0058).
- Quality-search hints: `hints.txt` (`hints.rs`), the level last chosen for each encoder, size
  step, source codec and search, under a first line naming Gracil's version, `ffmpeg.exe`'s size
  and time of change, the encoder and its settings; read once at engine start, written every
  three minutes when changed and on leaving (ADR 0070).
- No job history is persisted.

See [`../decisions/`](../decisions/) for the reasoning behind the main choices.
