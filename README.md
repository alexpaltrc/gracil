# Gracil

Gracil is a small Windows 11 app that compresses videos. Drop videos in, and Gracil makes them
smaller while keeping them looking the same. It picks the best way to compress on your computer
automatically: your NVIDIA graphics card when it can, your processor otherwise.

Gracil is free and private. It needs no account and no internet connection. The only exception is a
one-time automatic FFmpeg install on computers that don't have it yet.

Gracil is in early development (version `0.0.1-dev`).

## Building

Requirements: Windows 11 x64, Rust stable (MSVC), Windows 11 SDK, Git. Details in
[docs/development/setup.md](docs/development/setup.md).

```powershell
.\scripts\check.ps1
```

## Project layout

| Path | Contents |
|---|---|
| `crates/gracil-core` | The compression engine. Independent from the UI. |
| `docs/how-gracil-works.md` | How Gracil works, in plain language, for users |
| `docs/specification.md` | Product and engineering specification |
| `docs/architecture/` | How Gracil is put together |
| `docs/decisions/` | Why important decisions were made |
| `docs/development/` | Setup, versioning, release process |
| `scripts/` | Development scripts |

Further crates (`gracil-ui`, `gracil-windows`, `gracil-shell`) and the `packaging/` folder are
added in the milestones that need them; see [docs/architecture/overview.md](docs/architecture/overview.md).

## Main dependencies

- [Iced](https://iced.rs) with WGPU for the interface
- [FFmpeg](https://ffmpeg.org) and FFprobe for the actual video work. Gracil uses an FFmpeg already
  installed on the PC, or installs its own private copy invisibly.
- [windows-rs](https://github.com/microsoft/windows-rs) for Windows integration

Apart from that one-time FFmpeg install, Gracil works fully offline. It never uploads videos, file
names or any other data.
