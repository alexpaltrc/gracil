# Changelog

What's new in each version of Gracil.

## 0.0.1 (in development)

Gracil is still being built, but you can already compress videos with it.

### New

- Gracil is in English and Spanish. It uses the language Windows is shown in, so there is nothing
  to set.
- Drop videos onto the window, or choose them, and Gracil compresses them one after another. You
  can keep adding videos while it works.
- The video being compressed is shown large, with its progress and **Pause** and **Cancel**
  buttons. **Pause** frees up your computer while keeping the work done so far; **Resume** carries
  on from the same point.
  **View files** shows every other video and how much smaller each one became. Click one to show
  it large, or take it off the list with **×** or **Clear**.
- When a video is done, you see how much smaller it became and choose: **Replace** puts it in
  place of the original, keeping its name and the date it was created; **Save copy** keeps your
  original and saves the smaller one next to it as `Gracil-` followed by its name. With several
  videos done, **Replace all** and **Save all copies** handle them in one go.
- Replacing is done safely: if anything goes wrong, your original is put back. No other file is
  ever overwritten.
- If you take a finished video off the list or close Gracil without choosing, the smaller version
  is deleted and your original stays as it was.
- Every compressed video is an MP4, so it plays everywhere: on Windows, iPhone, Mac, Android, TVs
  and in web browsers. When that changes a video's name, Gracil shows it before you choose, for
  example `IMG_0042.MOV → IMG_0042.mp4`.
- Compressed videos keep their recording date, orientation, chapters, subtitles, audio languages
  and cover art. The few things an MP4 can't hold, such as subtitles that are pictures rather
  than text, are left out, and the video is still kept.
- With a recent NVIDIA graphics card, Gracil uses it to compress much faster. Without one, it uses
  the processor.
- If a video is unusual, Gracil quietly tries simpler ways until one works, instead of giving up.
- If a video can't be made smaller, Gracil doesn't keep it. Its buttons turn off, and a small "!"
  explains why when you point at it.
- Gracil needs FFmpeg, the free tool it uses to compress video. If your computer doesn't have it,
  Gracil downloads and installs it once, on its own.
