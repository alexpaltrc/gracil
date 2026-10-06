# Gracil

Gracil is a small Windows 11 app that compresses videos and images. Drop them in, and Gracil makes
them smaller while keeping them looking the same. It picks the best way to compress on your computer
automatically: your graphics card when it can (NVIDIA, Intel or AMD), your processor otherwise.

Gracil is free, open source and private. It needs no account, no installer and no internet
connection: unpack the zip anywhere, even on a USB drive, and open `gracil.exe`.

Gracil is in development towards its first public version, 1.0 (now `1.0.0-dev`).
How it works, in plain words: [docs/how-gracil-works.md](docs/how-gracil-works.md). Questions:
[docs/faq.md](docs/faq.md).

## Building

Requirements: Windows 11 x64, Rust stable (MSVC), Git. Details in
[docs/development/setup.md](docs/development/setup.md).

```powershell
.\scripts\check.ps1
```

## Project layout

| Path | Contents |
|---|---|
| `crates/gracil-core` | The compression engine. Independent from the UI. |
| `crates/gracil-ui` | The Gracil app: windows, tray, mini-window |
| `crates/gracil-windows` | Windows integration: single instance, pipe, tray icon, safe replace |
| `crates/gracil-shell` | File Explorer's Gracil menu |
| `packaging/` | The FFmpeg build that ships in the zip |
| `docs/how-gracil-works.md` | How Gracil works, in plain language, for users |
| `docs/specification.md` | Product and engineering specification |
| `docs/architecture/` | How Gracil is put together |
| `docs/decisions/` | Why important decisions were made |
| `docs/development/` | Setup, versioning, release process |
| `scripts/` | Development scripts |
| `.github/workflows/` | The same checks, run on GitHub for every change |

How the crates fit together: [docs/architecture/overview.md](docs/architecture/overview.md).
Building the zip: [docs/development/packaging.md](docs/development/packaging.md).

## Main dependencies

- [Iced](https://iced.rs) with WGPU for the interface
- [FFmpeg](https://ffmpeg.org) and FFprobe for the actual video work, included in the zip
- [jpegli](https://github.com/google/jpegli) for photos
- [windows-rs](https://github.com/microsoft/windows-rs) (`windows` and `windows-sys`) for Windows integration

Gracil works fully offline. It never uploads videos, file names or any other data.

## License

Copyright (C) 2026 Alex Palacios

Gracil is free software, licensed under the [GNU General Public License, version 3 or any later
version](LICENSE). The zip carries the licenses of everything inside it in its `licenses` folder:
Gracil's, FFmpeg's (GPL), jpegli's and its libraries' (BSD and similar), and those of the Rust
libraries built into `gracil.exe` and `gracil_shell.dll`.
