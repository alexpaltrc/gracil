# 0013 — Pause suspends FFmpeg's threads

**Status:** Accepted.

**Context.** Spec §47 asks for a genuine Pause: the same FFmpeg keeps its work and Resume carries
on, rather than Pause quietly meaning Cancel. FFmpeg has no pause command of its own when driven
as a process.

**Decision.**

1. Pause suspends every thread of the running FFmpeg; Resume resumes them. This is the documented
   Windows way (`CreateToolhelp32Snapshot` + `SuspendThread`/`ResumeThread`), in
   `gracil_windows::set_suspended`. The undocumented `NtSuspendProcess` was not used.
2. Suspending looks at the process's threads again until no new one appears, so a thread started
   halfway through is not left running. If any thread cannot be suspended, the ones already
   suspended are resumed: FFmpeg is never left half-frozen.
3. `gracil-core` gets the suspend function as a parameter (`JobControl::pause`), like the replace
   swap, so it stays free of Windows code. If Gracil is paused between two attempts, the next
   attempt starts paused.
4. Pause is offered only while compressing. Cancel and closing Gracil still work while paused:
   a suspended process can be ended.

**Consequences.**

- While paused, FFmpeg uses no processor or graphics time but keeps its memory, including graphics
  card memory when NVENC is used. Tested: a 4K NVENC compression paused for about four minutes
  resumed and produced a valid, complete video.
- Very long pauses rely on the graphics driver tolerating an idle, suspended client, which it
  does: no work is in flight while the threads are suspended.
