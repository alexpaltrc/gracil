# Packaging Gracil

Gracil ships as a portable zip, free and open source, with no installer and no Store
([ADR 0050](../decisions/0050-portable-and-open-source.md)). Everything goes through
`scripts/portable.ps1`.

## Build the zip

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/portable.ps1
```

This makes a release build and lays out `target\portable\Gracil`, then zips it as
`target\portable\Gracil-<version>-x64.zip`, with the version from `Cargo.toml`:

| | |
|---|---|
| `gracil.exe` | Gracil |
| `gracil_shell.dll` | File Explorer's menu ([ADR 0022](../decisions/0022-explorer-command.md)) |
| `cjpegli.exe` | jpegli, which compresses every photo ([ADR 0034](../decisions/0034-image-standard.md)) |
| `ffmpeg\` | `ffmpeg.exe` and `ffprobe.exe`, the pinned build ([FFmpeg](ffmpeg.md)) |
| `licenses\` | FFmpeg's license and where its source is, the licenses of jpegli and the libraries built into it, and Gracil's own |

FFmpeg is the build pinned in `packaging/ffmpeg.psd1`. Without `-FFmpeg <folder>`, the script uses
the first folder on `PATH` whose `ffmpeg.exe` and `ffprobe.exe` have the pinned hashes. When
`cjpegli.exe` has not been built yet, the script first runs `scripts/build-jpegli.ps1`, which needs
git and Visual Studio Build Tools with the C++ workload, CMake and Ninja.

Nothing is signed, so Windows' SmartScreen may warn the first time the zip's Gracil is opened.

## The Gracil in daily use

The Gracil used every day, also while developing, lives in `%LOCALAPPDATA%\Programs\Gracil`. Start
it from its `gracil.exe` there (pinned to Start or the taskbar). To update it with the current
code:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/portable.ps1 -Install
```

This closes the Gracil running from that folder, with its helpers, and copies the new files over,
keeping its `data` folder, so settings and unfinished lists stay. Then start `gracil.exe` again.
`-InstallTo <folder>` puts it somewhere else. A development build run from `target` keeps its own
`data` folder there and never touches the daily one's. On every start, a whole Gracil folder (with
FFmpeg and the DLL beside the program) registers File Explorer's menu for the current user and
starts with Windows; a development build in `target` leaves both as they are. On Windows 11 the
menu is under **Show more options**. If File Explorer still shows an old menu, restart it (Task
Manager → Windows Explorer → Restart).

To remove Gracil, turn off **Show Gracil in File Explorer's menu** in its tray menu, exit, and
delete the folder. Its start with Windows can be turned off in Settings → Apps → Startup.

## Where Gracil keeps its data

In `data\` beside `gracil.exe`: `settings.txt`, `logs\`, `unfinished.txt`, `temporary\` and
`shell\` (the copy of the menu's DLL that File Explorer loads). If that folder can't be written,
`%LOCALAPPDATA%\Gracil` is used instead.

## Versions

The zip's name carries the version from `Cargo.toml` ([versioning](versioning.md)).

## Icons

`scripts/icons.ps1` draws the window's two icon files in `assets/icons/ico` from the marble's
shapes, in Windows 11's neutrals: a dark tile for a light taskbar or theme, a light one for a dark
taskbar or theme ([ADR 0031](../decisions/0031-windows-11-colors.md)). The files are committed; run
the script again only when the artwork or the colors change. Windows keeps icons in a cache, so a
changed icon may only show after signing out.
