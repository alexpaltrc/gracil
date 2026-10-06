# Contributing to Gracil

Thank you for wanting to help. Gracil aims to stay small, calm and private, so before larger
changes please open an issue to talk about it first.

- **Start here.** [docs/development/setup.md](docs/development/setup.md) explains how to build and
  run Gracil. The product is described in [docs/specification.md](docs/specification.md); its
  section 81 lists what must never change silently.
- **Decisions.** Important choices are recorded in [docs/decisions/](docs/decisions/). To change
  one, add a new record rather than editing the old one away.
- **Checks.** Every commit must pass `scripts/check.ps1`: formatting, Clippy (warnings are
  errors) and tests. Tests that touch files work only in fresh temporary folders.
- **Boundaries.** `gracil-core` never depends on the interface or on Windows crates. `unsafe` is
  allowed only in `gracil-windows` and `gracil-shell`, each use with a safety comment.
- **Words.** `CHANGELOG.md` and [docs/how-gracil-works.md](docs/how-gracil-works.md) are for
  people who use Gracil: plain language, no technical terms. Update them in the same change as
  anything a user would notice. More in
  [docs/development/versioning.md](docs/development/versioning.md).
- **Privacy.** Gracil works offline and never sends anything anywhere. Changes that would add
  telemetry, accounts or network access won't be accepted.

By contributing, you agree that your work is licensed under the GPL version 3 or later, like the
rest of Gracil.
