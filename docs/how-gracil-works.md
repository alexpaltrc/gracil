# How Gracil works

*A plain-language explanation of what Gracil does behind its simple window. Written for anyone
who uses Gracil, and kept up to date as Gracil changes. Parts marked "(in progress)" describe
decided behavior that is still being built. Short answers to common questions are in
[Questions and answers](faq.md).*

---

## The promise

Gracil is free and private. You don't need an account, and Gracil doesn't need the internet to
work. Your videos never leave your computer: Gracil sends no videos, file names or usage data
anywhere.

Gracil is also open source: its code is public on GitHub for anyone to read. There is nothing to
install. Gracil comes as a zip; unpack it wherever you like, even on a USB drive, and open
`gracil.exe`. Everything it needs comes inside, and it keeps its settings and its diary in a `data`
folder next to itself, and nowhere else, so they go wherever the folder goes. If Gracil is on a
drive it can't write to, it still compresses, it just doesn't remember your settings. To remove
Gracil, delete its folder (see [Questions and answers](faq.md)). If you open Gracil straight from
inside its zip, without unpacking it, it says so and asks you to extract it to a folder first, and
it leaves Windows as it is until you open it from there.

## English and Spanish

Gracil speaks English and Spanish. It uses the language Windows is shown in, so there's nothing
to set: if Windows is in Spanish, Gracil is too; otherwise it's in English.

Numbers are written the way Windows writes them for your region, which you set in Windows'
**Settings → Time & language → Language & region**. In Mexico, for example, that is *15,031
files* and *1.2 GB*; in Spain, *15.031 archivos* and *1,2 GB*.

## Light and dark

Gracil follows Windows' light or dark mode, with nothing to set. It uses Windows 11's own colors,
so it looks like the rest of Windows, and its buttons and progress take the accent color you
picked in Windows' **Settings → Personalization → Colors**. If you switch Windows between light
and dark, or pick another accent color, while Gracil is open, Gracil follows right away, fading
gently instead of flashing. Its windows always open in the right colors from the very first
moment.
Every word and every button is chosen to stay easy to read in both, by the usual accessibility
measure: the buttons for what to do with a finished video are filled, so they stand out clearly.
The words on them are white or black, whichever reads better on your accent color, so even a pale
one stays readable.

If you turn on a contrast theme in Windows (**Settings → Accessibility → Contrast themes**),
Gracil draws with that theme's colors alone, and the moment you turn it on or off, Gracil changes
with it. Buttons light up in the theme's highlight color when you point at them or reach them with
the keyboard, buttons you can't use yet are drawn in its dimmed color, and the list and the file
you are looking at have a thin outline, so nothing depends on shades of grey. If you turn off **Animation effects** (**Settings → Accessibility → Visual effects**),
nothing in Gracil moves: the ring, the list and the colors change at once instead of gliding, and
the marble stays still while Gracil gets ready.

When what the window shows changes, for example from the work to the finished view, or to the
question before leaving, the new view fades in softly in a moment instead of jumping. With
**Animation effects** off, it simply appears.

Gracil's symbol is a marble resting on a support: the marble is your video, and the support is
Gracil, which takes care of the technical work. It is Gracil's icon in the window, the taskbar and
File Explorer. It rests on a small tile in Windows' own greys, always the
opposite of Windows so it stands out: a dark tile when Windows is light, a light tile when Windows
is dark. The window's icon changes with Windows' theme too. Next to the clock it appears without a
tile, in black on a light taskbar and in white on a dark one.

## Adding videos and images

Drop videos or images onto the window, or choose them with **or choose files**. A folder can be
dropped too: Gracil takes the videos and images inside it. You can add more at any time, even
while Gracil is working: drop them, or click the **+** at the bottom right of the window. The
kinds of files Gracil takes are listed under "One format: MP4" (videos) and "Images" (photos and
pictures) below.

