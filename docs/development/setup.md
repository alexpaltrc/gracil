# Development setup

## Requirements

- Windows 11 x64
- Rust stable (MSVC toolchain), at least the `rust-version` in `Cargo.toml`
- Windows 11 SDK (provides `makeappx.exe` and `signtool.exe` for packaging)
- Git
- Visual Studio Build Tools with the C++ workload, CMake and Ninja: only to build jpegli
  (`scripts/build-jpegli.ps1`, ADR 0034), the JPEG encoder images will use

## Everyday commands

```powershell
.\scripts\check.ps1   # formatting, Clippy (warnings are errors) and tests
cargo test -p gracil-core
```

Every commit must pass `scripts/check.ps1`.

## Running the app

`cargo run -p gracil-ui` opens Gracil. It never opens a console window, even in development builds,
so what you test is what users see; logs and crashes go to `%LOCALAPPDATA%\Gracil\logs`.

The interface follows the Windows display language. To check the other language, set
`GRACIL_LANGUAGE` to `en` or `es` before running (ADR 0011).

## Tests that touch files

Tests that create, rename or delete files must only do so inside a fresh directory under the
system temporary folder, and remove it afterwards. Never point tests at real personal files.
