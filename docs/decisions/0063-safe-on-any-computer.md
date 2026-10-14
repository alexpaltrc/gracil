# 0063 — Safe on any computer: laptops, sleep, memory and quiet when idle

**Status:** Accepted, built (2026-10-14).

**Context.** ADRs 0040, 0043, 0045, 0046, 0049 and 0051 keep Gracil cool, quiet and out of the
way. The product owner asked for a review of what could still strain someone's computer, above all
a simple laptop, and approved twelve changes. The gaps found:

- "Idle" meant only that the keyboard and mouse were untouched for a minute. Someone watching a
  film, playing a game, presenting or on a call doesn't touch them either, and Gracil then took
  three quarters of the processor and left efficiency mode.
- On a laptop, idle and plugged in, Gracil still took three quarters of the processor, more than a
  laptop's cooling is built to hold for hours. A battery running low was not looked at at all.
- When nothing could be read (no card temperature, no processor limit), Gracil assumed all was
  well. Some laptops hold the processor a little below full speed all the time; taking the
  highest value seen as usual handled that, but only once a full-speed reading had been seen.
- The computer was read every second, even with nothing to do, and NVIDIA's library was loaded
  even when no NVIDIA card was in use. The watch thread never slept.
- The temperature watched was that of every card at once, not the card doing the work.
- One process could hold three quarters of memory, but two together could take more. Two videos
  at once (one compressing, the next getting ready) could fill an 8 GB computer, and above 4K the
  quality measure alone could take several gigabytes.
- A very large image could still fail to fit only once it was being decoded, with Gracil's own
  memory exhausted.
- Sleep in the middle of a video often made FFmpeg fail when the computer woke up, and the video
  was marked as not compressed.
- Two files compressing at once onto the same nearly full drive each saw the same free room.
- Without a working Direct3D 12 or Vulkan driver (an old card, a remote session), the window
  couldn't be drawn at all.

**Decision.**

1. **Idle means nobody needs the computer.** On top of a minute without the keyboard or mouse,
   the computer isn't idle while Windows says a full-screen program, a presentation or "do not
   disturb" is on, or while any program keeps the screen on (a film, a call). A computer whose
   warmth can't be read at all is never treated as idle: Gracil couldn't tell if it got hot.
2. **Laptops get less.** On a computer with a battery, Gracil never takes more than half of the
   processor, idle or not, and its tools always run in efficiency mode (ADR 0051).
3. **The battery.** On battery below 30 %, the files being worked on finish and nothing new
   starts. Below 15 %, everything waits. Once low, it stays low until the laptop is plugged in, so
   a battery that recovers a point doesn't start and stop the work. Every file waiting says
   **Low battery · waiting**. Plugging in carries on by itself.
4. **The processor's usual speed is its full speed**, until it has held steady at a lower limit
   for a minute: then that is its usual, as some laptops keep it there on purpose. A higher
   reading always raises it.
5. **Read only as much as needed.** Every second only while files are being worked on; only the
   battery, every five seconds, while files wait to start; nothing at all while there is nothing
   to do, except Gracil's own memory every ten minutes. Efficiency mode is given to the tools
   when it changes and to each tool as it starts, instead of to all of them every second.
6. **The right card.** NVIDIA's library is loaded only when a video compresses on an NVIDIA card,
   and only that card is watched. For Intel and AMD encoders, only cards of that maker are read,
   from their driver. A video compressed on the processor (x265) relies on the processor's limit
   and on Gracil's own share of it.
7. **Memory.** Gracil and its tools together may hold at most about 65 % of the computer's
   memory, enforced by Windows over the whole job, not per process. The next video gets ready
   beside one compressing only with at least 4 GB free and never on a computer with 8 GB of
   memory or less. Tools and image threads ask Windows to give their memory up first when it runs
   short. Large image buffers are reserved before decoding; an image that doesn't fit is left as
   it is with a "!", as in ADR 0049.
8. **Images in the background.** Threads working on images run in Windows' background mode,
   which lowers their processor, disk and memory priority below everything else.
9. **Above 4K, measured at 4K.** The quality measure (VMAF) compares both pictures scaled down to
   fit 3840×2160, with the same model. The model judges what a person sees at normal viewing
   distance, which more pixels than 4K don't change, and the measure takes a quarter of the memory
   and time. The video itself keeps its resolution (ADR 0033). Above 4K, x265 encodes with one
   frame at a time in flight when memory is short, which keeps its memory to a fraction.
10. **Each tool's threads follow the share.** The threads a tool is told to use follow what
    Gracil is allowed right now (ADR 0049), not half of the whole processor; the processor's
    threads are counted from what Windows lets Gracil run on.
11. **Sleep.** When the computer goes to sleep, everything being worked on rests, as in a breath
    (ADR 0045), and for ten seconds after it wakes. A step that failed because the computer slept
    is tried once more, from the start of that step, before anything else is tried.
12. **Room on the drive counts the others.** Room on a drive is kept for each file compressing
    onto it until its result is written, and the next file only sees what is left (ADR 0049).
13. **A window on any computer.** If the graphics card can't draw the window, it is drawn by the
    processor instead. This adds about half a megabyte to Gracil.

**Consequences.** Gracil now does less while a film, a game or a call is on, less on any laptop,
and nothing on a battery running out. With nothing to do it no longer wakes every second. Videos
above 4K are measured with far less memory, and their quality is still judged on what a person
sees. A video interrupted by sleep is no longer marked as failed. On a computer of 8 GB or less,
videos no longer get ready while another compresses, so a list of videos takes a little longer
there. Nothing is asked of the person; the battery is the one reason ever shown, in the file's
own status line.

This changes in spirit ADRs 0040 (idle and the next video getting ready), 0043 (the processor's
usual limit), 0045 (resting while asleep or on a battery very low), 0049 (memory over the whole
job, room on the drive shared, laptops capped at half) and 0051 (efficiency mode on any laptop,
given as it changes).
