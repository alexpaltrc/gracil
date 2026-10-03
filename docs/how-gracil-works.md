# How Gracil works

*A plain-language explanation of what Gracil does behind its simple window. Written for anyone
who uses Gracil, and kept up to date as Gracil changes. Parts marked "(in progress)" describe
decided behavior that is still being built.*

---

## The promise

Gracil is free and private. You don't need an account, and Gracil doesn't need the internet to
work. Your videos never leave your computer: Gracil sends no videos, file names or usage data
anywhere.

## English and Spanish

Gracil speaks English and Spanish. It uses the language Windows is shown in, so there's nothing
to set: if Windows is in Spanish, Gracil is too; otherwise it's in English.

## Light and dark

Gracil follows Windows' light or dark mode, with nothing to set. Its colors are soft mineral greys
and ivory. If you switch Windows between light and dark while Gracil is open, Gracil follows right
away, fading gently from one to the other instead of flashing.

Gracil's symbol is a marble resting on a support: the marble is your video, and the support is
Gracil, which takes care of the technical work. It is Gracil's icon in the window, the taskbar
and File Explorer, and it sits next to the clock, in a version that suits a light or dark taskbar.

## Adding videos

Drop videos onto the window, or choose them with **or choose files**. You can add more at any
time, even while Gracil is working.

- Gracil compresses one video at a time, in the order you added them. The one being compressed
  is shown large, inside a ring: a marble rolls around it clockwise from the top as the work
  advances, and when it gets back to the top the video is done. The percentage is written just
  below.
- The middle of the ring holds the controls. Its top part is **pause** (II); click it and it turns
  into **play** (▶), which resumes. Its bottom part, under a thin line, is **stop** (■), which
  cancels. While Gracil is still getting ready, only stop works.
- Pausing freezes the work exactly where it is, and Gracil stops using the processor and
  graphics card. Resuming carries on from the same point; nothing is lost or started over. You
  can also stop while paused.
- **View files**, under it, shows every video you added and how each one went: how much smaller
  it became, or why it couldn't be compressed. Click a video there to show it large. The **×**
  beside it takes it off the list, and **Clear** empties the list. Neither touches your original.
- Adding a video that is already on the list does nothing. Folders are ignored for now.

## Replace, or keep a copy

When a video is done, Gracil shows how much smaller it became and two buttons:

- **Replace** puts the smaller video in place of the original. It keeps the same name and the
  date it was created, so it still looks like the same video in your folders. Its "date modified"
  becomes the moment you replaced it.
- **Save copy** keeps your original and saves the smaller video next to it (see "Saving a copy"
  below).

You decide after seeing the result, so you never trade your original for a video that didn't
shrink. With several videos done, **View files** offers **Replace all** and **Save all copies**.

If you always answer the same way, use the line at the foot of the window: **Always**, followed
by **Replace**, a small circle and **Save a copy**. Pick **Replace** or **Save a copy** and Gracil
does that with every video that finishes from then on, without asking. On the circle in the
middle, the setting is off and Gracil asks each time; that is how it starts. Videos already waiting for you
keep waiting, and if Gracil can't do what you picked for a video (for example replacing would
overwrite another file), that video waits for you with a **!** that explains why. When you
compress from File Explorer, the command you clicked decides instead.

A finished video waits until you choose. If you take it off the list or exit Gracil without
choosing, the smaller version is deleted and your original stays exactly as it was.

Every compressed video is an MP4 (see "One format: MP4" below). When that gives it a new name,
Gracil shows it under the buttons before you choose: `IMG_0042.MOV → IMG_0042.mp4`. Replacing
then creates `IMG_0042.mp4` and removes `IMG_0042.MOV`. If an `IMG_0042.mp4` already exists
there, Gracil replaces nothing and a **!** explains why; you can still save a copy.

Good to know: because the name changes, other apps that remembered the old file (for example a
video editing project, or a playlist) won't find it under its old name. If that matters for a
video, choose **Save copy** instead; your original stays exactly where it was.

When something goes wrong, there's no message box. A small **!** appears beside the video; point
at it to read what happened.

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

## One format: MP4

Every video Gracil compresses comes out as an MP4, whatever it was before: MOV from an iPhone,
MKV, AVI and the rest. MP4 plays everywhere (Windows, iPhone, Mac, Android, TVs and web
browsers), so you never have to wonder whether the result will open.

