# Builds Gracil's portable folder and zip (ADR 0050). See docs/development/packaging.md.
#
#   scripts/portable.ps1                       release build + folder in target\portable\Gracil
#                                               + target\portable\Gracil-<version>-x64.zip
#   scripts/portable.ps1 -FFmpeg <folder>      take ffmpeg.exe and ffprobe.exe from <folder>
#   scripts/portable.ps1 -Install <folder>     ...and copy the folder there, closing Gracil first
#
# FFmpeg is the pinned build in packaging\ffmpeg.psd1; without -FFmpeg, the first folder on PATH
# holding executables with the pinned hashes is used. Exit codes are checked explicitly, as in
# check.ps1.
param(
    [string]$FFmpeg,
    [string]$Install
)

$root = Split-Path $PSScriptRoot -Parent
Set-Location $root
$pin = Import-PowerShellDataFile (Join-Path $root 'packaging\ffmpeg.psd1')

$cargo = Get-Content (Join-Path $root 'Cargo.toml') -Raw
if ($cargo -notmatch '(?m)^version\s*=\s*"([^"]+)"') {
    Write-Error 'No workspace version found in Cargo.toml.'
    exit 1
}
$version = $Matches[1]

function Test-Pinned([string]$folder) {
    foreach ($pair in @(@('ffmpeg.exe', $pin.FFmpegSha256), @('ffprobe.exe', $pin.FFprobeSha256))) {
        $file = Join-Path $folder $pair[0]
        if (-not (Test-Path -LiteralPath $file)) { return $false }
        if ((Get-FileHash -LiteralPath $file -Algorithm SHA256).Hash -ne $pair[1]) { return $false }
    }
    return $true
}

if (-not $FFmpeg) {
    $FFmpeg = $env:PATH -split ';' | Where-Object { $_ -and (Test-Path -LiteralPath (Join-Path $_ 'ffmpeg.exe')) } |
        Where-Object { Test-Pinned $_ } | Select-Object -First 1
}
if (-not $FFmpeg -or -not (Test-Pinned $FFmpeg)) {
    Write-Error "The pinned FFmpeg ($($pin.Build)) was not found. Download it from $($pin.Builder) and pass its bin folder with -FFmpeg."
    exit 1
}

cargo build --release -p gracil-ui -p gracil-shell
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

# A fresh folder every time, so nothing from an older build ends up in the zip.
$portable = Join-Path $root 'target\portable'
$layout = Join-Path $portable 'Gracil'
if (Test-Path -LiteralPath $layout) { Remove-Item -LiteralPath $layout -Recurse -Force }
$licenses = Join-Path $layout 'licenses'
$tools = Join-Path $layout 'ffmpeg'
New-Item -ItemType Directory -Path $layout, $licenses, $tools | Out-Null
Copy-Item -LiteralPath (Join-Path $root 'target\release\gracil.exe') -Destination $layout
Copy-Item -LiteralPath (Join-Path $root 'target\release\gracil_shell.dll') -Destination $layout
Copy-Item -LiteralPath (Join-Path $FFmpeg 'ffmpeg.exe'), (Join-Path $FFmpeg 'ffprobe.exe') -Destination $tools
$utf8 = New-Object System.Text.UTF8Encoding $false

# FFmpeg's license, and where its source is (GPL section 6).
$ffmpegLicense = Join-Path (Split-Path $FFmpeg -Parent) 'LICENSE.txt'
if (Test-Path -LiteralPath $ffmpegLicense) {
    Copy-Item -LiteralPath $ffmpegLicense -Destination (Join-Path $licenses 'ffmpeg-license.txt')
}
$source = @"
Gracil runs FFmpeg as a separate program, from the ffmpeg folder. It is not part of Gracil.

FFmpeg build: $($pin.Build)
License:      $($pin.License) (ffmpeg-license.txt)
Built by:     $($pin.Builder) (build scripts and the sources of every library in it)
Source:       $($pin.Source)
"@
[System.IO.File]::WriteAllText((Join-Path $licenses 'ffmpeg-source.txt'), $source, $utf8)

# jpegli's cjpegli.exe compresses every photo (ADR 0034). Built from source when missing, and
# shipped next to gracil.exe with the licenses of jpegli and the libraries built into it.
$jpegli = Join-Path $root 'target\jpegli'
$cjpegli = Join-Path $jpegli 'build\tools\cjpegli.exe'
if (-not (Test-Path -LiteralPath $cjpegli)) {
    & powershell -NoProfile -ExecutionPolicy Bypass -File (Join-Path $PSScriptRoot 'build-jpegli.ps1') | Out-Null
    if ($LASTEXITCODE -ne 0 -or -not (Test-Path -LiteralPath $cjpegli)) {
        Write-Error 'cjpegli.exe could not be built; see scripts/build-jpegli.ps1.'
        exit 1
    }
}
Copy-Item -LiteralPath $cjpegli -Destination $layout
$notices = @(
    @('jpegli', 'LICENSE'),
    @('libjpeg-turbo', 'third_party\libjpeg-turbo\LICENSE.md'),
    @('libjpeg (Independent JPEG Group)', 'third_party\libjpeg-turbo\README.ijg'),
    @('Highway', 'third_party\highway\LICENSE'),
    @('libpng', 'third_party\libpng\LICENSE'),
    @('zlib', 'third_party\zlib\LICENSE'),
    @('skcms', 'third_party\skcms\LICENSE'),
    @('Little CMS', 'third_party\lcms\LICENSE')
) | ForEach-Object {
    "==== $($_[0]) ====`r`n`r`n" + (Get-Content -LiteralPath (Join-Path $jpegli $_[1]) -Raw)
}
[System.IO.File]::WriteAllText((Join-Path $licenses 'cjpegli-licenses.txt'), ($notices -join "`r`n`r`n"), $utf8)
if (Test-Path -LiteralPath (Join-Path $root 'LICENSE')) {
    Copy-Item -LiteralPath (Join-Path $root 'LICENSE') -Destination (Join-Path $licenses 'gracil-license.txt')
}

$zip = Join-Path $portable "Gracil-$version-x64.zip"
if (Test-Path -LiteralPath $zip) { Remove-Item -LiteralPath $zip -Force }
# Windows' own tar writes the zip, with the Gracil folder at its top; named in full, since Git's
# tar may come first on the PATH and can't read Windows paths.
& (Join-Path $env:SystemRoot 'System32\tar.exe') -a -c -f $zip -C $portable Gracil
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
Write-Host "Gracil $version ready in $layout and $zip"

if ($Install) {
    # The running Gracil and its helpers hold their files; they are closed before copying.
    Get-Process gracil, ffmpeg, ffprobe, cjpegli -ErrorAction SilentlyContinue |
        Where-Object { $_.Path -and $_.Path.StartsWith($Install, [System.StringComparison]::OrdinalIgnoreCase) } |
        Stop-Process -Force
    Wait-Process -Name gracil -Timeout 15 -ErrorAction SilentlyContinue
    New-Item -ItemType Directory -Force -Path $Install | Out-Null
    # Everything but the data folder, which keeps the settings and logs.
    Get-ChildItem -LiteralPath $layout | ForEach-Object {
        Copy-Item -LiteralPath $_.FullName -Destination $Install -Recurse -Force
    }
    Write-Host "Gracil $version copied to $Install"
}
