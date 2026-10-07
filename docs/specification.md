# Gracil — Master Product & Engineering Specification

This is the authoritative product and engineering specification for Gracil, condensed from the
original brief (wording tightened, requirements unchanged; section 83, the instructions for the
first planning response, is omitted). Section numbers are stable references; do not renumber them.
Additions after the original specification use sub-numbers (for example `54.1`).

---

# 1. Product priorities

Gracil has three primary priorities, in this order:

1. Ease of use
2. Optimization
3. Design

All three are extremely important. The ordering exists only to resolve conflicts.

A technically elegant implementation that makes the product harder to use is a regression.
A beautiful interface that wastes resources unnecessarily is also a regression.
A highly optimized implementation that becomes difficult to maintain without measurable benefit should be reconsidered.

# 2. Product philosophy

Gracil does one thing: **Compress video.**

It should feel like a small, focused consumer application. The interaction philosophy should be
closer to Squoosh than to HandBrake.

The user should not need to understand: FFmpeg, H.265, HEVC, NVENC, NVDEC, CUDA, x265, CQ, CRF,
bitrate, rate control, presets, hardware acceleration, muxers, streams. Do not expose those
concepts in the normal UI. The application makes technical decisions automatically.

Gracil should be ultra minimalistic in everything it does. Minimalism here means removing
unnecessary complexity, not removing useful functionality.

# 2.1 Seamless by default

*Added after the original specification.*

Everything in Gracil must feel seamless. The user never sets anything up, never waits on a
technical step they can see, and never meets a dialog that exists only because of how Gracil works
inside. The bar is high and applies everywhere: subtle but elegant design and animation, and code
and directories that are just as elegant, optimized, organized and functional.

# 2.2 Free, private, no account, no internet

*Added after the original specification.*

Gracil is always free and private. It never asks for an account and never needs an internet
connection to work. FFmpeg, which Gracil cannot compress without, comes inside Gracil's zip
(§20.1), so Gracil never connects to the internet at all (ADR 0050, ADR 0053).

# 2.3 Quiet signals instead of dialogs

*Added after the original specification.*

When something does not go as expected, Gracil does not open a dialog to explain it. It tells the
user inside the interface, in a way that feels natural: a control turns off, a small mark
appears, and hovering it gives a short, plain explanation. The user stays in their flow, and the
interface keeps its calm, minimal look. Dialogs are only for decisions with real consequences,
such as exiting while a compression is running (§54.1).

Example: when the compressed video is not smaller than the original, the save buttons are turned
off and a small "!" appears next to them. Hovering it says that Gracil does not save the result
because it could not make the file smaller than the original (§40.1).

# 3. Product identity

Application name: **Gracil** — always written without an accent.

Do not rename it to: Grácil, Gracile, Graysil, Graceful, Gracil Video Compressor.

The intended English pronunciation is approximately **GRAY-sil**. The pronunciation does not
affect the spelling.

The final application icon has not been designed yet. Use a clearly temporary development icon.
Do not invent or finalize the permanent icon. The final icon will later be designed according to
current Windows 11 iconography and visual guidelines.

*Update (ADR 0015):* the icon has been designed: the marble ("la canica"), with the Piedra palette, in
`assets/icons/`.

*Update (ADR 0031):* the Piedra palette is replaced by Windows 11's own colors and the user's
accent color, followed live.

# 4. Supported platform

Target: Windows 11, x86-64 / 64-bit Windows initially. Do not build a 32-bit application.

References to Win32 APIs do NOT mean the application is 32-bit. Win32 is the historical name for
many native Windows desktop APIs that are also used by 64-bit applications.

Future ARM64 support may be evaluated later but is not required now.

# 5. Primary technology stack

Use: Rust, Iced, WGPU as the Iced renderer, FFmpeg, FFprobe, windows-rs for Windows-specific APIs,
a lightweight native tray implementation where required. Gracil ships as a portable zip
published on GitHub (ADR 0050).

Do not use: Electron, Chromium, WebView, Tauri, Node.js, a local HTTP server, a browser-based
frontend.

Use current stable versions at implementation time unless there is a documented compatibility
reason not to.

# 6. WGPU and video processing are separate systems

Do not conflate UI GPU rendering with video encoding. WGPU is responsible for rendering Gracil's
interface. Video processing uses FFmpeg.

On supported NVIDIA hardware NVDEC/CUDA may decode video and NVENC may encode video. These are
independent from WGPU. Gracil choosing WGPU does not mean WGPU should implement video compression.

# 7. Repository and directory organization

Project organization must be treated as important from the first commit. Keep the repository
predictable and clean. A reasonable starting structure is:

```
gracil/
├── crates/
│   ├── gracil-core/
│   ├── gracil-ui/
│   └── gracil-windows/
├── docs/
│   ├── architecture/
│   ├── decisions/
│   └── development/
├── assets/
│   └── icons/
├── packaging/
│   └── ffmpeg.psd1
├── scripts/
├── Cargo.toml
├── Cargo.lock
├── CHANGELOG.md
├── README.md
└── .gitignore
```

This structure may evolve when there is a real reason. Do not create directories merely to appear
organized. Do not create empty architectural layers that provide no value. Every directory and
abstraction should have a clear responsibility.

# 8. Architectural separation

- **gracil-core**: video jobs, scheduler, file metadata, FFmpeg command construction, FFprobe
  inspection, hardware capability detection, encoder backend selection, progress, pause/resume,
  cancellation, output naming, safe replacement, metadata preservation, error handling.
- **gracil-ui**: Iced, WGPU, main window, Focus + Drawer interface, drag and drop, user-facing
  state, settings, tray mini-window UI.
- **gracil-windows**: Windows APIs, single-instance behavior, IPC, process management, system tray
  integration, window positioning, filesystem replacement, Explorer integration support.
- **gracil-shell**: modern Windows 11 File Explorer context-menu extension. *Removed by
  [ADR 0058](decisions/0058-explorer-send-to-and-main-menu.md):* File Explorer reaches Gracil
  through **Send to** and plain commands that start `gracil.exe`, written by gracil-windows.

Most importantly: **gracil-core must not depend on Iced.** The compression engine must be usable
without initializing Iced, WGPU or a visible window. This is required for background compression
initiated from File Explorer.

# 9. Git from the beginning

The project must use Git from the beginning. The repository history should remain useful and
understandable. Prefer small, logically complete commits. Do not create giant commits containing
unrelated changes. Do not introduce an unnecessarily complicated Git workflow.

`main` may contain the normal development history. Use temporary branches when they provide a real
benefit. Released versions must be represented by Git tags (`v0.0.1`, `v0.0.2`). Do not rewrite
published release history.

# 10. Versioning

Versions begin at `0.0.1`. The active development version always carries the `-dev` suffix
(`0.0.1-dev`). When that version is released it becomes `0.0.1` with the Git tag `v0.0.1`.
Immediately after beginning work on the next version, change the development version to
`0.0.2-dev`. Then `0.0.2`, `v0.0.2`, `0.0.3-dev`, and so on.

