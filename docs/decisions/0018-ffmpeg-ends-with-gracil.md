# 0018 — FFmpeg ends with Gracil, through a job object

**Status:** Accepted.

**Context.** Gracil runs FFmpeg as a separate process (ADR 0002). When Gracil exits normally it
cancels the running compression first, but if Gracil is killed, crashes or is ended from Task
Manager, nothing stops FFmpeg: it keeps compressing in the background for a video nobody will
receive, uses the processor or graphics card, and keeps its temporary file in use so the next
start cannot tidy it away (ADR 0017).

**Decision.** At startup Gracil creates a Windows job object with
`JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`, puts its own process in it and never closes the handle.
Every process Gracil starts afterwards joins the job. Windows closes the handle when Gracil's
process ends, for whatever reason, and that ends every process still in the job.

**Consequences.**

- No FFmpeg or FFprobe survives Gracil, so a sudden close leaves no work running and the leftover
  temporary files are free to remove at the next start.
- If the job object can't be created (for example a parent that forbids it), Gracil still works;
  the fact is logged and only this protection is lost.
- Anything Gracil might start in future that must outlive it (none is planned) would need to be
  started outside the job explicitly.
