# 0061 — Windows' animation and contrast settings

**Status:** Accepted, built (2026-10-07). From the 1.0 review. Extends
[0031](0031-windows-11-colors.md).

**Context.** Gracil draws everything itself, so Windows can't adjust it the way it adjusts its own
controls. Two settings were ignored: **Animation effects** (Accessibility → Visual effects), which
people turn off because motion bothers them, and **contrast themes** (Accessibility → Contrast
themes), which people need to tell anything apart. Separately, the text on the accent color was
fixed per mode (white in light, black in dark), which fades into a pale accent such as yellow.

**Decision.**

1. **Animation effects off: nothing moves.** The dial, the **Always** stone, the list unfolding,
   the done screen and the change of colors go straight to where they end, and the marble doesn't
   breathe while getting ready. Every easing in Gracil goes through one rule, which knows this.
2. **A contrast theme on: Gracil draws with its colors alone.** Its background, its text, and its
   highlight for filled controls; secondary text and edges take the text color, and states read
   in words and marks instead of their own colors. The faint touches that would disappear get a
   solid stand-in, such as the quiet middle of **Always**, which is outlined in full. Turning a
   contrast theme on or off changes the colors at once, as Windows does.
3. **Text on the accent is white or black, whichever reads better on it**, by the usual
   accessibility measure. That is what Windows does for its own blue in each mode, and it stays
   readable on pale accents.
4. Gracil notices when either setting changes while it is open, like the accent color.

**Consequences.**

- Gracil respects the two settings people use most to make Windows comfortable to look at.
- Only the Windows-integration crate reads the settings; the rest of Gracil asks one place.

*Update (2026-10-13):* in a contrast theme, Gracil also uses the theme's highlight-text and
dimmed-text colors: buttons light up in the highlight color on hover and focus, disabled buttons
use the dimmed color, and the list and the picked file get a thin outline.
