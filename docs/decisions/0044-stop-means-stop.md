# 0044 — Stop means stop, and Exit doesn't wait

**Status:** Accepted, built (2026-10-04).

**Context.** ADR 0040 made Stop cancel every file being worked on while the files still waiting
carried on. Working through a long list on a cloud drive (Filen, mounted as a network drive), the
product owner pressed Stop and Gracil seemed not to answer: the next files started straight away,
and the stopped ones stayed on screen until their work, stuck reading the slow drive, noticed it
had been cancelled. Exit was slow for the same reason: Gracil waited for every file's work to end,
then deleted unchosen results on the slow drive one by one before quitting.

**Decision.**

1. Stop stops the whole list, at once. Every file not finished leaves the list and joins the
   files offered with **Continue** (ADR 0042), with the choice File Explorer made for it, so
   nothing is lost and carrying on is one click. This reverses item 6 of ADR 0040 for Stop.
2. Files being worked on are told to stop and leave the list immediately; whatever their work
   still reports afterwards is let go, and a result it made meanwhile is deleted away from the
   window.
3. Folders still being looked through when Stop is pressed carry on, but what they find goes to
   the files offered with **Continue**, not to the list.
4. A result being put in place (replacing an original) always finishes; it is never cut halfway.
5. Exit waits only for a result being put in place. Work stuck on a slow drive ends with Gracil.
   Deleting unchosen results and other tidying gets at most three seconds; whatever is left is
   removed at the next start (ADR 0017).

**Consequences.** Stop and Exit answer at once, whatever the drive. Stopping a long list no
longer means losing it: the line with **Continue** brings it back. A file stopped halfway starts
again from its beginning.
