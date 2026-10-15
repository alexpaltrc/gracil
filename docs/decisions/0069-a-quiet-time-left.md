# 0069 — A quiet estimate of the time left

**Status:** Accepted, built (2026-10-14). Decided by the product owner, answering what ADR 0066
left open ("an estimate of the time left, waiting for a product decision"). Not tried by running
Gracil yet (see Consequences).

**Context.** With a long list, the ring and the count of files left say how far along the work is,
but not how long it will take, which is what people want to know before walking away. A count of
files says little: one video can take longer than a thousand photos, and an estimate that jumps
about or creeps up minute by minute is worse than none. Gracil stays calm (spec §2.3): no
dialogs, nothing that moves, nothing alarming.

**Decision.**

1. **Where.** In the footer, after the count of files left: *12 files left, about 20 min · View
   files* (*Quedan 12 archivos, unos 20 min · Ver archivos*). In the tray's tip and the window's
   title while compressing: *Gracil · compressing 42% · about 20 min left* (*Gracil · comprimiendo
   42% · quedan unos 20 min*). Not in the mini-window, which has no room for it.
2. **Coarse and rounded**: about 5, 10, 20, 30 or 45 minutes, then whole hours, each the nearest
   step (7½ minutes and over is 10, 52½ and over is an hour, 90 and over two hours). The words:
   *about 20 min left*, *about 1 h left*, *about 2 h left*; *quedan unos 20 min*, *queda cerca de
   1 h*, *quedan unas 2 h*. After the count: *about 20 min*, *about 1 h*; *unos 20 min*, *cerca de
   1 h*, *unas 2 h*. Numbers are written the way Windows writes them.
3. **Never creeping up.** What is shown may go down freely, but goes up only when the new estimate
   reaches the next step up and is at least a quarter more than what is shown (from *about 20 min*,
   only once the estimate is 30 minutes or more). Otherwise it holds.
4. **When it shows.** Only once at least three files of the round are done, the pace of every kind
   with work left has been measured three times, and more than two minutes are left. Hidden while
   paused, while resting (warm, just woken, very low battery), while a low battery holds the work,
   while folders are still being looked through, and before the pace is known. Once hidden near
   the end, it comes back only with five minutes or more left. A new round of work starts afresh.
5. **Worked out from the work, not the count.** Each file weighs its size in bytes, read with the
   folder's listing as it is looked through (`media::gather_each` now hands each file with its
   size, which on Windows comes with the listing, at no extra cost; a file sent by itself is asked
   about once, for what it is and its size, where it was asked twice before). Videos and images each have their own pace, in
   bytes a second, measured every 10 seconds of work from what the round has done, the files being
   compressed counted partway by their progress, and smoothed (each new measure counts a fifth).
   Time paused, resting or waiting for the battery isn't measured. Since one video compresses while
   the next gets ready and images go beside it (ADR 0040), the kinds are worked on side by side: the
   time left is the longer of the two. Measuring the pace over the whole time a kind is under way
   takes in getting the next video ready alongside, images sharing the computer, and pauses for
   heat.
6. **Cheap.** The round's bytes by kind, all and done, are kept in the summary's counts, file by
   file as they change (ADR 0065); each message then reads only those and the few files being
   compressed. A list of tens of thousands of files costs the same as one of ten. Weights of files
   that left the list are let go now and then, not while Undo can still bring them back.

**Considered and not done:** weighing videos by length × picture size (only known once a video
is got ready, so the files waiting would have no weight); a count-based estimate; showing seconds
or a countdown; showing it in the mini-window.

**Consequences.**

- People see about how long a long list will take, in the footer and on hovering the marble, and
  screen readers hear it in the window's title.
- Bytes are a rough measure of a video's work: a short high-quality clip weighs more than its
  compressing time. The pace is measured on the round itself, so a list of similar files reads
  well and a mixed one settles as it goes; the rounding and the rule against going up keep the
  shown time steady.
- When the videos are done the images speed up, so the estimate tends to run a little long near
  the end of a mixed list, and comes down.
- Not run on screen while it was built: how it reads in the footer in Spanish, how soon it shows
  on real lists, and how steady it stays should be looked at once it runs.
