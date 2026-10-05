# Architecture Decision Records

Short records of decisions that a future developer might otherwise undo without knowing why.
One file per decision, numbered. To change a decision, add a new record that supersedes the old one.

| # | Decision |
|---|---|
| [0001](0001-core-independent-from-ui.md) | The compression core does not depend on the UI |
| [0002](0002-ffmpeg-as-external-process.md) | FFmpeg runs as a bundled external process |
| [0003](0003-iced-with-wgpu.md) | Iced with the WGPU renderer |
| [0004](0004-one-encode-at-a-time.md) | One compression at a time |
| [0005](0005-msix-packaging.md) | MSIX packaging |
| [0006](0006-explorer-extension-forwards-only.md) | The Explorer extension only forwards requests |
| [0007](0007-close-to-tray.md) | Closing the window always minimizes to the tray |
| [0008](0008-ffmpeg-detected-or-provisioned.md) | FFmpeg is detected or provisioned, not bundled |
| [0009](0009-choose-after-compressing.md) | The main window asks replace or copy after compressing |
| [0010](0010-replacing-updates-modified-date.md) | Replacing keeps the creation date and updates the modified date |
| [0011](0011-english-and-spanish.md) | Gracil speaks English and Spanish, following Windows |
| [0012](0012-every-result-is-mp4.md) | Every compressed video is an MP4 |
| [0013](0013-pause-suspends-ffmpeg.md) | Pause suspends FFmpeg's threads |
| [0014](0014-progress-dial.md) | Progress is a dial that holds the controls |
| [0015](0015-piedra-identity.md) | The marble identity: icon, Piedra palette and following Windows' theme |
| [0016](0016-app-icon.md) | The app icon lives in gracil.exe |
| [0017](0017-leftovers-ledger.md) | Leftover temporary files are tracked in a ledger, not recognized by name |
| [0018](0018-ffmpeg-ends-with-gracil.md) | FFmpeg ends with Gracil, through a job object |
| [0019](0019-single-instance.md) | One Gracil per session: a named mutex and a named pipe |
| [0020](0020-tray.md) | The tray icon is drawn by Gracil, on a thread of its own |
| [0021](0021-mini-window.md) | The mini-window opens from the tray icon, and its one setting lives in the tray menu |
| [0022](0022-explorer-command.md) | File Explorer's menu is one Rust `IExplorerCommand` with two subcommands |
| [0023](0023-package-build.md) | One script builds the package from a manifest template |
| [0024](0024-start-with-windows.md) | Gracil starts with Windows, always |
| [0025](0025-always-replace-or-copy.md) | "Always": replace or save a copy without being asked |
| [0026](0026-icon-tile.md) | The app icon sits on its own Piedra tile, light or dark with the taskbar |
| [0027](0027-version-scheme.md) | Versions stay small: 1.0 at the Store, then slowly |
| [0028](0028-quality-standard.md) | Gracil's quality standard: each video finds its own level |
| [0029](0029-lighter-never-heavier.md) | Lighter, never heavier: predicted sizes and lighter choices for already-compressed videos |
| [0030](0030-faster-quality-measurement.md) | Faster quality measurement: every other frame, and on the GPU when FFmpeg can |
| [0031](0031-windows-11-colors.md) | Windows 11's own colors and the user's accent |
| [0032](0032-resolution-pill.md) | "Always": keep the resolution, ask, or reduce (withdrawn) |
| [0033](0033-keep-the-resolution.md) | Every video keeps its resolution; compressing again loses a little, without asking |
| [0034](0034-image-standard.md) | Gracil's image standard: PNG stays PNG, the rest becomes jpegli JPEG; images first, the dial counts the list |
| [0035](0035-explorer-folders-and-always.md) | File Explorer: folders, one kind at a time, words that follow the Always setting |
| [0036](0036-sizes-and-the-way-back.md) | Sizes before and after, the list added up, and the way back to the work |
| [0037](0037-looking-through-folders-in-the-background.md) | Folders from File Explorer are looked through in the background, folder by folder |
| [0038](0038-a-ring-without-its-track.md) | The ring has no visible track and no line between pause and stop |
| [0039](0039-a-clear-done.md) | A clear "done": a green check in the ring, the room saved, and how the list went |
| [0040](0040-several-files-at-once-without-getting-in-the-way.md) | Several files at once: one video, images beside it, the next file got ready, never in the way |
| [0041](0041-the-window-keeps-up-with-long-work.md) | The window keeps up with long work: Direct3D 12, library errors logged, list counted once per change |
| [0042](0042-carrying-on-after-closing.md) | Unfinished files are kept, and offered with **Continue** the next time Gracil opens |
| [0043](0043-never-too-hot.md) | Never too hot: fewer images when warm, everything waits when too hot, reading NVIDIA cards and Windows' processor limit |
| [0044](0044-stop-means-stop.md) | Stop stops the whole list at once and offers it with **Continue**; Exit never waits on a slow drive |
| [0045](0045-breathing-instead-of-cooling-down.md) | Breathing instead of cooling down: short unseen rests that grow with the heat, never a stop |
| [0046](0046-holding-a-steady-temperature.md) | Holding a steady temperature: about 70 °C at most, one-second breaths, the work share eased up and down gradually |
| [0047](0047-sound-ffmpeg-cannot-read.md) | Sound FFmpeg can't read, such as iPhones' spatial audio, is left out while other sound remains |
| [0048](0048-see-through-heif-and-avif.md) | See-through HEIF and AVIF keep their transparency; depth and mattes aren't transparency |
