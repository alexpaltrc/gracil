# 0072 — Developing from the cloud

**Status:** Accepted, built (2026-10-27). Decided by the product owner.

**Context.** The product owner wants to keep developing Gracil when their computer is off or
away, from Claude Code in the cloud, also from a phone. The code is on GitHub as a private
repository for that. Cloud sessions run on Linux, while Gracil is a Windows app: `gracil-ui` and
`gracil-windows` build only for Windows, and the app can't be opened there. `gracil-core` is
plain Rust and builds anywhere.

**Decision.**

1. **The toolchain is named in the repository.** `rust-toolchain.toml` asks for stable Rust with
   Clippy, rustfmt and the Windows x64 target, so any machine, cloud or not, gets the same tools
   by itself.
2. **A cloud session prepares itself.** `.claude/settings.json` runs `scripts/cloud-setup.sh` when
   a session starts; it does nothing outside the cloud. In the cloud it installs Rust if missing,
   the toolchain above, and downloads the crates.
3. **What the cloud checks.** `scripts/check.sh` runs on Linux: formatting of everything, Clippy
   (warnings are errors) and tests of `gracil-core`, and Clippy of `gracil-windows` for Windows,
   without building it. `gracil-ui` can't be checked there.
4. **Windows has the last word.** GitHub Actions runs `scripts/check.ps1` on Windows for every
   push and pull request, and `scripts/check.sh` on Linux, so the cloud's script stays true. Work
   from the cloud goes to a branch and a pull request; it reaches `main` only once both pass.
5. **Trying the app** still needs Windows: a change made in the cloud is opened on the
   maintainer's computer, after `git pull`, before it is called done.

**Consequences.** Gracil can be changed, checked and reviewed with the computer off. Changes to
the interface and to Windows integration are only checked by GitHub's Windows runner until they
are tried on a Windows computer, so they are marked as not yet tried in their pull request.
