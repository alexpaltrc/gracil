# 0015 — The marble identity: icon, Piedra palette and following Windows' theme

**Status:** Accepted, built (2026-10-03). Replaces the temporary icon and colors of spec §3 and
§78. Its palette (items 1 and 2) is superseded by [0031](0031-windows-11-colors.md); item 3 still
holds.

**Context.** The product owner commissioned the visual identity "La canica · Piedra": a marble
(the video) resting on a support (Gracil, which prepares the way). Its philosophy is "letting go is
enough": the power stays in the system, the calm in the experience. It comes with a palette, the
icon in every Windows size, and tray versions for light and dark taskbars.

Naming: the icon is the marble ("la canica"); "Piedra" is the name of the palette only.

**Decision.**

1. The palette is Piedra's, in `style::palette`:

   | Role | Light | Dark |
   |---|---|---|
   | Background | `#F3F1EB` (ivory) | `#252522` |
   | Text, the support | `#4D4B46` | `#DAD8CF` |
   | Marble | `#8C887E` | `#8C887E` |

   Text and hover states use the support color; the marble color is kept for the marble itself
   and the drop area.

   *Update (2026-10-03):* the product owner checked every role against WCAG (text at least
   4.5:1, controls at least 3:1) and set these, in `style::tokens`:

   | Role | Light | Dark | On the marble `#8C887E` |
   |---|---|---|---|
   | Text | `#4D4B46` (5.01:1) | `#DAD8CF` (5.02:1) | `#151413` (4.66:1) |
   | Secondary text, quiet actions | `#676561` | `#AAA8A1` | `#201F1D` |
   | Accent: filled buttons, the dial's trail | `#68655E` | `#ABA8A0` | `#191816` |
   | Border: edges of controls and areas | `#88847B` | `#8C887E` | `#3C3A36` |

   Secondary text is a color of its own rather than the text made transparent. Filled buttons use
   the accent with the background color as their text. A picked end of the "Always" pill is
   marble grey with the "on the marble" text. The warm "!" and the success and danger colors stay as they were, since they
   carry meaning.
2. The dial borrows the logo: its trail is the support color and the stone-grey marble rests on
   it, edged in the background color so it stays readable in both themes.
3. Gracil always follows Windows' light or dark theme. At start it reads the apps' theme from
   Windows' settings (`gracil_windows::light_apps`) and applies it at once, so the first window
   opens in the right colors: Iced only learns the theme once a window exists, and a mini-window
   opened from File Explorer used to appear light and then fade to dark. Later changes fade
   between the two palettes over 600 ms, slow at both ends. Frames are
   requested only during the fade (spec §51).
4. The icon files live in `assets/icons/`.

**Consequences.**

- The window and executable use the icon ([0016](0016-app-icon.md)). The tray icon follows
  the taskbar's theme ([0020](0020-tray.md)).
- The marble grey on ivory is low contrast (3.2:1), so it is used for shapes only, never for text.
