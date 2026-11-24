# Gracil — Master Product & Engineering Specification

This is the authoritative product and engineering specification for Gracil, condensed from the
original brief (wording tightened, requirements unchanged; section 83, the instructions for the
first planning response, is omitted). Section numbers are stable references; do not renumber them.
Additions after the original specification use sub-numbers (for example `54.1`). Where a
decision record changed a requirement, the section says how Gracil is now and names the ADR.

---

# 1. Product priorities

Gracil has three primary priorities, in this order:

1. Ease of use
2. Optimization
3. Design

All three are extremely important. The ordering exists only to resolve conflicts.

A technically elegant implementation that makes the product harder to use is a regression. A
beautiful interface that wastes resources unnecessarily is also a regression. A highly optimized
implementation that becomes difficult to maintain without measurable benefit should be reconsidered.

# 2. Product philosophy

Gracil does one thing: **Compress videos and images** (§77.1, §77.2). Documents are a proposal,
not built (ADR 0062).

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
user inside the interface, in a way that feels natural: a small "!" appears beside the file's
status, and a short, plain explanation is written out under the file shown large (ADR 0060);
in the list it is behind the "!" on hover, and in the mini-window the "!" opens the main window
on that file. The user stays in their flow, and the interface keeps its calm, minimal look.
Dialogs are only for decisions with real consequences, such as exiting while a compression is
running (§54.1).

Example: when the compressed video is not smaller than the original, it is labelled **Already
compressed**, a "!" sits beside that, and the line under it says Gracil could not make the file
smaller than the original, so nothing is saved. Replace and Save copy are not shown, since there
is nothing to choose (§40.1).

# 3. Product identity

Application name: **Gracil** — always written without an accent.

Do not rename it to: Grácil, Gracile, Graysil, Graceful, Gracil Video Compressor.

The intended English pronunciation is approximately **GRAY-sil**. The pronunciation does not
affect the spelling.

The icon is the marble ("la canica", ADR 0015) on a small tile in Windows 11's own neutral
greys, always the opposite of Windows' theme, linked into `gracil.exe` from
`crates/gracil-ui/icons/` (ADR 0016, ADR 0031, ADR 0077). Gracil wears Windows 11's own colors
and the user's accent color, followed live (ADR 0031; the earlier Piedra palette is gone).

# 4. Supported platform

Target: Windows 11, x86-64 / 64-bit Windows (the zip is `Gracil-X.Y.Z-x64.zip`). Do not build a
32-bit application.

References to Win32 APIs do NOT mean the application is 32-bit. Win32 is the historical name for
many native Windows desktop APIs that are also used by 64-bit applications.

ARM64 is not built; it may be evaluated later but is not required.

# 5. Primary technology stack

Use: Rust, Iced, WGPU as the Iced renderer, FFmpeg, FFprobe, windows-rs for Windows-specific APIs,
a lightweight native tray implementation where required, and jpegli for JPEG (§77.1). Gracil
ships as a portable zip published on GitHub (ADR 0050).

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
predictable and clean. The structure now (ADR 0077):

```
gracil/
├── .cargo/              how Gracil is built
├── .claude/             cloud sessions (ADR 0072)
├── .github/workflows/   checks on every push, releases from a tag (ADR 0072, ADR 0079)
├── crates/
│   ├── gracil-core/
│   ├── gracil-ui/       with icons/ and gracil.rc
│   └── gracil-windows/
├── docs/
│   ├── architecture/
│   ├── decisions/
│   ├── development/
│   └── images/          pictures the README and guides show
├── scripts/             with ffmpeg.psd1, the pinned FFmpeg
├── Cargo.toml, Cargo.lock, rust-toolchain.toml, clippy.toml
├── README.md, LICENSE, CHANGELOG.md, CONTRIBUTING.md, CLAUDE.md
├── .gitignore, .gitattributes
```

A crate holds what only it uses, a script's data sits beside the script, and the root holds only
what must be there (ADR 0077). Material that isn't code lives outside the repository (ADR 0074).
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
  integration, window positioning, filesystem replacement, Explorer integration support. It is
  the only crate allowed `unsafe`.

There is no shell-extension crate ([ADR 0058](decisions/0058-explorer-send-to-and-main-menu.md)):
File Explorer reaches Gracil through **Send to** and plain commands that start `gracil.exe`,
written by gracil-windows.

