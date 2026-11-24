# Makes Gracil's installer from a released Gracil: the zip GitHub built for that version (ADR
# 0079), so what is installed is exactly what was released. Needs the GitHub CLI and Inno Setup 6
# (winget install JRSoftware.InnoSetup). Exit codes are checked explicitly, as in check.ps1.
#
#   scripts/installer.ps1 -Version 1.1.0    target\installer\Gracil-1.1.0-setup.exe
param(
    [Parameter(Mandatory)][string]$Version
)

$root = Split-Path $PSScriptRoot -Parent
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
$zip = "Gracil-$Version-x64.zip"
gh release download "v$Version" --pattern $zip --dir $work --repo alexpaltrc/gracil
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
$released = Join-Path $work 'released'
Expand-Archive (Join-Path $work $zip) -DestinationPath $released
$folder = (Get-ChildItem $released -Directory | Select-Object -First 1).FullName
if (-not (Test-Path (Join-Path $folder 'gracil.exe'))) {
    Write-Error "$zip has no gracil.exe in its folder."
    exit 1
}

& $iscc /Q "/DSource=$folder" "/DVersion=$Version" "/DOutput=$work" (Join-Path $PSScriptRoot 'installer.iss')
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
Write-Output (Join-Path $work "Gracil-$Version-setup.exe")
