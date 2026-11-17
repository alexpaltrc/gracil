# 0074 — Gracil and its owner's personal standard

**Status:** Accepted, built (2026-11-15). Decided by the product owner.

**Context.** The product owner keeps a personal standard for every app they make (the
`alex-vibecode` skill): priorities, technology, repository, and how the app in daily use is
installed and kept up to date. It changes often, and a project that uses it follows its newest
version. Gracil was well along before the standard existed, and some of its earlier decisions
differ from it on purpose.

**Decision.**

1. **Gracil follows the standard**, and `CLAUDE.md` repeats its essentials for cloud sessions,
   which can't see it: how to talk to the owner, the priorities, checking every interface change
   on the real app, a clean repository, and pulling at the start and pushing at the end of every
   session.
2. **Where Gracil differs, on purpose:**
   - **A portable zip only**, with no MSIX and no installer ([ADR 0050](0050-portable-and-open-source.md)).
   - **No separate Dev copy.** The owner uses the newest Gracil every day, in `Documents\Gracil`
     ([ADR 0071](0071-daily-gracil-outside-the-project.md), [ADR 0073](0073-always-up-to-date.md)).
   - **Data beside `gracil.exe`**, in its `data` folder, so Gracil goes wherever its folder goes
     ([ADR 0054](0054-wholly-portable.md)), not in `%LOCALAPPDATA%`.
   - **GPL 3.0 or later**, and **public on GitHub** from its first release, where the standard
     has no license and keeps repositories private.
   - **Documentation in English**, where the standard has it in Spanish.
   - **Libraries are used** where the standard writes everything from scratch: Iced and WGPU for
     the interface, FFmpeg for video, jpegli and image codecs for pictures.
3. **Material that isn't code** lives outside the repository, in
   `Documents\Vibecode Archivos\Gracil Archivos`.
4. **Commits never carry a personal email.** They use GitHub's private address, and the earlier
   history was rewritten to it before the repository went public.

**Consequences.** A change to the standard that touches one of these points is raised with the
owner before Gracil follows it. Everything else in the standard applies as written.