Most importantly: **gracil-core must not depend on Iced.** The compression engine must be usable
without initializing Iced, WGPU or a visible window. This is required for background compression
initiated from File Explorer.

# 9. Git from the beginning

The project must use Git from the beginning. The repository history should remain useful and
understandable. Prefer small, logically complete commits. Do not create giant commits containing
unrelated changes. Do not introduce an unnecessarily complicated Git workflow.

`main` may contain the normal development history. Use temporary branches when they provide a real
benefit; work from the cloud goes to a branch and a pull request, and reaches `main` once
GitHub's checks pass on Windows and Linux (ADR 0072). Released versions must be represented by
Git tags (`v1.0.0`). Do not rewrite published release history.

# 10. Versioning

The first public release was `1.0.0` (2026-11-16; ADR 0027, ADR 0050); every release since is
in `CHANGELOG.md`. The development version always carries the `-dev` suffix (for example
`1.0.1-dev`). When it is released it becomes `1.0.1` with the Git tag `v1.0.1`, and the
development version moves on to the next one. The rules are in `docs/development/versioning.md`.

The version lives in one place, the workspace's `Cargo.toml`; the portable zip's name follows
it, and a release tag that doesn't match it stops the release (ADR 0079).

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

Avoid `unsafe`. It is denied workspace-wide; only gracil-windows, where Windows integration
genuinely requires it, opts in: isolate it, minimize its scope, document the safety assumptions,
test surrounding behavior.

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

*Update (ADR 0063):* with nothing to do, the thread that watches the computer sleeps, waking only
every ten minutes to note Gracil's memory; it reads the computer every second only while files are
being worked on.

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

Intel (Quick Sync) and AMD graphics are backends too, tried after NVIDIA and before the processor,
each checked with a tiny test compression like NVENC (ADR 0052). Do not expose backend names in
the normal UI.

# 23. CPU fallback

Gracil must work on machines without a dedicated GPU, a recent NVIDIA GPU, NVENC or supported GPU
drivers. If a suitable GPU backend is unavailable, use the CPU automatically.

# 24. Hardware decoding

When using NVIDIA hardware and the source is supported, use NVDEC/CUDA decoding when beneficial.
Hardware decoding is an optimization, not a requirement. If the codec is unsupported,
initialization fails, a driver problem exists, or FFmpeg reports hardware decoding cannot
proceed, gracefully retry using software decoding. If NVENC remains usable, continue using NVENC.
Do not fail an otherwise valid compression merely because GPU decoding was unavailable.

*Update (ADR 0065):* Intel (Quick Sync) and AMD graphics decode for their own encoder too, where
a one-frame test passes at start, with the same fallback to software decoding. The quality
measure's samples keep decoded frames on the card for the encoder, copying out only the
reference, with bit-identical results.

# 25. Capability detection

Use FFmpeg itself as an authoritative source (`-encoders`, `-decoders`, `-hwaccels`). Perform
small runtime validation when static listing is insufficient. Cache results. Do not run
expensive probes every time a file is dropped. Do not assume support from GPU model names.

# 26. Video queue responsibility

Gracil maintains a lightweight scheduler that decides which file is processed next; FFmpeg
processes each file. One video is compressed at a time; images compress beside it on the
processor, and the next file gets ready while the current one compresses (ADR 0040, §33). Keep
scheduler policy simple.

# 27. Adding files

Drag-and-drop is mandatory (one or many videos from File Explorer). A normal file picker exists
as a secondary method. Dragging files sends their paths; do not load contents into memory.

# 28. Additive drag-and-drop behavior

Newly dropped files are always ADDED to the current set; they never replace existing files. A new
drag operation is not a new session. The same applies to the file picker.

# 29. Duplicate paths

*Decided by the product owner (2026-10-03):* the list never stops a file from being compressed
again. Adding a path whose entry is finished (put in place, failed or cancelled) removes that
entry and adds the file again as new. Adding a path that is still waiting, running, or waiting
for the replace-or-copy choice does nothing.

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

The sketch is the original idea; the interaction model it shows IS final (§79). How it looks is
decided by the ADRs below, and may still be refined.

Progress is a ring the marble fills clockwise, with pause/play and stop as symbols in its middle
instead of the words in the sketch (ADR 0014, ADR 0038).

