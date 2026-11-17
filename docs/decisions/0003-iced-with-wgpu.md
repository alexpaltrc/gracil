# 0003 — Iced with the WGPU renderer

**Status:** Accepted, built (2026-10-02).

**Context.** Gracil needs a native, small, Rust-first UI with no browser engine. Electron, WebView
and Tauri are excluded by the product specification.

**Decision.** Use Iced with its WGPU renderer, in `daemon` mode so the process can live with zero
windows (tray only) and open the main window and mini-window on demand.

**Consequences.** Iced redraws only in response to events, which matches the idle-usage goals as
long as Gracil never runs a permanent animation or timer subscription. Idle CPU, GPU and memory are
measured from the first UI prototype.