*Update (ADR 0027, ADR 0050):* the first public release on GitHub is `1.0.0`, developed as
`1.0.0-dev` and tagged `v1.0.0`; after it, `1.0.1-dev`, and so on.

Keep the version synchronized wherever practical: Cargo workspace/package metadata, application
metadata, the name of the portable zip, About information when eventually implemented, release
scripts.

# 11. CHANGELOG philosophy

Maintain `CHANGELOG.md` from the beginning. The changelog is written for normal Gracil users. It
must NOT read like Git commit history. Avoid technical jargon. A person who does not understand
programming should understand every entry.

Good:

```
## 0.0.2

### New
- You can now add several videos at the same time.

### Improved
- Gracil preserves more information from the original video.

### Fixed
- Fixed a problem that could prevent videos with special characters in their name from being saved.
```

Bad: "Refactored PathBuf UTF-16 serialization.", "Fixed IExplorerCommand IPC race.", "Migrated
encoder abstraction to trait objects.", "Changed FFmpeg stderr parser."

Technical details belong in Git commits, code, architecture documentation and technical
development notes. The changelog should feel calm and friendly. Do not write alarming release
notes when a neutral explanation is sufficient. Do not exaggerate bugs. Do not expose
implementation details that provide no value to the user.

# 12. Architecture Decision Records

Use lightweight ADRs when a decision is important enough that a future developer or AI might
otherwise undo it without understanding why. Store them under `docs/decisions/`. Do not write an
ADR for trivial implementation details. Keep ADRs concise.

# 13. Code quality philosophy

The code must not merely look professional. It must actually be professional. Optimize for
clarity, correctness, maintainability, predictable behavior, testability, low resource usage and
minimal unnecessary complexity. Start from a clean base so future optimization does not require
undoing avoidable complexity.

# 14. Avoid overengineering

Do not introduce unnecessary abstractions, traits, generic layers, background services,
dependencies or wrappers, speculative systems for features that do not exist, or duplicated
functionality already handled well by Rust, Windows, Iced, WGPU, or FFmpeg. Abstractions should
solve current architectural problems or clearly defined near-term requirements.

# 15. Dependency discipline

Before adding a dependency: (1) determine whether Rust's standard library or an existing
dependency already solves the problem; (2) evaluate whether it is actively maintained; (3) prefer
focused libraries over massive frameworks when functionality is small; (4) avoid dependencies
that introduce large transitive dependency trees without justification. Do not reimplement
complex proven functionality merely to reduce dependency count.

# 16. Rust quality rules

Use idiomatic Rust. Run and maintain `cargo fmt`, `cargo clippy`, `cargo test`. Keep Clippy
clean. Do not suppress warnings globally.

Avoid `unwrap()` and `expect()` in production code unless the condition is a genuine invariant
and the reason is obvious or documented. Use structured error types. Do not swallow errors
silently.

Avoid `unsafe`. If `unsafe` is genuinely required for Windows integration: isolate it, minimize
its scope, document the safety assumptions, test surrounding behavior.

Comments should explain WHY something exists when the reason is non-obvious. Delete dead code
instead of leaving abandoned implementations commented out.

# 17. Resource philosophy

Gracil must remain lightweight. Do not load complete videos into application memory. For each
queued video store lightweight information: path, filename, size, relevant FFprobe information,
job status, progress, output action. There must be no arbitrary application-level
number-of-files limit.

# 18. Idle resource usage

When idle: CPU and GPU usage effectively idle; no continuous WGPU redraw loop; no unnecessary
polling; no unnecessary background worker; reasonable memory. Use event-driven behavior. Do not
render at 60 FPS simply because the window exists. Measure idle resource consumption from the
first prototypes.

# 19. FFmpeg philosophy

Gracil orchestrates FFmpeg. Do not attempt to reinvent FFmpeg. Allow FFmpeg to make its own
internal decisions. Do not manually set thread counts, obscure buffers, low-level decoder
parameters or unnecessary tuning values unless testing demonstrates a measurable reason.

Gracil primarily decides: what file is processed; what supported backend is appropriate; what
output behavior the user requested; when the next job begins; how the result is safely stored.

# 20. FFmpeg distribution

Users must not need to install FFmpeg separately. Bundle a known-compatible build of `ffmpeg.exe`
and `ffprobe.exe` and document its version, build source/provenance, enabled capabilities and
licensing obligations (`docs/development/ffmpeg.md`).

Keep FFmpeg as an external process. Do not link FFmpeg libraries directly without discussing the
change first.

# 20.1 Finding FFmpeg

*Added after the original specification; rewritten by ADR 0053.*

Gracil uses the copy in the `ffmpeg` folder beside `gracil.exe` (ADR 0050). While developing,
when that folder doesn't exist, it uses an FFmpeg found on `PATH`. A copy is only used if FFmpeg
itself confirms it has what Gracil needs (at least the CPU HEVC encoder) and `ffprobe.exe` sits
next to `ffmpeg.exe`. Gracil never downloads FFmpeg, never installs it system-wide and never
changes PATH. If no usable copy is found, a quiet line says Gracil is missing part of itself and
should be downloaded again.

Gracil looks for FFmpeg as soon as it launches, so it is normally ready before the first video
arrives. Videos added before then simply wait in their normal waiting state.

# 21. Reference NVIDIA compression behavior

```
ffmpeg -i "video.mp4" -c:v hevc_nvenc -preset p6 -tune hq -rc vbr -cq 27 -b:v 0 -c:a copy "video_comprimido.mp4"
```

This was the starting point. Gracil's own quality standard replaces its fixed values
(ADR 0028):

- HEVC / H.265, always 10-bit; NVENC when available and appropriate, with the best settings the
  GPU accepts (preset P7, high-quality tune, VBR with no target bitrate, lookahead,
  multipass; no adaptive quantization, which made files larger, ADR 0029).
- Each video finds its own quality level: short samples are encoded and measured against the
  source (VMAF), and the smallest level that still scores at least 95 is used. CQ 27 remains the
  fallback when no measurement is possible.
- Every video keeps its resolution (ADR 0033; at first, a short side above 1080 became 1080).
- A video that is already compressed is compressed again at a VMAF of 90 instead of 95, without
  asking, when that makes it clearly smaller (ADR 0033).
- Above 30 frames per second becomes 30; 30 or fewer is kept.
- Audio is copied where compatible, except heavy audio (lossless, or a very high bitrate), which
  becomes AAC.

These parameters must not appear in the normal user interface.

# 22. Automatic encoder selection

Users should not choose CPU vs GPU. Gracil chooses automatically:

1. Determine whether NVIDIA HEVC NVENC is actually usable.
2. Do not assume that detecting an NVIDIA GPU means NVENC works.
3. Verify FFmpeg encoder availability.
4. Validate that required driver/runtime support works.
5. Prefer NVENC when it is genuinely available and suitable.
6. Otherwise use CPU-based HEVC encoding with libx265.

The architecture should make it possible to add additional hardware backends later (Intel Quick
Sync, AMD) without rewriting the application. Do not expose backend names in the normal UI.

