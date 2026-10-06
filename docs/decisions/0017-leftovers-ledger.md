# 0017 — Leftover temporary files are tracked in a ledger, not recognized by name

**Status:** Accepted.

**Context.** Gracil writes each compression to a temporary sibling (`<name>.gracil-<id>.part`)
and, when replacing in place, parks the original as `<name>.gracil-<id>.bak` for a moment. A
normal run removes both. If Gracil is killed, crashes or loses power, they stay behind, and a
`.bak` may then be the only copy of the user's original. Spec §72 forbids deleting files merely
because their names look like Gracil's.

**Decision.**

1. Each Gracil process writes down every `.part` it creates and every `.bak` it is about to make
   (with the original it belongs to) in a ledger of its own:
   `<data folder>\temporary\<pid>-<start time>.txt`, one tab-separated line per file,
   written to disk at once.
2. The process holds its ledger open with no sharing for as long as it runs. Being able to open a
   ledger therefore proves its process has ended; no process ids are compared, so a reused id
   cannot fool it.
3. At startup, before any compression can begin, Gracil settles every ledger it can open:
   - a `.part` is deleted;
   - a `.bak` whose original's place is empty is renamed back (the replacement stopped halfway);
     if the compressed video is in place, the `.bak` is deleted;
   - an entry on a drive that is not connected, or whose file is in use or cannot be changed,
     stays in the ledger for the next start;
   - only absolute paths whose names follow Gracil's pattern are touched, and a line cut short
     by a crash is ignored, so a damaged ledger cannot delete anything else.
4. A ledger with nothing left is deleted.

**Consequences.**

- Temporary files from before this change are not in any ledger and are left alone.
- FFmpeg could once outlive a killed Gracil and keep its `.part` in use until a later start.
  Since ADR 0018 it ends with Gracil, so the file is free at the next start.
- A changed-format replacement interrupted after `Clip.mp4` was created but before `Clip.mov` was
  removed leaves both: the new file is a complete, validated video, so nothing is lost.
- If the ledger can't be created, compressing still works; only the cleanup is lost, and it is
  logged.
