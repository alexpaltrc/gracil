# 0006 — The Explorer extension only forwards requests

**Status:** Accepted, built (2026-10-03). Superseded in part by
[ADR 0058](0058-explorer-send-to-and-main-menu.md) (2026-10-07), below.

**Superseded in part** by [ADR 0058](0058-explorer-send-to-and-main-menu.md) (2026-10-07):
there is no Explorer extension any more. File Explorer reaches Gracil through **Send to** and,
when turned on, plain commands that start `gracil.exe`; no Gracil code runs inside
`explorer.exe`, which keeps this decision's aim.

**Context.** The context-menu handler runs inside `explorer.exe`. A slow or crashing handler
freezes or crashes the whole shell.

**Decision.** `gracil-shell.dll` only counts supported files by extension (no file access beyond
what Explorer passes), builds the menu, and on click hands the paths and action to `gracil.exe`
over the named pipe, starting it in background mode if needed. It never loads Iced, WGPU, FFmpeg
or any heavy dependency, and never waits for compression.

**Consequences.** All product logic stays in Gracil's own process. The DLL stays small enough to
review completely.
