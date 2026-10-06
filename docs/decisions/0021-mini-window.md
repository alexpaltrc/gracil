# 0021 — The mini-window opens from the tray icon, and its one setting lives in the tray menu

**Status:** Accepted. Changes point 2 of [0020](0020-tray.md): a click on the tray icon now
shows the mini-window instead of the main window.

**Context.** Spec §55 asks for a small window tied to the tray: the current video, its progress,
pause, and room to drop more videos. It stays open when the user clicks elsewhere, and goes where
the taskbar is, on whichever screen and edge. Spec §56 opens it when compressing from File
Explorer, behind a setting that is on by default (§66). Gracil has no settings screen, and one
setting does not justify one.

**Decision.**

1. A click or Enter on the tray icon shows the mini-window, or hides it if it is open. **Open
   Gracil** stays the menu's default item, and the mini-window has its own **Open Gracil**.
2. The mini-window is an undecorated, always-on-top window with rounded corners and a shadow, kept
   off the taskbar. It shows the video being compressed (or the one in focus) with a smaller
   progress dial that pauses like the big one, its status, how many are waiting, and where a
   finished result went. Videos dropped on it are added like any others. It closes only with its
   "×" or a second click on the icon.
3. It opens hidden and is placed before it is shown. `gracil_windows::place_near_tray` asks
   Windows where the icon is (`Shell_NotifyIconGetRect`), finds that screen's work area, and puts
   the window against the edge the taskbar takes, centred on the icon and always inside the work
   area. It is placed once more after it appears, in case that screen's scale changed its size.
   Without an icon to go by, it takes the main screen's clock corner.
4. The setting **Show mini-window when compressing from File Explorer** is a checkable item in the
   tray menu, saved as one line in `<data folder>\settings.txt`. A missing or damaged
   file means the default (on).
5. When File Explorer sends videos with the choice already made, the mini-window opens if the
   setting is on and Gracil's window is not already open. A Gracil started only for that work
   waits for the mini-window to be closed before leaving (§53), so it never vanishes under the
   user's eyes.
6. With the mini-window open, problems are in view, so the tray's amber "!" is not raised and
   opening it clears one already raised.

**Consequences.**

- Reaching the main window from the tray takes the menu or the mini-window's link rather than a
  plain click.
- A later setting adds a line to `settings.txt` and, if it fits, an item to the tray menu; a
  settings screen is only worth it when the menu stops being enough.
