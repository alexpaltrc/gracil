# Developing from the cloud

Gracil can be worked on from Claude Code in the cloud, with the computer off, also from a phone
([ADR 0072](../decisions/0072-developing-from-the-cloud.md)).

## Once

1. At [claude.ai/code](https://claude.ai/code), connect GitHub and give Claude access to the
   `gracil` repository.
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

Get `main` and merge the cloud branches whose checks passed. Changes to the window or to Windows
integration are tried in the development build before they count as done
([ADR 0075](../decisions/0075-a-development-build-of-its-own.md)), and then reach Gracil Dev
with `scripts/update.ps1` ([ADR 0082](../decisions/0082-gracil-dev-installed.md)). The owner's
installed Gracil changes only with a release ([ADR 0081](../decisions/0081-the-owners-gracil-installed.md),
[Packaging](packaging.md)).
