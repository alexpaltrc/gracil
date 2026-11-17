# 0001 — The compression core does not depend on the UI

**Status:** Accepted, built (2026-10-02).

**Context.** Compression can start from File Explorer with no window at all. Creating an Iced
window or a WGPU device costs memory, GPU work and startup time that a background job doesn't
need.

**Decision.** `gracil-core` has no dependency on Iced, WGPU or any windowing crate. It exposes jobs,
the scheduler and progress as plain Rust types. The UI observes it; it never contains it.

**Consequences.** Core logic is testable without a display. Background mode initializes the UI
layer lazily, only when a window must be shown.
