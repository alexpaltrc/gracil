# Versioning, releases and the changelog

## Version numbers

The version lives in one place: `[workspace.package] version` in the root `Cargo.toml`. Every
crate inherits it.

- During development: `X.Y.Z-dev` (the project starts at `0.0.1-dev`).
- Release: change to `X.Y.Z`, update `CHANGELOG.md`, commit, tag `vX.Y.Z`.
- Right after the release, change to the next `-dev` version (`0.0.2-dev`) in its own commit.

Published tags are never moved or deleted.

## MSIX version mapping

MSIX requires four numeric parts, and the Microsoft Store requires the last one to be `0`. Gracil
maps its versions like this:

| Gracil version | MSIX version |
|---|---|
| `X.Y.Z` (release) | `X.Y.Z.0` |
| `X.Y.Z-dev` | `X.Y.Z.0` |

Development and release packages of the same version share an MSIX version. Development packages
are only installed locally and are replaced with `Add-AppxPackage -ForceUpdateFromAnyVersion`, so
this never reaches users. The packaging script derives the MSIX version from `Cargo.toml`; it is
never typed by hand.

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
