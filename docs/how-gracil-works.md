# How Gracil works

*A plain-language explanation of what Gracil does behind its simple window. Written for anyone
who uses Gracil, and kept up to date as Gracil changes. Parts marked "(in progress)" describe
decided behavior that is still being built.*

---

## The promise

Gracil is free and private. You don't need an account, and Gracil doesn't need the internet to
work. Your videos never leave your computer: Gracil sends no videos, file names or usage data
anywhere.

## Adding videos

Drop videos onto the window, or choose them with **or choose files**. You can add more at any
time, even while Gracil is working.

- Gracil compresses one video at a time, in the order you added them. The one being compressed
  is shown large, with its progress and a **Cancel** button.
- **View files**, under it, shows every video you added and how each one went: how much smaller
  it became, or why it couldn't be compressed. Click a video there to show it large. The **×**
  beside it takes it off the list, and **Clear** empties the list. Neither touches your original.
- Adding a video that is already on the list does nothing. Folders are ignored for now.

## Replace, or keep a copy

When a video is done, Gracil shows how much smaller it became and two buttons:

- **Replace** puts the smaller video in place of the original. It keeps the same name and the
  same dates, so it sorts and looks like the same video in your folders.
- **Save copy** keeps your original and saves the smaller video next to it (see "Saving a copy"
  below).

You decide after seeing the result, so you never trade your original for a video that didn't
shrink. With several videos done, **View files** offers **Replace all** and **Save all copies**.

A finished video waits until you choose. If you take it off the list or exit Gracil without
choosing, the smaller version is deleted and your original stays exactly as it was.

If a video changes format when compressed (an AVI becomes MP4), replacing creates
`Clip.mp4` and then removes `Clip.avi`. If a `Clip.mp4` already exists there, Gracil replaces
nothing and a **!** explains why; you can still save a copy.
- When something goes wrong, there's no message box. A small **!** appears beside the video;
  point at it to read what happened.

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
- When you replace the original, the swap is done with Windows' own safe replacement, which
  keeps your original aside until the new file is fully in place. If anything goes wrong, your
  original is put back.
- If the compressed video isn't smaller than the original, Gracil doesn't keep it. Its buttons
  are shown turned off, and a small "!" explains why when you point at it.

## Saving a copy

When you choose **Save copy**, the compressed copy is saved next to it with
`Gracil-` in front of the name: `Vacation.mp4` becomes `Gracil-Vacation.mp4`. If that name is
already taken, Gracil uses `Gracil-Vacation (2).mp4`, and so on. Gracil never overwrites an
existing file.

## Closing Gracil

*(in progress)* Closing the window doesn't stop Gracil. It keeps working in the system tray, next to the clock,
until you choose **Exit** there. If a video is compressing at that moment, Gracil tells you what
will happen before it exits.

For now, closing the window quits Gracil. If a video is compressing, Gracil stops it first and
leaves the original untouched. Finished videos you didn't choose for are deleted; their
originals stay as they were.

## What Gracil writes down

Gracil keeps a simple diary of what it did, such as which way it compressed each video, so
problems can be understood later. It stays on your computer, in Gracil's own folder, and only the
last seven days are kept.
