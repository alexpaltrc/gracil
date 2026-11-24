# 0081 — The owner's Gracil is installed, from a release

**Status:** Accepted, built (2026-11-22). Decided by the product owner. Changes
[ADR 0071](0071-daily-gracil-outside-the-project.md), [ADR 0073](0073-always-up-to-date.md) and
item 1 of [ADR 0074](0074-gracil-and-the-personal-standard.md).

**Context.** The owner's personal standard now separates two copies of every app: a stable one,
used every day with the owner's real settings, which changes only with a release, and a Dev
copy where every change is tried. It ranks an installer above a portable folder. Gracil's daily
copy was a portable folder in `Documents\Gracil`, rebuilt from `main` after every change, so the
owner used work in progress with their real settings.

**Decision.**

1. **The owner's Gracil is installed**, for one user and without an administrator, in
   `%LOCALAPPDATA%\Programs\Gracil`, with a shortcut in the Start menu. `scripts/installer.ps1
   -Version X.Y.Z` makes the installer with Inno Setup from the zip GitHub built for that release
   ([ADR 0079](0079-releases-built-on-github.md)), so what is installed is exactly what was
   released; `scripts/installer.iss` describes it. Installing over it keeps its `data` folder.
   Uninstalling takes out the Start menu shortcut, **Send to** and the start with Windows.
2. **It changes only with a release.** Each new release is installed over it.
3. **Releases stay a portable zip** ([ADR 0050](0050-portable-and-open-source.md)). The installer
   is for the owner's computer and is not published.
4. **Gracil Dev**, an installed copy of its own with its own name, icon and data, is planned and
   not built. Until it exists, changes are tried in the development build
   ([ADR 0075](0075-a-development-build-of-its-own.md)), and `scripts/update.ps1` and the
   **Actualizar Gracil** shortcut are not run: they would build `main` into `Documents\Gracil`,
   and that Gracil, on starting, would take **Send to** and the start with Windows from the
   installed one.

**Consequences.** The owner's Gracil is what everyone else gets, settings included. File
Explorer's main menu, written for the whole computer, keeps pointing at the folder that turned it
on, so after moving it is turned on again from the installed Gracil's tray menu. Work in progress
reaches the owner only through the development build until Gracil Dev exists.