*Update (ADR 0069):* the footer's count of files left is followed, once it is known well enough,
by a coarse estimate of the time left ("12 files left, about 20 min · View files"), also in the
tray's tip and the window's title while compressing. Rounded to about 5, 10, 20, 30 or 45 minutes,
then whole hours; it never goes up in small steps; hidden while paused, resting, waiting for the
battery, while folders are being looked through, before three files are done and in the last two
minutes. Worked out from each kind's bytes and measured pace, not from the count of files.

*Update (ADR 0076):* the ring stays through everything that happens to a file: it fills as the file
compresses, then holds how much smaller the file came out, then the finished list's check. What it
stands for is on a line above it, and how it goes or what to do next below it. The focus has two
sizes, the ring in its middle or beside the words, and the window shows the one that fits. The line
under the focus is always one line, with **View files** and **+** as buttons of their own on its
right. The focus and the list are cards, as Windows 11 draws the surfaces content lies on; the
focus lights up softly in the accent while files are dragged over the window. What answers a
choice moves on springs (`motion.rs`).

# 32. Drawer behavior

**View files**, on the line under the focus, opens the file drawer (the list), which lets the user inspect the
current set of files, remove individual removable items, and quickly clear removable files. The
drawer is not visually dominant. When closed, the user should mostly forget a queue exists.

The drawer opens and closes at once however long the list is: every row has the same height and
only the rows in sight (and a few beyond) are built; the rest of the list is empty space of the
right height, so the scroll bar stays true.

The list unfolds and folds away rather than appearing at once (ADR 0056). An open list takes all
of the window but the small focus, and grows with it (ADR 0076).

# 32.1 Picking a file, and the way back (ADR 0036)

Clicking a file in the drawer shows it in the focus, and its row stays lightly marked. Picking is
a temporary look, never a trap: clicking the same row again, closing the drawer, or clicking the
quiet "‹ name · status" line under the picked file returns the focus to the work in progress.
That line appears only while something is being compressed or a result waits for a choice, and
it is not the picked file.

*Update (ADR 0076):* while the list is open the focus is small, and the line is under the picked
file there too.

# 32.2 Sizes (ADR 0036)

A result shows its size before and after under its choices ("120 MB → 78 MB"). The drawer row of a
placed file says how much smaller and where it went ("−35% · Replaced"). With two or more results on
the list, the drawer opens with their sum ("In all: 3.4 GB → 2.2 GB, 35% smaller") and the footer
adds the room freed ("1.2 GB smaller"). Only files still on the list count. Sizes are counted the
way File Explorer counts them (1 KB = 1024 bytes). Every number, sizes and counts alike, is written
the way Windows writes numbers for the user's region: its decimal mark and its digit grouping
(*15,031 files*, *1.2 GB* in Mexico; *15.031 archivos*, *1,2 GB* in Spain).

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
Gracil also has Prepared: got ready, waiting its turn to compress (ADR 0040). Transitions are
deterministic. The UI derives from state. Avoid scattered boolean flags.

# 37. Starting compression

Two output intentions: **Compress and replace** and **Compress and save a copy** (Spanish
"Comprimir y guardar como copia"), matching the **Replace** and **Save copy** buttons. Do not
expose codec choices. Do not require technical setup before starting.

# 37.1 Choosing after compressing

*Added after the original specification (ADR 0009).*

In the main window, files are compressed as soon as they are added. When a result is ready, the
window shows how much smaller it is and offers **Replace** and **Save copy** for it; with several
results waiting, both are offered for all of them at once. A result the user never chooses for is
discarded, and the original stays as it was. The **Always** setting can answer ahead of time
(§66); from File Explorer it decides the same way (ADR 0058).

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

*Update (ADR 0064):* the original's size and last-changed time are noted when Gracil starts on it
and checked again before it is replaced; if either changed, the result is deleted and the
original left as it is. A read-only original is never replaced and its read-only mark is never
cleared; the result stays for Save copy. Results are put in place one at a time, in the order
chosen. A tool that says nothing for five minutes (ten for the quality measure above 4K) is
stopped and the next way of compressing tried.

# 40. Compress and save a copy

Create a new file in the same directory with the `Gracil-` prefix (`Vacation.mp4` →
`Gracil-Vacation.mp4`). Do not silently overwrite an existing file; if the name exists, create a
non-conflicting name using a simple predictable strategy (`Gracil-Vacation (2).mp4`). The prefix
is `Gracil-`, with a hyphen. A video from another format keeps its stem and becomes `.mp4`
(`Gracil-IMG_0042.mp4`, §45).