# 23. CPU fallback

Gracil must work on machines without a dedicated GPU, a recent NVIDIA GPU, NVENC or supported GPU
drivers. If a suitable GPU backend is unavailable, use the CPU automatically.

# 24. Hardware decoding

When using NVIDIA hardware and the source is supported, use NVDEC/CUDA decoding when beneficial.
Hardware decoding is an optimization, not a requirement. If the codec is unsupported,
initialization fails, a driver problem exists, or FFmpeg reports hardware decoding cannot
proceed, gracefully retry using software decoding. If NVENC remains usable, continue using NVENC.
Do not fail an otherwise valid compression merely because GPU decoding was unavailable.

# 25. Capability detection

Use FFmpeg itself as an authoritative source (`-encoders`, `-decoders`, `-hwaccels`). Perform
small runtime validation when static listing is insufficient. Cache results. Do not run
expensive probes every time a file is dropped. Do not assume support from GPU model names.

# 26. Video queue responsibility

Gracil maintains a lightweight scheduler that decides which file is processed next; FFmpeg
processes each file. Initially run one heavy compression at a time unless measured testing
establishes a compelling reason to change that. Keep scheduler policy simple.

# 27. Adding files

Drag-and-drop is mandatory (one or many videos from File Explorer). A normal file picker exists
as a secondary method. Dragging files sends their paths; do not load contents into memory.

# 28. Additive drag-and-drop behavior

Newly dropped files are always ADDED to the current set; they never replace existing files. A new
drag operation is not a new session. The same applies to the file picker.

# 29. Duplicate paths

Exact duplicate-path behavior is not yet a product requirement. Choose the simplest safe interim
behavior, keep it easy to change, and document it.

*Current behavior (2026-10-03, at the product owner's request):* the list never stops a file from
being compressed again. Adding a path whose entry is finished (put in place, failed or cancelled)
removes that entry and adds the file again as new. Adding a path that is still waiting, running,
or waiting for the replace-or-copy choice does nothing.

# 30. Multiple files and UI philosophy

The application internally has a queue/scheduler. The interface should NOT look like a
traditional download manager with a dense permanent list. Gracil uses **Focus + Drawer**.

# 31. Focus + Drawer design

The currently active video gets visual priority. The remaining queue stays visually quiet.

```
┌─────────────────────────────────────────┐
│                 Gracil                  │
│             Vacation.mp4                │
│        ━━━━━━━━━━━━━━━────  68%         │
│              Pause                      │
│        8 videos  ·  View files          │
└─────────────────────────────────────────┘
```

The exact typography, spacing, animation and visual appearance are not final. The interaction
model IS final.

*Current look (ADR 0014):* progress is a ring the marble fills clockwise, with pause/play and
stop as symbols in its middle instead of the words in the sketch.

# 32. Drawer behavior

Selecting the subtle file-count control opens the file drawer, which lets the user inspect the
current set of files, remove individual removable items, and quickly clear removable files. The
drawer is not visually dominant. When closed, the user should mostly forget a queue exists.

The drawer opens and closes at once however long the list is: every row has the same height and
only the rows in sight (and a few beyond) are built; the rest of the list is empty space of the
right height, so the scroll bar stays true.

# 32.1 Picking a file, and the way back (ADR 0036)

Clicking a file in the drawer shows it in the focus, and its row stays lightly marked. Picking is
a temporary look, never a trap: clicking the same row again, closing the drawer, or clicking the
quiet "‹ name · status" line under the picked file returns the focus to the work in progress.
That line appears only while something is being compressed or a result waits for a choice, and
it is not the picked file.

# 32.2 Sizes (ADR 0036)

A result shows its size before and after under its choices ("120 MB → 78 MB"). The drawer row
of a placed file says how much smaller and where it went ("−35% · Replaced"). With two or more
results on the list, the drawer opens with their sum ("In all: 3.4 GB → 2.2 GB, 35% smaller")
and the footer adds the room freed ("1.2 GB smaller"). Only files still on the list count. Sizes
are counted the way File Explorer counts them (1 KB = 1024 bytes). Every number, sizes and counts alike, is written the way Windows writes
numbers for the user's region: its decimal mark and its digit grouping (*15,031 files*,
*1.2 GB* in Mexico; *15.031 archivos*, *1,2 GB* in Spain).

# 33. Active item behavior

*Update (2026-10-04, ADR 0040):* several files can be worked on at once: one video compressing,
the next one getting ready, and images beside it. Everything below applies to each of them.

The currently processing file must not disappear because the user pressed a generic Clear button.
Removing an actively processing file is a cancellation action and must be explicit. Do not
overload "Remove" or "Clear" with cancellation semantics. The active job may expose a dedicated
Cancel action.

# 34. Clear button

Clear removes pending, completed, failed and cancelled items. Clear must NOT silently cancel an
active compression.

# 35. Individual removal

Every removable file has a subtle individual remove action. Removing a pending item removes it
from the scheduler. Removing a completed/failed item only removes it from the visible list.
Removing the active file is cancellation and is treated separately.

# 36. Job states

At minimum: Pending, Preparing, Compressing, Paused, Finalizing, Completed, Failed, Cancelled.
Transitions are deterministic. The UI derives from state. Avoid scattered boolean flags.

# 37. Starting compression

Two output intentions: **Compress and replace** and **Compress and save a copy**. Do not expose
codec choices. Do not require technical setup before starting.

*Update (2026-10-03):* the second intention was first named "Compress and save as new"; at the
product owner's request it now uses the more familiar wording "Compress and save a copy" (Spanish
"Comprimir y guardar como copia") everywhere, matching the **Save copy** button.

# 37.1 Choosing after compressing

*Added after the original specification (ADR 0009).*

In the main window, videos are compressed as soon as they are added. When a result is ready, the
window shows how much smaller it is and offers **Replace** and **Save copy** for it; with several
results waiting, both are offered for all of them at once. A result the user never chooses for is
discarded, and the original stays as it was. From File Explorer, the **Always** setting decides
the same way (ADR 0058).

# 38. Compress and replace

Keep the exact same folder and filename; preserve metadata where technically meaningful; never
destroy the source until the new file has been successfully created and validated. Never encode
directly over the source file. Use a temporary sibling output on the same volume.

# 39. Safe replacement

Protect the original against FFmpeg failure, application crash, power failure, cancellation,
out-of-space conditions, replacement errors, invalid output and interrupted finalization.

1. Encode to a temporary sibling file.
2. Confirm FFmpeg exited successfully.
3. Confirm the output exists.
4. Confirm it is non-empty.
5. Validate it using FFprobe.
6. Apply required metadata/filesystem preservation.
7. Perform the safest appropriate Windows replacement operation (investigate `ReplaceFileW`).
8. Only delete temporary/backup material when the replacement succeeded.

The source must remain recoverable if final replacement fails. This is a hard requirement.

# 40. Compress and save a copy

