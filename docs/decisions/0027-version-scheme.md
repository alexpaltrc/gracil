# 0027 — Versions stay small: 1.0 at the first public release, then slowly

**Status:** Accepted, followed since Gracil 1.0.0 (2026-11-16). Decided by the product owner
(2026-10-03). Settles the "pending" note in `docs/development/versioning.md`.

**Changed** by [ADR 0050](0050-portable-and-open-source.md) (2026-10-05): 1.0 is the first public
release on GitHub.

**Context.** Before the first release the product owner wanted to consider a different-looking
version, such as `0.0.01` or `01.00`, and prefers numbers that grow slowly. A published version
can never go back down. Year-based versions (Apple's 26, Python 3.26, Minecraft 26.1) are the
current trend, but they start high and grow every year.

**Decision.**

1. Gracil keeps three numbers, `MAJOR.MINOR.PATCH`. Leading zeros and year-based numbers are not
   used.
2. The first public release is **1.0** (`1.0.0`). Until then development continues in `0.0.x`.
3. **MINOR** goes up only for a release with something new a user would notice (for example image
   compression would be 1.1). Several changes wait to ship together, so this happens a few times
   a year at most.
4. **PATCH** goes up only for fixes, with nothing new: 1.1 → 1.1.1 → 1.1.2. The next release with
   something new is 1.2, and its patch starts again at 0.
5. **MAJOR** is kept for a change of era, such as a complete redesign; Gracil may stay 1.x for
   years.
6. A zero patch is left out where Gracil writes the version itself: Gracil and the changelog
   write `1.1` and `1.1.1`. The zip is named with all three numbers (`Gracil-1.1.0-x64.zip`).

**Consequences.**

- Numbers stay small and say something: a change in the second number means something new to try,
  in the third only that something was fixed.