*Update (ADR 0064):* a name that would pass Windows' 255-character limit loses the end of its
stem, never half a character. On a drive without hard links, the copy is moved only to a name
Gracil has just claimed, so it never replaces a file that appeared meanwhile.

# 40.1 Results that are not smaller

*Added after the original specification.*

If the compressed file is not clearly smaller than the original (at most 80% of it, ADR 0029),
Gracil neither replaces the original nor saves a copy. Following §2.3, there is no error dialog.
The file is labelled **Already compressed**, like a file that was already about as small as it
gets (ADR 0055), with a "!" and the reason written out (ADR 0060). Replace and Save copy are not
shown for it, since there is nothing to choose. The original is never touched.

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

Aim for wide compatibility: any video FFmpeg can read is accepted (§77.2). Every output is an MP4,
whatever the source format, carrying HEVC video and audio plus whatever metadata and additional
streams MP4 can hold (ADR 0012). Replacing a video from another format puts the `.mp4` in its
place, with the same stem.

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

The empty state is immediately understandable: **Drop videos or images here**, with a way to
choose files, and the **Always** pill at its foot (ADR 0055, ADR 0057); the list's line comes up
with the first file. Folders can be dropped too. Avoid technical codec terms, terminal-like
presentation, giant settings pages, dense tables, excessive borders, unnecessary navigation,
advanced encoder controls and permanent diagnostic information.

The window opens 440 by 480 (ADR 0076). **Esc** steps back, **Space** pauses and resumes,
**Ctrl+O** chooses files (ADR 0055). **Tab** and **Shift+Tab** move a focus ring through the
window's controls, **Enter** or **Space** presses the ringed one, and the arrow keys move
**Always** (ADR 0059). Gracil follows Windows' **Animation effects** and contrast themes
(ADR 0061).

# 50. Design system

Build reusable visual primitives and design tokens (spacing, corner radius, typography hierarchy,
surface behavior, progress treatment, focus states, animation timing); they live in
`crates/gracil-ui/src/style.rs` and `motion.rs`. The visual identity is Windows 11's own: Segoe UI
Variable, Windows' colors and the user's accent, light or dark with Windows (ADR 0031), its
contrast themes and **Animation effects** (ADR 0061), cards and movement on springs (ADR 0076).
The interaction architecture stays independent from visual choices.

# 50.1 Languages

*Added after the original specification (ADR 0011).*

Gracil is available in English and Spanish and follows the Windows display language, with no
language setting. All interface wording lives in one place, so each language stays complete.

# 51. WGPU optimization

No unnecessary continuous redraws. Measure idle GPU, idle CPU, idle memory and active UI overhead
from early prototypes.

*Update (ADR 0066):* only WGPU's Direct3D 12 code is built in, and no particular graphics card is
asked for. Movements that only follow the work draw about thirty times a second; what answers the
pointer draws every frame. Drawings that stopped moving are drawn once and kept.

# 52. Single-instance behavior

Gracil behaves as one logical instance. Further invocations (launching again, Explorer commands,
future external actions) are forwarded to the existing instance, which owns the scheduler;
launching Gracil again from Start or the taskbar opens its window. It uses a named mutex and a
named pipe, one Gracil per session (ADR 0019).

A development build is a Gracil of its own, **Gracil Dev**, and runs beside the one in daily use
(ADR 0075).

# 53. No permanent service

Do not install a permanent Windows service. Do not keep a background process alive forever merely
to make Explorer integration possible. Start Gracil when needed. If Gracil was launched only for
background work and all jobs are finished, no window must remain visible and no other reason
exists to stay alive, allow it to exit cleanly.

Gracil starts with Windows, at the product owner's request, and stays in the tray for the
session; **Start with Windows** in the tray menu turns it off (ADR 0024, ADR 0054). It uses
`RunOnce`, written again on every start, so Windows forgets it once Gracil's folder is gone
(§64). A Gracil started only for File Explorer's work still exits when done.

# 54. System tray

Gracil has a system tray icon while it runs (§54.1). Logical tray states: Idle, Processing,
Paused, Error. The icon is the marble, drawn by Gracil on a thread of its own (ADR 0020): while
compressing it fills with the work done, and it turns to the warning color for an error the user
hasn't seen yet. When at least one compression job is active, the tray icon must visibly
communicate Processing. Tray state is driven by scheduler state, not timers. Its tip says the
state in words (ADR 0066).

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

