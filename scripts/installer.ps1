# Makes Gracil's installer (scripts/installer.iss). Needs Inno Setup 6
# (winget install JRSoftware.InnoSetup). Exit codes are checked explicitly, as in check.ps1.
#
#   scripts/installer.ps1 -Version 1.1.0    the released Gracil: the zip GitHub built for that
#                                           version (ADR 0079), so what is installed is exactly
#                                           what was released; needs the GitHub CLI
#                                           target\installer\Gracil-1.1.0-setup.exe
#   scripts/installer.ps1 -Dev -FFmpeg <folder>
#                                           Gracil Dev, built from this folder now (ADR 0082)
#                                           target\installer\Gracil Dev-<version>-setup.exe
param(
    [string]$Version,
    [switch]$Dev,
    [string]$FFmpeg
)

$root = Split-Path $PSScriptRoot -Parent
Set-Location $root
$work = Join-Path $root 'target\installer'
$iscc = @(
    "$env:LOCALAPPDATA\Programs\Inno Setup 6\ISCC.exe",
    "${env:ProgramFiles(x86)}\Inno Setup 6\ISCC.exe",
    "$env:ProgramFiles\Inno Setup 6\ISCC.exe"
) | Where-Object { Test-Path $_ } | Select-Object -First 1
if (-not $iscc) {
    Write-Error 'Inno Setup 6 was not found: winget install JRSoftware.InnoSetup'
    exit 1
}

# Made afresh, so nothing from another version comes along.
Remove-Item $work -Recurse -Force -ErrorAction SilentlyContinue
New-Item -ItemType Directory $work | Out-Null

if ($Dev) {
    # Each Gracil Dev has a higher number than the one before, so it installs over it: the
    # version in development and how many commits main has.
    $cargo = Get-Content (Join-Path $root 'Cargo.toml') -Raw
    if ($cargo -notmatch '(?m)^version\s*=\s*"([^"]+)"') {
        Write-Error 'No workspace version found in Cargo.toml.'
        exit 1
    }
    $Version = "$($Matches[1]).$(git rev-list --count HEAD)"
    $arguments = @('-Dev')
    if ($FFmpeg) { $arguments += @('-FFmpeg', $FFmpeg) }
    & powershell -NoProfile -ExecutionPolicy Bypass -File (Join-Path $PSScriptRoot 'portable.ps1') @arguments | Out-Host
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
    $folder = Join-Path $root 'target\portable\Gracil Dev'
    $name = 'Gracil Dev'
    $defines = @('/DDev')
} else {
    if (-not $Version) {
        Write-Error 'Give the released version: -Version 1.1.0, or -Dev for Gracil Dev.'
        exit 1
    }
    $zip = "Gracil-$Version-x64.zip"
    gh release download "v$Version" --pattern $zip --dir $work --repo alexpaltrc/gracil
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
    $released = Join-Path $work 'released'
    Expand-Archive (Join-Path $work $zip) -DestinationPath $released
    $folder = (Get-ChildItem $released -Directory | Select-Object -First 1).FullName
    $name = 'Gracil'
    $defines = @()
}
if (-not (Test-Path (Join-Path $folder 'gracil.exe'))) {
    Write-Error "$folder has no gracil.exe."
    exit 1
}

& $iscc /Q @defines "/DSource=$folder" "/DVersion=$Version" "/DOutput=$work" (Join-Path $PSScriptRoot 'installer.iss')
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
Write-Output (Join-Path $work "$name-$Version-setup.exe")
