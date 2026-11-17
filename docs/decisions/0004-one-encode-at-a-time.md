# 0004 — One compression at a time

**Status:** Accepted, built (2026-10-02). Superseded by
[ADR 0040](0040-several-files-at-once-without-getting-in-the-way.md) (2026-10-04): several files are
worked on at once, one video with images beside it.

**Context.** FFmpeg and its encoders already use all available CPU cores or the GPU's encode
engine. Running several encodes in parallel mostly competes for the same resources and makes
progress harder to understand.

**Decision.** The scheduler runs a single compression job at a time, in queue order.

**Consequences.** The scheduler stays trivial. If measurements later show a real benefit (for
example many tiny files on NVENC), this can be revisited with data.
*Revisited in [ADR 0040](0040-several-files-at-once-without-getting-in-the-way.md) (2026-10-04).*