*Update (ADR 0066):* Exit also asks while folders are still being looked through; they are kept
for next time with the other unfinished files.

# 55. Tray mini-window

A compact tray-associated mini-window (ADR 0021), opened and closed by a click on the tray icon:
the current operation, basic progress, pause/resume, and files dropped on it are added like any
others. It stays open when the user clicks elsewhere; the user intentionally closes/toggles it.
It is placed against the taskbar's edge, centred on the icon, without assuming the taskbar is
bottom-right, on whichever monitor holds it.

A finished result can be decided in the mini-window itself, with the same **Replace** and **Save
copy** choices (§37.1), so opening the main window is never required. It shows the running file
first; results that wait meanwhile are counted, then offered one at a time.

*Update (ADR 0055):* the mini-window's dial pauses and resumes but has no stop; stopping is in the
main window. When the tray icon turned to the warning color while no window was open, the
mini-window opens on the file that went wrong.

*Update (ADR 0066):* once the whole list is done, the mini-window shows the done view too: the
room saved, how the list went, and the files that failed, whose "!" opens the main window on the
first of them. A Gracil started with Windows makes the mini-window only when the pointer first
comes to the tray icon or files come.

*Update (ADR 0076):* the mini-window shows the small focus, the same one the main window shows
above an open list, with nothing drawn around it; its ring carries the finished list's check too.

# 56. Explorer-triggered mini-window

By default, when compression starts from File Explorer (§57): Gracil receives the jobs, the tray
icon switches to Processing and the mini-window appears.

Setting, in the tray menu (ADR 0021): **Show mini-window when compressing from File Explorer** —
default ON. When OFF, no window appears and compression proceeds entirely in the background. The
setting never changes compression behavior.

# 57. Windows 11 File Explorer integration

Required. Windows only offers its modern menu to packaged apps, and on Windows 11 File Explorer
ignores commands written for the current user only, so Gracil is in File Explorer in two ways
([ADR 0058](decisions/0058-explorer-send-to-and-main-menu.md)), both under **Show more options**:

- **Send to → Gracil**, a shortcut in the user's own Send to folder, written by Gracil itself on
  every start and turned off from the tray (**Show Gracil in Send to**). No administrator needed.
