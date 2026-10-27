# 0073 — The Gracil in daily use is always up to date

**Status:** Accepted, built (2026-10-27). Decided by the product owner, as part of their personal
standard for every project. Builds on [ADR 0071](0071-daily-gracil-outside-the-project.md) and
[ADR 0072](0072-developing-from-the-cloud.md).

**Context.** The product owner uses the newest Gracil every day, to find bugs and ideas and to
check that everything works; they never use an older, stable one. Updating the Gracil in
`Documents\Gracil` needed a command typed by hand, and work done from the cloud never reaches
that computer by itself.

**Decision.**

1. **`scripts/update.ps1`** brings it up to date: it gets `main` from GitHub, builds it with
   `scripts/portable.ps1 -Copy`, keeping the `data` folder, and opens Gracil again if it was open.
   It says at the end, in Spanish and in plain words, whether it worked or what failed; on any
   failure the Gracil in daily use stays as it was. It takes the pinned FFmpeg from `PATH`, or
   else from the Gracil in daily use, which already holds it, so nothing else needs installing.
2. **After every change made on this computer**, Claude runs `scripts/update.ps1 -NoPull
   -NoPause` without being asked, and says in a line that Gracil is up to date, or why not.
3. **For work from the cloud**, a shortcut **Actualizar Gracil** on the desktop runs
   `scripts/update.ps1`: one double-click on coming back.
4. `scripts/portable.ps1` also finds FFmpeg's license in the `licenses` folder beside the
   `ffmpeg` folder it is given, so FFmpeg taken from a Gracil folder keeps its license in the zip.

**Consequences.** The Gracil in daily use is never behind the code without saying so. An update
takes a few minutes, since it makes a release build. The shortcut points at this project folder
and at `Documents\Gracil`; moving either means making it again.
