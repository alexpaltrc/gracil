# 0030 — Faster quality measurement: every other frame, and on the GPU when FFmpeg can

**Status:** Accepted. Decided by the product owner (2026-10-03). Amends ADR 0028's search.

**Context.** On the product owner's machine (RTX 5060, Ryzen 5 5600G, 6 cores / 12 threads),
each 4 s Full HD sample took about 2.2 s, and used about 15 s of processor time: about 1.2 s and
3 s of it to decode and encode on the GPU, the rest VMAF (`libvmaf`) on the CPU. The CPU was the
bottleneck: running four samples at once instead of one after another saved only 15%. An already
compressed video spent up to a minute and a half getting ready.

No public FFmpeg build for Windows (gyan.dev, BtbN, which FFmpeg's site links to) has VMAF on the
GPU (`libvmaf_cuda`). It needs libvmaf built with NVIDIA's CUDA compiler.

**Decision.**

1. **Every other frame.** VMAF measures every second frame of each sample (`n_subsample=2`).
   Measured on three samples, the score moved by 0.07–0.2 points; each sample took about a fifth
   less time and a third less processor. The thresholds (95, 90) stay.
2. **On the GPU when FFmpeg can.** When FFmpeg lists `libvmaf_cuda`, `scale_cuda` and
   `hwupload_cuda` and decodes with CUDA, the samples never leave the GPU: the source stays in
   CUDA frames for the encoder and the reference, and the encoded sample, decoded on the CPU, is
   uploaded and compared there. The same chain, tested with a stand-in GPU filter, took 1.1 s and
   1.7 s of processor per sample. If a sample fails that way it is retried on the CPU, as GPU
   decoding already was. Elsewhere (other GPUs, FFmpeg builds without it) the CPU measures, as
   before.
3. **Gracil does not ship its own FFmpeg build with `libvmaf_cuda`.** Building, hosting and
   keeping it current and licensed costs more than it saves; the product owner declined it.
   Builds of unknown origin are not used.

**Consequences.**

- Getting ready is about a fifth quicker everywhere, and leaves the processor freer.
- The GPU path only runs with an FFmpeg that has `libvmaf_cuda`, which today means one the user
  built or installed themselves; it is written against FFmpeg's documented filters and checked
  with a stand-in, not with `libvmaf_cuda` itself.
- Each machine uses what it has: NVENC and CUDA when the GPU offers them, the CPU otherwise.

*Update (2026-10-14):* looked at again, with the same answer: Gracil keeps measuring on the CPU
with the bundled build, and still uses `libvmaf_cuda` on its own whenever the bundled FFmpeg has it.

- No reputable Windows build has `libvmaf_cuda`. BtbN declined it several times because of the
  CUDA SDK's license (https://github.com/BtbN/FFmpeg-Builds/issues/328). Neither gyan.dev
  (https://www.gyan.dev/ffmpeg/builds/) nor jellyfin-ffmpeg has it. Netflix and NVIDIA give no
  Windows support, and community attempts failed
  (https://forums.developer.nvidia.com/t/ffmpeg-vmaf-cuda-windows-support/348373).
- FFmpeg itself is not the obstacle. Its `libvmaf_cuda`, `scale_cuda` and `hwupload_cuda` need
  only `ffnvcodec` (and clang for `scale_cuda`), not `--enable-nonfree`
  (https://ffmpeg.org/pipermail/ffmpeg-devel/2023-August/313645.html).
- The obstacle is libvmaf's GPU code. Built with NVIDIA's compiler and headers, it falls where
  FFmpeg and BtbN treat a build as not redistributable
  (https://docs.nvidia.com/cuda/pdf/EULA.pdf). Built with clang alone, it is probably
  redistributable but uncertain, its build still calls a CUDA Toolkit tool (`bin2c`), and one
  attempt was reported as not fully correct.
- Building our own would be an ongoing project: a clang-only build, GPU scores checked against
  CPU scores, and a rebuild with every FFmpeg update, with the license still uncertain. The
  speed round (ADR 0065) shortens the CPU measurement instead.
- If BtbN ever ships `libvmaf_cuda`, updating the bundled FFmpeg is enough: no code changes.
