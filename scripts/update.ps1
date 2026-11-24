# Brings Gracil Dev up to date (ADR 0082): gets what was merged on GitHub, such as work from the
# cloud, builds Gracil Dev's installer and runs it quietly over the installed Gracil Dev, which
# keeps its data folder. After every change made on this computer, Claude runs it with -NoPull
# -NoPause. Its messages are in Spanish, for the product owner. Exit codes are checked
# explicitly, as in check.ps1.
#
#   scripts/update.ps1             get main from GitHub, build and install Gracil Dev
#   scripts/update.ps1 -NoPull     build what is in this folder now, without asking GitHub
#   scripts/update.ps1 -NoPause    end without waiting for Enter
param(
    [switch]$NoPull,
    [switch]$NoPause
)

$root = Split-Path $PSScriptRoot -Parent
Set-Location $root
$programs = Join-Path $env:LOCALAPPDATA 'Programs'
$dev = Join-Path $programs 'Gracil Dev'
$Host.UI.RawUI.WindowTitle = 'Actualizar Gracil Dev'

function Finish([bool]$ok, [string]$message) {
    Write-Host ''
    if ($ok) { Write-Host $message -ForegroundColor Green } else { Write-Host $message -ForegroundColor Red }
    if (-not $NoPause) {
        Write-Host ''
        Read-Host 'Presiona Enter para cerrar' | Out-Null
    }
    if ($ok) { exit 0 } else { exit 1 }
}

if (-not $NoPull) {
    $branch = git branch --show-current
    if ($branch -ne 'main') {
        Finish $false "No se actualizó: el proyecto está en la rama '$branch', no en 'main'. Pídele a Claude que lo revise."
    }
    Write-Host 'Bajando los cambios de GitHub...'
    git pull --ff-only
    if ($LASTEXITCODE -ne 0) {
        Finish $false 'No se actualizó: no se pudieron bajar los cambios de GitHub (¿sin internet, o hay cambios en esta computadora que no están en GitHub?). Pídele a Claude que lo revise.'
    }
}

# The pinned FFmpeg comes from an installed Gracil, which already holds it, else from PATH;
# portable.ps1 checks it is the pinned build either way.
$arguments = @('-Dev')
$ffmpeg = @((Join-Path $dev 'ffmpeg'), (Join-Path $programs 'Gracil\ffmpeg')) |
    Where-Object { Test-Path -LiteralPath (Join-Path $_ 'ffmpeg.exe') } | Select-Object -First 1
if ($ffmpeg) { $arguments += @('-FFmpeg', $ffmpeg) }

$running = Get-Process gracil -ErrorAction SilentlyContinue |
    Where-Object { $_.Path -and $_.Path.StartsWith("$dev\", [System.StringComparison]::OrdinalIgnoreCase) }

Write-Host 'Fabricando Gracil Dev (puede tardar unos minutos)...'
$setup = & powershell -NoProfile -ExecutionPolicy Bypass -File (Join-Path $PSScriptRoot 'installer.ps1') @arguments |
    Select-Object -Last 1
if ($LASTEXITCODE -ne 0 -or -not $setup -or -not (Test-Path -LiteralPath $setup)) {
    Finish $false 'No se actualizó: Gracil Dev no se pudo fabricar. Sigue como estaba. Pídele a Claude que lo revise.'
}

# The installer closes Gracil Dev if it is open, replaces it and keeps its data.
Write-Host 'Instalando Gracil Dev...'
$install = Start-Process -FilePath $setup -ArgumentList '/VERYSILENT', '/SUPPRESSMSGBOXES', '/NORESTART' -Wait -PassThru
if ($install.ExitCode -ne 0) {
    Finish $false "No se actualizó: el instalador de Gracil Dev terminó con el código $($install.ExitCode). Pídele a Claude que lo revise."
}
# It opens again if it was open, now up to date.
if ($running) { Start-Process -FilePath (Join-Path $dev 'gracil.exe') }

$when = git log -1 --format=%cd --date=format:'%Y-%m-%d %H:%M'
Finish $true "Listo: Gracil Dev ya tiene los últimos cambios, del $when."
