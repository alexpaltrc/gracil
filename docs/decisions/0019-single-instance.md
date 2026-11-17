# 0019 — One Gracil per session: a named mutex and a named pipe

**Status:** Accepted, built (2026-10-03).

**Context.** Spec §52 asks for one logical Gracil: a second launch, "Open with" from File Explorer
and the Explorer commands (M7) must all reach the Gracil already running, which owns the list and
the scheduler. A process started only for Explorer's commands must not show a window (§62) and
must leave once its work is done (§53).

**Decision.**

1. At startup Gracil creates the mutex `Local\Gracil.SingleInstance`. The process that creates
   it is *the* Gracil of the session; it keeps the handle until it ends, so Windows frees the name
   even after a crash.
2. That Gracil serves the pipe `\\.\pipe\Gracil.<session id>` on a thread of its own: one inbound
   byte pipe, created as the first instance (so nothing else can take the name first), local
   clients only, Windows' default permissions (only the same user and administrators can write).
3. Any later process finds the mutex taken and sends its command line through the pipe: each
   argument in UTF-16 followed by a NUL, so any file name survives. It waits up to ten seconds
   for a Gracil that is still starting, allows it to take the foreground, and exits. If it can't
   get through, it does the work itself rather than lose the request.
4. The command line and the pipe carry the same thing, read by one parser:
   - `gracil.exe [videos]` shows the window and adds the videos; the user chooses afterwards;
   - `gracil.exe --replace videos` and `--save-copy videos` (what Explorer will send) compress in
     the background and place each result as asked.

   Paths are made absolute where they are read, and anything else is checked like a dropped file.
5. Gracil starts without a window. The first request decides whether one opens. A Gracil that
   never showed its window leaves once nothing is waiting, running or left to decide on.
6. Gracil ends its own process when it leaves (after deleting results nobody chose for and
   writing out its log), because Iced 0.14's daemon does not return from `run` once no window is
   open.

**Consequences.**

- Requests are additive: Explorer and later launches add to the one list (§63).
- A video added in the window and later sent with an Explorer command takes that command's
  choice; if it is already done, it is placed straight away.
- A result an Explorer command could not place (for example, a name already taken) keeps a
  background Gracil running so the result isn't lost; the tray icon shows that something needs attention ([0020](0020-tray.md)).
- The pipe is reachable by other programs of the same user. They can only ask for what the user
  could ask for on the command line, and every path is still checked.
