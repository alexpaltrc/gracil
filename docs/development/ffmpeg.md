# FFmpeg

How Gracil finds, installs and drives FFmpeg. Decision record: [ADR 0008](../decisions/0008-ffmpeg-detected-or-provisioned.md).

## Finding it

`toolchain::resolve` runs at startup, off the UI thread:

1. Every folder on `PATH` holding both `ffmpeg.exe` and `ffprobe.exe` (winget, Scoop, manual
   installs). The first one whose `-encoders` listing includes `libx265` wins.
2. Gracil's private copy in `%LOCALAPPDATA%\Gracil\ffmpeg\<build id>\`.
3. Otherwise the pinned build is downloaded and installed there, silently.

`toolchain::find` does steps 1–2 only; tests use it so they never download anything.

## The pinned build

| | |
|---|---|
| Build | Gyan.dev `ffmpeg-9.0.2-essentials` (GPL) |
| Source | `https://github.com/GyanD/codexffmpeg/releases/download/9.0.2/ffmpeg-9.0.2-essentials_build.zip` |
| Size | 114,768,076 bytes |
| SHA-256 | `60f467265b1e312373dbcd92200c2618a74850f98d3d078e94296bb3fa2047ba` |

The archive is fetched with Windows' own `curl.exe` (HTTPS only), checked against size and hash,
and only `bin/ffmpeg.exe` and `bin/ffprobe.exe` are extracted with Windows' `tar.exe`. The folder
appears in one rename, and copies of older pinned builds are removed afterwards.

To move to a new build: download it, record its size and SHA-256 in `PINNED_BUILD`
(`crates/gracil-core/src/toolchain/provision.rs`) and here, give it a new `id`, and run the
integration tests with that copy first on `PATH`.

**Licensing.** The essentials build is GPL-licensed. Gracil runs it as a separate program and
does not link to it. Gyan's builds are distributed with their sources and license texts.
Before release, the About screen and the Store listing must credit FFmpeg, state its license and
point to its source.

## Capabilities

Detected once per process (`Capabilities::detect`):

- **NVENC**: `hevc_nvenc` is listed *and* a three-frame test encode succeeds. A listed encoder
  says nothing about the GPU or the driver.
- **NVENC tier** (`standard::NvencLevel`, ADR 0028): the test encode is run with each tier's
  options, best first, and the first that succeeds is used. `Full` is 10-bit with lookahead,
  B-frame references and multipass; `Basic` drops the B-frame settings; `Legacy` is the original
  8-bit p6 setting. No tier uses adaptive quantization or the temporal filter (ADR 0029).
- **CUDA decoding**: NVENC works and `-hwaccels` lists `cuda`.
- **Filters**: `-filters` lists `scale_cuda` (resizing frames left on the GPU) and `libvmaf` (the
  quality search). The essentials build has both. `libvmaf_cuda` with `hwupload_cuda` (VMAF on the
  GPU, ADR 0030) is used when present; no public Windows build has it.
- **x265 10-bit**: `-h encoder=libx265` lists `yuv420p10le`.
- Without NVENC, `libx265` on the CPU does the encoding.

## One compression

`compress::compress` probes the source, reserves `<name>.gracil-<id>.part` next to it, then tries
`plan::attempts` in order:

| Order | Encoder | Decoding | Streams kept |
|---|---|---|---|
| 1 | NVENC | CUDA | all MP4 can hold |
| 2 | NVENC | software | all MP4 can hold |
| 3 | NVENC | software | video + AAC audio |
| 4 | x265 | software | video + AAC audio |

Without NVENC it is x265 with all streams, then x265 essential. A failed attempt moves on to the
next one only if it used CUDA or got nowhere. A failure partway through a software decode means
the source is damaged at that point, and the next attempt would stop there too.

The output is accepted when FFprobe reads it, it holds HEVC video, and its length matches the
source within 1% or one second.

An attempt that decodes on the GPU is skipped when the video must be resized and `scale_cuda` is
missing.

### Settings (ADR 0028)

- NVENC: `hevc_nvenc` with the detected tier's options (`standard::NvencLevel::options`) and
  `-cq <level>`. With `Full`:
  `-preset p7 -tune hq -rc vbr -b 0 -profile main10 -highbitdepth 1 -rc-lookahead 32 -bf 4
  -b_ref_mode middle -multipass fullres`.
- x265: `-preset medium -x265-params log-level=error -crf <level>`, 10-bit when supported.
- HEVC is tagged `hvc1` so Apple devices and Windows players accept it.
- **Size and frame rate** (`standard::Shape`): a short side above 1080 is scaled to 1080 (Lanczos,
  even dimensions); an average above 30.5 fps becomes 30, or 30000/1001 for the NTSC family.
  Smaller or slower videos are left as they are. Filters go per stream (`-filter:<out>`):
  `fps`, then `scale_cuda` on GPU frames or `scale` plus `format=p010le`/`yuv420p10le`/`yuv420p`
  on CPU frames. GPU frames need no format filter; `-highbitdepth 1` handles 10-bit.

