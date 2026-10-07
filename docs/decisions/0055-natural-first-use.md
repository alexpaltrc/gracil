# 0055 — Natural from the first use: a review of every screen before the first release

**Status:** Accepted, built (2026-10-05). Asked for by the product owner. Amends
[0025](0025-always-replace-or-copy.md), [0039](0039-a-clear-done.md) and spec §40.1, §49, §54.1
and §55.

**Changed** by [ADR 0058](0058-explorer-send-to-and-main-menu.md) (2026-10-07): in the tray menu
(item 12), **Show Gracil in Send to** and **Show Gracil in File Explorer's menu…** take the place
of **Show in File Explorer's menu**.

**Extended** by [ADR 0059](0059-keyboard-reaches-everything.md) (2026-10-07): beyond the few keys
of item 10, Tab reaches every control and Enter or Space presses it.

**Context.** Before the first public release the product owner asked for a review of the whole
app: someone opening Gracil for the first time should know what to do without thinking, nothing
should be out of place or extra, and nothing should look stuck or say something untrue. Walking
through every screen found a few places that fell short:

- a folder dropped on the window was ignored, though File Explorer's menu takes folders;
- the empty window showed the **Always** pill before there was anything to apply it to;
- the done screen showed a green check even when every file had failed;
- the first moments of a list, while Gracil gets ready, showed a ring that didn't move;
- Exit didn't count finished results nobody had chosen for yet, which are let go on leaving;
- "Not smaller" and "Already compressed" said two things about one outcome: the original stays;
- when FFmpeg was missing, files said "Waiting", as if something would happen;
- the mini-window's dial could stop the whole list from a window too small to explain it;
- a problem that turned the tray icon red was gone from view by the time the mini-window opened.

**Decision.**

1. **Folders can be dropped.** A dropped folder brings in the videos and images inside it, the
   same as from File Explorer's menu.
2. **The empty window shows only where to drop.** **Always** and the list's line come up with the
   first file. Hovering the word **Always** says what it does; hovering the **×** of the line about
   unfinished files says it forgets them. *Changed by [0057](0057-always-in-sight.md):* **Always**
   stays in sight, the empty window included.
3. **An honest done screen.** The green middle and check mark appear only when something became
   smaller. Otherwise the big line says *Nothing to make smaller* or *Nothing was compressed*.
   Files that failed are counted apart, as a quiet link with a "!", which opens the list on the
   first of them, where its own "!" explains what happened. Once done, the line under the ring
   only opens the list; the done screen already says how it went.
4. **Getting ready breathes.** Until the ring starts filling, the middle of the marble slowly
   fades and returns, so a long first moment never looks stuck. It stops as soon as there is
   progress, or while paused.
5. **Exit counts every loss.** Exit asks first, too, when finished results wait for a choice, and
   says they are let go and will be compressed again next time (ADR 0042 already carries them
   over).
6. **One outcome, one name.** A file that couldn't be made smaller and a file that was already
   about as small as it gets are both **Already compressed**, with the "!" saying which. The
   greyed-out Replace and Save copy under such a file are gone: there is nothing to choose.
7. **Plain words for a missing FFmpeg.** Files say **Can't start**, with a "!" saying Gracil is
   missing part of itself and can be downloaded again from GitHub.
8. **The mini-window only pauses.** Its dial has no stop square; its whole middle pauses and
   resumes. Stopping stays in the main window, where it is clear what stops.
9. **A problem stays in sight.** When the tray icon turned red while no window was open, the
   mini-window opens on the file that went wrong.
10. **A few keys.** **Esc** steps back (from the Exit question, the mini-window, a picked file or
    the open list), **Space** pauses and resumes, **Ctrl+O** chooses files. Nothing needs them.
11. **Percentages round down,** so 100% only shows once a file is truly done.
12. **The tray menu is grouped:** Open; then Start with Windows, Show in File Explorer's menu and
    the mini-window setting; then Prepare to delete Gracil and Exit.
13. The pill's choices are a little taller, easier to hit.

**Consequences.**

- No new settings and no new dependencies. The breathing marble asks for frames only while it
  breathes (spec §51).
- The drawer still opened without an animation: animating its height seemed to mean measuring
  its rows ahead of time. **Built** by [0056](0056-the-list-unfolds.md), which needed no such
  measuring.
