# Development setup

## Requirements

- Windows 11 x64
- Rust stable (MSVC toolchain), at least the `rust-version` in `Cargo.toml`
- Git
- Visual Studio Build Tools with the C++ workload, CMake and Ninja: only to build jpegli
  (`scripts/build-jpegli.ps1`, ADR 0034), the JPEG encoder Gracil uses for photos
- Optionally NASM (on `PATH`, in `%ProgramFiles%\NASM`, or its portable zip from nasm.us unpacked
  into `%LOCALAPPDATA%\bin\NASM`): with it, the build script builds jpegli's JPEG reader with
  SIMD, which reads JPEGs a little quicker with the same output (ADR 0068). An existing
  `target\jpegli\build` must be deleted for the script to build it again

## Everyday commands

```powershell
.\scripts\check.ps1   # formatting, Clippy (warnings are errors) and tests
cargo test -p gracil-core
```

Every commit must pass `scripts/check.ps1`. On Linux, such as in a cloud session,
`scripts/check.sh` runs what can run there ([Developing from the cloud](cloud.md)).

`rust-toolchain.toml` names the toolchain, so `rustup` installs the right one by itself.

`cargo run --release -p gracil-core --example compress -- <file>` compresses one file with the
engine alone, without the interface: proof that the engine stands on its own (ADR 0001).

## Running the app

`cargo run -p gracil-ui` opens a development build, **Gracil Dev**. It is a Gracil of its own:
it runs beside the Gracil in daily use and never hands it its files
([ADR 0075](../decisions/0075-a-development-build-of-its-own.md)). To compress, it needs the
pinned FFmpeg build on `PATH` ([FFmpeg](ffmpeg.md)); images also need `cjpegli.exe`, built once
with `scripts/build-jpegli.ps1`. It never opens a console window, even in development builds, so
what you test is what users see. Logs and crashes go to the `data\logs` folder beside the build,
such as `target\debug\data\logs` (ADR 0050). A build in `target` keeps its own settings there
and never puts itself in File Explorer or in the start with Windows.

The interface follows the Windows display language. To check the other language, set
`GRACIL_LANGUAGE` to `en` or `es` before running (ADR 0011). In the same way, `GRACIL_THEME` set
to `light` or `dark` shows the other theme without changing Windows'.

## Tests that touch files

Tests that create, rename or delete files must only do so inside a fresh directory under the
system temporary folder, and remove it afterwards. Never point tests at real personal files.
