# 0082 — Gracil Dev, installed beside the owner's Gracil

**Status:** Accepted, built (2026-11-23). Decided by the product owner. Completes item 4 of
[ADR 0081](0081-the-owners-gracil-installed.md) and changes
[ADR 0073](0073-always-up-to-date.md) and [ADR 0075](0075-a-development-build-of-its-own.md).

**Context.** The owner's Gracil is installed from a release and changes only with one (ADR
0081). Their personal standard asks for a second copy, Dev, where every change arrives and is
tried: installed the same way as the stable copy, so trying Dev is trying what a release will
be, with a name, an identity, an icon and data of its own, and never the owner's settings.

**Decision.**

1. **Gracil Dev is a build with the `dev` feature**, installed per user in
   `%LOCALAPPDATA%\Programs\Gracil Dev` by the same installer (`scripts/installer.iss` with
   `/DDev`), under its own AppId, with its own Start menu shortcut and its own `data` folder.
2. **It never touches the owner's Gracil.** Its **Send to** shortcut, its start with Windows, its
   File Explorer main menu and its commands all say **Gracil Dev**, under keys and names of their
   own; it goes by `Gracil.Dev` among running programs.
3. **It is told apart at a glance**: its window and tray say **Gracil Dev**, and its icons, in the
   window, the taskbar, the Start menu and the tray, carry an amber band that says "Dev"
   (`scripts/icons.ps1`, drawn in code for the tray). The owner chose the band among three
   marks.
4. **Every change reaches it.** `scripts/update.ps1` builds Gracil Dev's installer from this
   folder (or from `main` after getting it from GitHub) and runs it quietly over the installed
   one, which keeps its data and opens again if it was open. Its version is the one in
   development followed by the number of commits, so each one installs over the last.
5. **A development build** (`cargo run -p gracil-ui`) also says **Gracil Dev** and carries its
   icons, but goes by `Gracil.Build` among running programs, so it runs beside both installed
   copies, for checking a change frame by frame before it reaches Gracil Dev.

**Consequences.** The owner uses their Gracil with their own settings, untouched by work in
progress, and tries every change in Gracil Dev, installed the way a release is. Gracil Dev's
File Explorer main menu, like the other's, is turned on from its own tray menu with an
administrator's permission.
