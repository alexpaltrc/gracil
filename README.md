# Gracil

Gracil is a small Windows 11 app that compresses videos and images. Drop them in, and Gracil makes
them smaller while keeping them looking the same. It picks the best way to compress on your computer
automatically: your NVIDIA graphics card when it can, your processor otherwise.

Gracil is free, open source and private. It needs no account, no installer and no internet
connection: unpack the zip anywhere, even on a USB drive, and open `gracil.exe`.

Gracil is in early development (version `0.0.1-dev`).

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

How the crates fit together: [docs/architecture/overview.md](docs/architecture/overview.md). Building
the zip: [docs/development/packaging.md](docs/development/packaging.md).

## Main dependencies

- [Iced](https://iced.rs) with WGPU for the interface
- [FFmpeg](https://ffmpeg.org) and FFprobe for the actual video work, included in the zip
- [jpegli](https://github.com/google/jpegli) for photos
- [windows-rs](https://github.com/microsoft/windows-rs) for Windows integration

Gracil works fully offline. It never uploads videos, file names or any other data.
