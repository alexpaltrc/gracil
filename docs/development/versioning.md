# Versioning, releases and the changelog

## Version numbers

The version lives in one place: `[workspace.package] version` in the root `Cargo.toml`. Every
crate inherits it.

- During development: `X.Y.Z-dev` (after `1.0.0`, `1.0.1-dev`).
- Before a release, a pass over the documentation: search it for *planned*, *proposed*,
  *later*, *for now*, *(in progress)*, *not built*, *not yet* and *untried*, and confirm each is
  still true or fix it ([ADR 0080](../decisions/0080-documentation-always-true.md)).
- Release: change to `X.Y.Z`, give its section of `CHANGELOG.md` its date, commit, tag `vX.Y.Z`
  and push the tag. GitHub's **release** workflow (`.github/workflows/release.yml`) stops if the
  tag isn't the version in `Cargo.toml`, builds the zip from that tag and leaves a draft release
  with the zip and that version's section of the changelog as its notes
  (`scripts/release-notes.ps1`); the owner publishes it
  ([ADR 0079](../decisions/0079-releases-built-on-github.md)). To try the build without
  releasing anything, run the **release** workflow by hand: the zip is kept with the run and no
  release is made.
- Once the owner publishes it, install it on their computer with its installer
  (`scripts/installer.ps1 -Version X.Y.Z`, [Packaging](packaging.md)).
- Right after the release, change to the next `-dev` version (`1.0.1-dev`) in its own commit.

Published tags are never moved or deleted.

**How numbers grow** ([ADR 0027](../decisions/0027-version-scheme.md)). The first public release
on GitHub was `1.0.0`, on 16 November 2026
([ADR 0050](../decisions/0050-portable-and-open-source.md)). The second number
goes up only for releases with something new a user would notice, the third only for fixes
(`1.0.1`), and the first only for a change of era. Gracil and the changelog write `1.1` and
`1.0.1`. The zip is named after the version: `Gracil-1.0.0-x64.zip`.

## Changelog

`CHANGELOG.md` is for people who use Gracil, not for developers:

- one section per released version, newest first, plus a section for the version in development;
- entries grouped under **New**, **Improved** and **Fixed**;
- plain, calm language with no technical terms; describe what the user notices;
- changes with no visible effect (refactors, internal tooling, tests) are not listed.

Technical detail belongs in commit messages and in `docs/`.

## Commits

Small commits that each do one complete thing and pass `scripts/check.ps1`. Work on this computer
happens on `main`; use a short-lived branch only when it helps (for example a risky experiment).
Work from the cloud goes to a branch and a pull request, merged once GitHub's checks pass
([Developing from the cloud](cloud.md)).
