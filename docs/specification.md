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

# 3. Product identity

Application name: **Gracil** — always written without an accent.

Do not rename it to: Grácil, Gracile, Graysil, Graceful, Gracil Video Compressor.

The intended English pronunciation is approximately **GRAY-sil**. The pronunciation does not
affect the spelling.

The final application icon has not been designed yet. Use a clearly temporary development icon.
Do not invent or finalize the permanent icon. The final icon will later be designed according to
current Windows 11 iconography and visual guidelines.

# 4. Supported platform

Target: Windows 11, x86-64 / 64-bit Windows initially. Do not build a 32-bit application.

References to Win32 APIs do NOT mean the application is 32-bit. Win32 is the historical name for
many native Windows desktop APIs that are also used by 64-bit applications.

Future ARM64 support may be evaluated later but is not required now.

# 5. Primary technology stack

Use: Rust, Iced, WGPU as the Iced renderer, FFmpeg, FFprobe, windows-rs for Windows-specific APIs,
a lightweight native tray implementation where required, MSIX packaging.

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
│   ├── gracil-windows/
│   └── gracil-shell/
├── docs/
│   ├── architecture/
│   ├── decisions/
│   ├── design/
│   └── development/
├── assets/
│   ├── icons/
│   └── development/
├── packaging/
│   └── msix/
├── scripts/
├── tests/
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
- **gracil-shell**: modern Windows 11 File Explorer context-menu extension.

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

Keep the version synchronized wherever practical: Cargo workspace/package metadata, application
metadata, MSIX package metadata where compatible, About information when eventually implemented,
release scripts.

Where MSIX versioning rules cannot directly represent the exact application semantic version
string, keep a documented mapping instead of silently inventing one.

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

Users must not need to install FFmpeg separately. ~~Bundle known-compatible builds of `ffmpeg.exe`
and `ffprobe.exe`. Do not depend on PATH; resolve Gracil's bundled binaries explicitly.~~
*Superseded by §20.1.* Document FFmpeg version, build source/provenance, enabled capabilities and
licensing obligations.

Keep FFmpeg as an external process. Do not link FFmpeg libraries directly without discussing the
change first.

# 20.1 Detect FFmpeg, or install it invisibly

*Added after the original specification; replaces bundling.*

Gracil first detects whether FFmpeg is already installed on the computer. An installation is only
used if FFmpeg itself confirms it has what Gracil needs (at least the CPU HEVC encoder) and
`ffprobe.exe` sits next to `ffmpeg.exe`.

If no suitable FFmpeg exists, Gracil installs its own copy automatically into its private
per-user folder. This is invisible: no window, no prompt, no progress the user has to watch. The
copy is a pinned, documented build verified by SHA-256 before use. Gracil never installs FFmpeg
system-wide and never changes PATH.

Gracil starts this preparation as soon as it launches, so it is normally finished before the first
video arrives. Videos added before it finishes simply wait in their normal waiting state.

# 21. Reference NVIDIA compression behavior

```
ffmpeg -i "video.mp4" -c:v hevc_nvenc -preset p6 -tune hq -rc vbr -cq 27 -b:v 0 -c:a copy "video_comprimido.mp4"
```

Intended behavior: HEVC / H.265; NVENC when available and appropriate; preset P6; high-quality
tune; VBR; CQ 27; no fixed target video bitrate; preserve audio without recompression where
compatible. These parameters must not appear in the normal user interface.

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

# 32. Drawer behavior

Selecting the subtle file-count control opens the file drawer, which lets the user inspect the
current set of files, remove individual removable items, and quickly clear removable files. The
drawer is not visually dominant. When closed, the user should mostly forget a queue exists.

# 33. Active item behavior

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

Two output intentions: **Compress and replace** and **Compress and save as new**. Do not expose
codec choices. Do not require technical setup before starting.

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

# 40. Compress and save as new

Create a new file in the same directory with the `Gracil_` prefix (`Vacation.mp4` →
`Gracil_Vacation.mp4`). Do not silently overwrite an existing file; if the name exists, create a
non-conflicting name using a simple predictable strategy.

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
read is accepted. The output keeps the source's container when that container can hold HEVC
properly; otherwise Gracil produces an MP4. Ensure the output container can safely contain HEVC
video and audio, plus whatever metadata and additional streams it can hold.

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

# 50. Design system

Build reusable visual primitives and design tokens (spacing, corner radius, typography hierarchy,
surface behavior, progress treatment, focus states, animation timing). Do not prematurely lock the
visual identity. The interaction architecture stays independent from visual choices.

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

# 55. Tray mini-window

A compact tray-associated mini-window. Eventually: see the current operation, basic progress,
pause/resume, drag additional videos into Gracil. It stays open when the user clicks elsewhere;
the user intentionally closes/toggles it. Position it relative to the taskbar/tray without
assuming the taskbar is bottom-right. Support multiple monitors correctly.

# 56. Explorer-triggered mini-window

By default, when compression starts from the File Explorer context menu: Gracil receives the
jobs, the tray icon switches to Processing and the mini-window appears.

Setting: **Show mini-window when compressing from File Explorer** — default ON. When OFF, no window
appears and compression proceeds entirely in the background. The setting never changes
compression behavior.

# 57. Windows 11 File Explorer integration

