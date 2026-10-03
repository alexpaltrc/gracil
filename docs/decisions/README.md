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
