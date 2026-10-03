# Questions and answers

*Questions people have asked about Gracil, with plain answers. Collected here for a future help
page; [How Gracil works](how-gracil-works.md) explains everything in more depth.*

---

## After installing, is Gracil next to the clock right away?

Not until it has started once. Right after installing, Gracil isn't running yet. It starts the
first time you open it from the Start menu, and from then on it also starts by itself every time
you sign in to Windows, waiting next to the clock without opening a window.

Closing Gracil's window doesn't quit it: the window goes away and Gracil stays next to the clock,
ready for more videos. Only **Exit**, in the menu of its marble next to the clock, quits it.

## If I quit Gracil, can I still compress videos from File Explorer?

Yes. Right-click videos in File Explorer and **Gracil** is always in the menu, with **Compress and
replace** and **Compress and save a copy**, whether Gracil is running or not. The menu belongs to
Gracil as installed on your computer, not to Gracil while it runs.

If Gracil isn't running when you pick one of them, it starts quietly, without its big window, and
compresses the videos. The small window next to the clock opens so you can watch, unless you
turned that off. When the work is done and you've closed the small window, Gracil quits again by
itself. The next time you sign in to Windows, it starts as usual.

## After replacing a video, a file ending in ".bak" appeared next to it. What is it?

It's your original video, set aside while Gracil put the smaller one in its place, for example
`Clip.mp4.gracil-1a2b3.bak`. Gracil removes it as soon as the smaller video is in place. If
another program was looking at the file just then (your antivirus, Windows search or File
Explorer's thumbnails often do, for a moment), Gracil keeps trying for about a minute, and if it
still can't, it removes the file the next time it starts. You can also delete it yourself: the
smaller video is already in place.

## Why does Windows show Gracil's version as 0.0.1.0?

Gracil's version is 0.0.1. Windows writes the version of every app installed this way with four
numbers, so in **Settings → Apps → Installed apps** it appears as 0.0.1.0. The extra 0 means
nothing; it is always 0.
