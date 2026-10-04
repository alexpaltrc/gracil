# Changelog

What's new in each version of Gracil.

## 0.0.1 (in development)

Gracil is still being built, but you can already compress videos with it.

### New

- Gracil is in English and Spanish. It uses the language Windows is shown in, so there is nothing
  to set.
- Gracil wears Windows 11's own colors, in light and dark, so it looks like part of Windows. Its
  buttons and progress take the accent color you picked in Windows. It follows Windows' light or
  dark mode and your accent color, and when you change either, Gracil fades gently to match.
  Every word and button stays easy to read in both.
- Gracil has its own icon, a marble resting on a support, in the window, the taskbar, the Start
  menu and File Explorer. It sits on a small tile in Windows' own greys, always the opposite of
  Windows: dark when Windows is light, light when Windows is dark, so it stands out in both. It
  stays sharp at any screen size.
- Drop videos onto the window, or choose them, and Gracil compresses them one after another. You
  can keep adding videos while it works.
- Gracil runs only once. Opening it again, or opening videos with it while it is running, brings
  the open Gracil forward and adds the videos to the same list.
- Closing the window no longer stops Gracil: it keeps working next to the clock. Its marble there
  fills up as a video compresses, shows two bars when paused, and changes to Windows' warning
  color if something went wrong while the window was closed. Right-click it for **Open Gracil**
  or **Exit**. If a video is compressing or waiting, Gracil first asks, and tells you exactly what
  would stop.
- Click the marble next to the clock and a small window opens beside it, wherever your taskbar
  is. It shows the video being compressed and how far along it is, and its ring pauses and
  resumes just like the big one. It also says how many videos are waiting and where a finished
  one went, and you can drop more videos on it. When a video is done, you can choose **Replace**
  or **Save copy** right there, without opening the big window. It stays open while you do other things until you
  close it or click the marble again; **Open Gracil** in it brings up the big window.
- Right-click videos in File Explorer and choose **Gracil**, which shows how many videos you
  picked, for example **Gracil (3)**. Inside are **Compress and replace** and **Compress and save
  a copy**. The videos are compressed without opening the big window, and other files picked along
  with them are left alone. The small window opens by itself so you can watch; if you'd rather it
  didn't, untick **Show mini-window when compressing from File Explorer** in the marble's
  right-click menu.
- Gracil installs like any other Windows app, and starts by itself when you sign in, waiting
  quietly next to the clock until you give it videos. To stop that, turn Gracil off in Windows'
  **Settings → Apps → Startup**.
- The video being compressed is shown large, inside a ring: a marble rolls around it as the work
  advances. The middle of the ring holds the controls, as symbols instead of words: the top part
  pauses (II) and turns into play (▶) to resume, and the bottom part stops (■). Pausing frees up
  your computer while keeping the work done so far; resuming carries on from the same point.
  **View files** shows every other video and how much smaller each one became. Click one to show
  it large, or take it off the list with **×** or **Clear**. A video can always be compressed
  again, even right after replacing it, without clearing the list first.
- When a video is done, you see how much smaller it became and choose: **Replace** puts it in
  place of the original, keeping its name and the date it was created; **Save copy** keeps your
  original and saves the smaller one next to it as `Gracil-` followed by its name. With several
  videos done, **Replace all** and **Save all copies** handle them in one go.
- If you always answer the same way, the line at the foot of the window lets you say so once:
  **Always**, then **Replace**, a small circle, and **Save a copy**. Pick either end and Gracil
  does that with every video that finishes from then on. On the circle in the middle, where it
  starts, Gracil asks each time.
- Replacing is done safely: if anything goes wrong, your original is put back. No other file is
  ever overwritten.
- If Gracil is closed suddenly, for example when the computer loses power or it is forced to
  quit, the compression it was doing stops with it instead of carrying on unseen. Gracil tidies
  away what it left half done the next time it starts. If it was replacing a video at that moment, it puts
  your original back. It only ever removes files it made itself.
- If you take a finished video off the list or exit Gracil without choosing, the smaller version
  is deleted and your original stays as it was.
- Every compressed video is an MP4, so it plays everywhere: on Windows, iPhone, Mac, Android, TVs
  and in web browsers. When that changes a video's name, Gracil shows it before you choose, for
  example `IMG_0042.MOV → IMG_0042.mp4`.
- Compressed videos keep their recording date, orientation, chapters, subtitles, audio languages
  and cover art. The few things an MP4 can't hold, such as subtitles that are pictures rather
  than text, are left out, and the video is still kept.
- With a recent NVIDIA graphics card, Gracil uses it to compress much faster, with the card's
  quality features that make videos smaller without making them look worse. Without one, it uses
  the processor.
- Each video gets its own amount of compression. Gracil first tries a few short moments of the
  video and picks the strongest compression that still looks the same as the original, so easy
  videos come out much smaller and demanding ones keep their detail. This takes a few seconds
  while Gracil is getting ready, and you can stop it at any time. It looks at every other picture,
  which is just as reliable and quicker, and leaves the processor freer.
- Gracil makes videos lighter, never heavier. Those same short moments tell it how big the video
  will come out, so it only compresses when the result will be clearly smaller, instead of working
  for minutes to find out it isn't.
- Videos that are already compressed, like most web downloads or a video Gracil already
  compressed, are compressed a little harder on their own, without asking. They may look a little
  less sharp than the first time, which is what compressing a video again asks for. If not even
  that would make the video clearly lighter, it stays as it is and shows **Already compressed**
  with a small "!" saying it's already about as small as it can get.
- Colours are stored more finely, so skies and shadows stay smooth instead of banded.
- Every video keeps its picture size, 4K included. Videos with more than 30 frames per second
  become 30; slower ones keep exactly the rate they had. Slow motion recorded by a phone loses its
  slowed-down effect.
- Sound that takes much more room than it needs, such as uncompressed sound from cameras, is made
  compact and still sounds the same for normal listening.
- If a video is unusual, Gracil quietly tries simpler ways until one works, instead of giving up.
- If a video can't be made smaller, Gracil doesn't keep it. Its buttons turn off, and a small "!"
  explains why when you point at it.
- Gracil needs FFmpeg, the free tool it uses to compress video. If your computer doesn't have it,
  Gracil downloads and installs it once, on its own.
