# 0022 — File Explorer's menu is one Rust `IExplorerCommand` with two subcommands

**Status:** Accepted. Carries out [0006](0006-explorer-extension-forwards-only.md); registered by
the package of [0005](0005-msix-packaging.md).

**Changed** by [ADR 0050](0050-portable-and-open-source.md) (2026-10-06): the command is registered for the current user in File Explorer's classic menu instead of by a package; how it works is unchanged.

**Context.** Spec §57–§62 ask for **Gracil (N)** in Windows 11's modern context menu, with
**Compress and replace** and **Compress and save a copy**, for one or many videos. The handler
runs inside `explorer.exe`, must answer at once and must never take the shell down. §61 prefers
Rust with `windows-rs`.

**Decision.**

1. `crates/gracil-shell` builds `gracil_shell.dll`, an in-process COM server with one class
   (CLSID `8D7CC31A-14B9-490F-A5B0-3A2B63DAC4EF`, also in the package manifest). It uses the
   `windows` crate, which can implement COM interfaces; the rest of Gracil keeps `windows-sys`.
2. The class is the top entry: an `IExplorerCommand` with `ECF_HASSUBCOMMANDS` whose
   `EnumSubCommands` gives the two commands. Its title counts the selected files whose name ends
   in a video extension (`gracil_core::media`, the same list as the file picker): "Gracil" for
   one, "Gracil (N)" from two (`selection::title`, the one place to change that). It is hidden
   when the selection holds no video. Only names are read; no file is opened.
3. The package registers it for every file type (`*`), and the count decides whether it shows.
   That way a mixed selection still offers Gracil for its videos, and the extension list lives in
   one place.
4. A click hands the videos over on a thread of the DLL, so Explorer's menu closes at once. If
   Gracil is running (its single-instance mutex exists, ADR 0019), the request goes through the
   pipe. Otherwise Gracil is started (since ADR 0023, through the package's alias when the DLL is
   packaged; `gracil.exe` next to the DLL otherwise) with `--replace` or `--save-copy` and
   as many paths as fit in 30,000 characters of command line, passed as separate arguments, never
   as a shell string; the rest follow through the pipe once Gracil is listening. The DLL stays
   loaded while that thread or any of its objects lives (`DllCanUnloadNow`).
5. `#[implement]` generates `unsafe` code, so `unsafe` is allowed in the one module that only
   declares the COM objects. Elsewhere each `unsafe` block has its own allowance and reason.
6. `examples/invoke.rs` loads the built DLL and clicks a command as Explorer would, so the
   hand-over can be tried without installing the package. Explorer itself can only load the menu
   from a registered package.

**Consequences.**

- Any file with a video extension shows the menu, even if it turns out not to be a video;
  Gracil then reports that quietly, as it would for a dropped file.
- A video already on Gracil's list is not added twice, also from Explorer.
- The DLL links `gracil-core` for the extension list, and `gracil-windows` for the language,
  the mutex and the pipe; nothing that draws, encodes or probes.
