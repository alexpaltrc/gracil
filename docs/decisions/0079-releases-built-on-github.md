# 0079 — Releases are built on GitHub

**Status:** Accepted, built (2026-11-16). Decided by the product owner, who made it part of
their personal standard for every project. Changes how a release is made
(`docs/development/versioning.md`); what is released is unchanged
([ADR 0050](0050-portable-and-open-source.md)).

**Context.** The zip of a release was built on the owner's computer and uploaded by hand. It
worked, and it had three weak points: what people download came from one personal machine, with
whatever was installed on it (another FFmpeg on its `PATH` once broke the build); nobody else
could tell that the zip came from the public code; and a release needed that computer.

**Decision.**

1. **`.github/workflows/release.yml` builds the zip** on a clean Windows machine of GitHub's,
   from the tagged code alone: it fetches the pinned FFmpeg from the address in
   `scripts/ffmpeg.psd1`, builds jpegli, and runs `scripts/portable.ps1`, which still refuses
   any FFmpeg but the pinned one by its hashes.
2. **A pushed tag `vX.Y.Z` ends in a draft release**, with the zip attached and that version's
   section of `CHANGELOG.md` as its notes (`scripts/release-notes.ps1`). The tag must be the
   version in `Cargo.toml`, or the run stops. **The owner publishes the draft**; nothing is
   published by itself.
3. **Run by hand, it is a trial**: the zip is kept with the run for a few days and no release is
   made. A change to how Gracil is packaged is tried this way first.
4. **The pinned FFmpeg is the one that stays downloadable**: the month's last build of 31 July
   2026, of the same source commit as the copy Gracil had been tested with, which was a daily
   build no longer kept anywhere. Every test and a real video gave the same results with it.
5. **No release is kept on the owner's computer.** `scripts/portable.ps1` is still what builds
   the Gracil in daily use there (ADR 0073), which is for trying changes at once.

**Consequences.** A release takes about a quarter of an hour on GitHub rather than three minutes
here, and can be made from anywhere. It depends on the pinned FFmpeg staying where it is: the
builder keeps each month's last build for about two years, so a new one is pinned well before
then (`docs/development/ffmpeg.md`).