A few things only some formats can hold don't fit in MP4, so they are left out and the video is
kept:

- subtitles that are pictures rather than text (as on some movie rips) are left out; text
  subtitles are kept, though fancy styling such as colors and fonts becomes plain text;
- fonts packed inside the file are left out;
- a few kinds of sound MP4 can't hold are converted to a format it can, and still sound the same
  for normal listening.

A video that was already an MP4 keeps its name. Any other video gets `.mp4` at the end of its
name.

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
- If Gracil is closed suddenly (the computer loses power, or Gracil is forced to quit), the
  compression it was doing stops with it; nothing keeps working in the background. It may leave
  its temporary files behind. The next time Gracil starts, it tidies them away. If it was
  in the middle of replacing a video, it puts your original back. Gracil keeps a note of each
  temporary file it makes and only ever removes those, never a file just because its name looks
  similar. Files on a drive that isn't connected are tidied once it is.
- If the compressed video isn't smaller than the original, Gracil doesn't keep it. Its buttons
  are shown turned off, and a small "!" explains why when you point at it.

## Saving a copy

When you choose **Save copy**, the compressed copy is saved next to it with
`Gracil-` in front of the name: `Vacation.mp4` becomes `Gracil-Vacation.mp4`. If that name is
already taken, Gracil uses `Gracil-Vacation (2).mp4`, and so on. Gracil never overwrites an
existing file.

## Only one Gracil

Gracil runs once. If you open it again, or open videos with it while it is already running, the
Gracil you have comes forward and the videos join the same list.

Right-click videos in File Explorer and you'll find **Gracil** in the menu, with
the number of videos in brackets when you picked more than one, for example **Gracil (3)**. Other
files you picked along with them are left alone, and if none of them is a video the menu doesn't
show Gracil at all. Inside are two choices: **Compress and replace** and **Compress and save a
copy**. Either one compresses the videos without opening the big window, and replaces them or
saves copies as you chose. If Gracil is already running, the videos join its list. If Gracil wasn't
running, it starts for that, and closes by itself once the work is done.

## Starting with Windows

Gracil starts by itself when you sign in to Windows, and waits quietly next to the clock, with no
window, until you give it videos. There is nothing to set in Gracil. If you'd rather it didn't
start with Windows, turn it off where Windows lists the apps that start with it: **Settings →
Apps → Startup**, or the **Startup apps** page of Task Manager. Gracil respects that and never
turns it back on; you can still open it from the Start menu whenever you like.

## Closing Gracil

Closing the window doesn't stop Gracil. It keeps working next to the clock, where its marble
tells you how things are going:

- the whole marble: nothing is compressing;
- a ring filling up around the marble: a video is compressing, and the ring shows how far along
  it is;
- two bars: the compression is paused;
- an amber marble: something went wrong while the window was closed. Open Gracil to see what;
  the marble turns back to normal.

Pointing at the marble says the same in words. Right-click it for **Open Gracil** and **Exit**.

Click the marble and a small window opens right next to it, wherever your taskbar is. It shows
the video being compressed and how far along it is; click its circle to pause or carry on, just
like in the big window. When a video is done, it shows how much smaller it became with
**Replace** and **Save copy**, so you can choose right there without opening the big window
(unless **Always** already chose for you). If more videos finish while another is compressing, it
says how many are ready and shows them one by one when the work is done. It also tells you how many videos are waiting and, once one is placed,
where it went. You can drop more videos on it. It stays open while you do other things; close it
with its **×** or by clicking the marble again, and use **Open Gracil** in it to get the big
window.

When you compress from File Explorer, this small window opens by itself so you can see the
work happen. If you'd rather it didn't, right-click the marble and untick **Show mini-window when
compressing from File Explorer**. Gracil remembers your choice, and it never changes how videos
are compressed.

**Exit** is how you quit Gracil. If nothing is compressing or waiting, Gracil quits straight away.
Otherwise it first asks, and tells you exactly what will happen: the video being compressed stops
and its original stays exactly as it was, and videos still waiting won't be compressed. Choose
**Keep working** to carry on. If the work finishes while you decide, the question goes away.

When Gracil quits, finished videos you didn't choose for are deleted; their originals stay as
they were.

## What Gracil writes down

Gracil keeps a simple diary of what it did, such as which way it compressed each video, so
problems can be understood later. It stays on your computer, in Gracil's own folder, and only the
last seven days are kept.
