# 0066 — Quiet states in words, and a lighter start

**Status:** Accepted, built (2026-10-14). Decided by the product owner after a review of what the
window and the tray say in each state, and of what Gracil does while nobody is looking at it. Not
tried by running Gracil yet (see Consequences).

**Context.** The review found states that said nothing or said it only partly, and work done
sooner or more often than it needed:

- The tray's tip and the window's title, which are also what screen readers read (the window
  itself isn't readable by them yet), said only "Gracil" while results waited for a choice, while
  a low battery held the work, while Gracil found out what the computer can do and once the list
  was done. The warning marble's tip didn't say what to do.
- The mini-window showed the last file once the whole list was done, not the done view; a Gracil
  started for File Explorer's work could leave before its warning marble was clicked.
- The finished view counted files that failed but didn't say why; its "!" always went to the
  first of them. Files left unfinished long ago, from an earlier session, took the check mark
  away from a list that went well.
- A Gracil inside its zip, or missing its `ffmpeg` folder, said so only once files failed. Files
  kept only online said "Getting ready" while they downloaded.
- Exit while folders were still being looked through left without asking and forgot them.
- Several words were unclear: "Forget these files", "more waiting", Spanish "Limpiar", the reason
  for a name already taken.
- Starting, Gracil found out what the computer can do before its first window was drawn, made the
  hidden mini-window two seconds in even when it stayed in the tray all day, waited on the taskbar
  before going on, and opened its diary even in a launch that only hands its request to the
  running Gracil, which then waited up to a tenth of a second to hand it over.
- WGPU asked for the most powerful graphics card, which on a laptop with two wakes the large one,
  and Gracil carried WGPU's Vulkan and OpenGL code although it draws only with Direct3D 12
  (ADR 0041).
- The dial drew every frame while only the slow ring moved, and breathed while Gracil rested; the
  dial and the finished view were drawn again whenever the window drew for any reason; a fast list
  of images faded the window in again for each file; a new window waited 50 ms after its first
  frame before coming into view; adding a folder of thousands again removed its finished files
  one by one; the list of unfinished files was written out every 30 s however long it was.

**Decision.**

1. **Words for quiet states** in the tray's tip and the window's title: *Gracil · 2 files waiting
   for you* (*Gracil · 2 archivos te esperan*) while results wait and nothing runs; *Gracil ·
   getting ready* while files wait for the engine; *Gracil · low battery, waiting*; *Gracil ·
   downloading*; *Gracil · done, 130 MB smaller*, or *Gracil · done*; and the warning marble's
   *Gracil · click to see what happened*. The tray's picture is unchanged (a quiet picture for
   results waiting was left for later).
2. **The mini-window's done view**: once the list is done and nothing is picked, it shows how much
   room was saved, how the list went and how many files failed, whose "!" opens the big window on
   the first of them. A Gracil started for background work stays while a problem nobody has seen
   is there, so the warning marble stays until it is clicked.
3. **Why files failed**: under the count, the shared reason when every failure shares it, or
   *Most often: …* (*Lo más frecuente: …*). Worked out only once the list is done. Pressing the
   "!" again shows the next failed file, round to the first.
4. **Said from the start**: inside its zip, or without its `ffmpeg` folder, the empty window and
   the mini-window say so under where to drop. The missing-FFmpeg words no longer name a video
   tool, since images can't start without it either: *Gracil is missing part of itself. Download
   Gracil again from where you got it*.
5. **Downloading**: a file kept only online says *Downloading* while it is got ready, through a
   new report from the work (`Report::Downloading`), sent before getting ready starts; a `false`
   report can end it sooner once the download is done.
6. **Exit while looking**: folders still being looked through count as something to lose, Exit
   asks, says *Folders still being looked through are kept for next time*, and they join the list
   of unfinished files, like whatever was being looked through when Gracil closed suddenly.
7. **The check mark** is taken away only by files Stop set aside in this round of work, not by
   files left from an earlier time.
8. **Words**: *Don't continue with these files. They stay as they are*; *{n} more to compress*;
   *Looking for videos and images…*; Spanish *Vaciar*, *Vaciar la lista* and *No queda ningún
   archivo*; the footer leaves out *No files left* when it has more to say; plainer reasons for a
   file that couldn't be compressed and for a name already taken; Exit's words for unchosen
   results name **Replace** and **Save copy**. File names show their whole path on hover.
9. **A lighter start.** The engine starts once the first window is in view, at once when a
   request brings files, or a minute after start in a Gracil resting in the tray. A Gracil started
   with Windows makes its mini-window only when the pointer first comes to the tray icon or files
   come. The tray icon is added from its own thread's loop, so starting never waits on the
   taskbar; until it is added, the mini-window is placed by the taskbar alone. A later launch
   hands its request over before opening the diary, which only the running Gracil writes, and
   tries every 20 ms instead of 100 ms.
10. **Drawing less** (spec §51): WGPU is asked for no particular card (`WGPU_POWER_PREF=none`,
    unless set), so Windows gives Gracil the card any ordinary window gets. Only WGPU's Direct3D
    12 code is built in: Iced's `wgpu-bare` and WGPU 27.0.1 with `dx12`, `wgsl`, `std` and
    `parking_lot`; about two dozen libraries leave the build, among them Vulkan's (`ash`) and OpenGL's
    (`glow`), and `opengl32.dll` is no longer named in `gracil.exe`. This narrows ADR 0041,
    item 1: `WGPU_BACKEND` can no longer pick Vulkan or OpenGL. The processor renderer (ADR 0063)
    is unchanged.
11. **Frames.** While only the ring filling or the marble breathing moves, the dial asks for a
    frame every 33 ms rather than every frame; what answers the pointer still moves every frame.
    The marble doesn't breathe while Gracil rests (warm, just woken, very low battery). A settled
    dial and the finished view after its entrance are drawn once and kept, by size, colors and
    what they show. While the ring stands for a round of files, the files taking turns in it no
    longer fade the view in again. A new window comes into view once Windows has put its first
    frame on screen (`DwmFlush`, twice for safety) instead of 50 ms later; where that can't be
    waited for, the 50 ms stay, and the third frame or three seconds still bring it in at the
    latest.
12. **The list**: adding files already finished on the list takes them off in one pass, as Clear
    does. The list of unfinished files is written out every 30 s up to a thousand files, 30 s more
    for each further thousand, up to every 3 minutes; a sudden close loses at most that much, and
    those files are simply compressed again.

**Considered and not done:** an estimate of the time left, waiting for a product decision.
*Done in [ADR 0069](0069-a-quiet-time-left.md) (2026-10-14).*

**Consequences.**

- Screen readers hear how the work goes in every state, through the window's title and the
  tray's tip; the window itself still isn't readable by them.
- The first window no longer shares its first moment with FFmpeg's listings and the graphics
  cards' test encodes; the first file waits for them instead, a little under half a second on the
  reference computer, as it did when it came during start.
- A Gracil resting in the tray all day makes no window and runs no FFmpeg in its first minute.
- None of this was run on screen while it was built (another Gracil was in daily use on the same
  computer): the words, the mini-window's fit in Spanish, the reveal without a white flash and
  the steadier frame rate should be looked at once it runs. `Report::Downloading(false)` is not
  sent yet: the download's end is known to the chunked read of online-only files being built
  alongside, which should send it. *Done when the two were merged (2026-10-14): getting a file
  ready sends it once the download is over.*
