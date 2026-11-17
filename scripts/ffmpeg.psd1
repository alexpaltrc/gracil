# The FFmpeg that ships inside Gracil's zip (ADR 0050): the exact build Gracil is tested with.
# scripts/portable.ps1 only accepts executables with these hashes. See docs/development/ffmpeg.md.
@{
    Build         = 'N-125875-g5d4d3bdc61-20260731 win64-gpl'
    Builder       = 'https://github.com/BtbN/FFmpeg-Builds'
    Download      = 'https://github.com/BtbN/FFmpeg-Builds/releases/download/autobuild-2026-07-31-14-10/ffmpeg-N-125875-g5d4d3bdc61-win64-gpl.zip'
    Source        = 'https://git.ffmpeg.org/gitweb/ffmpeg.git/commit/5d4d3bdc61'
    License       = 'GPL version 3 or later'
    FFmpegSha256  = 'dcec5129f94a0e7338303a9bdb6548889d28238f57e1a2315884946c47fa1c40'
    FFprobeSha256 = 'ad3dd773bd94cd86906eb451d83112181bc884df7a5d57d15797bfad1f093da8'
}
