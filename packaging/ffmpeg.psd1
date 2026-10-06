# The FFmpeg that ships inside Gracil's zip (ADR 0050): the exact build Gracil is tested with.
# scripts/portable.ps1 only accepts executables with these hashes. See docs/development/ffmpeg.md.
@{
    Build         = 'N-125875-g5d4d3bdc61-20260731 win64-gpl'
    Builder       = 'https://github.com/BtbN/FFmpeg-Builds'
    Source        = 'https://git.ffmpeg.org/gitweb/ffmpeg.git/commit/5d4d3bdc61'
    License       = 'GPL version 3 or later'
    FFmpegSha256  = '851aa8ea5366b5af33c0681cc292d662829c7088e9ac96f6e9b37030e6835ba4'
    FFprobeSha256 = '01dd9a776db5df93d0e3a9b8714a7a7500402e8e628cdc68608ed781454f8b71'
}
