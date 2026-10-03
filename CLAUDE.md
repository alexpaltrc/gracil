# Gracil — notes for AI assistants

- The product and engineering specification is `docs/specification.md`. Read the relevant
  sections before changing behavior; section 81 lists what must never change silently.
- Architecture: `docs/architecture/overview.md`. Decisions: `docs/decisions/` (add an ADR rather
  than silently reversing one).
- Run `scripts/check.ps1` before every commit; Clippy warnings are errors.
- `gracil-core` must never depend on Iced, WGPU or any windowing crate.
- `unsafe` is denied workspace-wide; only Windows-integration crates may opt in, with documented
  safety comments.
- Versioning, MSIX mapping, changelog style and commit rules: `docs/development/versioning.md`.
  The changelog is user-facing, plain language, no technical terms.
- Tests that touch the filesystem use fresh temporary directories only.
