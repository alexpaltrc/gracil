# 0080 — The documentation always says what is true today

**Status:** Accepted, built (2026-11-17). Decided by the product owner, who found the
documentation still calling things planned that had been built or decided long before.

**Context.** A review of every document found the specification still listing image compression
as "planned, not started" six weeks after it shipped, its image section headed "Open decisions,
to settle before any work starts" with every one long settled, about fifty decision records whose
status didn't say they were built or later changed, and smaller drift in the guides: a setting
said to live in memory that is kept in a file, a version number that ages with every release.
Each document was right on the day it was written; nothing made it right afterwards. The rule
that did exist covered only `how-gracil-works.md`.

**Decision.**

1. **Every document is updated in the same commit as the change it describes**, not only
   `how-gracil-works.md`. When something is built, changed or dropped, the commit searches
   `docs/`, `README.md` and `CLAUDE.md` for it and updates every place that mentions it: the
   specification, the guides, the architecture, the decision record's **Status** line and its
   row in the index, and the status line of any earlier record it changes ("changed by ADR
   NNNN").
2. **Documents describe the present.** Only the decision records keep history. The
   specification and the guides say how Gracil is now, with the record that decided it; words
   such as *planned*, *proposed*, *for now*, *later* or *(in progress)* appear only for what is
   truly not built, and then name the record that says so.
3. **Nothing written ages by itself.** No "the current version is", no list of releases (the
   changelog keeps them), no "not tried yet" once it has been. A version in an example stays an
   example.
4. **Before every release, a pass over the documentation.** The words of rule 2 and *not built*,
   *not yet* and *untried* are searched for across the documents, and each one is confirmed
   still true or fixed, before the tag is pushed (`docs/development/versioning.md`).

**Consequences.** A commit that changes behavior takes a little longer. The documentation can be
trusted as it stands, by the owner, by a new contributor and by a session in the cloud that sees
only the repository.
