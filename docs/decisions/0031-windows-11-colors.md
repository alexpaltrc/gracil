# 0031 — Windows 11's own colors and the user's accent

**Status:** Accepted. Requested by the product owner (2026-10-03). Replaces the Piedra palette of
[0015](0015-piedra-identity.md) (items 1 and 2) and the Piedra icon tiles that followed it;
0015's item 3, following Windows' light and dark theme, still holds.

**Context.** Gracil should look as native as possible on Windows 11. The Piedra palette (ivory,
mineral greys, a stone-grey marble) was Gracil's own, so it never quite matched the windows
around it, and it ignored the accent color the user picked in Windows.

**Decision.**

1. The palette is Windows 11's own (Fluent), in `style::palette` and `style::tokens`:

   | Role | Light | Dark |
   |---|---|---|
   | Background (Mica's fallback) | `#F3F3F3` | `#202020` |
   | Text | `#1A1A1A` | `#FFFFFF` |
   | Secondary text, quiet actions | `#5C5C5C` | `#CCCCCC` |
   | Border: edges of controls and areas | `#868686` | `#9A9A9A` |
   | Accent: filled buttons, the dial's trail, a picked end of a pill | the user's accent, dark shade 1 | the user's accent, light shade 2 |
   | Text on the accent | `#FFFFFF` | `#000000` |
   | A slider's thumb (the dial's marble) | `#FFFFFF` | `#454545` |
   | Caution (the "!") | `#9D5D00` | `#FCE100` |
   | Success | `#0F7B0F` | `#6CCB5F` |
   | Critical | `#C42B1C` | `#FF99A4` |

   These are Windows' own values for the same roles, so text keeps at least 4.5:1 and controls
   at least 3:1 on their background. The caution color also fixes Piedra's amber, which was too
   pale on a light background.
2. The accent is the user's, read from Windows (`gracil_windows::accent`, the `AccentPalette`
   that Windows keeps in the registry). Light mode fills with its first darker shade, dark mode
   with its second lighter shade, as Windows' own accent buttons do. If it can't be read, Windows'
   default blue is used. When the user changes it, Windows tells every window
   (`WM_SETTINGCHANGE` with `ImmersiveColorSet`); the tray's hidden window passes that on as
   `TrayEvent::Colors`, and Gracil reads the accent again and fades to it over the same 600 ms as
   a change between light and dark. Nothing polls.
3. The dial's marble is drawn like the thumb of Windows' sliders: a solid face with a thin edge
   and the accent in its middle, riding on a trail in the accent.
4. Accent buttons turn a little see-through under the pointer and when pressed, like Windows'.
5. The icon is recolored in the same neutrals and is always the opposite of Windows, so it stands
   out (replacing Piedra's tiles):
   - light taskbar or theme: a dark tile (`#202020`) with a white support;
   - dark taskbar or theme: a light tile (`#F3F3F3`) with a `#1A1A1A` support;
   - the marble is a neutral grey (`#8A8A8A`) on both, since an icon file can't follow the
     accent.
   `scripts/icons.ps1` draws the two icon files, `gracil-app-dark.ico` and `gracil-app-light.ico`.
   Both are linked into gracil.exe; File Explorer shows the dark one, and each window gets the one
   for Windows' current theme and changes with it (`gracil_windows::set_icon_theme`). Next to the
   clock the icon stays a single color like Windows' own tray icons: `#1A1A1A` on a light taskbar,
   white on a dark one, and its warning uses the caution color above.

**Consequences.**

- Gracil follows the user's colors with nothing to set, and changes with them while open.
- Piedra's colored icon files are no longer in `assets/icons/`, whose SVGs now hold the marble's
  shapes in the new colors. *Update (2026-10-07):* nothing read those SVGs, so they were removed;
  the shapes live in the `.ico` files and in the code that draws the tray icon.
