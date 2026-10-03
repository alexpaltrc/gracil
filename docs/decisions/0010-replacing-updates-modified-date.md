# 0010 — Replacing keeps the creation date and updates the modified date

**Status:** Accepted. Amends point 5 of [0009](0009-choose-after-compressing.md).

**Context.** ADR 0009 kept both the creation date and the last-modified date of a replaced video,
so it would look untouched. The product owner prefers the modified date to say what happened: the
file changed when Gracil replaced it.

**Decision.** A replaced video keeps the original's creation date, which is when the video
first appeared in that folder. Its modified date is set to the moment it was replaced. The
recording date stored inside the video is unaffected; Gracil keeps it as before (§41). This is
how §42 reads "feels like the same file was compressed": same name, same place, same creation
date, and an honest modified date.

**Consequences.**

- Folders sorted by date modified show freshly compressed videos first. Folders sorted by date
  created, and galleries that read the recording date, keep their order.
- Backup and sync tools see the replaced video as changed, which is true, and copy the smaller
  file.
