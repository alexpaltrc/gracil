# Writes the licenses of every Rust library built into gracil.exe to one
# text file, for the zip's licenses folder (ADR 0054). Called by portable.ps1.
#
#   scripts/licenses.ps1 -Out <file>
#
# Only libraries that end up in the program count: those the two crates depend on for Windows
# x64, not the ones used just to build or test them. Each license text is written once, followed
# by every library that ships it word for word. Exit codes are checked explicitly, as in
# check.ps1.
param(
    [Parameter(Mandatory)][string]$Out
)

$root = Split-Path $PSScriptRoot -Parent
Set-Location $root

$json = cargo metadata --format-version 1 --locked --filter-platform x86_64-pc-windows-msvc
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
$metadata = ($json -join "`n") | ConvertFrom-Json

$packages = @{}
foreach ($package in $metadata.packages) { $packages[$package.id] = $package }
$nodes = @{}
foreach ($node in $metadata.resolve.nodes) { $nodes[$node.id] = $node }

# Walks the normal dependencies from the shipped crate.
$shipped = $metadata.packages | Where-Object { $_.name -eq 'gracil-ui' }
$pending = New-Object System.Collections.Generic.Queue[string]
$seen = New-Object 'System.Collections.Generic.HashSet[string]'
foreach ($package in $shipped) { $pending.Enqueue($package.id) }
while ($pending.Count -gt 0) {
    $id = $pending.Dequeue()
    if (-not $seen.Add($id)) { continue }
    foreach ($dep in $nodes[$id].deps) {
        $normal = $dep.dep_kinds | Where-Object { $null -eq $_.kind }
        if ($normal) { $pending.Enqueue($dep.pkg) }
    }
}

$texts = [ordered]@{}
$unfiled = New-Object System.Collections.Generic.List[string]
$libraries = $seen | ForEach-Object { $packages[$_] } |
    Where-Object { $_.name -notlike 'gracil-*' } |
    Sort-Object name, version
foreach ($package in $libraries) {
    $label = "$($package.name) $($package.version) ($($package.license))"
    $folder = Split-Path $package.manifest_path -Parent
    $files = Get-ChildItem -LiteralPath $folder -File |
        Where-Object { $_.Name -match '^(LICEN[CS]E|COPYING|NOTICE|UNLICENSE)' } |
        Sort-Object Name
    if (-not $files) {
        $unfiled.Add($label)
        continue
    }
    foreach ($file in $files) {
        $text = ([System.IO.File]::ReadAllText($file.FullName)).Replace("`r`n", "`n").Trim()
        if (-not $texts.Contains($text)) { $texts[$text] = New-Object System.Collections.Generic.List[string] }
        if (-not $texts[$text].Contains($label)) { $texts[$text].Add($label) }
    }
}

$sections = New-Object System.Collections.Generic.List[string]
$sections.Add("The Rust libraries built into gracil.exe, and their licenses.")
foreach ($entry in $texts.GetEnumerator()) {
    $sections.Add("==== Used by: $($entry.Value -join ', ') ====`n`n$($entry.Key)")
}
if ($unfiled.Count -gt 0) {
    $sections.Add("==== Without a license file of their own: under the license named, whose text is above ====`n`n" + ($unfiled -join "`n"))
}
$utf8 = New-Object System.Text.UTF8Encoding $false
[System.IO.File]::WriteAllText($Out, ($sections -join "`n`n`n").Replace("`n", "`r`n") + "`r`n", $utf8)
Write-Host "Licenses of $($libraries.Count) Rust libraries written to $Out"
