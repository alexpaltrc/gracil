# 0015 — The Piedra identity: icon, palette and following Windows' theme

**Status:** Accepted. Replaces the temporary icon and colors of spec §3 and §78.

**Context.** The product owner commissioned the visual identity "La canica · Piedra": a marble
(the video) resting on a support (Gracil, which prepares the way). Its philosophy is "letting go is
enough": the power stays in the system, the calm in the experience. It comes with a palette, the
icon in every Windows size, tray versions for light and dark taskbars, and MSIX assets.

**Decision.**

1. The palette is Piedra's, in `style::palette`:

   | Role | Light | Dark |
   |---|---|---|
   | Background | `#F3F1EB` (ivory) | `#252522` |
   | Text, the support | `#4D4B46` | `#DAD8CF` |
   | Marble | `#8C887E` | `#8C887E` |

   Text and hover states use the support color; the marble color is kept for the marble itself
   and the drop area. The warm "!" and the success and danger colors stay as they were, since they
   carry meaning. The previous violet palette is kept in
   [`../design/previous-palette.md`](../design/previous-palette.md).
2. The dial borrows the logo: its trail is the support color and the stone-grey marble rests on
   it, edged in the background color so it stays readable in both themes.
3. Gracil always follows Windows' light or dark theme. The first answer is applied at once, so the
   window opens in the right colors; later changes fade between the two palettes over 600 ms,
   slow at both ends. Frames are requested only during the fade (spec §51).
4. The identity's files live in the repository: icons (ICO, PNG, SVG) in `assets/icons/`, MSIX
   assets in `packaging/msix/Assets/`, the presentation and its notes in `docs/design/piedra/`.

**Consequences.**

- Using the icon (window, executable, tray, MSIX manifest) is still to be done; the tray must swap
  its icon when the taskbar's theme changes (see `docs/design/piedra/LEEME.md`).
- The marble grey on ivory is low contrast (3.2:1), so it is used for shapes only, never for text.
