# 0007 — Closing the window always minimizes to the tray

**Status:** Accepted (product requirement, spec §54.1)

**Context.** Compressions can take a long time. Users close windows expecting the work to
continue, and Explorer-started work already lives in the tray.

**Decision.** Closing the main window (or the mini-window) never quits Gracil and never affects
running work. The tray icon is always present while the process runs, and its right-click menu
contains Exit, which is the only way to quit. This is intentionally not a setting.

**Consequences.** The process outlives its windows, so the UI runs as an Iced daemon. Exit must
handle running work safely (cancel FFmpeg, remove Gracil's temporary output, leave originals
untouched). A process started from Explorer that never showed a window still exits by itself when
its work is done, so Gracil does not linger indefinitely (spec §53).
