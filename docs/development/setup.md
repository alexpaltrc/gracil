# Development setup

## Requirements

- Windows 11 x64
- Rust stable (MSVC toolchain), at least the `rust-version` in `Cargo.toml`
- Windows 11 SDK (provides `makeappx.exe` and `signtool.exe` for packaging)
- Git

## Everyday commands

```powershell
.\scripts\check.ps1   # formatting, Clippy (warnings are errors) and tests
cargo test -p gracil-core
```

Every commit must pass `scripts/check.ps1`.

## Tests that touch files

Tests that create, rename or delete files must only do so inside a fresh directory under the
system temporary folder, and remove it afterwards. Never point tests at real personal files.