Create a new file in the same directory with the `Gracil-` prefix (`Vacation.mp4` →
`Gracil-Vacation.mp4`). Do not silently overwrite an existing file; if the name exists, create a
non-conflicting name using a simple predictable strategy (`Gracil-Vacation (2).mp4`).

*Amended after the original specification: the prefix is `Gracil-`, with a hyphen, not
`Gracil_`.*

# 40.1 Results that are not smaller

*Added after the original specification.*

If the compressed file is not smaller than the original, Gracil neither replaces the original nor
saves a copy. Following §2.3, there is no error dialog. The save actions are turned off for that
video and a small "!" sits beside them; hovering it explains that the file could not be made
smaller than the original, so it is not saved. The original is never touched.

*Update (ADR 0055):* such a file is labelled **Already compressed**, like a file that was already
about as small as it gets; the "!" says which. Replace and Save copy are not shown for it, since
there is nothing to choose.

# 41. Metadata preservation

Preserve source metadata as completely as the output format allows: container/global metadata,
creation metadata, rotation/orientation, chapters, language tags, title, artist, comments,
location metadata, relevant stream metadata. Do not fabricate metadata that cannot legitimately
survive the operation.

*Clarified after the original specification:* metadata preservation is best effort and never a
reason to reject a video. Gracil keeps everything that can be kept reliably. Whatever cannot be
carried over without complication is dropped, and what was dropped is recorded in the log, not
shown to the user.

# 42. Filesystem metadata

With Compress and replace, preserve appropriate filesystem properties (such as timestamps) where
doing so accurately represents replacing the contents of the original file. It should feel like
"the same file was compressed". Do not preserve properties in misleading or unsafe ways.

# 43. Audio

Preserve audio without recompression whenever the target container supports it. If
compatibility requires different handling, use the safest compatible strategy.

# 44. Other streams

Inspect the source with FFprobe and preserve compatible subtitles, chapters, attached artwork,
language information and secondary streams.

*Clarified after the original specification:* like metadata (§41), stream preservation is best
effort. Gracil never rejects a video because a secondary stream cannot be kept; it keeps the video
and audio and drops only what cannot be carried over, recording that in the log. A video fails
only when it cannot be compressed at all (for example, it is not a readable video), with a
human-readable explanation without muxer terminology.

# 45. Containers

~~Do not pretend to support every format. Use a deliberately safe supported set.~~
*Clarified after the original specification:* aim for wide compatibility. Any video FFmpeg can
read is accepted. ~~The output keeps the source's container when that container can hold HEVC
properly; otherwise Gracil produces an MP4.~~ *Changed after the original specification
(ADR 0012):* every output is an MP4, whatever the source format, carrying HEVC video and audio
plus whatever metadata and additional streams MP4 can hold.

# 46. Progress

Use FFmpeg machine-readable progress output (`-progress pipe:...`). Do not scrape human-oriented
console output. Use FFprobe for duration. Throttle UI updates to avoid unnecessary redraws.

# 47. Pause and resume

Gracil must support genuine Pause and Resume: suspend the running work so Resume continues the
same job. Pause does not terminate FFmpeg, discard progress or restart later. Investigate the
safest reliable Windows-native mechanism and keep it inside the Windows layer. If suspending a
GPU-backed FFmpeg process introduces platform or driver limitations, explain them before changing
the requirement. Do not silently redefine Pause as Cancel.

# 48. Cancellation

Cancellation stops the correct FFmpeg process, cleans Gracil-owned incomplete temporary output,
preserves the source and transitions the job to Cancelled. Never remove the original source as
part of cancellation.

# 49. Main UI

The empty state is immediately understandable: **Drop videos here**. Avoid technical codec terms,
terminal-like presentation, giant settings pages, dense tables, excessive borders, unnecessary
navigation, advanced encoder controls and permanent diagnostic information.

*Update (ADR 0055):* the empty window says **Drop videos or images here** with a way to choose
files, and the **Always** pill at its foot (ADR 0057); the list's line comes up with the first file.
Folders can be dropped too. **Esc** steps back, **Space** pauses and resumes, **Ctrl+O** chooses
files.

# 50. Design system

Build reusable visual primitives and design tokens (spacing, corner radius, typography hierarchy,
surface behavior, progress treatment, focus states, animation timing). Do not prematurely lock the
visual identity. The interaction architecture stays independent from visual choices.

# 50.1 Languages

*Added after the original specification (ADR 0011).*

Gracil is available in English and Spanish and follows the Windows display language, with no
language setting. All interface wording lives in one place, so each language stays complete.

# 51. WGPU optimization

No unnecessary continuous redraws. Measure idle GPU, idle CPU, idle memory and active UI overhead
from early prototypes.

# 52. Single-instance behavior

Gracil behaves as one logical instance. Further invocations (launching again, Explorer commands,
future external actions) are forwarded to the existing instance, which owns the scheduler. Use a
robust Windows mechanism such as a named mutex plus named pipes.

# 53. No permanent service

Do not install a permanent Windows service. Do not keep a background process alive forever merely
to make Explorer integration possible. Start Gracil when needed. If Gracil was launched only for
background work and all jobs are finished, no window must remain visible and no other reason
exists to stay alive, allow it to exit cleanly. Do not add "Launch at startup" unless requested
later.

*Update (ADR 0024, ADR 0054):* requested. Gracil starts with Windows and stays in the tray for
the session; **Start with Windows** in the tray menu turns it off. Still no Windows service, and a
Gracil started only for File Explorer's work still exits when done.

# 54. System tray

Gracil uses a system tray icon when background work is active. Logical tray states: Idle,
Processing, Paused, Error. Final artwork is not defined; use temporary development assets. When at
least one compression job is active, the tray icon must visibly communicate Processing. Tray state
is driven by scheduler state, not timers.

# 54.1 Closing the window minimizes to the tray

*Added after the original specification.*

Closing Gracil's window always minimizes Gracil to the system tray. This is not configurable; it
must always behave this way. Closing the window never stops, pauses or cancels work.

The tray icon offers an **Exit** option on right-click. Exit is how the user fully quits Gracil.

If a compression is in progress, Exit first asks for confirmation, and the question must tell the
user exactly what will happen: the current compression stops, the original video stays exactly as
it was, and videos still waiting will not be compressed. With nothing in progress, Exit quits
immediately.

Consequences for other sections:

- While the Gracil process is running, its tray icon is present (not only while work is active),
  so the user can always reopen the window or exit.
- Section 53 still applies to processes started only for background work from File Explorer: if
  no window was ever opened and all work has finished, Gracil may exit on its own.

*Update (ADR 0055):* Exit also asks when finished results wait for a choice, and says they are
let go and will be compressed again next time.

# 55. Tray mini-window

A compact tray-associated mini-window. Eventually: see the current operation, basic progress,
pause/resume, drag additional videos into Gracil. It stays open when the user clicks elsewhere;
the user intentionally closes/toggles it. Position it relative to the taskbar/tray without
assuming the taskbar is bottom-right. Support multiple monitors correctly.

