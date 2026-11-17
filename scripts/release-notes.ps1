# Writes what CHANGELOG.md says of one version, for its release on GitHub (ADR 0079).
#
#   scripts/release-notes.ps1 -Version 1.1.0
#
# The changelog writes 1.1 for 1.1.0 and 1.1.1 for 1.1.1 (docs/development/versioning.md).
param([Parameter(Mandatory)][string]$Version)

$name = $Version -replace '\.0$', ''
$lines = Get-Content (Join-Path (Split-Path $PSScriptRoot -Parent) 'CHANGELOG.md') -Encoding UTF8
$start = -1
for ($i = 0; $i -lt $lines.Count; $i++) {
    if ($lines[$i] -match "^## $([regex]::Escape($name)) \(") { $start = $i; break }
}
if ($start -lt 0) {
    Write-Error "CHANGELOG.md has no section for $name."
    exit 1
}
$end = $lines.Count
for ($i = $start + 1; $i -lt $lines.Count; $i++) {
    if ($lines[$i] -match '^## ') { $end = $i; break }
}
($lines[($start + 1)..($end - 1)] -join "`n").Trim()
