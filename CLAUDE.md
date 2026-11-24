# Gracil — notes for AI assistants

Gracil is a small, free and open-source Windows 11 app that compresses videos and images: drop
them in and it makes them smaller while they look the same. Rust workspace, portable zip, GPL.

## The owner's standard

This project follows its owner's personal standard (the `alex-vibecode` skill), in every
conversation, local or in the cloud, even when nobody mentions it. The cloud can't see the skill,
so its essentials are here:

- **Talking to the owner:** Spanish, short and concise, for someone who doesn't program. Propose
  and ask before any change that could break the project; wait for a clear yes.
- **Priorities, in order:** ultraminimalist, ultra simple (few options, nothing to read or learn,
  the app does what it can by itself), modern, native to Windows 11 (Fluent, Mica, Segoe UI
  Variable, system theme and accent color), fast (measured, not assumed), elegant down to the
  code and the repository.
- **Every interface change** is checked on the real app before it's done: alignment to the pixel,
  no slower or heavier, and as beautiful as before or more. Animations follow springs and
  choreography, respect Windows' **Animation effects** and contrast themes, and are checked frame
  by frame. The check is made in a development build (`cargo run -p gracil-ui`), **Gracil Dev**,
  which runs beside the owner's Gracil: never close theirs to look at a change, never move their
  pointer, and read the window's picture from the window itself, never off their screen
  (ADR 0075).
- **Clean repository:** every file has a reason to exist; no leftover files, empty folders or dead
  code. Material that isn't code (screenshots, brand material, backups) lives outside the repo, in
  `Documents\Vibecode Archivos\Gracil Archivos`.
- **Pull at the start, push at the end.** Every session, local or in the cloud, starts by getting
  the latest from GitHub and ends by pushing its work, so the two never collide. Back at the
  computer, before anything else: pull and merge cloud branches whose CI passed, then say in one
  line what arrived.

Where Gracil deliberately differs from the standard ([ADR 0074](docs/decisions/0074-gracil-and-the-personal-standard.md)):
releases are a portable zip only (the installer is for the owner's computer, ADR 0081); data
beside `gracil.exe`; GPL 3.0 or later; documentation in English; Iced, FFmpeg, jpegli and other
libraries are used.

## Project rules

- The product and engineering specification is `docs/specification.md`. Read the relevant
  sections before changing behavior; section 81 lists what must never change silently.
- Architecture: `docs/architecture/overview.md`. Decisions: `docs/decisions/` (add an ADR rather
  than silently reversing one).
- `gracil-core` must never depend on Iced, WGPU or any windowing crate.
- `unsafe` is denied workspace-wide; only Windows-integration crates may opt in, with documented
  safety comments.
- Versioning, changelog style and commit rules: `docs/development/versioning.md`.
  The changelog is user-facing, plain language, no technical terms.
- Tests that touch the filesystem use fresh temporary directories only.
- **The documentation always says what is true today** (ADR 0080). Every commit that builds,
  changes or drops something searches `docs/`, `README.md` and `CLAUDE.md` for it and updates
  every place that mentions it, including the decision record's **Status** line, its index row,
  and the status of any earlier record it changes. Only decision records keep history; the
  rest describe the present. *Planned*, *proposed*, *later*, *for now* or "(in progress)" only
  for what is truly not built. Nothing that ages by itself (current version, list of releases).
  Before every release, those words are searched for and each one confirmed or fixed.
- `docs/how-gracil-works.md` is the user-facing explanation of how Gracil works. Update it in the
  same commit as any change to behavior a user could notice or ask about. Plain language, no
  jargon; mark decided-but-unbuilt behavior "(in progress)" and remove the mark once it ships.
- When something goes wrong, Gracil shows it quietly in the interface instead of opening a dialog
  (spec §2.3).
- Commits are authored by Alex Palacios with GitHub's private address,
  `324600491+alexpaltrc@users.noreply.github.com`; never a personal email.

## Layout and commands

| Path | Contents |
|---|---|
| `crates/gracil-core` | The compression engine: pure Rust, also built and tested on Linux |
| `crates/gracil-ui` | The app: windows, tray, mini-window (Iced + WGPU, DX12) |
| `crates/gracil-windows` | Windows integration; the only crate allowed `unsafe` |
| `scripts/` | `check.ps1` / `check.sh` (checks), `portable.ps1` (the zip, with the FFmpeg pinned in `ffmpeg.psd1`), `installer.ps1` / `installer.iss` (the owner's installer, from a release), `update.ps1` (the former daily copy, not run, ADR 0081), `build-jpegli.ps1`, `licenses.ps1`, `icons.ps1`, `release-notes.ps1` (a release's notes from the changelog), `cloud-setup.sh` (prepares a cloud session) |
| `.github/workflows/` | `check.yml` (checks on Windows and Linux), `release.yml` (a release's zip, from a pushed tag) |
| `docs/` | Specification, architecture, decisions, development guides, user explanations |

- **Check:** `scripts/check.ps1` before every commit; Clippy warnings are errors.
- **Releases** are built by GitHub from a pushed tag and left as a draft; only the owner
  publishes them, and no release is built or kept on this computer (ADR 0079).
- **The owner's Gracil** is installed in `%LOCALAPPDATA%\Programs\Gracil` and changes only with
  a release: once the owner publishes one, make its installer with
  `scripts/installer.ps1 -Version X.Y.Z` and run it over the installed Gracil (ADR 0081). Gracil
  Dev, an installed copy of its own, is planned and not built; until then changes are tried only
  in the development build, and `scripts/update.ps1` is not run: the Gracil it makes in
  `Documents\Gracil` would take Send to and the start with Windows from the installed one.
- **Cloud (Linux):** run `scripts/check.sh` instead. It checks formatting, Clippy and the tests
  of `gracil-core`, and Clippy of `gracil-windows`; `gracil-ui` can't be built there. Work on a
  branch, open a pull request and say in it that changes to `gracil-ui` or `gracil-windows` are
  not yet tried on Windows (`docs/development/cloud.md`, ADR 0072). GitHub's CI checks Windows and
  Linux; a change isn't done while it fails.
