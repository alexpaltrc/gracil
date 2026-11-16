# 0077 — Where things live in the repository

**Status:** Accepted, built (2026-11-16). Decided by the product owner, who asked for the whole
project folder to be reviewed: nothing in it without a reason, and nothing in two places.

**Context.** The repository was clean, with a few leftovers of how it grew: three folders deep
for two icon files that once had drawings beside them, a folder of its own for one ten-line
file, a module folder holding a single file, two ways of laying out a module with parts, and
zips of older versions left beside each new one.

**Decision.**

1. **A crate holds what only it uses.** The two icon files are in `crates/gracil-ui/icons`,
   beside the `gracil.rc` that links them in; `assets/` is gone.
2. **A script's data is beside the script.** The pinned FFmpeg is `scripts/ffmpeg.psd1`, read by
   `scripts/portable.ps1`; `packaging/` is gone.
3. **One way to lay out a module.** A module with parts is a folder with `mod.rs`; a module
   without is one file. So `toolchain.rs` in `gracil-core`, and `tray/mod.rs` in
   `gracil-windows`.
4. **`scripts/portable.ps1` leaves one zip**, the one it just made.
5. **The root holds only what must be there**: the three files Cargo reads (`Cargo.toml`,
   `Cargo.lock`, `rust-toolchain.toml`), `clippy.toml` and `.cargo/` for how Gracil is built,
   the five documents GitHub shows (`README`, `LICENSE`, `CHANGELOG`, `CONTRIBUTING`, `CLAUDE`),
   and `crates/`, `docs/`, `scripts/`, `.github/` and `.claude/`.

**What was looked at and left.** Two files share a name in two crates (`appear.rs`,
`numbers.rs`): each is that crate's side of one subject, the interface's and Windows', and they
share no code. Earlier decision records name the old paths; they are records of their day and
stay as written. Material that isn't code lives outside the repository
([ADR 0074](0074-gracil-and-the-personal-standard.md)).

**Consequences.** Spec §7 shows the layout as it is now.