*Update (2026-10-03, product owner):* a finished result can be decided in the mini-window itself,
with the same **Replace** and **Save copy** choices (§37.1), so opening the main window is never
required. It shows the running video first; results that wait meanwhile are counted, then offered
one at a time.

*Update (ADR 0055):* the mini-window's dial pauses and resumes but has no stop; stopping is in the
main window. When the tray icon turned red while no window was open, the mini-window opens on the
file that went wrong.

# 56. Explorer-triggered mini-window

By default, when compression starts from the File Explorer context menu: Gracil receives the
jobs, the tray icon switches to Processing and the mini-window appears.

Setting: **Show mini-window when compressing from File Explorer** — default ON. When OFF, no window
appears and compression proceeds entirely in the background. The setting never changes
compression behavior.

# 57. Windows 11 File Explorer integration

Required. Windows only offers its modern menu to packaged apps, and on Windows 11 File Explorer
ignores commands written for the current user only, so Gracil is in File Explorer in two ways
([ADR 0058](decisions/0058-explorer-send-to-and-main-menu.md)), both under **Show more options**:

- **Send to → Gracil**, a shortcut in the user's own Send to folder, written by Gracil itself on
  every start and turned off from the tray (**Show Gracil in Send to**). No administrator needed.
- **Its own commands** in the main menu, written for the whole computer when the user turns them
  on from the tray (**Show Gracil in File Explorer's menu…**), after Windows asks for an
  administrator's permission once. *(In progress: asking for the permission from inside Gracil
  is still being made to work.)*

Both start `gracil.exe --explorer` with what was picked, and the running Gracil applies the
**Always** setting to it.

*Before ADR 0058,* a per-user `IExplorerCommand` DLL (ADR 0022, ADR 0050) filled this role; File
Explorer never showed it.

# 58. Explorer multi-selection

Selecting several videos and right-clicking shows approximately **Gracil (8)** with the
subcommands **Compress and replace** and **Compress and save a copy**. Support one and several
selected videos. Whether a single file shows "Gracil" or "Gracil (1)" is a small copy decision;
keep it easy to change.

*Amended by [ADR 0035](decisions/0035-explorer-folders-and-always.md) (2026-10-04):* folders are
accepted too, there are three commands (videos and images, videos, images), and their words follow
the **Always** setting: "Compress and replace", "Compress and save copies of", or just "Compress",
which opens the window and asks.

*Amended by [ADR 0037](decisions/0037-looking-through-folders-in-the-background.md) (2026-10-04):*
folders are looked through away from the window, folder by folder, and work starts with the
first files found.

*Amended by [ADR 0058](decisions/0058-explorer-send-to-and-main-menu.md) (2026-10-07):* the words
no longer follow **Always** and there is no count. Picked videos and images show **Compress with
Gracil**, which sends every picked file; a folder shows **Gracil** with **Compress videos and
images**, **Compress videos only** and **Compress images only**. The running Gracil applies
**Always**: set, the files are compressed in the background as it says; in the middle, the window
opens and asks. **Send to → Gracil** sends files and folders the same way, both kinds.

# 59. Dynamic Explorer count

Use the selected item collection to count supported videos for the title. Do not perform
expensive work to calculate the title. Explorer callbacks must remain extremely fast.

*Update (ADR 0058):* plain commands can't count, so the title is fixed. No Gracil code runs
inside File Explorer any more.

# 60. Explorer shell extension safety

The shell extension must be tiny. It must NOT load Iced, initialize WGPU or FFmpeg, detect GPU
capabilities, run FFprobe, process video, wait for compression, or scan the filesystem
expensively. It only inspects the selection cheaply, displays the commands, forwards paths and
action to Gracil and returns promptly.

*Update (ADR 0058):* there is no shell extension. File Explorer starts `gracil.exe`, which hands
the request to the running Gracil and leaves, so nothing slow can ever run inside File Explorer.

# 61. Shell extension language

Prefer Rust + windows-rs. If a robust `IExplorerCommand` in Rust becomes unusually fragile, a very
small isolated C++ shim is acceptable — explain why first, put no product logic in it, and only
forward paths/actions to Rust. Do not migrate the application to C++.

*Update (ADR 0058):* no longer applies; there is no shell extension.

# 62. Background Explorer workflow

1. Explorer extension forwards selected paths/action.
2. If Gracil is not running, start it in background mode.
3. If Gracil is running, send the request through IPC.
4. Add the videos to the existing scheduler.
5. Change tray state immediately.
6. Optionally open the mini-window according to the setting.
7. Process the videos.

No console window. Do not initialize the main window unless needed.

*Update (ADR 0058):* step 1 is File Explorer starting `gracil.exe --explorer` with the picked
paths. With **Always** in the middle there is no action, so the main window opens and asks.

# 63. Additive Explorer requests

Explorer requests are additive to the existing scheduler. Do not create unrelated scheduler
instances.

# 64. Portable distribution

Gracil is portable (ADR 0050). It ships as a zip holding Gracil, jpegli, FFmpeg and FFprobe, and
their licenses, built by `scripts/portable.ps1`. It keeps its data in a `data` folder beside
itself and nowhere else; without a writable one it runs without saving anything (ADR 0054).
Removing Gracil is deleting its folder: its start with Windows uses `RunOnce`, which Windows
forgets by itself once the folder is gone, and **Prepare to delete Gracil** in the tray menu
takes it out of File Explorer first (ADR 0058). It is free and open source
(GPL-3.0-or-later), published on GitHub, outside the Microsoft Store.

# 66. Settings philosophy

Settings stay intentionally small. Currently required: **Show mini-window when compressing from
File Explorer** (default ON). In the tray menu too: **Show Gracil in Send to** (ADR 0050,
ADR 0058) and **Start with Windows** (ADR 0054), both default ON, and **Show Gracil in File
Explorer's menu…** (ADR 0058), off until the user turns it on and Windows' permission is given.
Do not add GPU/CPU/codec selection, bitrate, FFmpeg arguments, thread count or advanced encoder
configuration.

*Update (ADR 0025):* a second setting, requested by the product owner: **Always (Replace / ○ /
Save a copy)** in the main window. The middle, the default, asks after each video as in §37.1; an
end answers that question ahead of time for every video that finishes afterwards.

# 67. Error handling

Errors shown to users use normal language. When no user action is required, retry automatically
(for example software decoding) without interrupting. Only show an error when compression
ultimately failed, the user must act, or a file cannot be safely handled. Technical detail goes
to logs.

# 68. Failure isolation

One failed video does not stop unrelated queued videos. Only stop globally if continuing would be
unsafe.

# 69. Logging

Structured logging that can diagnose hardware detection, selected backend, FFmpeg invocation and
exit code, GPU decode fallback, CPU fallback, file replacement, metadata preservation, queue state
changes, IPC and Explorer requests, packaging/runtime issues. Do not log media contents. Avoid
preserving private path history forever. Use reasonable log rotation.

# 70. Security

Treat every path from drag-and-drop, file picker, Explorer, IPC and command line as untrusted.
Never build FFmpeg commands by concatenating a shell string; use structured process arguments. Do
not invoke `cmd.exe` or `powershell.exe` to run FFmpeg. Handle spaces, apostrophes, parentheses,
Unicode, emoji, long paths, non-English folders and unusual valid Windows filenames.

