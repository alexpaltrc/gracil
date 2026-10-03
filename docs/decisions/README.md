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
