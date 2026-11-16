# 0075 — A development build runs beside the Gracil in daily use

**Status:** Accepted, built (2026-11-16). Decided by the product owner, who asked for the way
Gracil is developed to be reviewed. Builds on [ADR 0019](0019-single-instance.md),
[ADR 0071](0071-daily-gracil-outside-the-project.md) and [ADR 0073](0073-always-up-to-date.md).

**Context.** There is one Gracil per session: a second one hands its request to the first and
leaves. That is right for people who use Gracil, and it was in the way of making it: a build
started from the project opened the window of the Gracil in daily use instead of its own, so
every interface change could only be looked at by closing the owner's Gracil, in the middle of
whatever it was doing. Changes were checked less often than they should be, or by interrupting
the owner.

**Decision.**

1. **A development build is a Gracil of its own.** A build made the quick way (`cargo run -p
   gracil-ui`, the `dev` profile) goes by its own name among the running programs
   (`Gracil.Dev`), so it runs beside the Gracil in daily use and never hands it a request. Its
   window's title and the words beside its tray icon say **Gracil Dev**. As before, it keeps its
   own `data` folder in `target\debug` and leaves Windows as it is: no Send to, no start with
   Windows. A release build, the one in the zip, is unchanged.
2. **It draws like a release.** Everything Gracil is built on is optimized in the `dev` profile
   too; only Gracil's own code is built the quick way. So a movement can be judged in a
   development build, frame by frame.
3. **`GRACIL_THEME=light` or `dark`** shows the other theme without changing Windows', as
   `GRACIL_LANGUAGE` does for the language.
4. **Every interface change is looked at in a development build** before the Gracil in daily use
   is updated: with its own window, out of the owner's way, read from the window itself and
   never from the screen, and without moving the owner's pointer.

**What stays.** The owner uses one Gracil, the newest, in `Documents\Gracil`, brought up to date
after every change (ADR 0073). Releases are made from `main` as described in
`docs/development/versioning.md`. There is still no separate Dev copy to install
([ADR 0074](0074-gracil-and-the-personal-standard.md)): the development build lives in `target`
and is only ever started from there.

**Consequences.** The owner's Gracil is closed only by its own update. The first development
build after this change takes a few minutes longer, once, while the libraries are optimized.