Required. Use the modern Windows 11 context menu, not "Show more options" or legacy-only
integration. Expected technologies: MSIX package identity, `windows.fileExplorerContextMenus`,
`IExplorerCommand`, COM shell extension. Verify current Microsoft requirements before
implementing.

# 58. Explorer multi-selection

Selecting several videos and right-clicking shows approximately **Gracil (8)** with the
subcommands **Compress and replace** and **Compress and save as new**. Support one and several
selected videos. Whether a single file shows "Gracil" or "Gracil (1)" is a small copy decision;
keep it easy to change.

# 59. Dynamic Explorer count

Use the selected item collection to count supported videos for the title. Do not perform
expensive work to calculate the title. Explorer callbacks must remain extremely fast.

# 60. Explorer shell extension safety

The shell extension must be tiny. It must NOT load Iced, initialize WGPU or FFmpeg, detect GPU
capabilities, run FFprobe, process video, wait for compression, or scan the filesystem
expensively. It only inspects the selection cheaply, displays the commands, forwards paths and
action to Gracil and returns promptly.

# 61. Shell extension language

Prefer Rust + windows-rs. If a robust `IExplorerCommand` in Rust becomes unusually fragile, a very
small isolated C++ shim is acceptable — explain why first, put no product logic in it, and only
forward paths/actions to Rust. Do not migrate the application to C++.

# 62. Background Explorer workflow

1. Explorer extension forwards selected paths/action.
2. If Gracil is not running, start it in background mode.
3. If Gracil is running, send the request through IPC.
4. Add the videos to the existing scheduler.
5. Change tray state immediately.
6. Optionally open the mini-window according to the setting.
7. Process the videos.

No console window. Do not initialize the main window unless needed.

# 63. Additive Explorer requests

Explorer requests are additive to the existing scheduler. Do not create unrelated scheduler
instances.

# 64. MSIX installation

Gracil is an installed application, not portable. Use MSIX from the beginning, designed for local
development, sideload testing, signed direct distribution and eventual Microsoft Store
submission. The package includes Gracil, Explorer integration components, assets and manifest
configuration. *FFmpeg and FFprobe are no longer packaged; see §20.1.*

# 65. Future Microsoft Store distribution

Do not hardcode a final Store Publisher identity. Keep identity values easy to replace and
document the process. Eventually pass Windows App Certification. Do not add Store-specific
features that are not required yet.

# 66. Settings philosophy

Settings stay intentionally small. Currently required: **Show mini-window when compressing from
File Explorer** (default ON). Do not add GPU/CPU/codec selection, bitrate, FFmpeg arguments,
thread count or advanced encoder configuration.

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

Keep README.md, CHANGELOG.md, docs/architecture/, docs/decisions/, docs/design/,
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
- **M4 — Safe output**: replace, save as new, safe replacement, collision-safe names, metadata,
  stream and filesystem-metadata preservation, cancellation, pause/resume.
- **M5 — Background architecture**: single instance, named-pipe IPC, background invocation, tray
  icon and states, clean process lifetime.
- **M6 — Tray mini-window**: mini-window, progress, pause/resume, drag and drop, persistent on
  focus loss, multi-monitor positioning, Explorer mini-window preference.
- **M7 — File Explorer integration**: modern context menu, supported-file filtering,
  multi-selection, dynamic Gracil (N), both commands, IPC forwarding, shell stability testing.
- **M8 — Packaging**: MSIX, manifest registration, clean install/update/uninstall, packaged FFmpeg
  and shell extension, versioning scripts, Store readiness.

Do not attempt to build every milestone in one giant pass.

# 78. Temporary UI decisions

Intentionally unfinished: final icon, colors, typography, animation language, detailed styling,
marketing copy, final localized wording. Use clean temporary solutions; do not couple core logic
to them.

# 79. Focus + Drawer is not temporary

The interaction model is decided. Do not replace it with permanent queue tables, download-manager
UI, full-window cards per video or complex navigation without explicit approval.

# 80. Product principle for technical decisions

Prefer: ease of use > resource efficiency > visual elegance. When equivalent to the user, prefer
the simpler, more maintainable, testable, profilable and replaceable implementation.

# 81. Things you must not do silently

Do not silently: change Iced; remove WGPU; replace FFmpeg; change H.265; expose advanced settings;
remove CPU fallback; remove hardware-decoding fallback; remove metadata preservation; change the
safe-replace model; change MSIX; use a legacy-only Explorer menu; change Focus + Drawer into a
conventional queue; add a permanent Windows service; add startup execution; add analytics or
telemetry; introduce cloud processing; upload videos anywhere; change versioning rules.

If a requirement becomes technically problematic, explain the conflict before changing behavior.

# 82. Privacy

All video processing is local. Do not upload videos, metadata, filenames or paths. No telemetry,
no analytics, no crash reporting that sends user data without explicit future approval. Gracil
works offline after installation.

# 84. Definition of success

A normal Windows 11 user can install Gracil, drag videos into it, understand it immediately, add
more files without losing existing ones, compress them without understanding codecs, pause and
resume, safely replace originals, save compressed copies, preserve important metadata, use the
best hardware path automatically with CPU fallback, select several videos in Explorer, right-click,
choose Gracil, start compression, receive immediate tray feedback and keep working without opening
the full application — while Gracil remains ultra minimalistic, fast, resource-conscious, safe
with user files, clean internally, maintainable and ready to improve continuously.
