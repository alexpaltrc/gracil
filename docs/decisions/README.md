# Architecture Decision Records

Short records of decisions that a future developer might otherwise undo without knowing why.
One file per decision, numbered. To change a decision, add a new record that supersedes the old one.

| # | Decision |
|---|---|
| [0001](0001-core-independent-from-ui.md) | The compression core does not depend on the UI |
| [0002](0002-ffmpeg-as-external-process.md) | FFmpeg runs as a bundled external process |
| [0003](0003-iced-with-wgpu.md) | Iced with the WGPU renderer |
| [0004](0004-one-encode-at-a-time.md) | One compression at a time |
| 0005 | MSIX packaging (removed by 0053; Gracil is a portable zip, 0050) |
| [0006](0006-explorer-extension-forwards-only.md) | The Explorer extension only forwards requests |
| [0007](0007-close-to-tray.md) | Closing the window always minimizes to the tray |
| [0008](0008-ffmpeg-detected-or-provisioned.md) | FFmpeg is found, never downloaded: the copy in the zip, or PATH while developing (changed by 0050, 0053) |
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
| 0023 | Building the MSIX package (removed by 0053) |
| [0024](0024-start-with-windows.md) | Gracil starts with Windows, always |
| [0025](0025-always-replace-or-copy.md) | "Always": replace or save a copy without being asked |
| 0026 | Piedra tiles for the package icons (removed by 0053) |
| [0027](0027-version-scheme.md) | Versions stay small: 1.0 at the first public release, then slowly |
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
| [0049](0049-limits-windows-enforces.md) | Limits Windows enforces: part of the processor (less while in use and on battery), memory per process, room on the drive |
| [0050](0050-portable-and-open-source.md) | Portable, free and open source on GitHub, without the Store: a zip with FFmpeg, data beside the program, the classic File Explorer menu |
| [0051](0051-efficiency-mode-and-any-card-temperature.md) | Windows' efficiency mode for the tools while the computer is in use or on battery; the temperature of Intel and AMD graphics cards from their driver |
| [0052](0052-intel-and-amd-encoders.md) | Compressing on Intel (Quick Sync) and AMD graphics, after NVIDIA and before the processor, with tiers and test encodes as for NVENC |
| [0053](0053-ready-for-github.md) | Ready for GitHub: no FFmpeg download, nothing left from the Store, `-Copy` keeps the daily Gracil in one place |
| [0054](0054-wholly-portable.md) | Wholly portable: data only in Gracil's folder, a start with Windows that forgets itself, **Prepare to delete Gracil**, GPL-3.0-or-later with every library's license |
| [0055](0055-natural-first-use.md) | Natural from the first use: folders can be dropped, an empty window with only where to drop, an honest done screen, a breathing "getting ready", Exit that counts every loss, a few keys |
| [0056](0056-the-list-unfolds.md) | The list unfolds and folds away instead of appearing at once |
| [0057](0057-always-in-sight.md) | **Always** stays in sight, the empty window included |
| [0058](0058-explorer-send-to-and-main-menu.md) | File Explorer: **Send to** for everyone, and a main menu for the whole computer turned on from the tray; the menu DLL is gone |
| [0059](0059-keyboard-reaches-everything.md) | The keyboard reaches every control: Tab moves a ring, Enter or Space presses, arrows move **Always** |
| [0060](0060-reasons-in-sight.md) | Reasons are written out under the file shown large; the middle of **Always** says **Ask** |
| [0061](0061-windows-accessibility-settings.md) | Gracil follows Windows' **Animation effects** and **contrast themes**; text on the accent is white or black, whichever reads better |
| [0062](0062-compressing-documents.md) | *Proposed:* compressing documents (Word, Excel, PowerPoint, OpenDocument, PDF) by making the pictures inside them smaller, nothing else changed |
| [0063](0063-safe-on-any-computer.md) | Safe on any computer: idle only when nobody needs it, laptops capped at half, a low battery waits, sleep rests and retries, memory over the whole job, above 4K measured at 4K, a window drawn without a graphics card |
| [0064](0064-robust-with-real-files.md) | Robust with real files: a file that changed is left alone, a stuck tool is stopped, results placed in order, cloud files freed up again, read-only never replaced, system folders left out, shutdown keeps the list, a Gracil inside its zip says so |
| [0065](0065-speed.md) | Speed: the list counts as it goes, samples stay on the graphics card and run together on an idle desktop, each search starts where the last video like it ended, capabilities detected all at once, a few more images on a desktop in use, Intel and AMD graphics decode too (untried on that hardware) |
| [0066](0066-quiet-states-in-words-and-a-lighter-start.md) | Quiet states in words (tray tip and title, the mini-window done, why files failed, inside the zip, downloading, Exit while looking), and a lighter start: the engine after the first window, no hidden mini-window in the tray, Direct3D 12 alone, fewer frames |
| [0069](0069-a-quiet-time-left.md) | A quiet time left: *about 20 min* after the files left and in the tray tip, rounded coarsely, never creeping up, hidden while paused, resting, looking or near the end; worked out from each kind's bytes and pace |
