# How Gracil works

*A plain-language explanation of what Gracil does behind its simple window. Written for anyone
who uses Gracil, and kept up to date as Gracil changes. Parts marked "(in progress)" describe
decided behavior that is still being built.*

---

## The promise

Gracil is free and private. You don't need an account, and Gracil doesn't need the internet to
work. Your videos never leave your computer: Gracil sends no videos, file names or usage data
anywhere.

## FFmpeg, the engine underneath

To compress video, Gracil relies on **FFmpeg**, the free, open-source tool trusted across the
industry. It is the gold standard for this kind of work, and Gracil cannot compress anything
without it.

- If FFmpeg is already on your computer, Gracil uses it.
- If it isn't, Gracil downloads it once from its official build distributor and installs it in
  its own folder. This happens quietly the first time Gracil needs it. You don't have to do
  anything.
- Before using what it downloaded, Gracil checks that it is exactly the expected file. If
  anything differs, Gracil discards it.

This one-time download is the only time Gracil uses the internet.

## Choosing the fastest way to compress

When Gracil starts, it checks what your computer can do. It does more than ask whether a feature
exists: it runs a tiny test compression to be sure.

- **With a recent NVIDIA graphics card**, the card does the work. That is usually many times
  faster than the processor, and the computer stays responsive.
- **Without one**, Gracil uses the processor. It is slower but just as good.

You never choose between these. Gracil picks the best one available.

## Never giving up on a video

Videos come from phones, cameras, screen recorders and editing apps, and some are unusual.
Gracil does not reject a video because of that. If the first way of compressing it doesn't work,
Gracil quietly tries simpler ways, one after another:

1. The graphics card reads and compresses the video.
2. The processor reads the video and the graphics card compresses it.
3. Only the picture and sound are kept, in their most compatible form.
4. The processor does everything.

Usually the first way works. You only hear about a problem when none of them succeed.

## What Gracil keeps

A compressed video should still feel like the same video. Gracil keeps:

- the date and time it was recorded, so it still sorts correctly in your gallery;
- its orientation, so phone videos stay upright;
- its title and other details, chapters, the language of each audio track, subtitles and cover
  art;
- the extra details iPhones record.

If a detail can't fit in the compressed file, Gracil leaves just that detail out and keeps the
video.

MP4, MOV and MKV videos keep their format. Videos in older or less common formats (such as AVI)
become MP4, which plays everywhere.

## Your original is safe

Gracil never writes over your video while it works.

- The compressed version is written to a separate temporary file next to the original.
- When it finishes, Gracil checks the result: it must open correctly, be compressed properly and
  last as long as the original.
- If you cancel, or anything fails, the temporary file is removed and your original is exactly as
  it was.
- *(in progress)* When you replace the original, the swap is done with Windows' own safe
  replacement. Even a power cut at the wrong moment cannot leave you without your video.
- *(in progress)* If the compressed video isn't smaller than the original, Gracil doesn't save
  it. The save buttons turn off and a small "!" explains why when you point at it.

## Saving a copy instead

*(in progress)* If you choose to keep your original, the compressed copy is saved next to it with
`Gracil-` in front of the name: `Vacation.mp4` becomes `Gracil-Vacation.mp4`. If that name is
already taken, Gracil uses `Gracil-Vacation (2).mp4`, and so on. Gracil never overwrites an
existing file.

## Closing Gracil

*(in progress)* Closing the window doesn't stop Gracil. It keeps working in the system tray, next to the clock,
until you choose **Exit** there. If a video is compressing at that moment, Gracil tells you what
will happen before it exits.