- **Its own commands** in the main menu, written for the whole computer when the user turns them
  on from the tray (**Show Gracil in File Explorer's menu…**), after Windows asks for an
  administrator's permission once.

Both start `gracil.exe --explorer` with what was picked, and the running Gracil applies the
**Always** setting to it. There is no `IExplorerCommand` DLL (ADR 0022 is superseded): File
Explorer never showed one written for the current user.

# 58. Explorer multi-selection

One or several picked videos and images show **Compress with Gracil**, which sends every picked
file; a folder shows **Gracil** with **Compress videos and images**, **Compress videos only** and
**Compress images only** ([ADR 0035](decisions/0035-explorer-folders-and-always.md),
[ADR 0058](decisions/0058-explorer-send-to-and-main-menu.md)). **Send to → Gracil** sends files
and folders the same way, both kinds. The words are fixed; the running Gracil applies
**Always**: set, the files are compressed in the background as it says; at **Ask**, the window
opens and asks.

Folders are looked through away from the window, folder by folder, and work starts with the
first files found ([ADR 0037](decisions/0037-looking-through-folders-in-the-background.md)).

# 59. Dynamic Explorer count

There is no count: File Explorer's plain commands can't count, so the title is fixed, and no
Gracil code runs inside File Explorer (ADR 0058).

# 60. Explorer shell extension safety

There is no shell extension (ADR 0058). File Explorer starts `gracil.exe`, which hands the
request to the running Gracil and leaves, so nothing slow can ever run inside File Explorer.

# 61. Shell extension language

No longer applies: there is no shell extension (ADR 0058). File Explorer integration is Rust +
windows-rs in gracil-windows. Do not migrate the application to C++.

# 62. Background Explorer workflow

1. File Explorer starts `gracil.exe --explorer` with the picked paths (ADR 0058).
2. If Gracil is not running, it starts in background mode.
3. If Gracil is running, the request is sent through IPC and the new process leaves.
4. Add the files to the existing scheduler.
5. Change tray state immediately.
6. Optionally open the mini-window according to the setting.
7. Process the files.

No console window. Do not initialize the main window unless needed. With **Always** at **Ask**
there is no action, so the main window opens and asks.

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

Releases are built on GitHub, on a clean Windows machine, from the tagged code alone, with the
pinned FFmpeg (`scripts/ffmpeg.psd1`): a pushed tag ends in a draft release with the zip and that
version's changelog notes, which the owner publishes (ADR 0079). No release is built or kept on
the owner's computer. The owner's Gracil is installed there from each published release's zip
(`scripts/installer.ps1`), per user in `%LOCALAPPDATA%\Programs\Gracil`; the installer is not
published (ADR 0081). A development build (`cargo run -p gracil-ui`) is **Gracil Dev**, which
runs beside it (ADR 0075).

# 66. Settings philosophy

Settings stay intentionally small. In the tray menu: **Show mini-window when compressing from
File Explorer** (default ON), **Show Gracil in Send to** (ADR 0050, ADR 0058) and **Start with
Windows** (ADR 0054), both default ON, and **Show Gracil in File Explorer's menu…** (ADR 0058),
off until the user turns it on and Windows' permission is given. Do not add GPU/CPU/codec
selection, bitrate, FFmpeg arguments, thread count or advanced encoder configuration.

In the main window, requested by the product owner: **Always**, a pill reading **Replace · Ask ·
Save copy** (ADR 0025, ADR 0060). **Ask**, the default, asks after each file as in §37.1; an end
answers that question ahead of time for every file that finishes afterwards.

Settings are kept in `data\settings.txt` beside `gracil.exe` (ADR 0054).

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
preserving private path history forever. Logs are plain text in `data\logs`, one file a day, the
last week kept, with a cap on what a day's log takes (ADR 0064); they never leave the computer.

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

*Update (ADR 0064):* temporary names carry the run that made them (process and start time) and
are shortened to fit 255 characters. The sweep of an earlier run's files runs on its own thread
beside the work, checks each drive once, and covers image scratch files too. Before compressing,
an empty temporary file is made and removed beside the first file of each folder, so a folder
that takes no new files fails at once. Unused results are deleted by one background worker,
which Gracil waits for on leaving.

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
docs/development/, and the user-facing docs/how-gracil-works.md and docs/faq.md. README explains
what Gracil is, how to build/run it, the project layout, relevant dependencies and where deeper
documentation lives. Documentation is in English (ADR 0074).

# 77. Development milestones

Every milestone compiles and is testable. All of them are built; Gracil 1.0 was released on
2026-11-16. Where a milestone's original wording was later changed, the
section it points to says how it is now.

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
- **M7 — File Explorer integration**: **Send to** and File Explorer's main menu, multi-selection,
  folders, IPC forwarding (§57–§62; the context-menu extension and its count were replaced by
  ADR 0058).
- **M8 — Packaging**: a portable zip with FFmpeg and jpegli inside, built by
  `scripts/portable.ps1` (ADR 0050), on GitHub for releases (ADR 0079).
- **M9 — Image compression** *(added 2026-10-03 at the product owner's request, built
  2026-10-04)*: compress photos and other images the way Gracil compresses video. See §77.1.

Do not attempt to build every milestone in one giant pass. Compressing documents is a proposal,
not a milestone, and is not built (ADR 0062).

# 77.1 Image compression

*Added after the original specification; built 2026-10-04. Gracil's image standard is
[ADR 0034](decisions/0034-image-standard.md), made faster without changing its results by
ADR 0068 and ADR 0078.*

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

- **Two output formats, chosen by Gracil, never by the user.** A PNG stays a PNG, made smaller
  losslessly (oxipng) or with a palette that looks the same, so screenshots, graphics and
  transparency stay as they were. Every other image (JPEG, WebP, HEIC, TIFF, BMP and the rest)
  becomes a JPEG, made with Google's jpegli encoder at a quality where the difference can't be
  seen, or a PNG when it has transparency, which JPEG can't hold. JPEG opens everywhere, the way
  MP4 does for video (ADR 0012), and a JPEG keeps its name. An image that would come out bigger,
  as an iPhone's HEIC often would, is not kept (§40.1).
- **One standard, always.** Fixed, tested settings, like the H.265 standard for video. No
  sliders, presets or advanced options (§66).
- **The same flows.** Drop images on the main window or the mini-window, or send them from File
  Explorer (§58). The same choices after compressing: **Replace** or **Save copy**, and the
  **Always** setting (ADR 0025). The same safe replacement (§39) and collision-safe names (§40).
- **Never reject an image** because it is unusual; fall back to simpler ways first. A result that
  is not smaller is not kept (§40.1).
- **Keep what matters**: date taken, orientation, color profile and the other metadata people
  expect a photo to keep, as for video (§41).
- Everything stays local and private (§82).

The decisions (ADR 0034 unless named):

1. **The format**: PNG stays PNG; everything else becomes JPEG, or PNG with transparency.
2. **The compression**: the same rules as video. Every image keeps its resolution (ADR 0033).
   A JPEG is made at jpegli distance 1.0, fixed; compressing an image already compressed searches
   distances 1.5–3.0 for SSIMULACRA2 80 or more, without asking. A PNG is made lossless plus a
   palette at SSIMULACRA2 90 or more, and 80 or more when compressing again. Kept only at most
   80% of the original, as for video. A PNG's dithered palettes are tried only when the plain one
   with the most colors scores within 5 points of what is needed, and a palette too big to keep
   is still measured, so the ones with fewer colors are not made for nothing (ADR 0078).
3. **What is accepted**:
   - An image with transparency that is not a PNG (a WebP, TIFF, HEIC or AVIF, for example)
     becomes a PNG, so its transparency is kept (ADR 0048 for HEIF and AVIF).
   - **Animated images** (GIF, animated PNG, animated WebP) are not supported: they are neither
     photo nor video, and are left exactly as they are.
   - **Camera RAW** files are not accepted: they are a photographer's negative, and turning one
     into a JPEG is developing it, not compressing it.
   - **JPEG XL** files are not accepted either: the format never became common, and whoever made
     one already has a well-compressed image.
   - A gain-map photo becomes its normal picture; true HDR (PQ, HLG) is left as it is; there is
     no size limit.
4. **The engine**: the JPEG encoder is jpegli only, built by Gracil from Google's official
   repository (github.com/google/jpegli) at a pinned commit and shipped inside the zip; nothing
   from libjxl or JPEG XL is used. jpegli reads JPEG directly (through the libjpeg-turbo copy in
   its repository). Other formats are read inside Gracil (the `image` crate for WebP, TIFF and
   BMP) or by the hidden FFmpeg (HEIC, HEIF, AVIF); Windows' own WIC is not used. oxipng and a
   palette quantizer make PNGs; SSIMULACRA2 is measured inside Gracil (ADR 0068).
5. **One list for every file**, the ring and the tray for the whole list. Files are worked on
   side by side: one video, images beside it, the next file getting ready (ADR 0040). With more
   than one file the ring is titled with how many it holds (**Compressing 24 files**), shows the
   whole list's percentage, and never closes before the real end of the list; how it looks is
   ADR 0076.

# 77.2 Video formats

What Gracil reads, and what it writes (ADR 0012):

| Opened | Saved as |
|---|---|
| MP4 (`.mp4`) | MP4 (H.265), same name |
| M4V, MOV, MKV, AVI, WMV, WebM, MTS, M2TS, TS, 3GP, FLV, MPG, MPEG | MP4 (H.265), `.mp4` |

These are the extensions offered by the file picker and File Explorer's menu
(`media::VIDEO_EXTENSIONS`). Any other file dropped on the window that is not a known image is
handed to FFmpeg as a video and tried, never rejected up front (§2).

# 77.3 Asked for after the plan

*What the product owner asked for once Gracil was working, each now built; the ADR holds the
reasons, and the section named holds the rule.*

- **How much was saved, and the way back to the work**: sizes before and after, the list added
  up, and a quiet line back from a picked file (ADR 0036, §32.1, §32.2).
- **A list that opens at once**, however long (§32), and **numbers in the system's format**
  (§32.2).
- **A clear "done"**: the ring's middle turns green with a check mark, with the room saved and
  how the list went (ADR 0039, now one of the ring's faces, ADR 0076), and **Clear the list**
  offered under it.
- **A ring without its track** and no line between pause and stop (ADR 0038).
- **Several files at once, never in the way**: one video at a time, since one already fills the
  encoder; images several at a time on the processor beside it, fewer when they are very large;
  the next file got ready while the current one compresses, for every kind of file; more while
  the computer is idle, less as soon as the person works in other applications (ADR 0040, §26,
  §33).
- **Carry on after closing**: files left unfinished are offered with **Continue** the next time
  (ADR 0042). **Stop means stop**: the whole list at once, offered with **Continue**; Exit never
  waits on a slow drive (ADR 0044).
- **Never too hot**: Gracil holds the card it uses at about 70 °C with short unseen rests, never a
  stop to cool down (ADR 0043, ADR 0045, ADR 0046), and Intel and AMD cards are read too
  (ADR 0051).
- **Never harm the computer**: Windows itself holds Gracil to part of the processor, memory and
  drive (ADR 0049); efficiency mode while the person works or on battery (ADR 0051); idle only
  when nobody needs the computer, laptops capped at half, on battery below 30 % nothing new
  starts and below 15 % everything waits (**Low battery · waiting**), Gracil's tools together
  hold at most about 65 % of memory, and above 4K the quality is measured at 4K (ADR 0063).
- **Videos from recent iPhones**, whose spatial sound FFmpeg can't read (ADR 0047), and
  **see-through HEIC and AVIF** (ADR 0048).
- **Free and open source, outside the Store**, wholly portable (ADR 0050, ADR 0053, ADR 0054,
  §64). **Intel and AMD graphics compress too** (ADR 0052, §22).
- **Natural from the first use** (ADR 0055), **the list unfolds** (ADR 0056), **Always stays in
  sight** (ADR 0057), **Gracil really in File Explorer** (ADR 0058, §57), **the keyboard reaches
  everything** (ADR 0059), **reasons in sight** (ADR 0060), **Windows' accessibility settings**
  (ADR 0061).
- **Robust with real files** (ADR 0064, §39, §40, §72), **speed** (ADR 0065, ADR 0067,
  ADR 0068, ADR 0070, ADR 0078), **quiet states in words and a lighter start** (ADR 0066),
  **a quiet time left** (ADR 0069, §31).
- **The ring stays, and the window lays itself out around it** (ADR 0076, §31, §32, §55).

Not built: **compressing documents** (Word, Excel, PowerPoint, OpenDocument, PDF) by making the
pictures inside them smaller is a proposal only, waiting for the product owner's decision
([ADR 0062](decisions/0062-compressing-documents.md)).

# 78. UI decisions, no longer temporary

The original specification left these unfinished; all are now decided, and none is coupled to
core logic:

- **Icon**: the marble on a neutral tile (§3; ADR 0015, ADR 0016, ADR 0031).
- **Colors**: Windows 11's own and the user's accent, following Windows' light or dark theme with
  a soft fade, and its contrast themes (ADR 0031, ADR 0061).
- **Typography**: Segoe UI Variable, as Windows 11 uses it.
- **Layout and styling**: the ring, the large and small focus, cards (ADR 0076).
- **Animation**: easing for what follows the work or the pointer, springs for what answers a
  choice, nothing moving when Windows' **Animation effects** are off (ADR 0061, ADR 0076).
- **Wording**: English and Spanish, kept together in `crates/gracil-ui/src/text.rs`
  (ADR 0011).

They can still be refined, through an ADR like any other decision.

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

*Update (ADR 0063):* Iced's processor renderer (tiny-skia) is built in as a fallback, used only
when the graphics card can't draw the window. WGPU stays the renderer.

*Update (ADR 0064):* the safe-replace model is kept and made stricter: an original that changed
since it was compressed, or that is read-only, is never replaced (§39).

If a requirement becomes technically problematic, explain the conflict before changing behavior.

# 82. Privacy

All video processing is local. Do not upload videos, metadata, filenames or paths. No telemetry,
no analytics, no crash reporting that sends user data without explicit future approval. Gracil
never connects to the internet: FFmpeg comes inside its zip, and there is no download (§2.2,
ADR 0053).

# 84. Definition of success

A normal Windows 11 user can unpack Gracil, drag videos into it, understand it immediately, add
more files without losing existing ones, compress them without understanding codecs, pause and
resume, safely replace originals, save compressed copies, preserve important metadata, use the
best hardware path automatically with CPU fallback, select several videos in Explorer, right-click,
choose Gracil, start compression, receive immediate tray feedback and keep working without opening
the full application — while Gracil remains ultra minimalistic, fast, resource-conscious, safe
with user files, clean internally, maintainable and ready to improve continuously.
