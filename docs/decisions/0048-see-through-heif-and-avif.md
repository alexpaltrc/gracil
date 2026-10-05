# 0048 — See-through HEIF and AVIF

**Status:** Accepted, built (2026-10-05).

**Context.** FFmpeg decodes only the main picture of a HEIF or AVIF file, so Gracil refused any
whose transparency it found (ADR 0034). It also counted any auxiliary picture as
transparency, so iPhone photos with depth or portrait mattes, which are opaque, were refused too:
136 in one of the product owner's lists.

**Decision.**

1. Transparency is only an auxiliary picture linked to the main one (`auxl`) whose type says
   alpha: `urn:mpeg:hevc:2015:auxid:1` (HEIF) or `urn:mpeg:mpegB:cicp:systems:auxiliary:alpha`
   (AVIF). Depth (`auxid:2`), Apple's mattes and HDR gain maps are not.
2. FFmpeg exposes each item as a stream whose id is the item's, so the alpha item is decoded on
   its own (`-map 0:i:<id>`, gray) and put into the main picture's transparency. A size mismatch
   fails quietly.
3. Whether the picture is see-through is then judged from its pixels, as for any image: an alpha
   that covers nothing gives a JPEG, a real one a PNG.
4. A transparency made of tiles (`grid`) can't be put together through FFmpeg, which reads such
   items one tile at a time; that picture is left as it is, as already as small as it gets.

**Consequences.** iPhone photos with depth or mattes compress like any other photo, and real
see-through HEIC and AVIF keep their transparency. Tiled transparency, which is rare, stays
untouched.
