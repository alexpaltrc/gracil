# Versioning, releases and the changelog

## Version numbers

The version lives in one place: `[workspace.package] version` in the root `Cargo.toml`. Every
crate inherits it.

- During development: `X.Y.Z-dev` (the first public version is developed as `1.0.0-dev`).
- Release: change to `X.Y.Z`, give its section of `CHANGELOG.md` its date, commit, tag `vX.Y.Z`
  and push the tag. GitHub builds the zip from that tag and leaves a draft release with the zip
  and the changelog's notes; the owner publishes it
  ([ADR 0079](../decisions/0079-releases-built-on-github.md)). To try the build without
  releasing anything, run the **release** workflow by hand.
- Right after the release, change to the next `-dev` version (`1.0.1-dev`) in its own commit.

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