# 71. Unicode

Support Unicode paths (`C:\Videos\Mi cumpleaños 🎂.mp4`, `C:\日本語\動画.mp4`,
`C:\Videos\José\Vacación.mp4`). Preserve exact filenames. Do not transliterate or normalize names.

# 72. Temporary output files

Create temporary output safely, as same-volume siblings for replacement. Temporary names avoid
collisions, are recognizable internally and do not conflict with user files. Never delete files
merely because their names resemble Gracil temporary files; track ownership/state and clean
abandoned temporary files conservatively.

# 73. Testing

Automated testing is required, covering at least: output naming, naming collisions, Unicode
paths, spaces, unusual filenames, metadata preservation, chapters, rotation metadata, audio
preservation, job state transitions, scheduler ordering, additive drops, individual removal,
Clear behavior, active job protection, NVENC detection, CPU fallback, hardware decode fallback,
FFmpeg errors, pause/resume state, cancellation, safe replacement, failed replacement, invalid
temporary output, IPC, single-instance behavior, Explorer multi-selection, unsupported formats,
shell path serialization. Destructive tests use disposable directories only.

# 74. Performance testing

Maintain a repeatable performance validation process. Idle: CPU, GPU, RAM. During compression:
Gracil UI CPU/GPU, FFmpeg CPU, GPU Video Decode/Encode, memory, total processing time. The purpose
is regression detection, not marketing claims.

# 75. Optimization philosophy

Optimization is permanent work, but measure first. Prefer reducing idle work, allocations, copies,
filesystem access, redraws and dependencies; simplify hot paths. Do not sacrifice correctness or
maintainability for tiny theoretical gains.

# 76. Documentation quality

Keep README.md, CHANGELOG.md, docs/architecture/, docs/decisions/,
docs/development/. README explains what Gracil is, how to build/run it, the project layout,
relevant dependencies and where deeper documentation lives.

# 77. Development milestones

Every milestone compiles and is testable.

- **M0 — Repository foundation**: Git, Cargo workspace, directory structure, version 0.0.1-dev,
  README, CHANGELOG, fmt/clippy/test baseline, initial architecture documentation.
- **M1 — Compression core**: bundled FFmpeg/FFprobe discovery, source inspection, NVIDIA
  capability detection, NVENC encoding, CPU fallback, optional GPU decoding, software decoding
  fallback, structured progress, temporary output, basic validation.
- **M2 — Main application**: Iced, WGPU, main window, drag and drop, file picker, additive file
  handling, multiple job records, scheduler, temporary Focus + Drawer.
- **M3 — File management UX**: individual removal, Clear, active-job protection, output-mode
  selection, polished Focus + Drawer.
- **M4 — Safe output**: replace, save a copy, safe replacement, collision-safe names, metadata,
  stream and filesystem-metadata preservation, cancellation, pause/resume.
- **M5 — Background architecture**: single instance, named-pipe IPC, background invocation, tray
  icon and states, clean process lifetime.
- **M6 — Tray mini-window**: mini-window, progress, pause/resume, drag and drop, persistent on
  focus loss, multi-monitor positioning, Explorer mini-window preference.
- **M7 — File Explorer integration**: modern context menu, supported-file filtering,
  multi-selection, dynamic Gracil (N), both commands, IPC forwarding, shell stability testing.
- **M8 — Packaging**: a portable zip with FFmpeg and jpegli inside, built by
  `scripts/portable.ps1` (ADR 0050).
- **M9 — Image compression** *(planned, not started; added 2026-10-03 at the product owner's
  request)*: compress photos and other images the way Gracil compresses video. See §77.1.

Do not attempt to build every milestone in one giant pass.

# 77.1 Image compression

*Added 2026-10-03 as a plan; built 2026-10-04. All open decisions below are settled by
[ADR 0034](decisions/0034-image-standard.md), Gracil's image standard.*

What Gracil reads, and what it writes (videos: §77.2):

| Opened | Saved as |
|---|---|
| JPEG (`.jpg`, `.jpeg`, `.jpe`, `.jfif`) | JPEG, same name |
| PNG (`.png`) | PNG, same name |
| HEIC, HEIF, AVIF, WebP, TIFF (`.tif`, `.tiff`), BMP | JPEG (`.jpg`), or PNG with transparency |
| GIF, animated PNG or WebP, JPEG XL, camera RAW, true HDR (PQ, HLG) | not compressed: the quiet "!" |

Every JPEG is encoded by jpegli; libjpeg-turbo, built into jpegli, only reads JPEG. Images are
read inside Gracil and by a hidden FFmpeg; no window ever opens.

Gracil compresses images with the same philosophy as video:

- **Two output formats, chosen by Gracil, never by the user** *(decided 2026-10-03 by the product
  owner)*. A PNG stays a PNG, made smaller without losing anything (a lossless optimizer such as
  oxipng), so screenshots, graphics and transparency stay exactly as they were. Every other image
  (JPEG, WebP, HEIC, TIFF, BMP and the rest) becomes a JPEG, made with Google's jpegli encoder at
  a quality where the difference can't be seen. JPEG opens everywhere, the way MP4 does for video
  (ADR 0012), and a JPEG keeps its name. An image that would come out bigger, as an iPhone's HEIC
  often would, is not kept (§40.1).
- **One exact compression, always.** A fixed, tested quality setting, like the fixed H.265
  settings for video. No sliders, presets or advanced options (§66).
- **The same flows.** Drop images on the main window or the mini-window, or use File Explorer's
  menu (which would then count images as well as videos, **Gracil (N)**). The same choices after
  compressing: **Replace** or **Save copy**, and the **Always** setting (ADR 0025). The same safe
  replacement (§39) and collision-safe names (§40).
- **Never reject an image** because it is unusual; fall back to simpler ways first. A result that
  is not smaller is not kept (§40.1).
- **Keep what matters**: date taken, orientation, color profile and the other metadata people
  expect a photo to keep, as for video (§41).
- Everything stays local and private (§82).

Open decisions, to settle before any work starts:

1. ~~The final format~~: decided above (PNG stays PNG; everything else becomes JPEG).
2. **The exact compression**: jpegli's fixed quality and the PNG optimizer's settings, verified on a
   representative set of photos, screenshots and graphics, following in spirit the quality
   standard Gracil first settles on for video. *Agreed direction (2026-10-03):* the same
   rules as video. Every image keeps its resolution (ADR 0033); a JPEG is made at jpegli's
   "looks the same" distance and kept only when clearly smaller; an image already compressed is
   compressed again at a looser distance, without asking, or keeps the quiet "!"; never heavier.
   *Decided in ADR 0034:* JPEG at jpegli distance 1.0, fixed; compressing again searches distances
   1.5–3.0 for SSIMULACRA2 80 or more; PNG lossless plus a palette at SSIMULACRA2 90 or more, and
   80 or more when compressing again; at most 80% of the original, as for video.
