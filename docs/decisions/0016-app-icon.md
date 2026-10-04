# 0016 — The app icon lives in gracil.exe

**Status:** Accepted. Carries out the icon part of [0015](0015-piedra-identity.md). The icon files
and their colors are now those of [0031](0031-windows-11-colors.md), which links two icons.

**Context.** File Explorer, the taskbar and the window's title bar each need Gracil's icon, at
sizes that depend on the screen's scale. Iced can set a window icon only from a single bitmap,
which Windows then shrinks or stretches, so the 16 px title-bar icon would lose the version drawn
for that size.

**Decision.**

1. `crates/gracil-ui/gracil.rc` links `assets/icons/ico/piedra-app.ico` into gracil.exe as icon 1,
   compiled by `build.rs` with `embed-resource`. Explorer shows it with nothing else to do.
2. When the window opens, and again whenever its scale changes, `gracil_windows::set_window_icon`
   loads that same icon at the small and large sizes the window's DPI calls for and gives both to
   the window. Windows picks the best-drawn picture in the file for each size.
3. The neutral marble icon ("la canica") is used, not the light or dark tray versions, because
   it reads on both light and dark title bars and taskbars.
4. Icons are loaded with `LoadImageW` at an explicit size. `LR_SHARED` is not used because the
   shared copy ignores the size asked for; `LoadIconMetric` is not used because it needs version 6
   of the common controls, which Gracil does not otherwise load. The previous pair is released
   when replaced.

**Consequences.**

- Changing the icon means replacing the ICO file; the build picks it up.
- The tray icon is drawn by Gracil from the tray artwork's shapes and follows the taskbar's
  theme ([0020](0020-tray.md)). The MSIX manifest still needs wiring.
