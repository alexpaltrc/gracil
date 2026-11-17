# 0053 — Ready for GitHub: nothing left from the Store, and no FFmpeg download

**Status:** Accepted, built (2026-10-05); item 4 replaced by item 5 of
[ADR 0054](0054-wholly-portable.md). Changes [0008](0008-ffmpeg-detected-or-provisioned.md) and
[0050](0050-portable-and-open-source.md).

**Context.** Before the first public release on GitHub, the product owner asked for everything
tied to the Microsoft Store, and everything else no longer needed, to be removed from the
repository and the code: "what we left out stays out". After ADR 0050 three kinds of leftovers
remained. The decision records of the MSIX package (0005, 0023, 0026) and notes about it across
the specification and other records. The Piedra design delivery in `docs/design/`, retired by
[0031](0031-windows-11-colors.md). And the FFmpeg download of ADR 0008: with FFmpeg inside the zip
it only ran if the `ffmpeg` folder was deleted, yet it kept a downloader, a pinned archive with
its hash, a hashing library and a message about needing the internet in a program that promises
never to use it.

**Decision.**

1. **No FFmpeg download.** Gracil uses the `ffmpeg` folder beside `gracil.exe`, or, while
   developing, an FFmpeg on `PATH`. Without a usable one, the waiting files show a quiet line
   saying Gracil is missing part of itself and should be downloaded again. `provision.rs`, the
   pinned download build and the `sha2` dependency are gone, and `Engine::start` returns an
   `Option`.
2. **The Store is gone from the repository.** ADRs 0005, 0023 and 0026 are deleted; their rows
   stay in the index, marked removed, so their numbers are never reused. The specification and
   the other records describe the portable zip as the way Gracil is, without notes about the
   package that was planned.
3. **`docs/design/` is gone.** The icon's artwork stays in `assets/icons/`, drawn by
   `scripts/icons.ps1`; the retired palette and its presentation live on in Git's history only.
4. **One Gracil in daily use.** `scripts/portable.ps1 -Copy` builds the zip and copies it to
   `Documents\Gracil` (first `%LOCALAPPDATA%\Programs\Gracil`, until
   [ADR 0054](0054-wholly-portable.md)), closing that Gracil first and keeping its `data` folder.
   That copy is the one the product owner runs every day, also while Gracil is being developed;
   `-CopyTo <folder>` puts it elsewhere. Development builds in `target` keep their own data.
5. **Whole-program optimization.** Release builds link with full LTO instead of thin: measured,
   `gracil.exe` goes from 13.8 MB to 13.3 MB, and the code can only get as fast or faster; only
   the release build takes longer. `panic = "abort"` was not taken: a panic in a worker thread
   would close the whole of Gracil instead of failing one file.

**Consequences.** Gracil never touches the internet, with no exception. A damaged `ffmpeg` folder
is fixed by downloading Gracil again rather than silently. The repository holds only what the
zip and its development need.
