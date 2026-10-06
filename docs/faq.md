# Questions and answers

*Questions people have asked about Gracil, with plain answers. Collected here for a future help
page; [How Gracil works](how-gracil-works.md) explains everything in more depth.*

---

## How do I get Gracil?

Download Gracil's zip from its page on GitHub, unpack it wherever you like (your Programs folder,
your Documents, even a USB drive) and open `gracil.exe`. There is nothing to install. Gracil is
free, and its code is public for anyone to read.

## Windows warned me before opening Gracil. Is it safe?

Windows warns about programs it hasn't seen many people use, and Gracil is new and doesn't pay for
a signing certificate. Choose **More info** and then **Run anyway**. Gracil's code is public on
GitHub, so anyone can check what it does.

## After unpacking, is Gracil next to the clock right away?

Not until you open it once. From then on it also starts by itself every time you sign in to
Windows, waiting next to the clock without opening a window.

Closing Gracil's window doesn't quit it: the window goes away and Gracil stays next to the clock,
ready for more videos. Only **Exit**, in the menu of its marble next to the clock, quits it.

## If I quit Gracil, can I still compress videos from File Explorer?

Yes. Right-click videos in File Explorer and **Gracil** is always in the menu (on Windows 11, under
**Show more options**), whether Gracil is running or not. The menu belongs to Gracil's folder on
your computer, not to Gracil while it runs.

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

## Can I move Gracil's folder?

Yes. Gracil keeps its settings and its diary in a `data` folder next to `gracil.exe`, so they move
with it. Open Gracil once from its new place and File Explorer's menu and the start with Windows
follow it.

## How do I remove Gracil?

In the menu of its marble next to the clock, turn off **Show Gracil in File Explorer's menu**,
choose **Exit**, and delete Gracil's folder. To stop it starting with Windows too, turn it off in
**Settings → Apps → Startup**. Apart from that switched-off entry, nothing is left on your
computer.
