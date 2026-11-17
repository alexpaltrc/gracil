# 0050 — Portable, free and open source, without the Store

**Status:** Accepted, built (2026-10-05); items 4 to 6 superseded by ADR 0058 (2026-10-07), below.
Decided by the product owner. Supersedes ADR 0005 (MSIX packaging), ADR 0023 (package build) and
ADR 0026 (icon tile), all three since removed ([0053](0053-ready-for-github.md)), the package
parts of [0022](0022-explorer-command.md) and [0024](0024-start-with-windows.md), and the Store
part of [0027](0027-version-scheme.md); replaces the FFmpeg lookup order of
[0008](0008-ffmpeg-detected-or-provisioned.md). Changes spec §64 and §65.

**Superseded in part** by [ADR 0058](0058-explorer-send-to-and-main-menu.md) (2026-10-07):
items 4 to 6. File Explorer ignored the per-user menu, so the zip has no `gracil_shell.dll`.
Gracil puts itself in **Send to** on every start (the tray setting now reads **Show Gracil in
Send to**), and its main menu is written for the whole computer only when turned on from the
tray.

**Context.** The product owner reserved Gracil's name in the Microsoft Store and then learned that
Gracil could not stay there, even for free. Gracil goes on as a free, open-source program
published on GitHub, and portable: a zip that is unpacked and run, with no installer. Without a
package Windows gives Gracil no identity, so the modern menu of Windows 11
(`windows.fileExplorerContextMenus`), the package's start-up task and its own copy of
`%LOCALAPPDATA%` are gone. HEVC and HEIC stay: for free software distributed without charge the
patent pools ask for nothing in practice.

**Decision.**

1. **A zip.** `scripts/portable.ps1` builds `Gracil-<version>-x64.zip`, holding one `Gracil`
   folder: `gracil.exe`, `gracil_shell.dll`, `cjpegli.exe`, `ffmpeg\ffmpeg.exe` and
   `ffmpeg\ffprobe.exe`, and `licenses\`. Nothing is signed; Windows' SmartScreen may warn on
   first run.
2. **Data beside the program.** Settings, logs, unfinished files and temporary files live in
   `data\` next to `gracil.exe`, so the folder can be moved or carried on a USB drive. When that
   folder can't be written (a read-only drive, or a folder only an administrator can change),
   `%LOCALAPPDATA%\Gracil` was used instead, until [ADR 0054](0054-wholly-portable.md) removed
   that: Gracil then runs without saving anything.
3. **FFmpeg comes in the zip**: the BtbN `win64-gpl` build Gracil was already tested with, pinned
   by hash in `packaging/ffmpeg.psd1`, with its license and a pointer to its source in
   `licenses\`. Gracil looks for FFmpeg beside itself first, then, while developing, on `PATH`.
   The private download of ADR 0008 was kept here as a last fallback;
   [ADR 0053](0053-ready-for-github.md) removed it, so Gracil never uses the internet.
4. **File Explorer's classic menu.** The same `IExplorerCommand` (ADR 0022) is registered for the
   current user only (`HKCU\Software\Classes`, no administrator), for files and folders. On
   Windows 11 it shows under **Show more options**. Explorer loads a copy kept in
   `data\shell\`, named after the shipped DLL's size and date, so the DLL beside `gracil.exe` is
   never held and an update can replace it. The registration also records where `gracil.exe` and
   the data folder are, so the menu starts the right Gracil and reads its settings.
5. **Settled on every start.** Each time Gracil starts, it registers the menu (or removes it) and
   its start with Windows (`HKCU\...\Run`, `--startup`) again, so a moved or updated folder keeps
   working. Only a whole Gracil folder, with FFmpeg and the DLL beside the program, does this: a
   development build in `target` leaves Windows as it is.
6. **The menu can be turned off** from the tray menu (**Show Gracil in File Explorer's menu**),
   saved as `explorer_menu` in the settings. Turning it off removes the registration, but only
   when it points at this folder.
7. **Removing Gracil** is deleting its folder; the next start of another copy replaces the
   registration, and turning the menu off first leaves nothing behind but the start-up entry,
   which Windows' **Startup apps** can turn off.
8. **Open source.** The code is published on GitHub. Versions stay as in ADR 0027, with 1.0 at
   the first public release instead of the first Store release.

**Consequences.** No Store review, no certificate and no installer. The menu sits one click deeper
on Windows 11. The zip is large (FFmpeg is about 290 MB of it). A folder deleted while its menu is
registered leaves a menu entry that does nothing until another Gracil starts, or until it is
removed by hand. Building our own LGPL FFmpeg is no longer needed for licensing; it waits until it
brings something else, such as quality measurement on the graphics card (ADR 0030).
