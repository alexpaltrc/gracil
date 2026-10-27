# Developing from the cloud

Gracil can be worked on from Claude Code in the cloud, with the computer off, also from a phone
([ADR 0072](../decisions/0072-developing-from-the-cloud.md)).

## Once

1. At [claude.ai/code](https://claude.ai/code), connect GitHub and give Claude access to the
   private `gracil` repository.
2. Create a cloud environment for it. The default network access is enough: it reaches crates.io,
   the Rust toolchain and GitHub. Nothing else needs setting up, since the repository prepares
   each session by itself (below).

## Every session

- Start a session on the `gracil` repository from claude.ai/code or the Claude app on the phone.
- When it starts, `.claude/settings.json` runs `scripts/cloud-setup.sh`: it installs Rust if
  missing, the toolchain in `rust-toolchain.toml` and the crates. Outside the cloud it does
  nothing.
- The session runs on Linux, so it checks with `scripts/check.sh` instead of `scripts/check.ps1`:
  formatting, Clippy and tests of `gracil-core`, and Clippy of `gracil-windows` for Windows.
  `gracil-ui` can't be built there.
- Work goes to a branch and a pull request. GitHub checks it on Windows (`scripts/check.ps1`) and
  on Linux (`scripts/check.sh`); merge only once both pass.

## Back at the computer

Double-click **Actualizar Gracil** on the desktop. It runs `scripts/update.ps1`, which gets what
was merged from the cloud, builds it and updates the Gracil in daily use in `Documents\Gracil`
([ADR 0073](../decisions/0073-always-up-to-date.md), [Packaging](packaging.md)). Changes to the window or to Windows
integration are tried there before they count as done.
