# Versioning, releases and the changelog

## Version numbers

The version lives in one place: `[workspace.package] version` in the root `Cargo.toml`. Every
crate inherits it.

- During development: `X.Y.Z-dev` (the project starts at `0.0.1-dev`).
- Release: change to `X.Y.Z`, update `CHANGELOG.md`, commit, tag `vX.Y.Z`, build the zip with
  `scripts/portable.ps1` and attach it to the GitHub release for that tag.
- Right after the release, change to the next `-dev` version (`0.0.2-dev`) in its own commit.

Published tags are never moved or deleted.

**How numbers grow** ([ADR 0027](../decisions/0027-version-scheme.md)). The first public release
on GitHub is `1.0.0` ([ADR 0050](../decisions/0050-portable-and-open-source.md)). The second number
goes up only for releases with something new a user would notice, the third only for fixes
(`1.1.1`), and the first only for a change of era. Gracil and the changelog write `1.1` and
`1.1.1`. The zip is named after the version: `Gracil-1.1.0-x64.zip`.

## Changelog

`CHANGELOG.md` is for people who use Gracil, not for developers:

- one section per released version, newest first, plus a section for the version in development;
- entries grouped under **New**, **Improved** and **Fixed**;
- plain, calm language with no technical terms; describe what the user notices;
- changes with no visible effect (refactors, internal tooling, tests) are not listed.

Technical detail belongs in commit messages and in `docs/`.

## Commits

Small commits that each do one complete thing and pass `scripts/check.ps1`. Work happens on `main`;
use a short-lived branch only when it helps (for example a risky experiment).