### Quality search (`search.rs`)

Before the first attempt, Gracil looks for the highest level (smallest file) whose VMAF reaches
95: NVENC CQ 24–38, x265 CRF 20–34. Between a level that reaches the target and one that misses
it, the next guess is on a straight line through their scores; with only a level that reaches it,
the search steps past it by the score it has to spare (about a point per level); otherwise it
halves what is left. Each step encodes 4 s samples, one per 15 s of video up to four, centred in
equal shares of the video, and averages their scores. Every measured level is cached for the
whole search. One FFmpeg process per sample does both jobs, writing nothing to disk:

```text
-ss START -t 4 -i file:SRC
-filter_complex "[0:N]fps…,scale…,split[source][reference];[source]format=…[encode];[reference]format=yuv420p[ref]"
-map [encode] <encoder options at level> -stats_enc_post:0 pipe:1 -stats_enc_post_fmt:0 "{n} {size}" -f null -
-dec 0:0 -filter_complex "[dec:0]format=yuv420p[encoded];[encoded][ref]libvmaf=n_threads=…:pool=harmonic_mean:n_subsample=2[vmaf]"
-map [vmaf] -f null -
```

The loopback decoder (`-dec 0:0`) decodes what the encoder just produced. GPU decoding is used
without `-hwaccel_output_format`, so both branches get ordinary frames; if it fails, the sample is
retried on the CPU. The last `VMAF score:` line in the error output is the score; standard output
lists every encoded frame's size.

VMAF measures every other frame (`n_subsample=2`, ADR 0030). With `libvmaf_cuda`, the input is
decoded with `-hwaccel_output_format cuda` on a named device (`-init_hw_device cuda=gpu
-filter_hw_device gpu -hwaccel_device gpu`), the encoder takes the CUDA frames as they are, the
reference is `scale_cuda=format=yuv420p`, and the encoded sample is decoded on the CPU and sent
back with `format=yuv420p,hwupload_cuda` to `libvmaf_cuda` (`-dec` takes no `-hwaccel`).

**Predicted size (ADR 0029).** The samples' average frame size without their first frame, times
the video's frame count, plus their average first (key) frame once per 250 frames, plus every
stream that is not re-encoded at its size in the source. A level that reaches 95 is used only if
it is predicted to be at most 80% of the original; the search gives up early once a level that
misses 95 is already predicted above 80%.

**Already compressed.** Otherwise nothing is encoded and `find_level` returns `Found::Lighter`
with up to two choices at VMAF ≥ 90 (NVENC CQ 24–42, x265 CRF 20–38): the video's own size, and
one size down (short side 1080 → 720 → 480, never below 480, scored against the source scaled to
that same size). Each is kept only if predicted at most 80% of the original, and the smaller size
only if lighter than the first. `compress_with` then encodes the user's pick without searching
again.

The search is skipped (fixed level: CQ 27, CRF 26) for videos under 6 s, of unknown length, under
60 s with x265, or without `libvmaf`, and whenever a measurement fails. If no level reaches 95, the
best quality in the range is used. Attempts with a different encoder than the first use their
fixed level. Cancelling stops the search. Every measured level, its score and predicted video size
are logged, as are the choice and the lighter choices.

### Keeping what matters (spec §41)

- **Container**: always MP4 (ADR 0012). `.mp4` keeps its spelling; every other extension becomes
  `.mp4`.
- **Rotation**: `-noautorotate` keeps the pixels as recorded and copies the rotation flag.
- **Metadata**: `-map_metadata 0` is required, because without it FFmpeg drops
  `creation_time`. Per-stream metadata such as language is mapped one stream at a time.
  Chapters are copied.
- **iPhone keys** (`com.apple.*`) need `-movflags +use_metadata_tags`. That flag also writes
  junk keys and can hide the title from Explorer, so it is only used when such keys exist.
- **Streams**: audio is copied when the container accepts it, otherwise converted to AAC
  (64 kbps per channel, 128–512). Lossless audio (PCM, FLAC, ALAC) and tracks above twice that
  AAC bitrate are converted too (ADR 0028). Text subtitles (SRT, ASS, WebVTT) become `mov_text`. Cover art
  is copied. Anything MP4 cannot hold (bitmap subtitles, data tracks, fonts) is left out and
  logged, never refused.

### Safety

- Paths go to FFmpeg as `file:<path>` arguments. A name never becomes a protocol or a shell
  string, and no shell is involved.
- Every process starts with `CREATE_NO_WINDOW` and a null stdin.
- Progress comes from `-progress pipe:1`. Error output is drained on its own thread, and its
  last 4 KB goes to the log.
