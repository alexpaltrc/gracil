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

Yes. Right-click videos in File Explorer, choose **Show more options**, then **Send to** and
**Gracil**, whether Gracil is running or not. The same goes for Gracil's own commands in File
Explorer's menu, if you turned them on in the menu of its marble *(in progress)*.

If Gracil isn't running when you send videos, it starts quietly and compresses them: without its
big window when the **Always** line is on **Replace** or **Save copy**, and with it, to ask you,
when it is on **Ask**, in the middle. The small window next to the clock opens so you can
watch, unless you turned that off. When the work is done and you've closed the small window,
Gracil quits again by itself. The next time you sign in to Windows, it starts as usual.

## After replacing a video, a file ending in ".bak" appeared next to it. What is it?

It's your original video, set aside while Gracil put the smaller one in its place, for example
`Clip.mp4.gracil-1a2b3.bak`. Gracil removes it as soon as the smaller video is in place. If
another program was looking at the file just then (your antivirus, Windows search or File
Explorer's thumbnails often do, for a moment), Gracil keeps trying for about a minute, and if it
still can't, it removes the file the next time it starts. You can also delete it yourself: the
smaller video is already in place.

## Can I move Gracil's folder?

Yes. Gracil keeps everything it needs, its settings and its diary included, in its own folder, so
they move with it. Open Gracil once from its new place and **Send to** and the start with Windows
follow it. If you turned on Gracil's own commands in File Explorer's menu, tick **Show Gracil in
File Explorer's menu…** again from the new place.

## How do I remove Gracil?

Delete its folder. Gracil keeps nothing anywhere else. To leave Windows exactly as it was, first
choose **Prepare to delete Gracil** in the menu of its marble next to the clock: it takes Gracil
out of File Explorer and out of starting with Windows, closes Gracil and shows you its folder,
ready to delete (if you turned on its own commands in File Explorer's menu, Windows asks you once
more). If you delete the folder without doing that, nothing breaks: Gracil may still be listed in
**Send to** or File Explorer's menu, where it does nothing, and Windows forgets the start the
next time you sign in.
