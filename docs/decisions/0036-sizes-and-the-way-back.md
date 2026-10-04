# 0036 — Sizes before and after, and the way back to the work

**Status:** Accepted, built (2026-10-04). Asked for by the product owner (spec §77.3). Adds spec
§32.1 and §32.2.

**Context.** After a long list there was no way to know how much room Gracil had freed: a result
said "35% smaller" until it was placed, then only "Replaced" or "Copy saved", and nothing added
the list up. Separately, clicking a file in the drawer (for example one that showed **Already
compressed**) kept it in the focus for good. The ring of the file being compressed was gone, and
the only way back was to take the picked file off the list.

**Decision.**

1. **Each file keeps its sizes.** A result remembers the original's size and its own, before and
   after it is placed. The focus shows them under the choices: *120 MB → 78 MB*. The drawer row of
   a placed file keeps the percentage beside where it went: *−35% · Replaced*.
2. **The list adds up.** With two or more results on the list, the drawer starts with *In all:
   3.4 GB → 2.2 GB, 35% smaller*, and the footer gains *1.2 GB smaller*. Results waiting for a
   choice count too; files that failed or were stopped don't. Only what is on the list counts, so
   taking files off it or **Clear** starts the count again. Nothing is stored between sessions.
3. **Sizes read like File Explorer:** KB, MB, GB counted in 1024s, one decimal under 10, the
   language's decimal mark (*1.2 GB*, *1,2 GB*).
4. **Picking is a look, not a trap.** The picked row stays lightly marked. Clicking it again,
   closing **View files**, or clicking the quiet *‹ name · 42%* line under the picked file goes
   back to the work. That line shows only while a file is being compressed or a result waits for
   a choice, so it never points at nothing.

**Rejected.** Dropping the pick by itself when the next file starts: the user would lose what they
were reading without asking. A "Back" button with no destination: the line names the file and
its progress instead, so it also works as a glance at the work. A running total kept across
sessions: it would mean keeping a record on disk of what was compressed, for little use.

**Consequences.** `Outcome::Ready` and `Outcome::Placed` carry `Sizes` instead of a ratio.
`Gracil::focused` is the pick, else `Gracil::current`; `Gracil::way_back` decides the line.
