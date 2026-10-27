# 0054 — Wholly portable: everything in Gracil's folder, and nothing left once it's gone

**Status:** Accepted, built (2026-10-05). Decided by the product owner. Changes
[0024](0024-start-with-windows.md), [0034](0034-image-standard.md) and
[0050](0050-portable-and-open-source.md), and corrects [0053](0053-ready-for-github.md).

**Superseded in part** by [ADR 0058](0058-explorer-send-to-and-main-menu.md) (2026-10-07):
**Prepare to delete Gracil** deletes the **Send to** shortcut and, when the main menu starts this
Gracil, asks Windows' permission once more to remove it; there are no DLL copies to move aside.
The licenses cover the libraries of `gracil.exe` only.

**Context.** ADR 0050 made Gracil a portable zip but kept a habit from the Store package: when
its own `data` folder couldn't be written, Gracil quietly used `%LOCALAPPDATA%\Gracil` instead,
and the Gracil in daily use lived in `%LOCALAPPDATA%\Programs\Gracil`. Images were worked on in a
`Gracil` folder in Windows' temporary folder that was never removed. The product owner wants the
opposite, as Gracil was meant to be from the start: move the folder to Documents and open it from
there, and when it is no longer wanted, delete that folder knowing nothing else needs deleting.
Starting with Windows and File Explorer's menu can only be had through a few values in the
current user's registry, which outlive a deleted folder. Gracil is also published under the GPL
now ([license](../../README.md#license)), so the licenses of its Rust libraries must ship too.

**Decision.**

1. **Data only beside the program.** Settings, logs, unfinished files and leftovers' notes live
   in `data\` next to `gracil.exe`, and nowhere else. When that folder can't be written (a
   read-only drive, or a folder only an administrator can change), Gracil still compresses but
   remembers nothing: no settings saved, no logs, no list to carry on with.
   `toolchain::default_private_dir` has no fallback, and `private_dir_of` is gone.
2. **Temporary image files are single files** in Windows' temporary folder
   (`gracil-image-<process>-<n>-…`), deleted as soon as the image is done and noted in the
   leftovers ledger in case Gracil is stopped first. No `Gracil` folder is created there.
3. **Starting with Windows cleans up after itself.** Gracil writes its start to the current
   user's `RunOnce` key instead of `Run`, again on every start. Windows deletes a `RunOnce` value
   just before running it at sign-in, so once the folder is gone the value disappears at the next
   sign-in, and a running Gracil puts it back for the next one. Because `RunOnce` entries are not
   listed in Windows' startup settings, the tray menu gets **Start with Windows**, on by default
   and saved as `start_with_windows`; turning it off removes the value. This replaces items 1
   and 3 of ADR 0024.
4. **Prepare to delete Gracil.** File Explorer's menu has no such way to clean up: its
   registration stays until something removes it. The tray menu gets **Prepare to delete
   Gracil**, which takes Gracil out of File Explorer's menu and out of starting with Windows,
   moves aside any copy of the menu File Explorer still holds, exits and shows Gracil's folder in
   File Explorer, picked, ready to delete. Opening Gracil again puts both back, as
   the settings say. A registration left behind by a folder deleted without this step is
   harmless: File Explorer finds no DLL and shows nothing.
5. **The Gracil in daily use** lives in `Documents\Gracil`; `scripts/portable.ps1 -Copy`
   copies there. This replaces item 4 of ADR 0053. *Update (2026-10-14):* it now lives in
   `daily\` inside the project folder (ignored by git), as the product owner asked. *Update
   (2026-10-27):* back in `Documents\Gracil`, outside the project ([ADR 0071](0071-daily-gracil-outside-the-project.md)).
6. **Licenses.** Gracil is GPL-3.0-or-later; `LICENSE` holds the text and every crate declares
   it. `scripts/licenses.ps1` gathers the license files of every Rust library built into
   `gracil.exe` and `gracil_shell.dll` (normal dependencies for Windows x64, from `cargo
   metadata`), each text once with the libraries that ship it, into
   `licenses\rust-libraries.txt`. `portable.ps1` runs it and copies `LICENSE` as
   `licenses\gracil-license.txt`. This finishes the credits ADR 0034 left in progress.

**Consequences.** Deleting Gracil's folder removes Gracil. Besides that folder, Gracil only ever
writes its own registry values for the current user (gone after the next sign-in for the start,
or with **Prepare to delete Gracil** for the menu) and short-lived files in Windows' temporary
folder. A Gracil on a read-only drive works without remembering anything.
