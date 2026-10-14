# FFmpeg

How Gracil finds and drives FFmpeg. Decision records:
[ADR 0008](../decisions/0008-ffmpeg-detected-or-provisioned.md),
[ADR 0050](../decisions/0050-portable-and-open-source.md),
[ADR 0053](../decisions/0053-ready-for-github.md).

## Finding it

`toolchain::find` runs at startup, off the UI thread:

1. The copy in Gracil's zip, the `ffmpeg` folder beside `gracil.exe` (`Origin::Bundled`).
2. While developing, every folder on `PATH` holding both `ffmpeg.exe` and `ffprobe.exe`
   (`Origin::System`).

The first one whose `-encoders` listing includes `libx265` wins. Gracil never downloads FFmpeg;
without a usable copy it shows a quiet line asking to download Gracil again.

## The bundled build

`packaging/ffmpeg.psd1` pins the build that goes in the zip, by the SHA-256 of `ffmpeg.exe` and
`ffprobe.exe`: BtbN's `N-125875-g5d4d3bdc61-20260731 win64-gpl`, the build Gracil was tested with
(it has `libvmaf`, NVENC, Quick Sync, AMF, `libx265` and SVT-AV1). `scripts/portable.ps1` refuses
any other. To move to a new build, put its `bin` folder first on `PATH` or pass it with
`-FFmpeg`, run the integration tests with it, and record its name, source commit and hashes in
`ffmpeg.psd1`.

**Licensing.** The build is GPL version 3 or later. Gracil runs it as a separate program and does
not link to it. The zip carries FFmpeg's license and `ffmpeg-source.txt`, which names the build
and points to FFmpeg's source commit and to the builder's scripts with every library's source.

## Capabilities

Detected once per process (`Capabilities::detect`), never kept from one run to the next. The
listings (`-hwaccels`, `-filters`, `-h encoder=libx265`, `-codecs`) and each listed card
encoder's test encodes run at the same time, on scoped threads; each card's tiers are still tried
best first, and the first card in the order below whose test passed is used (ADR 0065).

- **Card encoder** (ADR 0052): `hevc_nvenc`, then `hevc_qsv`, then `hevc_amf`; the first that is
  listed *and* passes a three-frame test encode is used. A listed encoder says nothing about the
  card or the driver. The test feeds the same pixel format and a quality level as real encodes.
- **Tier** (`standard::HardwareLevel`, ADR 0028, ADR 0052): the test encode is run with each
  tier's options, best first, and the first that succeeds is used. For NVENC, `Full` is 10-bit
  with lookahead, B-frame references and multipass; `Basic` drops the B-frame settings; `Legacy`
  is the original 8-bit p6 setting. No tier uses adaptive quantization or the temporal filter (ADR 0029).
- **Card decoding** (`card_decode`): for NVENC, `-hwaccels` lists `cuda`. For Quick Sync and AMF
  (ADR 0065), `-hwaccels` lists `qsv` or `d3d11va`, the conversion filter is listed (`scale_qsv`),
  and a one-frame test passes: an H.264 frame made on the processor with `libx264` is piped into
  a second FFmpeg that decodes it on the card, keeps the frames there and encodes them with the
  chosen tier. On a computer without that card the test fails at once and nothing changes.
- **Filters**: `-filters` lists `libvmaf` (the quality search); the bundled build has it.
  `libvmaf_cuda` with `scale_cuda` and `hwupload_cuda` (VMAF on the GPU, ADR 0030) is used when
  present; no public Windows build has it.
- **x265 10-bit**: `-h encoder=libx265` lists `yuv420p10le`.
- Quick Sync tiers: `Full` is 10-bit `veryslow` with `extbrc 1`, `look_ahead_depth 40` and
  adaptive frame types; `Basic` is 10-bit `veryslow`; `Legacy` is 8-bit `slow`. AMF: `Full` and
  `Basic` are 10-bit (`profile main10`, `bitdepth 10`), `usage transcoding`, `quality quality`,
  `rc cqp`; `Legacy` is the same in 8-bit.
- Without a card encoder, `libx265` on the CPU does the encoding.

## One compression

`compress::compress` probes the source, reserves `<name>.gracil-<id>.part` next to it, then tries
`plan::attempts` in order:

| Order | Encoder | Decoding | Streams kept |
|---|---|---|---|
| 1 | NVENC | CUDA | all MP4 can hold |
| 2 | NVENC | software | all MP4 can hold |
| 3 | NVENC | software | video + AAC audio |
| 4 | x265 | software | video + AAC audio |

With Quick Sync or AMF it is the card (software decoding) with all streams, then essential, then
x265 essential, after a first attempt decoding on the card where its test passed (ADR 0065):
Quick Sync with `-hwaccel qsv -hwaccel_output_format qsv` and `scale_qsv=format=p010` (`nv12` for
the 8-bit tier) before the encoder; AMF with `-hwaccel d3d11va -hwaccel_output_format d3d11`, the
frames taken as they are (`scale_d3d11` couldn't create its output picture with the bundled build
on the NVIDIA test computer, even for `nv12`). Neither was tried on Intel or AMD hardware. Without
a card encoder it is x265 with all streams, then x265 essential. A failed attempt moves on to the
next one only if it decoded on the card or got nowhere. A failure partway through a software decode means
the source is damaged at that point, and the next attempt would stop there too.

The output is accepted when FFprobe reads it, it holds HEVC video, and its length matches the
source within 1% or one second.

### Settings (ADR 0028)