- Gracil works on several files at once, each kind in the order you added them. Every file goes
  through two steps: first Gracil gets it ready, then it compresses it. While one video
  compresses, the next one gets ready, so no time is lost between them. Two videos are never
  compressed at once: one already keeps the graphics card (or the processor) fully busy. Images
  compress a few at a time alongside the video, on the processor while the video is on the
  graphics card. How many depends on your computer and on what you are doing (see "Working
  without getting in the way" below).
- The file shown large is the video being compressed, or, with only images, the first image being
  compressed. It sits inside a ring: a marble rolls clockwise from the top as the work advances,
  filling an invisible circle with color behind it, and when it gets back to the top the file is
  done. The percentage is written just below. While Gracil gets a list ready, before the marble
  starts moving, the middle of the marble slowly fades and returns, so you know it is working.
- The middle of the ring holds the controls. Its top part is **pause** (II); click it and it turns
  into **play** (▶), which resumes. Its bottom part is **stop** (■); point at it, or reach it with
  **Tab**, and the square turns into the words **Stop all**. Pause acts on every file being worked on, all together; while
  paused, nothing new starts. Stop stops the whole list at once: files being compressed stop and
  their originals stay as they were, and every file not done yet leaves the list and is kept on a
  quiet line with **Continue**, so you can carry on whenever you like (see "Carrying on after
  closing"). Stop answers straight away, even on a slow network or cloud drive.
- Pausing freezes the work exactly where it is, and Gracil stops using the processor and
  graphics card. Resuming carries on from the same point; nothing is lost or started over. You
  can also stop while paused.
- Everything works from the keyboard too. **Tab** moves a ring from one button to the next
  (**Shift+Tab** goes back), including the pause and stop parts of the ring, and **Enter** or
  **Space** presses the one with the ring. A button with the ring lights up as if you pointed at
  it, and shows its reminder, such as **Add files** for the **+**. Buttons you can't use yet are
  skipped. On the **Always** line, the arrow keys move the choice. With no ring, **Space** pauses
  and resumes, **Esc** steps back (closes the list, or the small window next to the clock),
  **Ctrl+O** chooses files and **Ctrl+Z** undoes **Clear**.
- **View files**, under it, shows every video and image you added and how each one went: how much
  smaller it became, where it went (for example **−35% · Replaced**), or why it couldn't be
  compressed. With two or more done, a line at its top adds them all up, for example **In all:
  3.4 GB → 2.2 GB, 35% smaller**, and the line with **View files** says how many are left, for
  example **12 files left · View files**. The count goes down as each file
  finishes, until none are left. Only what is still on the list counts:
  taking files off it, or **Clear**, takes them out of the total.
  The list unfolds gently from under that line and folds away when closed, quickly even with
  tens of thousands of files: Gracil only draws the rows you can see.
- Click a file in **View files** to show it large; it stays lit in the list while it shows. To go
  back to the work, click it again, close **View files**, or click the quiet line under it that
  names the file being compressed and how far along it is (‹ name · 42%). That line appears
  whenever you are looking at another file while Gracil is working or a result waits for you.
- When the whole list is done (nothing left waiting, being compressed or waiting for your choice),
  the window says so clearly: the ring stays full, its middle fills with green and a check mark
  draws itself, and just below, in large letters, how much room the list saved, for example **130 MB
  smaller**. If nothing came out smaller, or files were left unfinished (by **Stop**, for example),
  there is no check mark. When nothing came out smaller, the large line says **Nothing to make
  smaller** (every file was already compressed) or **Nothing was compressed** (something went
  wrong). Under it, a quiet line says how the list went, for example **22 files compressed · 4 were
  already compressed**. Files that couldn't be compressed are counted apart, with a **!**: click it
  and the list opens on the first of them, with what happened. Below that, **Clear the list** takes
  you back to the empty window, ready for your next files. Pick a file in **View files** to look at
  it (while files failed, the **!** is the way into the list); add more files and Gracil gets back
  to work.
- The **×** beside a file takes it off the list, and **Clear** empties the list. Neither touches
  your original. After **Clear**, a quiet line says **List cleared · Undo** for a few seconds:
  **Undo** (or **Ctrl+Z**) puts every file back where it was, finished results included. Once the
  line is gone, results nobody chose for are deleted, as when you take a file off the list.
- You can always compress a video again, even one already on the list: if the list is done with
  it (it was replaced, saved, couldn't be compressed or was stopped), it starts again as new.
  Adding a video that is still waiting, being compressed or waiting for your choice does nothing.

## Replace, or keep a copy

When a file is done, Gracil shows how much smaller it became, its size before and after (for
example **120 MB → 78 MB**), and two buttons:

- **Replace** puts the smaller file in place of the original. It keeps the same name and the
  date it was created, so it still looks like the same file in your folders. Its "date modified"
  becomes the moment you replaced it.
- **Save copy** keeps your original and saves the smaller file next to it (see "Saving a copy"
  below).

You decide after seeing the result, so you never trade your original for a file that didn't
shrink. With several files done, **View files** offers **Replace all** and **Save all as copies**.

If you always answer the same way, use the line at the foot of the window, always there, even before
you add files: **Always**, followed by **Replace**, **Ask** and **Save copy** (point at **Always**
for a reminder of what it does). Pick **Replace** or **Save copy** and Gracil does that with every
file that finishes from then on, without asking. On **Ask**, in the middle, the setting is off
and Gracil asks each time; that is how it starts. Files already waiting for you keep waiting,
and if Gracil can't do what you picked for a file (for example replacing would overwrite another
file), that file waits for you with a **!** that explains why. When you compress from File Explorer,
the command you clicked decides instead.

A finished file waits until you choose. If you take it off the list, the smaller version is
deleted and your original stays exactly as it was. If you exit Gracil first, Gracil asks before
leaving, and the file is compressed again the next time you carry on.

Every compressed video is an MP4 (see "One format: MP4" below). When that gives it a new name,
Gracil shows it under the buttons before you choose: `IMG_0042.MOV → IMG_0042.mp4`. Replacing
then creates `IMG_0042.mp4` and removes `IMG_0042.MOV`. If an `IMG_0042.mp4` already exists
there, Gracil replaces nothing and a **!** explains why; you can still save a copy.

Good to know: because the name changes, other apps that remembered the old file (for example a
video editing project, or a playlist) won't find it under its old name. If that matters for a
video, choose **Save copy** instead; your original stays exactly where it was.

When something goes wrong, there's no message box. A small **!** appears beside the file; point
at it, or reach it with **Tab**, to read what happened and what to do, for example **Another
program is using this file. Close it, then choose again**.

## FFmpeg, the engine underneath

To compress video, Gracil relies on **FFmpeg**, the free, open-source tool trusted across the
industry. It is the gold standard for this kind of work, and Gracil cannot compress anything
without it.

FFmpeg comes inside Gracil's zip, in its `ffmpeg` folder, so there is nothing to download and Gracil
never uses the internet. Its license and where to find its source code are in the `licenses`
folder.

If that folder is ever removed or damaged, Gracil can't compress videos: they show **Can't start**,
with a **!** that says Gracil is missing its video tool. Download Gracil again from where you got
it and it works as before. Gracil never
downloads anything by itself.

## Choosing the fastest way to compress

When Gracil starts, it checks what your computer can do. It does more than ask whether a feature
exists: it runs a tiny test compression to be sure.

- **With a graphics card that can compress video**, the card does the work: a recent NVIDIA
  card, an Intel one (including the graphics built into most Intel processors) or an AMD one. That
  is usually many times faster than the processor, and the computer stays responsive. If a
  computer has more than one, NVIDIA comes first, then Intel, then AMD.
- **Without one**, Gracil uses the processor. It is slower but just as good.

You never choose between these. Gracil picks the best one available. The test also tells Gracil
which of the graphics card's quality features work, and it uses the ones that make videos
smaller without making them look worse.

## Working without getting in the way

Gracil is meant to be fast without ever making your computer feel slow or sound like it is
straining.

- Everything Gracil does runs below your other programs: whatever you are using comes first, and
  Gracil works with what is left free. Gracil's own window still answers at once.
- Gracil never uses the whole processor. Windows itself keeps Gracil, and every tool it uses, to
  part of it: half while you are using the computer, three quarters when you have left it alone,
  and a quarter on a laptop running on its battery or when Windows is saving energy. A laptop,
  even plugged in, never gives Gracil more than half. The rest is
  always free for you, which also keeps the computer cooler and its fans quieter, even on a
  simple laptop. On top of that, the video encoder and the quality measure each use at most half
  of what Gracil may use.
- Gracil counts the computer as left alone only when nobody needs it: not while a film, a game, a
  presentation or a call is on, even if the keyboard and mouse are untouched. If it can't tell how
  the computer is doing, it treats it as in use.
- While you are using the computer, and always on a laptop, the tools Gracil uses run in Windows'
  efficiency mode, the one Task Manager marks with a green leaf: Windows runs them at the
  processor's calmest speed, so the computer stays cooler and quieter. When you have left a
  plugged-in desktop computer alone, Windows lets them go at full speed. Gracil's own window never uses
  efficiency mode, so it always answers at once.
- On battery, Gracil also takes on fewer images at once. The work goes slower, and the battery
  lasts longer. Below 30 % battery, the files under way finish and no new one starts; below 15 %,
  everything waits. Each waiting file says **Low battery · waiting**. Plug the laptop in and
  Gracil carries on by itself.
- Gracil only checks the computer while it is working, and only the graphics card it is using.
  With nothing to do, it leaves the computer alone.
- If the computer goes to sleep, Gracil rests with it, and once it wakes up it tries again
  whatever sleep interrupted.
- While you are using the computer, Gracil takes on fewer images at once. When you have left it
  alone for a minute, it takes on more, and fewer again as soon as you come back.
- Large images need more memory, so Gracil starts fewer of them at once and always leaves at least
  half of the free memory for your programs.
- Gracil and all its tools together may never take more than about two thirds of the computer's
  memory: Windows refuses them more, so the computer never slows to a crawl. On a computer with
  8 GB of memory or less, or little of it free, the next video waits for the current one to finish
  before getting ready. An image so large it would need more
  than half of all the memory is left as it is, with a small "!".
- Gracil only compresses a file when its drive has room for the result and 2 GB to spare, so the
  drive never fills up. When several files go to the same drive, each one counts the room the
  others will take. Otherwise the file is left as it is, and a small "!" says the drive is
  almost full.
- The window stays quick to answer however long the list is and however long Gracil has been
  working, even with tens of thousands of files and hours of work: pause, stop, **Clear** and
  **View files** always respond at once. Gracil keeps its counts of the list up to date as each
  file changes, instead of going through the whole list again, so a huge list costs the window no
  more than a short one.
- Gracil keeps an eye on how hot the computer runs, so a long list never cooks it, even with
  poor cooling:
  - Gracil never stops to let the computer cool down: it keeps it from getting hot in the first
    place, without you noticing.
  - Gracil reads the graphics card's temperature every second and keeps it at
    about 70 °C at most. Below that it works at full speed. Above it, it takes on fewer images at
    once and works a little less of each second, resting the rest of it, in pauses so short that
    nothing on screen seems to stop and the card's temperature stays even rather than going up
    and down. If the card is still above 70 °C a second later, it eases off a little more, and as
    soon as it is below, it takes on a little more again. So instead of slowing down sharply and
    speeding back up, it settles at the pace that holds the card at 70 °C, never less than a
    quarter of the time. Cards that start slowing themselves down early are kept 20 degrees below
    that point instead. The work only goes a little slower; it never stops.
  - If the card is well above 70 °C, slows itself down because of heat, or Windows holds the
    processor back well below its usual speed for a few seconds, Gracil goes straight to the
    least work, a quarter of the time, and then comes back up step by step.
  - The processor's own temperature can't be read without installing extra software that needs
    full control of the computer, which Gracil never asks for. So for the processor Gracil relies
    on Windows noticing it is held back, and on never using all of it in the first place.
  - This works with NVIDIA, Intel and AMD cards whose driver tells Windows their temperature,
    the same one Task Manager shows. Only NVIDIA cards also say when they start slowing
    themselves down. Most graphics built into the processor don't report a temperature; there
    Gracil relies on the rest of this list.

## Each video gets its own amount of compression

Videos differ: a still interview compresses far more than a football match before anyone can
see a difference. So instead of squeezing every video the same amount, Gracil measures.

- Before compressing, it tries a few short moments from different parts of the video at
  different strengths, and compares each attempt with the original using a well-known measure of
  how people see picture quality. It measures every other picture, which is just as
  reliable and quicker. When the computer's graphics card and FFmpeg allow it, the measuring
  happens on the graphics card too; otherwise on the processor.
- It picks the strongest compression that still looks the same as the original when you watch
  it normally. Easy videos come out much smaller; demanding ones keep the detail they need.
- This takes a few seconds to a minute, while Gracil shows that it's getting ready. Stop works
  the whole time.
- With an NVIDIA graphics card, the short moments stay on the card while they are compressed,
  and only what they are compared with goes to the processor. That makes getting ready quicker
  and leaves the processor freer, with exactly the same result.
- Very short videos, and videos the measurement can't handle, use a safe standard amount
  instead.

The picture is stored in a finer format than most phones use (10-bit), which keeps skies and
shadows smooth instead of banded.

The same short moments also tell Gracil how big the whole video will come out, before it
compresses anything. Gracil exists to make videos lighter, never heavier: it only goes ahead when
the result will be clearly smaller, about a fifth smaller or more.

## Videos that are already compressed

Some videos have already been squeezed hard, like most downloads from the web or a video Gracil
already compressed once. Compressing them again without any visible change would barely make them
smaller. If you give Gracil one of these, you are asking for it to be smaller still, so Gracil
does that on its own, without asking: it compresses a little harder, keeping the picture's size.
It may look a little less sharp than the first time. Looking exactly like the original is the
promise of a video's first compression; after that, you decide by compressing it again.

Gracil still only goes ahead when the result will be clearly smaller. If not even compressing
harder would manage that, the video stays as it is, and it shows **Already compressed** with a
small "!" saying it is already about as small as it can get.

Checking this takes a little longer than usual for these videos, up to about a minute for a long
Full HD video.

## Size and smoothness

- Every video keeps its picture size: a 4K video stays 4K and a 480p video stays 480p. Gracil
  never makes the picture smaller. For videos larger than 4K, the quality check looks at the
  picture at 4K size, which is what a person sees anyway and needs far less memory; the video
  itself keeps its full size.
- Videos with more than 30 frames per second (60 fps, or slow motion recorded at 120 or 240)
  become 30 frames per second. Videos at 30 or fewer keep exactly the rate they had. Slow-motion
  clips that your phone plays slowed down lose that effect, because the extra frames that made it
  possible are gone.
- Sound that takes much more room than it needs to (for example uncompressed sound from
  cameras) is converted to a compact format that sounds the same for normal listening. Other
  sound is kept as it is.
- iPhones (from the iPhone 16) record two kinds of sound in each video: the usual one, and
  Apple's spatial sound, which FFmpeg can't read yet. Gracil keeps the usual one, which every
  player uses, and leaves the spatial one out. A video whose only sound can't be read isn't
  compressed, so it never loses its sound.

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

| You add | You get |
|---|---|
| MP4 (`.mp4`) | MP4, same name |
| M4V, MOV, MKV, AVI, WMV, WebM, MTS, M2TS, TS, 3GP, FLV, MPG, MPEG | MP4 (`.mp4` at the end of the name) |

These are the kinds the file picker and File Explorer's menu offer. A video of another kind that
you drop onto the window is tried too: Gracil never turns a video away just because it is
unusual.

A few things only some formats can hold don't fit in MP4, so they are left out and the video is
kept:

- subtitles that are pictures rather than text (as on some movie rips) are left out; text
  subtitles are kept, though fancy styling such as colors and fonts becomes plain text;
- fonts packed inside the file are left out;
- a few kinds of sound MP4 can't hold are converted to a format it can, and still sound the same
  for normal listening (see "Size and smoothness" above).

A video that was already an MP4 keeps its name. Any other video gets `.mp4` at the end of its
name.

## Images

Gracil compresses photos and other images the same way it compresses videos: you add them, it
makes them smaller, and you choose **Replace** or **Save copy**.

| You add | You get |
|---|---|
| JPEG (`.jpg`, `.jpeg`, `.jpe`, `.jfif`) | JPEG, same name |
| PNG (`.png`) | PNG, same name |
| HEIC and HEIF (iPhone photos), AVIF, WebP, TIFF (`.tif`, `.tiff`), BMP | JPEG (`.jpg` at the end of the name), or PNG if the image has see-through parts |
| GIF, moving PNG or WebP, JPEG XL, camera RAW (`.dng`, `.cr2`, `.nef`, `.arw` and the like) | Not compressed: left exactly as it is, with a small "!" |

- **Two formats, chosen by Gracil.** A PNG stays a PNG, so screenshots and graphics stay sharp.
  Every other image becomes a JPEG, which opens everywhere. An image with see-through parts
  becomes a PNG, since JPEG can't hold them.
  HEIC and AVIF keep their see-through parts too. The extra layers iPhones save with their photos
  (the depth used for portrait mode, for example) aren't see-through parts: those photos become a
  JPEG. A see-through HEIC or AVIF made of tiles, which is rare, is left as it is.
- **One photo compressor: jpegli.** Every JPEG Gracil makes is made by jpegli, Google's modern
  JPEG compressor, which gives smaller photos at the same look than the usual one. It comes
  inside Gracil; nothing is downloaded for it.
- **Nothing pops up.** Gracil reads images itself, in the background. No image viewer or other
  window opens while it works.
- **Looks the same.** The first time, a photo is made smaller without any difference you can see,
  at its own size: Gracil never makes an image smaller in width or height. A PNG is first packed
  more tightly without changing a single dot; when using fewer colors looks the same, Gracil
  does that too.
- **Compressing again** an image Gracil already made smaller loses a little, without asking, as
  it does for videos. If it still can't get clearly smaller, it shows **Already compressed** with
  a small "!".
- **Never heavier.** An image is only offered when the result is clearly smaller.
- **What stays:** the date taken, the camera, the place, descriptions and captions, which way is
  up, and the colors.
- **Not accepted:** moving images (for now), camera RAW files, JPEG XL, and photos stored in HDR.
  They are left exactly as they are, and the "!" says why. They aren't offered in the file
  picker or in File Explorer's menu.
- **How long:** a phone photo takes under a second; a large PNG a few seconds.
- **With videos.** Images and videos share one list. Images compress a few at a time on the
  processor while a video compresses on the graphics card.
- **The circle for the whole list.** With more than one file, the whole circle stands for the whole
  list: it is split into equal parts, one per file, and the marble crosses one part per file, moving
  along as the files being compressed advance. Above it, instead of a file's name, it says how many
  files it holds, for example **Compressing 24 files**, and the percentage below it is the whole
  list's. Adding files while Gracil works makes the parts smaller, so the marble moves back a
  little. While folders are still being looked through, it never reaches the top: more files may
  still come, and it only closes the circle at the real end of the list. The marble next to the
  clock follows the same circle. If you pick another file in **View files** to look at while Gracil
  works, its circle shows just that file. When the whole list is done, the next file you add starts
  a fresh circle.

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
- While it is set aside, your original sits next to the video for a moment with a name like
  `Clip.mp4.gracil-1a2b3.bak`, and Gracil removes it once the smaller video is in place. Right
  then, other programs (your antivirus, Windows search, File Explorer's thumbnails) sometimes
  open it briefly, so Gracil may not be able to remove it at once. It keeps trying for about a
  minute, and anything still left is removed the next time Gracil starts.
- If Gracil is closed suddenly (the computer loses power, or Gracil is forced to quit), the
  compression it was doing stops with it; nothing keeps working in the background. It may leave
  its temporary files behind. The next time Gracil starts, it tidies them away. If it was
  in the middle of replacing a video, it puts your original back. Gracil keeps a note of each
  temporary file it makes and only ever removes those, never a file just because its name looks
  similar. Files on a drive that isn't connected are tidied once it is.
- If the compressed file isn't smaller than the original, Gracil doesn't keep it. It shows
  **Already compressed**, with no buttons, since there is nothing to choose, and says why just
  below.
- If you (or another program) change a file while Gracil compresses it, Gracil notices before
  putting anything in its place. It leaves your file as it is now, lets the compressed version go,
  and says so with a "!". Add the file again to compress the new version.
- A file marked read-only is never replaced, and Gracil never takes the mark off. **Replace** is
  turned off for it, with a "!" saying why, and you can still **Save copy**.
- Before compressing, Gracil makes sure it can save files in the folder, by making an empty file
  there and removing it at once. If the folder doesn't take new files, Gracil says so straight
  away instead of after the whole compression.
- If FFmpeg stops answering (a damaged file, a network drive that stalled), Gracil waits five
  minutes (ten for a check of a video larger than 4K), then stops it and tries the next way of
  compressing. Time paused, resting or asleep doesn't count.
- Results are put in place one at a time, in the order you chose them. Finished files you let go
  (with **×**, **Clear**, or when **Undo** goes away) are deleted in the background, so the window
  never waits on a slow drive.
- Very long names still work: when a name is near Windows' limit, Gracil shortens the end of the
  temporary, copy and backup names so they fit, never cutting a letter in two.

## Saving a copy

When you choose **Save copy**, the compressed copy is saved next to it with
`Gracil-` in front of the name: `Vacation.mp4` becomes `Gracil-Vacation.mp4`. If that name is
already taken, Gracil uses `Gracil-Vacation (2).mp4`, and so on. Gracil never overwrites an
existing file, even on older USB drives where Windows moves files differently.

## Only one Gracil

Gracil runs once. If you open it again while it is already running, from its folder, the taskbar
or anywhere else, the Gracil you have opens its window and comes forward, even when it was working
quietly next to the clock. If you open videos with it, they join the same list.

## From File Explorer

Right-click videos, images or a folder in File Explorer, choose **Show more options**, then
**Send to** and **Gracil**. Gracil is there from the first time you open it, with nothing to set
up. You can send many files and folders at once; other files picked along with them are left
alone.

Gracil can also have its own commands in File Explorer's menu, without going through **Send to**.
Tick **Show Gracil in File Explorer's menu…** in the menu of its marble next to the clock. These
commands are for everyone who uses the computer, so Windows asks you once whether to allow it. Then,
under **Show more options**, videos and images show **Compress with Gracil**, and a folder shows
**Gracil** with **Compress videos and images**, **Compress videos only** and **Compress images
only**.

What happens next follows the **Always** line at the foot of Gracil's window, the same for
**Send to** and for Gracil's own commands:

- On **Replace** or **Save copy**: the files are compressed without opening the big window, and
  each one is put where the line says.
- On **Ask**, in the middle: Gracil's window opens and asks you after each file, just as when
  you drop files on it.

So the words in File Explorer never change; changing the **Always** line is all it takes.

For a folder, Gracil compresses every video and image inside it, subfolders included. *Compress
videos only* leaves the images alone and *Compress images only* leaves the videos alone; **Send
to** always takes both.

Gracil looks through a folder in the background, so its window keeps responding even when the folder
is large or lives on a network or cloud drive (OneDrive, Filen, a NAS…). The same goes for whatever
you drop on the window or choose with **+**, however many files there are. Files join the list
folder by folder, and compressing starts with the first ones found while Gracil keeps looking; until
it finds the first, the window says **Looking through the folder…**, and after that the line with
**View files** says **looking for more**. Looking only reads the names of the files. On a cloud
drive, each file is still downloaded when its turn comes to be compressed, and the smaller version
is uploaded when it is saved, so that part goes as fast as your connection. A file that was kept
only online needs room on your drive for its download as well as for its smaller version, and once
Gracil is done with it, the cloud drive may free up that room again, as **Free up space** does.
Each image is read from the drive only once, and a few are read ahead while others compress, so
the waiting overlaps with the work, but never so many that they crowd your computer's memory.

A folder brings in only your own files: hidden and system files, the Recycle Bin, Windows'
`System Volume Information` and the extra files a Mac leaves on USB drives (names starting with
`._`, and `.Trashes`) are left out. A hidden file you add by itself is still taken.

If Gracil is already running, the files join its list. If Gracil wasn't running, it starts for
that, and when it worked without opening the big window, it closes by itself once the work is
done.

Gracil puts itself in **Send to** each time it starts, so after moving its folder, open it once
from the new place. If you'd rather not have it there, untick **Show Gracil in Send to** in the
menu of its marble next to the clock; tick it again to bring it back. Gracil's own commands in
File Explorer's menu stay as they were written: after moving the folder, tick **Show Gracil in
File Explorer's menu…** again from the new place. Untick it to take them away; Windows asks you
once more.

## Starting with Windows

Gracil starts by itself when you sign in to Windows, and waits quietly next to the clock, with no
window, until you give it videos. If you'd rather it didn't, untick **Start with Windows** in the
menu of its marble next to the clock; you can still open `gracil.exe` from its folder whenever
you like. Gracil isn't listed in Windows' own startup settings: it asks Windows to start it just
once, at the next sign-in, and asks again each time it runs. That way, once its folder is
deleted, Windows forgets it by itself.

## Removing Gracil

**Prepare to delete Gracil**, in the menu of its marble next to the clock, takes Gracil out of
**Send to** and out of starting with Windows, closes it and shows you its folder in File
Explorer. If you turned on Gracil's own commands in File Explorer's menu, Windows asks you once
more, so they can go too. Delete that folder, and nothing of Gracil is left. Opening Gracil again
instead puts **Send to** and the start with Windows back.

## Closing Gracil

Closing the window doesn't stop Gracil. It keeps working next to the clock, where its marble
tells you how things are going:

- the whole marble: nothing is compressing;
- a ring filling up around the marble: Gracil is working, and the ring shows how far along it
  is, for the whole list when there is more than one file;
- two bars: the compression is paused;
- a marble in the warning color (orange-brown on a light taskbar, yellow on a dark one): something
  went wrong while the window was closed. Click it and the small window opens on the file that
  went wrong; the marble turns back to normal.

Pointing at the marble says the same in words, and so does Gracil's name in the taskbar and in
Alt+Tab, for example "Gracil · compressing 42%", or "Gracil · getting ready" before the work
starts moving. Right-click the marble for its menu: **Open
Gracil**; then the settings, **Start with Windows**, **Show Gracil in Send to**, **Show Gracil in
File Explorer's menu…** and **Show mini-window when compressing from File Explorer**; then **Prepare
to delete Gracil** and **Exit**.

Click the marble and a small window opens at once, right next to it, wherever your taskbar is. It
shows the file being compressed and how far along it is; click the middle of its circle to pause or
carry on (stopping is only in the big window, where it is clear what stops). When a file is done, it
shows how much smaller it became with **Replace** and **Save copy**, so you can choose right there
without opening the big window (unless **Always** already chose for you). If more files finish while
another is compressing, it says how many more are left to decide on and shows them one by one when
the work is done. It also tells you how many more files are waiting after it and, once one is
placed, where it went. If a file couldn't be compressed, click the small "!" beside it and the big
window opens on that file, with the reason written out. You can drop more files or folders on it. It
stays open while you do other things; close it with its **×** or by clicking the marble again, and
use **Open Gracil** in it to get the big window.

When you compress from File Explorer, this small window opens by itself so you can see the
work happen. If you'd rather it didn't, right-click the marble and untick **Show mini-window when
compressing from File Explorer**. Gracil remembers your choice, and it never changes how files
are compressed.

**Exit** is how you quit Gracil. If nothing is compressing, waiting or waiting for your choice,
Gracil quits straight away. Otherwise it first asks, and tells you exactly what will happen: the
file being compressed stops and its original stays exactly as it was, files still waiting are kept
for next time, and finished files you haven't saved yet are let go, to be compressed again next
time (see below). Choose **Keep working**, the highlighted button, or press **Esc**, to carry on. If
the work finishes while you decide, the question goes away. Gracil quits within a few seconds, even
if a file it was working on is on a slow network or cloud drive; only a file being put in its place
is always finished first.

When Gracil quits, finished files you didn't choose for are deleted; their originals stay as
they were, and they are on the list to carry on with.

Gracil never holds up Windows shutting down, restarting or signing out. When Windows ends, Gracil
writes down the files that were left at once and stops its work, so you can carry on next time.

## Carrying on after closing

If Gracil closes before its list is done, because you chose **Exit** while it worked, because
something made it stop suddenly or because the computer was turned off, it remembers which files
were left. The next time you open its window, a quiet line says so, for example **12 files were
left unfinished**, with **Continue** to put them back on the list and carry on, and **×** to let
them go. Files that were being compressed start again from their beginning; files already done
aren't done again, except perhaps the last few before a sudden stop. Finished videos you
hadn't chosen for count as unfinished, since they are deleted when Gracil closes. If you used
File Explorer's **Replace** or **Save copy**, that choice comes back with them. The list stays
on your computer, in Gracil's `data` folder, and is gone once everything on it is done.

## What Gracil writes down

Gracil keeps a simple diary of what it did, such as which way it compressed each video, so
problems can be understood later. It stays on your computer, in Gracil's `data` folder, and only
the last seven days are kept. Each day's diary stops growing at about 20 MB: past that, one line
says so, and only problems are written down, briefly.
