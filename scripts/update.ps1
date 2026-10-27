# Brings the Gracil in daily use, in Documents\Gracil, up to date (ADR 0073): gets what was merged
# on GitHub, such as work from the cloud, builds it and copies it over, keeping its data folder.
# The "Actualizar Gracil" shortcut on the desktop runs it; after every change made on this
# computer, Claude runs it with -NoPull -NoPause. Its messages are in Spanish, for the product
# owner. Exit codes are checked explicitly, as in check.ps1.
#
#   scripts/update.ps1             get main from GitHub, build and update Documents\Gracil
#   scripts/update.ps1 -NoPull     build what is in this folder now, without asking GitHub
#   scripts/update.ps1 -NoPause    end without waiting for Enter
param(
    [switch]$NoPull,
    [switch]$NoPause
)

$root = Split-Path $PSScriptRoot -Parent
Set-Location $root
$daily = Join-Path ([Environment]::GetFolderPath('MyDocuments')) 'Gracil'
$Host.UI.RawUI.WindowTitle = 'Actualizar Gracil'

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

# The pinned FFmpeg comes from PATH when it is there, else from the Gracil in daily use, which
# already holds it; portable.ps1 checks it is the pinned build either way.
$arguments = @('-Copy')
$dailyFFmpeg = Join-Path $daily 'ffmpeg'
if (-not (Get-Command ffmpeg.exe -ErrorAction SilentlyContinue) -and
    (Test-Path -LiteralPath (Join-Path $dailyFFmpeg 'ffmpeg.exe'))) {
    $arguments += @('-FFmpeg', $dailyFFmpeg)
}

$running = Get-Process gracil -ErrorAction SilentlyContinue |
    Where-Object { $_.Path -and $_.Path.StartsWith($daily, [System.StringComparison]::OrdinalIgnoreCase) }

Write-Host 'Fabricando la versión nueva de Gracil (puede tardar unos minutos)...'
& powershell -NoProfile -ExecutionPolicy Bypass -File (Join-Path $PSScriptRoot 'portable.ps1') @arguments
if ($LASTEXITCODE -ne 0) {
    Finish $false 'No se actualizó: la versión nueva no se pudo fabricar. Tu Gracil sigue como estaba. Pídele a Claude que lo revise.'
}

# portable.ps1 closed the Gracil that was open; it opens again, now up to date.
if ($running) { Start-Process -FilePath (Join-Path $daily 'gracil.exe') }

$when = git log -1 --format=%cd --date=format:'%Y-%m-%d %H:%M'
Finish $true "Listo: tu Gracil (Documentos\Gracil) ya tiene los últimos cambios, del $when."
