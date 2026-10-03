# 0004 — One compression at a time

**Status:** Accepted

**Context.** FFmpeg and its encoders already use all available CPU cores or the GPU's encode
engine. Running several encodes in parallel mostly competes for the same resources and makes
progress harder to understand.

**Decision.** The scheduler runs a single compression job at a time, in queue order.

**Consequences.** The scheduler stays trivial. If measurements later show a real benefit (for
example many tiny files on NVENC), this can be revisited with data.
