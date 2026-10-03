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
- **CUDA decoding**: NVENC works and `-hwaccels` lists `cuda`.
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

### Settings

- NVENC (spec §21): `hevc_nvenc -preset p6 -tune hq -rc vbr -cq 27 -b:v 0`.
- x265: `-preset medium -crf 26`.
- HEVC is tagged `hvc1` so Apple devices and Windows players accept it.

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
  (64 kbps per channel, 128–512). Text subtitles (SRT, ASS, WebVTT) become `mov_text`. Cover art
  is copied. Anything MP4 cannot hold (bitmap subtitles, data tracks, fonts) is left out and
  logged, never refused.

### Safety

- Paths go to FFmpeg as `file:<path>` arguments. A name never becomes a protocol or a shell
  string, and no shell is involved.
- Every process starts with `CREATE_NO_WINDOW` and a null stdin.
- Progress comes from `-progress pipe:1`. Error output is drained on its own thread, and its
  last 4 KB goes to the log.