3. **Which images are accepted** *(partly decided 2026-10-03 by the product owner)*:
   - An image with transparency that is not a PNG (a WebP or TIFF, for example) becomes a PNG,
     so its transparency is kept; JPEG has none.
   - **Animated images** (GIF, animated PNG, animated WebP) are not supported for now: they are
     neither photo nor video. They are left exactly as they are, and are a plan for the future.
   - **Camera RAW** files are not accepted: they are a photographer's negative, and turning one
     into a JPEG is developing it, not compressing it.
   - **JPEG XL** files are not accepted either: the format never became common, and whoever made
     one already has a well-compressed image.
   - HDR and very large images: decided in ADR 0034 (a gain-map photo becomes its normal picture;
     true HDR is left as it is for now; no size limit).
4. **The engine**: FFmpeg, which Gracil already relies on, or a dedicated image library.
   *Proposed (2026-10-03):* Windows itself (WIC) reads images, with FFmpeg as a fallback; jpegli
   makes the JPEGs (FFmpeg has no jpegli and a weak JPEG encoder); oxipng optimizes PNGs.
   *Decided:* the JPEG encoder is jpegli only (Google's, the newest and the best at high
   quality), built by Gracil from Google's official repository (github.com/google/jpegli) at a
   pinned commit and shipped inside the zip; nothing from libjxl or JPEG XL is used (ADR
   0034). It reads JPEG directly (through the libjpeg-turbo copy in jpegli's repository).
   *Built (2026-10-04):* other formats are read inside Gracil (the `image` crate for WebP, TIFF
   and BMP) or by the hidden FFmpeg (HEIC, HEIF, AVIF); WIC was set aside (ADR 0034, item 9).
5. **How images and videos share the list**, the dial and the tray without adding complexity.
   *Decided 2026-10-03 by the product owner (ADR 0034, item 13):* one list for every file;
   images are compressed before waiting videos, since they take seconds; with more than one file
   the dial's whole circle is the whole list, and the marble leaves a hollow marble at the end of
   each file's part. A design pass to make the dial more refined comes later. *Built
   (2026-10-04):* the one list and images first (since superseded by ADR 0040), and the dial and
   the tray for the whole list. *Changed 2026-10-04 by the product owner:* no hollow marbles; the
   ring is titled with how many files it holds (**Compressing 24 files**), shows the whole list's
   percentage, and never closes before the real end of the list.

# 77.2 Video formats

What Gracil reads, and what it writes (ADR 0012):

| Opened | Saved as |
|---|---|
| MP4 (`.mp4`) | MP4 (H.265), same name |
| M4V, MOV, MKV, AVI, WMV, WebM, MTS, M2TS, TS, 3GP, FLV, MPG, MPEG | MP4 (H.265), `.mp4` |

These are the extensions offered by the file picker and File Explorer's menu
(`media::VIDEO_EXTENSIONS`). Any other file dropped on the window that is not a known image is
handed to FFmpeg as a video and tried, never rejected up front (§2).

# 77.3 Planned

*Recorded 2026-10-04 by the product owner. Both built the same day (ADR 0036, §32.1, §32.2).*

- **How much was saved.** After compressing many videos or images there is no way to see how much
  smaller each file became, nor how much the whole batch saved. Show both, quietly.
- **Getting the dial back.** In a long list, selecting a file that failed (for example "Already
  compressed") replaces the ring in the main window, and the only way back to the ring is to take
  that file off the list. Going back to the work in progress must be easy and natural.

*Recorded 2026-10-04 by the product owner, after compressing a cloud-drive folder of more than
15,000 files. Not built; to be evaluated before building.*

- **A list that opens instantly.** With a very long list (15,031 files and growing while Gracil
  still looks through the folder), opening and closing **View files** in the main window takes a
  few seconds. It must respond at once, however long the list is. *Built 2026-10-04 (§32).*
- **Numbers in the system's format.** Large counts must use the separators Windows is set to,
  for example *15,031 files* rather than *15031 files*. The same goes for every number Gracil
  shows. *Built 2026-10-04 (§32.2).*
- **A clear "done".** When the work is finished, the main window shows it instead of leaving the
  last state of the ring: for example the middle of the ring filled with a green check mark, and
  the amount saved ("130 MB smaller") brought to the center, larger, with a subtle animation, just
  below the ring. It could add a short summary of the whole batch, such as *22 files compressed*
  and *4 files were already compressed*. The aim is quick feedback that Gracil has finished,
  above all after a long list. The exact form is still open. *Built 2026-10-04 (ADR 0039).*
- **A ring without its track.** The track the marble rolls along in the main window (the "pipe"
  of the ring) is no longer drawn: only the marble and the part it has filled are seen, so the
  marble fills an invisible circle. The thin line between pause and stop in the middle of the
  ring goes too. Meant to look more elegant and give Gracil personality. Changes the look recorded
  in ADR 0014, so an ADR amends it when it is built. *Built 2026-10-04 (ADR 0038).*
- **Several images at once.** Gracil compresses one file at a time, so a long list of images
  (14,916 still waiting in the owner's folder) takes far longer than it needs to. Images should
  be compressed several at a time, and fewer when the images are very large, so memory never runs
  short. On a network or cloud drive this also overlaps the waiting for downloads and uploads.
  Pause and stop act on all of them together, and the ring shows the progress of the whole list
  (ADR 0034). *Built 2026-10-04 (ADR 0040), the ring for the whole list included.*
- **Fast, but never alarming.** *Decided by the product owner.* Gracil must never make the
  computer feel slow, and never run it flat out: the fans would roar, and that would not inspire
  trust. How much Gracil uses adapts to what the user is doing: more while the computer is
  otherwise idle, less as soon as the user is working in other applications. This holds even for
  a list of 15,000 files. High performance, yes; scaring the user, never. It applies to all the
  work, not just images. *Built 2026-10-04 (ADR 0040).*
- **One video at a time.** *Decided by the product owner.* Two videos are never compressed at
  once: one video already uses all of the graphics card's encoder (or the processor), so two
  would each go at half speed and gain nothing. *Built 2026-10-04 (ADR 0040).*
- **Get the next file ready during the current one.** *Decided by the product owner, as the base
  of how Gracil works.* Every file's work has two stages: getting ready (for a video, the short
  test samples that choose its compression, mostly on the processor) and compressing (for a
  video, mostly on the graphics card). While one file compresses, the next one gets ready, instead
  of the two stages always following each other. This is built once, for every kind of file:
  videos, images, and any kind added later. *Built 2026-10-04 (ADR 0040).*
- **Images on the processor while a video is on the graphics card.** *Decided by the product
  owner.* While a video compresses on the graphics card, images compress on the processor, so
  neither sits idle. Still within "fast, but never alarming". *Built 2026-10-04 (ADR 0040).*
- Together these change the rule that only one file is worked on at a time (§33, the queue) and
  the order of ADR 0034 (images before waiting videos), so an ADR records them when they are
  built. *Built 2026-10-04 (ADR 0040).*
- **Carry on after closing.** *Decided by the product owner.* If Gracil closes before its list is
  done (Exit, a sudden stop, the computer turned off), the next time it opens it offers to carry
  on with the files left, with a button. *Built 2026-10-04 (ADR 0042).*
- **Never too hot.** *Decided by the product owner.* When the graphics card or the processor
  Gracil uses gets too hot, Gracil does less by itself: fewer images at once when warm, and
  everything waits when too hot, until the computer has cooled down. *Built 2026-10-04 (ADR
  0043).* *Changed by the product owner:* Gracil never stops to cool down; it keeps a margin
  instead, with short unnoticed rests that grow as the card warms. *Built 2026-10-04 (ADR 0045).*
- **Stop means stop.** *Decided by the product owner.* Stop stops the whole list at once, and
  the files not done are offered with **Continue**; Exit never waits on a slow drive. *Built
  2026-10-04 (ADR 0044).*

*Recorded 2026-10-05 by the product owner.*

- **A steady temperature.** *Decided by the product owner.* Instead of resting harder the warmer
  the card gets and then speeding back up, Gracil holds a safe temperature, about 70 °C, and
  works as much as that temperature allows. Rests and work alternate fast enough that the parts
  stay at an even temperature. *Built 2026-10-05 (ADR 0046).*
- **Back to the start when done.** *Decided by the product owner.* When the whole list is done,
  **Clear the list** is offered right under the summary, so starting again feels natural. *Built
  2026-10-05.*
- **Videos from recent iPhones.** Their spatial sound can't be read by FFmpeg, which made every
  one of them fail. It is left out while the usual sound is kept. *Built 2026-10-05 (ADR 0047).*
- **See-through HEIC and AVIF.** These were refused. Their transparency is now kept. *Built
  2026-10-05 (ADR 0048).*
- **Opening Gracil again shows it.** Opening Gracil from Start or the taskbar while it runs next
  to the clock must open its window. *Fixed 2026-10-05.*
- **Never harm the computer.** *Asked by the product owner,* with simple laptops in mind: no
  overheating, no roaring fans, no memory or disk filled. Windows itself holds Gracil to part of
  the processor (less while the person works and on battery), to part of the memory, and to a
  drive with room to spare. *Built 2026-10-05 (ADR 0049).*
- **Free and open source, outside the Store.** *Decided by the product owner.* Gracil can't stay
  in the Microsoft Store, even for free, so it is published on GitHub as a portable zip with
  FFmpeg inside, keeping its data beside itself, with Gracil in File Explorer's classic menu.
  *Built 2026-10-05 (ADR 0050).*
- **Cooler while in use, and any graphics card.** The tools run in Windows' efficiency mode
  while the person works or the computer is on battery, and the temperature of Intel and AMD
  cards is read from their driver and held like an NVIDIA card's. *Built 2026-10-05 (ADR 0051).*
- **Intel and AMD graphics compress too.** *Asked by the product owner.* After NVIDIA and before
  the processor, each checked with a tiny test compression. *Built 2026-10-05 (ADR 0052).*
- **Ready for GitHub.** *Asked by the product owner.* Everything left from the Store and anything
  no longer needed is removed, FFmpeg is never downloaded, and one Gracil in daily use is kept
  in one place. *Built 2026-10-05 (ADR 0053).*
- **Wholly portable.** *Asked by the product owner.* Everything in Gracil's own folder and
  nowhere else; its start with Windows forgets itself once the folder is gone; **Start with
  Windows** and **Prepare to delete Gracil** in the tray menu; GPL-3.0-or-later, with the
  licenses of every Rust library in the zip. *Built 2026-10-05 (ADR 0054).*
- **Natural from the first use.** *Asked by the product owner* before the first release: someone
  opening Gracil for the first time knows what to do without thinking, nothing is out of place
  or extra, nothing looks stuck or says something untrue. Folders can be dropped, the empty
  window shows only where to drop, the done screen is honest when nothing was compressed,
  getting ready breathes, Exit counts every loss, and a few keys work. *Built 2026-10-05 (ADR
  0055).*
- **The list unfolds.** *Asked by the product owner.* **View files** unfolds and folds away
  instead of appearing at once, as gently as the rest of Gracil moves. *Built 2026-10-06 (ADR
  0056).*
- **Always stays in sight.** *Asked by the product owner.* The **Always** pill is at the foot of
  the main window even before there are files, since File Explorer's menu follows it and it is
  best set before a large folder starts. *Built 2026-10-06 (ADR 0057).*
- **Gracil really in File Explorer.** On Windows 11, File Explorer never showed the menu Gracil
  wrote for the current user. Gracil is now in **Send to** for everyone, and its own commands in
  File Explorer's menu can be turned on from the tray, with Windows' permission. Their words no
  longer change with **Always**, which Gracil applies itself. *Send to built 2026-10-07; the
  menu's permission in progress (ADR 0058).*

# 78. Temporary UI decisions

Intentionally unfinished: final icon, colors, typography, animation language, detailed styling,
marketing copy, final localized wording. Use clean temporary solutions; do not couple core logic
to them.

*Update (ADR 0015):* the icon (the marble) and the colors (the Piedra palette) are now decided,
and Gracil follows Windows' light or dark theme with a soft fade.

*Update (ADR 0031):* the colors are now Windows 11's own, with the user's accent color.

# 79. Focus + Drawer is not temporary

The interaction model is decided. Do not replace it with permanent queue tables, download-manager
UI, full-window cards per video or complex navigation without explicit approval.

# 80. Product principle for technical decisions

Prefer: ease of use > resource efficiency > visual elegance. When equivalent to the user, prefer
the simpler, more maintainable, testable, profilable and replaceable implementation.

# 81. Things you must not do silently

Do not silently: change Iced; remove WGPU; replace FFmpeg; change H.265; expose advanced settings;
remove CPU fallback; remove hardware-decoding fallback; remove metadata preservation; change the
safe-replace model; change how Gracil is distributed (ADR 0050); change Focus + Drawer into a
conventional queue; add a permanent Windows service; add startup execution; add analytics or
telemetry; introduce cloud processing; upload videos anywhere; change versioning rules.

*Update (ADR 0024):* startup execution was requested and added; see §53.

If a requirement becomes technically problematic, explain the conflict before changing behavior.

# 82. Privacy

All video processing is local. Do not upload videos, metadata, filenames or paths. No telemetry,
no analytics, no crash reporting that sends user data without explicit future approval. Gracil
works offline after installation, apart from the one-time FFmpeg download described in §2.2.

# 84. Definition of success

A normal Windows 11 user can install Gracil, drag videos into it, understand it immediately, add
more files without losing existing ones, compress them without understanding codecs, pause and
resume, safely replace originals, save compressed copies, preserve important metadata, use the
best hardware path automatically with CPU fallback, select several videos in Explorer, right-click,
choose Gracil, start compression, receive immediate tray feedback and keep working without opening
the full application — while Gracil remains ultra minimalistic, fast, resource-conscious, safe
with user files, clean internally, maintainable and ready to improve continuously.
