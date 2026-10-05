# 0043 — Never too hot

**Status:** Accepted, built (2026-10-04).

**Context.** Lists of 15,000 files keep the computer busy for many hours, often overnight. On a
computer with poor cooling, hours of work at a steady load can run the graphics card or the
processor hot. The product owner asked for a limit: when the graphics card or the processor
Gracil uses gets too hot, Gracil itself must do less. This adds to "fast, but never alarming"
(ADR 0040), which already keeps Gracil below other programs and away from the whole processor.

What can be read without a driver or administrator rights:

- NVIDIA graphics cards: `nvml.dll`, installed with NVIDIA's driver, gives the temperature, the
  temperature at which the card starts slowing itself down, and whether it is slowing down
  because of heat right now. NVENC, which Gracil prefers for video, is NVIDIA's, so this covers
  most of the video work.
- The processor's temperature can't be read from an ordinary program: tools that show it install
  a kernel driver, which Gracil must never do (it needs full control of the computer, the
  Microsoft Store would not allow it, and several such drivers are flagged as vulnerable by
  Windows). ACPI thermal zones exist on
  some laptops, but on most desktops they are missing or report a fixed value.
- Windows' "% Performance Limit" counter says how much of its full speed the processor is allowed
  right now. It drops when the processor is held back, for heat among other reasons. Some
  computers keep it below 100 all the time on purpose, so only a drop from its usual value means
  anything.
- AMD and Intel graphics cards have no simple equivalent of NVML; reading them would mean their
  full SDKs.

**Decision.**

1. A thread reads the computer every 5 seconds: every NVIDIA card through NVML, loaded from
   System32 only, if present; and the processor's "% Performance Limit", by its English name so it
   works in any Windows language. Whatever can't be read is left out. Changes are logged.
2. A card is too hot at 80 °C, or at 8 °C below its own slowdown temperature if that is lower, or
   whenever it says it is slowing down because of heat. It is warm from 5 °C below that.
3. The processor is too hot when its limit is at least 10 points below the highest value seen,
   for two readings in a row.
4. Warm: images at once drop to what they are while the person is using the computer.
5. Too hot: everything being worked on is frozen, exactly like Pause, and nothing new starts.
   The status line says **Letting the computer cool down**. When every card is at least 10 °C
   below its "too hot" mark, the processor is no longer held back, and at least 30 seconds have
   passed, work carries on by itself. The person's own Pause is separate: resuming never
   overrides cooling down, and cooling down ending never undoes a Pause.
6. No setting. The limits are fixed, well below where cards protect themselves.
7. Not done now: Windows' efficiency mode (EcoQoS) for Gracil's helper programs, which lowers the
   processor's speed and heat further at some cost in speed. It can be added if the processor
   still runs hot.

**Consequences.** On a hot computer a long list takes longer, with pauses, instead of running hot
for hours. With an NVIDIA card, the main source of heat during video work is watched directly.
Without one, Gracil relies on Windows holding the processor back and on never using the whole
processor. Nothing is installed and no administrator rights are needed.

*Amended by ADR 0045:* Gracil no longer stops to cool down; it takes short unseen rests that
grow with the heat instead.
