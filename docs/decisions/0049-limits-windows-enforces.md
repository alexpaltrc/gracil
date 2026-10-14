# 0049 — Limits Windows enforces

**Status:** Accepted, built (2026-10-05).

*Update (2026-10-14):* by [ADR 0063](0063-safe-on-any-computer.md), memory is capped at about
65 % for Gracil and its tools together rather than three quarters per process; a computer with a
battery never gets more than half of the processor; room on a drive is shared among the files
compressing onto it; and the threads each tool uses follow the share allowed right now.

**Context.** The product owner asked whether Gracil could still harm someone's computer: run it
hot, make its fans roar, or fill its memory or disk, above all on a simple laptop with poor
cooling and no NVIDIA card. Until now, Gracil only lowered its priority (ADR 0040), and the
graphics card's temperature was held (ADR 0046). Lower priority keeps other programs responsive,
but it doesn't lower heat: on a computer without NVENC, x265 (half the threads), the quality
measure of the next video (half the threads) and images (several threads each) together could
keep every processor thread busy. The processor's temperature can't be read (ADR 0043). Nothing
checked the drive's free space, and one huge image always went ahead, whatever its memory.

**Decision.**

1. **A ceiling on the processor.** The job object of ADR 0018, which holds Gracil and every tool
   it starts, is limited to some of the processor's threads: three quarters while the computer is
   idle, half while the person is using it, a quarter while it runs on its battery or Windows is
   saving energy; at least one. Windows enforces it for every process in the job, tools already
   running included, so no split of the work can exceed it. The last threads are chosen: on
   processors with efficient cores those are usually the efficient ones, and the first threads
   are those Windows and other programs lean on most. Rechecked every second.
2. **On battery**, fewer images at once too, as while the person uses the computer.
3. **A ceiling on memory.** Any one process in the job, Gracil included, may hold at most three
   quarters of the computer's memory. A runaway tool is refused more memory and fails quietly,
   instead of pushing the whole computer into the swap file.
4. **An image too large for this computer** (needing more than half of all its memory, by the
   estimate of ADR 0040) is left as it is, with a "!".
5. **Room on the drive.** A file is compressed only if its drive has room for a result as large
   as the original plus 2 GB to spare. Otherwise it is left as it is, with a "!" saying the drive
   is almost full. A drive that doesn't say how much room it has is trusted.
6. **Checking for memory leaks.** Every ten minutes Gracil writes in its log how much memory it
   holds, so long runs can be checked for memory never given back.

**Consequences.** On a laptop without NVIDIA, a quarter to three quarters of the processor stays
free at all times, which keeps the fans and the temperature well below full load. Work on such
computers goes slower, more so on battery. On a computer with a good NVIDIA card little changes,
since the card does most of the work. Nothing ever asks the person anything.
