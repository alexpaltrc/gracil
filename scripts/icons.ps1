# Draws Gracil's icons from the marble's shapes, in Windows 11's own neutral colors (ADR 0031):
# the app-list, Start and Store icons in packaging/msix/Assets, and the window's icons in
# assets/icons/ico. The icon always contrasts with Windows: on a light taskbar or theme it is the
# dark version, on a dark one the light version.
#
#   scripts/icons.ps1
#
#   *_altform-lightunplated, *_theme-light   light Windows   dark tile, light support
#   *_altform-unplated, *_theme-dark         dark Windows    light tile, dark support
#   plain files                              anywhere else   the dark tile
#   gracil-app-dark.ico, gracil-app-light.ico                the window's icon, chosen by theme
# Run it again only when the artwork or the colors change; the files are committed.

Add-Type -AssemblyName System.Drawing

$root = Split-Path $PSScriptRoot -Parent
$assets = Join-Path $root 'packaging\msix\Assets'
$icons = Join-Path $root 'assets\icons\ico'

# Windows 11's app backgrounds and text, and a neutral grey marble that reads on both.
$sphere = '#8A8A8A'
$tones = @{
    dark  = @{ Tile = '#202020'; Support = '#FFFFFF'; Sphere = $sphere; Edge = '#FFFFFF' }
    light = @{ Tile = '#F3F3F3'; Support = '#1A1A1A'; Sphere = $sphere; Edge = '#1A1A1A' }
}

function Color([string]$hex, [int]$alpha = 255) {
    $c = [System.Drawing.ColorTranslator]::FromHtml($hex)
    [System.Drawing.Color]::FromArgb($alpha, $c.R, $c.G, $c.B)
}

# The artwork's coordinates (a 249 x 279 box centred on 124.5, 139.5) to pixels: the glyph sits
# in the middle of the tile at `glyph` of its usual size on the 48 grid.
function Shapes([single]$px, [single]$glyph) {
    $k = $px / 48 * 0.245398773006 * $glyph
    $at = { param($x, $y) New-Object System.Drawing.PointF(($px / 2 + ($x - 124.5) * $k), ($px / 2 + ($y - 139.5) * $k)) }
    # A quadratic curve as the cubic Windows draws.
    $quad = {
        param($path, $x0, $y0, $cx, $cy, $x1, $y1)
        $path.AddBezier((& $at $x0 $y0), (& $at ($x0 + 2 / 3 * ($cx - $x0)) ($y0 + 2 / 3 * ($cy - $y0))),
            (& $at ($x1 + 2 / 3 * ($cx - $x1)) ($y1 + 2 / 3 * ($cy - $y1))), (& $at $x1 $y1))
    }

    $l = New-Object System.Drawing.Drawing2D.GraphicsPath
    $l.AddLine((& $at 43 66), (& $at 73 66))
    $l.AddLine((& $at 73 66), (& $at 73 157))
    & $quad $l 73 157 73 183 100 183
    $l.AddLine((& $at 100 183), (& $at 206 183))
    $l.AddLine((& $at 206 183), (& $at 206 213))
    $l.AddLine((& $at 206 213), (& $at 96 213))
    & $quad $l 96 213 43 213 43 159
    $l.CloseFigure()

    $o = New-Object System.Drawing.Drawing2D.GraphicsPath
    $o.AddBezier((& $at 168 145), (& $at 168 124), (& $at 151 108), (& $at 131 108))
    $o.AddBezier((& $at 131 108), (& $at 111 108), (& $at 94 124), (& $at 94 145))
    $o.AddBezier((& $at 94 145), (& $at 94 166), (& $at 111 183), (& $at 131 183))
    $o.AddBezier((& $at 131 183), (& $at 151 183), (& $at 168 166), (& $at 168 145))
    $o.CloseFigure()
    , @($l, $o)
}

function Tile([single]$px, [single]$inset, [single]$radius) {
    $path = New-Object System.Drawing.Drawing2D.GraphicsPath
    $a = $inset; $b = $px - $inset; $d = 2 * $radius
    $path.AddArc($a, $a, $d, $d, 180, 90)
    $path.AddArc($b - $d, $a, $d, $d, 270, 90)
    $path.AddArc($b - $d, $b - $d, $d, $d, 0, 90)
    $path.AddArc($a, $b - $d, $d, $d, 90, 90)
    $path.CloseFigure()
    $path
}

