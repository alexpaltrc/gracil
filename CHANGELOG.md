# Changelog

What's new in each version of Gracil.

## 0.0.1 (in development)

Gracil is still being built, but you can already compress videos with it.

### New

- Drop videos onto the window, or choose them, and Gracil compresses them one after another. You
  can keep adding videos while it works.
- The video being compressed is shown large, with its progress. **View files** shows every other
  video and how much smaller each one became.
- Each compressed video is saved next to the original as `Gracil-` followed by its name. Your
  original is never changed, and no existing file is ever overwritten.
- Compressed videos keep their recording date, orientation, chapters, subtitles, audio languages
  and cover art.
- With a recent NVIDIA graphics card, Gracil uses it to compress much faster. Without one, it uses
  the processor.
- If a video is unusual, Gracil quietly tries simpler ways until one works, instead of giving up.
- If a video can't be made smaller, Gracil doesn't save it. A small "!" explains why when you
  point at it.
- Gracil needs FFmpeg, the free tool it uses to compress video. If your computer doesn't have it,
  Gracil downloads and installs it once, on its own.
