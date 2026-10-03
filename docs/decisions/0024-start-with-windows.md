# 0024 — Gracil starts with Windows, always

**Status:** Accepted. Requested by the product owner (2026-10-03), as spec §53 and §81 foresaw
("unless requested later"). Changes §53 for Gracil's own process; still no Windows service.

**Context.** Gracil lives in the tray (spec §54.1). Started with the session, it is ready the
moment a video is dropped on the mini-window or sent from File Explorer, with its marble always
in place.

**Decision.**

1. The package declares a second, hidden application (`Startup`, not in the Start menu) with a
   `windows.startupTask` (`GracilStartup`, enabled) that runs `gracil.exe --startup`
   (`uap10:Parameters`). Windows runs it at sign-in with the package's identity.
2. `--startup` adds nothing and opens no window: Gracil sits in the tray. A Gracil started this
   way stays for the whole session; it does not leave when background work is done (`resident`
   in `app.rs`). Sent to a Gracil already running, it changes nothing.
3. Gracil has no setting for it. People who don't want it turn it off where Windows already lets
   them: Settings → Apps → Startup, or Task Manager's Startup apps. Gracil respects that choice
   and never turns it back on.
4. A Gracil started any other way keeps the old rule: started only for File Explorer's work and
   never shown, it leaves when that work is done.

**Consequences.**

- One small process with a tray icon is present all session, idle until there is work.
- Only the packaged Gracil starts with Windows; development builds run from `target/` don't.
