# 0058 — File Explorer: Send to for everyone, and a main menu turned on from the tray

**Status:** Accepted, built (2026-10-07). Send to and the main menu confirmed working (2026-10-14).
Supersedes the per-user COM menu of [0022](0022-explorer-command.md) and items 4 to 6 of
[0050](0050-portable-and-open-source.md), the menu's words that followed **Always** in item 2 of
[0035](0035-explorer-folders-and-always.md) (and the reason item 1 of
[0057](0057-always-in-sight.md) gave for them), and the `gracil-shell` parts of
[0006](0006-explorer-extension-forwards-only.md), [0011](0011-english-and-spanish.md) and
[0054](0054-wholly-portable.md). Changes spec §58.

**Context.** Since ADR 0050, File Explorer's menu was `gracil_shell.dll`, an `IExplorerCommand`
COM server registered for the current user under `HKCU\Software\Classes`, so no administrator was
needed. On the product owner's Windows 11 (an Insider build, 26300) it never showed. Real File
Explorer there ignores every command registered under `HKCU\Software\Classes`, whether a COM
handler or a plain `command` verb; only commands written for the whole computer
(`HKEY_LOCAL_MACHINE`), such as Git's or Visual Studio's, appear. Tests that list the verbs from
inside a process (`Shell.Application`'s `Verbs()`) do list the per-user ones, which is why the
menu looked registered. Windows 11's short top menu is reserved for packaged apps, which a portable
zip is not (ADR 0050). So the only menu a portable Gracil can count on, without an administrator,
is **Send to**, which Windows builds from a folder of shortcuts in the user's own profile.

**Decision.**

1. **Send to.** Gracil puts a shortcut, `%APPDATA%\Microsoft\Windows\SendTo\Gracil.lnk`, that
   starts `gracil.exe --explorer` with what was picked. It needs no administrator and File
   Explorer always shows it (on Windows 11 under **Show more options** → **Send to**). Like
   starting with Windows (ADR 0054), it is written again on every start of a whole Gracil folder,
   so a moved or updated folder keeps working. The existing tray setting, saved as
   `explorer_menu`, now reads **Show Gracil in Send to** (Spanish: «Mostrar Gracil en «Enviar
   a»»); turning it off deletes the shortcut.
2. **The main menu, opt-in.** A tray item, **Show Gracil in File Explorer's menu…**
   (Spanish: «Mostrar Gracil en el menú del Explorador de archivos…»), writes plain commands for
   the whole computer, under `HKEY_LOCAL_MACHINE\SOFTWARE\Classes`:
   - for each video and image type Gracil compresses,
     `SystemFileAssociations\.<ext>\shell\Gracil`: **Compress with Gracil** (Spanish: «Comprimir
     con Gracil»), with `MultiSelectModel` `Player` so every picked file is sent;
   - for folders, `Directory\shell\Gracil`: **Gracil**, opening onto **Compress videos and
     images**, **Compress videos only** and **Compress images only** (Spanish: «Comprimir videos e
     imágenes», «Comprimir solo los videos», «Comprimir solo las imágenes»), the last two with
     `--videos` or `--images` (ADR 0035). A `GracilProgram` value beside it records which
     `gracil.exe` wrote the menu, so the tray item shows a tick only for the Gracil it starts.

   Writing for the whole computer takes an administrator, so Windows asks permission once, when
   the user picks the item; nothing is written at start-up. An administrator's Gracil does the
   writing (`gracil.exe --add-main-menu`, or `--remove-main-menu` to take it away) with no window
   and no claim on the running Gracil, which watches the menu for up to two minutes and then
   updates the tick. The commands show under **Show more options**.

   Writing works: run from an administrator's terminal, the commands appeared. Asking for the
   permission from inside Gracil is still denied by Windows on the product owner's PC. Gracil runs
   in its own job object (ADR 0018), the suspected cause, so it now asks File Explorer itself to
   start the administrator's Gracil, as the user's own **Run as administrator** would. *Update
   (2026-10-14):* confirmed on the product owner's PC: Windows asks once, and the commands appear
   for files and folders.
3. **`--explorer` leaves the choice to Always.** Send to and the main menu both send `--explorer`.
   The running Gracil applies the **Always** setting (ADR 0025): set to an end, the files are
   compressed in the background and placed as it says; in the middle, the window opens and asks,
   as with dropped files. So the menu's words never change with **Always**, and changing **Always**
   never needs the permission again. This replaces item 2 of ADR 0035 (*Compress and replace*,
   *Compress and save copies of*, or just *Compress*). `--replace` and `--save-copy` stay as a
   choice made in advance.
4. **The old menu is cleared.** On every start Gracil removes what the per-user menu left: its
   `HKCU\Software\Classes` keys, both COM classes (`{8D7CC31A-14B9-490F-A5B0-3A2B63DAC4EF}` and
   `{A7D6F05F-328B-4A10-A19E-C58E90EB7038}`) and the DLL copies in `data\shell`.
5. **Prepare to delete Gracil** (ADR 0054) also deletes the Send to shortcut, and when the main
   menu starts this Gracil, Windows asks permission once more to remove it. Saying no leaves
   commands that simply find no Gracil once the folder is gone.
6. **One shipped program.** `crates/gracil-shell` is deleted; the zip holds `gracil.exe` and no
   DLL, and a whole Gracil folder is recognised by FFmpeg beside the program alone.
   `gracil-windows` is now the only crate allowed `unsafe`, and uses the `windows` crate for the
   few COM calls the shortcut and the permission need.

**Consequences.**

- Gracil is in File Explorer for everyone from the first start, one level deeper than before:
  **Show more options** → **Send to** → **Gracil**. Send to has no folder commands, so a folder
  sent to Gracil brings in its videos and images, both kinds.
- The main menu is a choice, and the only thing Gracil writes outside the current user. It is
  written once, so moving the folder leaves its commands pointing at the old place until the item
  is turned on again from the new one.
- The menu's words are fixed, in the language Windows had when it was written.
- No DLL inside `explorer.exe`: File Explorer never loads Gracil's code, and an update never finds
  the DLL held.
- In-process verb listings are not a test of what File Explorer shows; only File Explorer itself
  is.
