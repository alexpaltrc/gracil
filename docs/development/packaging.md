# Packaging Gracil

Gracil ships as a portable zip, free and open source, with no installer and no Store
([ADR 0050](../decisions/0050-portable-and-open-source.md)). Everything goes through
`scripts/portable.ps1`.

## Build the zip

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/portable.ps1
```

This makes a release build and lays out `target\portable\Gracil`, then zips it as
`target\portable\Gracil-<version>-x64.zip`, with the version from `Cargo.toml`. The folder is
made afresh each time and older zips are removed, so only the one just made is left
([ADR 0077](../decisions/0077-where-things-live.md)):

| | |
|---|---|
| `gracil.exe` | Gracil |
| `cjpegli.exe` | jpegli, which compresses every photo ([ADR 0034](../decisions/0034-image-standard.md)) |
| `ffmpeg\` | `ffmpeg.exe` and `ffprobe.exe`, the pinned build ([FFmpeg](ffmpeg.md)) |
| `licenses\` | Gracil's own license (GPL-3.0-or-later), FFmpeg's and where its source is, the licenses of jpegli and the libraries built into it, and those of the Rust libraries built into Gracil |

FFmpeg is the build pinned in `scripts/ffmpeg.psd1`. Without `-FFmpeg <folder>`, the script uses
the first folder on `PATH` whose `ffmpeg.exe` and `ffprobe.exe` have the pinned hashes. When
`cjpegli.exe` has not been built yet, the script first runs `scripts/build-jpegli.ps1`, which needs
git and Visual Studio Build Tools with the C++ workload, CMake and Ninja.

Nothing is signed, so Windows' SmartScreen may warn the first time the zip's Gracil is opened.

The zip of a release is built by GitHub, with this same script, from the tagged code
([ADR 0079](../decisions/0079-releases-built-on-github.md), [versioning](versioning.md)). Here
the script builds the Gracil in daily use, below.

## Maintainer's daily copy

The Gracil used every day, also while developing, lives in `Documents\Gracil`, outside this
project, so the project holds no Gracil to open by mistake ([ADR 0071](../decisions/0071-daily-gracil-outside-the-project.md)).
Start it from its `gracil.exe` there (pinned to Start or the taskbar). It is always kept up to
date ([ADR 0073](../decisions/0073-always-up-to-date.md)): after every change made on this
computer, and with the **Actualizar Gracil** shortcut on the desktop, which first gets what was
merged on GitHub. Both run:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/update.ps1   # -NoPull: only this folder
```

which builds with `scripts/portable.ps1 -Copy`. That closes the Gracil running from that folder,
with its helpers, and copies the new files over, keeping its `data` folder, so settings and
unfinished lists stay; `update.ps1` then opens Gracil again if it was open.
`-CopyTo <folder>` puts it somewhere else. A development build run from `target` keeps its own
`data` folder there and never touches the daily one's. On every start, a whole Gracil folder (with
FFmpeg beside the program) puts itself in File Explorer's **Send to** and its start at the next
sign-in (`RunOnce`), and removes what the former per-user menu left; a development build in
`target` leaves Windows as it is. File Explorer's main menu is written for the whole computer
only when turned on from the tray, after Windows asks for an administrator's permission
([ADR 0058](../decisions/0058-explorer-send-to-and-main-menu.md)). On Windows 11 both
are under **Show more options**. If File Explorer still shows an old menu, restart it (Task
Manager → Windows Explorer → Restart).

To remove Gracil, choose **Prepare to delete Gracil** in its tray menu and delete the folder it
shows (ADR 0054).

## Where Gracil keeps its data

In `data\` beside `gracil.exe`, and nowhere else: `settings.txt`, `logs\`, `unfinished.txt`,
`hints.txt` (the quality search's levels, [ADR 0070](../decisions/0070-hints-kept-between-starts.md))
and `temporary\`. If that folder can't be written, Gracil runs without saving anything. Images
being compressed pass through short-lived `gracil-image-…` files in Windows' temporary folder.

## Licenses

`licenses\` in the zip holds Gracil's license (`gracil-license.txt`, the repository's `LICENSE`,
GPL-3.0-or-later), FFmpeg's (`ffmpeg-license.txt`) with where its source is
(`ffmpeg-source.txt`), jpegli's and its libraries' (`cjpegli-licenses.txt`), and
`rust-libraries.txt`, written by `scripts/licenses.ps1` from `cargo metadata` with the license
files of every Rust library built into `gracil.exe`.

## Versions

The zip's name carries the version from `Cargo.toml` ([versioning](versioning.md)).

## Icons

`scripts/icons.ps1` draws the window's two icon files in `crates/gracil-ui/icons` from the marble's
shapes, in Windows 11's neutrals: a dark tile for a light taskbar or theme, a light one for a dark
taskbar or theme ([ADR 0031](../decisions/0031-windows-11-colors.md)). The files are committed; run
the script again only when the artwork or the colors change. Windows keeps icons in a cache, so a
changed icon may only show after signing out.
