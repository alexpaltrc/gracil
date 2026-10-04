# 0037 — Looking through folders in the background

**Status:** Accepted, built (2026-10-04). Fixes a freeze reported by the product owner. Amends how
ADR 0035 item 1 is carried out; what the user is offered does not change.

**Context.** A folder sent from File Explorer was looked through, and every file in it checked,
on the thread that draws Gracil's windows. On a folder on a network or cloud drive (reported with
Filen's drive in *This PC*), each listing and each check waits on the network. The window stopped
responding until all of it was done: it never changed, a second attempt could not even bring it
up, and Gracil had to be closed from Task Manager. Finding a file in the list also went through
the whole list each time, which grows quickly with folders of thousands of files.

**Decision.**

1. Folders are looked through on their own thread. What is found is handed over folder by folder
   as soon as each folder is read: first that folder's own files, in name order, then its
   subfolders, in name order. Compressing starts with the first files found while Gracil keeps
   looking.
2. Looking reads names only. Whether an entry is a file or a folder comes with the folder's
   listing; files found that way are not checked again, and no file is opened, so a cloud drive
   doesn't have to download anything until a file's turn comes.
3. While nothing has been found yet, the window and the mini-window say *Looking through the
   folder…*; once there are files, the footer adds *looking for more* until the search ends.
4. The list keeps an index of its files by path, so finding one no longer goes through the list.

**Not changed.** Compressing a file on a cloud drive still reads all of it (downloading it), and
saving the result writes it back (uploading it). That is the work itself, and is as fast as the
connection.

**Consequences.** `media::gather_each` reports batches through a callback; `work::gather` turns
them into `Message::Found`. `Gracil::gathering` counts the searches still running. `Queue`
stores a `HashMap` from path to `JobId`.
