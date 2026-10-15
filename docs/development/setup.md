# Development setup

## Requirements

- Windows 11 x64
- Rust stable (MSVC toolchain), at least the `rust-version` in `Cargo.toml`
- Git
- Visual Studio Build Tools with the C++ workload, CMake and Ninja: only to build jpegli
  (`scripts/build-jpegli.ps1`, ADR 0034), the JPEG encoder images will use
- Optionally NASM (on `PATH`, or installed in `%ProgramFiles%\NASM`): with it, the build script
  builds jpegli's JPEG reader with SIMD, which reads JPEGs quicker (ADR 0068). An existing
  `target\jpegli\build` must be deleted for the script to build it again

## Everyday commands

```powershell
.\scripts\check.ps1   # formatting, Clippy (warnings are errors) and tests
cargo test -p gracil-core
```

Every commit must pass `scripts/check.ps1`.

## Running the app

`cargo run -p gracil-ui` opens Gracil. To compress, it needs the pinned FFmpeg build on `PATH`
([FFmpeg](ffmpeg.md)); images also need `cjpegli.exe`, built once with `scripts/build-jpegli.ps1`.
It never opens a console window, even in development builds,
so what you test is what users see. Logs and crashes go to the `data\logs` folder beside the
build, such as `target\debug\data\logs` (ADR 0050). A build in `target` keeps its own settings
there and never puts itself in File Explorer or in the start with Windows.

The interface follows the Windows display language. To check the other language, set
`GRACIL_LANGUAGE` to `en` or `es` before running (ADR 0011).

## Tests that touch files

Tests that create, rename or delete files must only do so inside a fresh directory under the
system temporary folder, and remove it afterwards. Never point tests at real personal files.
