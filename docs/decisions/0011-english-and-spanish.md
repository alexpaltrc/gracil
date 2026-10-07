# 0011 — Gracil speaks English and Spanish, following Windows

**Status:** Accepted.

**Superseded in part** by [ADR 0058](0058-explorer-send-to-and-main-menu.md) (2026-10-07):
`gracil-shell` is gone. File Explorer's main menu takes its words from Gracil's own tables, in the
Windows language of the moment it is turned on.

**Context.** The product owner wants Gracil available in English and Spanish from the first
version. Gracil shows very few words, and a language setting would be one more thing to think
about (§66).

**Decision.**

1. The interface follows the Windows display language: Spanish for any Spanish variant, English
   for everything else. There is no language setting.
2. Every word the interface shows lives in one place, `gracil-ui/src/text.rs`, with one table per
   language. Screens ask for text; they never spell it out. `gracil-core` stays free of
   user-facing wording.
3. Spanish wording is neutral Latin American ("videos", "agregar"), short, and as plain as the
   English.
4. `GRACIL_LANGUAGE=en` or `es` overrides the detection, for development only.

**Consequences.**

- Adding a language later means adding one table; a missing entry fails to compile, so no screen
  can show a half-translated state.
- File names Gracil creates keep the `Gracil-` prefix in every language, so they sort the same
  everywhere.
- File Explorer's menu runs outside Gracil's window, so it keeps its own English and Spanish
  wording (`gracil-shell`'s `selection.rs`) and follows the same Windows language.
