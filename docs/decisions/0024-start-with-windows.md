# 0024 — Gracil starts with Windows, always

**Status:** Accepted. Requested by the product owner (2026-10-03), as spec §53 and §81 foresaw
("unless requested later"). Changes §53 for Gracil's own process; still no Windows service.

**Changed** by [ADR 0050](0050-portable-and-open-source.md) (2026-10-05): Gracil starts with
Windows through the current user's `Run` key, written again on every start. **Changed** by
[ADR 0054](0054-wholly-portable.md): the `RunOnce` key instead, so a deleted folder leaves nothing,
and a **Start with Windows** setting in the tray menu replaces items 1 and 3.

**Context.** Gracil lives in the tray (spec §54.1). Started with the session, it is ready the
moment a video is dropped on the mini-window or sent from File Explorer, with its marble always
in place.

**Decision.**

1. Each time Gracil starts, it writes `"<its folder>\gracil.exe" --startup` to
   `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`, so a moved or updated folder keeps
   starting. Windows runs it at sign-in.
2. `--startup` adds nothing and opens no window: Gracil sits in the tray. A Gracil started this
   way stays for the whole session; it does not leave when background work is done (`resident`
   in `app.rs`). Sent to a Gracil already running, it changes nothing.
3. Gracil has no setting for it. People who don't want it turn it off where Windows already lets
   them: Settings → Apps → Startup, or Task Manager's Startup apps. Windows keeps that choice
   apart from the `Run` value, so writing the value again never turns it back on.
4. A Gracil started any other way keeps the old rule: started only for File Explorer's work and
   never shown, it leaves when that work is done.

**Consequences.**

- One small process with a tray icon is present all session, idle until there is work.
- Only a whole Gracil folder starts with Windows; development builds run from `target/` don't.