- NVENC: `hevc_nvenc` with the detected tier's options (`standard::HardwareLevel::options`) and
  `-cq <level>`. With `Full`:
  `-preset p7 -tune hq -rc vbr -b 0 -profile main10 -highbitdepth 1 -rc-lookahead 32 -bf 4
  -b_ref_mode middle -multipass fullres`.
- Quick Sync: `hevc_qsv` with the tier's options and `-global_quality <level>` (ICQ).
- AMF: `hevc_amf` with the tier's options and `-qp_i <level> -qp_p <level>`: HEVC AMF has no
  constant-quality mode without a bitrate, so a constant quantizer is searched instead.
- x265: `-preset medium -x265-params log-level=error -crf <level>`, 10-bit when supported.
- HEVC is tagged `hvc1` so Apple devices and Windows players accept it.
- **Size and frame rate** (`standard::output_rate`): the picture size is never changed (ADR 0033). An
  average above 30.5 fps becomes 30, or 30000/1001 for the NTSC family; slower videos are left as
  they are. Filters go per stream (`-filter:<out>`): `fps`, then, on CPU frames only,
  `format=p010le` (any card)/`yuv420p10le`/`yuv420p`. GPU frames need no format filter; `-highbitdepth 1` handles 10-bit.

### Quality search (`search.rs`)

Before the first attempt, Gracil looks for the highest level (smallest file) whose VMAF reaches
95: NVENC CQ and AMF QP 24–38, Quick Sync ICQ 20–36, x265 CRF 20–34. Between a level that reaches the target and one that misses
it, the next guess is on a straight line through their scores; with only a level that reaches it,
the search steps past it by the score it has to spare (about a point per level); otherwise it
halves what is left. The first level measured is the one the last video with the same encoder,
size step and source codec needed (`search::Hints`, in memory only, ADR 0065); the search for
compressing again starts at the level the first search chose, or the one the last such video
needed. Each step encodes 4 s samples, one per 15 s of video up to four, centred in
equal shares of the video, and averages their scores. Every measured level is cached for the
whole search. One FFmpeg process per sample does both jobs, writing nothing to disk:

```text
-ss START -t 4 -i file:SRC
-filter_complex "[0:N]fps…,split[source][reference];[source]format=…[encode];[reference]format=yuv420p[ref]"
-map [encode] <encoder options at level> -stats_enc_post:0 pipe:1 -stats_enc_post_fmt:0 "{n} {size}" -f null -
-dec 0:0 -filter_complex "[dec:0]format=yuv420p[encoded];[encoded][ref]libvmaf=n_threads=…:pool=harmonic_mean:n_subsample=2[vmaf]"
-map [vmaf] -f null -
```

The loopback decoder (`-dec 0:0`) decodes what the encoder just produced. The last `VMAF score:`
line in the error output is the score; standard output lists every encoded frame's size.

**CUDA decoding (ADR 0065).** For an 8-bit or 10-bit 4:2:0 source (`pix_fmt` `yuv420p` or
`yuv420p10le`, the 10-bit kind only for a 10-bit tier), the sample is decoded with `-hwaccel cuda
-hwaccel_output_format cuda`: the encoder takes the CUDA frames as they are (after `fps`, as in the
real encode), and only the reference is copied out:

```text
[0:N]fps…,split[source][reference];[source]null[encode];
[reference]hwdownload,format=nv12|p010le,format=yuv420p[ref]
```

No `scale_cuda` conversion is used. Measured on 8-bit, 10-bit, 60 fps and 4K clips, the encoded
frames and the scores are bit-identical to decoding into ordinary frames, and the samples take
about a fifth less processor. Other sources (4:4:4, 12-bit, 10-bit with an 8-bit tier) are decoded
with `-hwaccel cuda` alone, so both branches get ordinary frames. Either way, if GPU decoding
fails, the sample is retried on the CPU.

**Samples together (ADR 0065).** A level's samples run up to four at once, on scoped threads, each
with `n_threads` = `threads_for_one` / K (at least one), so together they never use more VMAF
threads than one alone; their scores are collected in sample order, so the pooled score and size
are computed exactly as one at a time (and measured bit-identical). K is 1 with x265, for a
picture measured above 1440p (or of unknown size), with less than 4 GB free, and while the app
says the tools go easy (`share::go_easy`: efficiency mode or battery, which also means whenever
the computer is in use); otherwise up to 4, with 2 GB free for each. Every process is under the
job's `JobControl`, which now holds several children: pause, resume, stop and each one's stall
watchdog apply to each. A sample that fails together is measured again alone; if its log says
the encoder couldn't be opened (`OpenEncodeSessionEx`, `Error while opening encoder`), that is
not taken as a decoding failure, and samples go one at a time for the rest of the session.
Otherwise the failure is handled as one at a time would: decoding moves to the CPU.

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

**Already compressed (ADR 0033).** Otherwise the video was already compressed, and compressing it
again is allowed to lose a little: the search continues for the highest level reaching VMAF 90
(NVENC CQ and AMF QP 24–42, Quick Sync ICQ 20–40, x265 CRF 20–38), at the video's own size. That level is used if predicted at most
80% of the original; if not, nothing is encoded and `find_level` returns `Found::AsSmallAsItGets`.

The search is skipped (fixed level: CQ 27, CRF 26) for videos under 6 s, of unknown length, under
60 s with x265, or without `libvmaf`, and whenever a measurement fails. If no level reaches 95, the
best quality in the range is used. Attempts with a different encoder than the first use their
fixed level. Cancelling stops the search. Every measured level, its score and predicted video size
are logged, as is the choice.

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
