# 0071 — The Gracil in daily use lives outside the project

**Status:** Accepted, built (2026-10-27). Decided by the product owner. Changes item 5 of
[ADR 0054](0054-wholly-portable.md), which had moved it into `daily\` inside the project. Changed by
[ADR 0081](0081-the-owners-gracil-installed.md): the owner's Gracil is installed.

**Context.** With the Gracil in daily use in `daily\`, the project folder held four
`gracil.exe`: that one, the release and debug builds in `target\`, and the portable layout in
`target\portable\Gracil`. Looking at the folder, the product owner could not tell which was the
current Gracil. The project folder should hold the code, and only the code.

**Decision.**

1. **The Gracil in daily use** lives in `Documents\Gracil`, the user's Documents folder as
   Windows names it. `scripts/portable.ps1 -Copy` copies there, keeping its `data` folder.
   `-CopyTo <folder>` still puts it anywhere else.
2. **The project holds no Gracil to open.** Builds stay in `target\`, which is ignored by git and
   can be deleted at any time; nothing there is the Gracil in daily use. `daily\` is no longer
   ignored by git, since nothing puts it there.

**Consequences.** One Gracil to open, in one obvious place, and a project folder that is only
code and documents. On its first start from the new folder, Gracil puts itself in File Explorer's
**Send to** and the start with Windows from there, as on every start (ADR 0054); pins to Start or
the taskbar made from the old folder must be made again.
