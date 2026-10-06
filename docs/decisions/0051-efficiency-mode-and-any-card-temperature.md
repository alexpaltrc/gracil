# 0051 — Efficiency mode for the tools, and the temperature of any graphics card

**Status:** Accepted, built (2026-10-05).

**Context.** ADR 0049 holds Gracil to part of the processor, and ADR 0046 holds an NVIDIA card at
about 70 °C. Two gaps remained. First, the threads Gracil may use still ran at full clock: on a
laptop, a few threads at top speed heat the processor and spin the fans more than many threads at
an efficient speed. Windows 11 has an "efficiency mode" (EcoQoS) for exactly this, the one Task
Manager shows with a green leaf: the process is scheduled on efficient cores where there are
some, and runs at the processor's most efficient speed. Second, the temperature of Intel and AMD
graphics cards wasn't read at all (ADR 0043), so on those computers the card had no guard.

**Decision.**

1. **Efficiency mode for the tools.** Every tool Gracil starts (FFmpeg, FFprobe, cjpegli) runs in
   Windows' efficiency mode while the person is using the computer, and always while it runs on
   its battery or Windows is saving energy. When the computer is idle and plugged in, Gracil
   leaves the choice to Windows, so the work goes at full speed while nobody is there. Gracil's
   own process is never put in efficiency mode, so its window always answers at once.
2. **Given every second.** The tools in Gracil's job object (ADR 0018) are listed every second,
   on the same breath as the processor share of ADR 0049, and each gets the current mode. Tools
   start all the time, so giving it once isn't enough. A tool that ends between listing and
   setting is simply skipped.
3. **The temperature of any graphics card.** When NVIDIA's own library isn't there or finds no
   card, Gracil asks Windows for each graphics card's temperature, as reported by its driver
   (the same report Task Manager shows). A driver that doesn't report it (most integrated
   graphics) is skipped. Those cards are held at 70 °C the same way as in ADR 0046, but without
   the early-slowdown point or the "slowing down because of heat" signal, which only NVIDIA
   reports.

**Consequences.** On laptops the processor runs cooler and quieter while the person works, at the
cost of slower work then; nothing changes when the computer is left alone and plugged in.
Computers with an Intel or AMD card that reports its temperature now have the same guard as
NVIDIA ones. No extra software, no administrator rights, nothing asked.