# One icon of `px` pixels. `rounded`: a free-standing rounded tile with a hairline edge, as on the
# taskbar; otherwise the tile fills the square and Windows shapes it.
function Picture([int]$px, $tone, [bool]$rounded) {
    $bitmap = New-Object System.Drawing.Bitmap($px, $px, [System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
    $g = [System.Drawing.Graphics]::FromImage($bitmap)
    $g.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::AntiAlias
    $g.PixelOffsetMode = [System.Drawing.Drawing2D.PixelOffsetMode]::HighQuality
    $g.Clear([System.Drawing.Color]::Transparent)

    $unit = $px / 48
    if ($rounded) {
        $tile = Tile $px (2 * $unit) (10 * $unit)
        $g.FillPath((New-Object System.Drawing.SolidBrush (Color $tone.Tile)), $tile)
        $edge = New-Object System.Drawing.Pen((Color $tone.Edge 46), [single][Math]::Max(1, $unit))
        $edge.Alignment = [System.Drawing.Drawing2D.PenAlignment]::Inset
        $g.DrawPath($edge, $tile)
        $glyph = 0.7
    } else {
        $g.Clear((Color $tone.Tile))
        $glyph = 0.74
    }
    $shapes = Shapes $px $glyph
    $g.FillPath((New-Object System.Drawing.SolidBrush (Color $tone.Support)), $shapes[0])
    $g.FillPath((New-Object System.Drawing.SolidBrush (Color $tone.Sphere)), $shapes[1])
    $g.Dispose()
    $bitmap
}

function Draw([int]$px, $tone, [bool]$rounded, [string]$file) {
    $bitmap = Picture $px $tone $rounded
    $bitmap.Save((Join-Path $assets $file), [System.Drawing.Imaging.ImageFormat]::Png)
    $bitmap.Dispose()
}

# An icon file holding the rounded tile at every size Windows asks for, each stored as PNG.
function Ico($tone, [string]$file) {
    $sizes = 16, 20, 24, 32, 40, 48, 64, 256
    $images = foreach ($px in $sizes) {
        $bitmap = Picture $px $tone $true
        $stream = New-Object System.IO.MemoryStream
        $bitmap.Save($stream, [System.Drawing.Imaging.ImageFormat]::Png)
        $bitmap.Dispose()
        , $stream.ToArray()
    }
    $out = New-Object System.IO.MemoryStream
    $w = New-Object System.IO.BinaryWriter($out)
    $w.Write([uint16]0); $w.Write([uint16]1); $w.Write([uint16]$sizes.Count)
    $offset = 6 + 16 * $sizes.Count
    for ($i = 0; $i -lt $sizes.Count; $i++) {
        $side = if ($sizes[$i] -ge 256) { 0 } else { $sizes[$i] }
        $w.Write([byte]$side); $w.Write([byte]$side); $w.Write([byte]0); $w.Write([byte]0)
        $w.Write([uint16]1); $w.Write([uint16]32)
        $w.Write([uint32]$images[$i].Length); $w.Write([uint32]$offset)
        $offset += $images[$i].Length
    }
    foreach ($image in $images) { $w.Write($image) }
    $w.Flush()
    [System.IO.File]::WriteAllBytes((Join-Path $icons $file), $out.ToArray())
}

$scales = @{ 100 = 1.0; 125 = 1.25; 150 = 1.5; 200 = 2.0; 250 = 2.5; 300 = 3.0; 400 = 4.0 }
foreach ($px in 16, 20, 24, 30, 32, 36, 40, 48, 60, 64, 72, 80, 96, 256) {
    Draw $px $tones.light $true "AppList.targetsize-${px}_altform-unplated.png"
    Draw $px $tones.dark $true "AppList.targetsize-${px}_altform-lightunplated.png"
    Draw $px $tones.dark $false "AppList.targetsize-$px.png"
}
foreach ($scale in $scales.GetEnumerator()) {
    Draw ([int](44 * $scale.Value)) $tones.dark $false "AppList.scale-$($scale.Key).png"
    foreach ($logo in @{ Square150x150Logo = 150; StoreLogo = 50 }.GetEnumerator()) {
        $px = [int]($logo.Value * $scale.Value)
        Draw $px $tones.dark $true "$($logo.Key).scale-$($scale.Key).png"
        Draw $px $tones.dark $true "$($logo.Key).scale-$($scale.Key)_altform-colorful_theme-light.png"
        Draw $px $tones.light $true "$($logo.Key).scale-$($scale.Key)_altform-colorful_theme-dark.png"
    }
}
foreach ($name in 'AppList', 'Square150x150Logo', 'StoreLogo') {
    Copy-Item -LiteralPath (Join-Path $assets "$name.scale-100.png") -Destination (Join-Path $assets "$name.png") -Force
}
Ico $tones.dark 'gracil-app-dark.ico'
Ico $tones.light 'gracil-app-light.ico'
Write-Host "Icons drawn in $assets and $icons"
