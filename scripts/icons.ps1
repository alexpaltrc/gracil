# Draws the app-list icons in packaging/msix/Assets: the marble and its support on a Piedra tile,
# so Windows never fills the background with the accent colour (ADR 0026).
#
#   scripts/icons.ps1
#
# The shapes are those of assets/icons/vector/piedra-*.svg. Each taskbar gets its own tile:
#   *_altform-unplated       dark taskbar   graphite tile, light support
#   *_altform-lightunplated  light taskbar  ivory tile, dark support
#   plain targetsize, scale  plated places  the graphite tile, filling the square
# Run it again only when the artwork or the palette changes; the PNGs are committed.

Add-Type -AssemblyName System.Drawing

$root = Split-Path $PSScriptRoot -Parent
$assets = Join-Path $root 'packaging\msix\Assets'

# Piedra (docs/design/piedra/paleta-y-especificacion.json).
$support = '#4D4B46'
$sphere = '#8C887E'
$light = '#F3F1EB'
$dark = '#252522'
$reverse = '#DAD8CF'

$tones = @{
    dark  = @{ Tile = $dark; Support = $reverse; Sphere = $sphere; Edge = $reverse }
    light = @{ Tile = $light; Support = $support; Sphere = $sphere; Edge = $support }
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
function Draw([int]$px, $tone, [bool]$rounded, [string]$file) {
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
    $bitmap.Save((Join-Path $assets $file), [System.Drawing.Imaging.ImageFormat]::Png)
    $bitmap.Dispose()
}

foreach ($px in 16, 20, 24, 30, 32, 36, 40, 48, 60, 64, 72, 80, 96, 256) {
    Draw $px $tones.dark $true "AppList.targetsize-${px}_altform-unplated.png"
    Draw $px $tones.light $true "AppList.targetsize-${px}_altform-lightunplated.png"
    Draw $px $tones.dark $false "AppList.targetsize-$px.png"
}
foreach ($scale in @{ 100 = 44; 125 = 55; 150 = 66; 200 = 88; 250 = 110; 300 = 132; 400 = 176 }.GetEnumerator()) {
    Draw $scale.Value $tones.dark $false "AppList.scale-$($scale.Key).png"
}
Copy-Item -LiteralPath (Join-Path $assets 'AppList.scale-100.png') -Destination (Join-Path $assets 'AppList.png') -Force
Write-Host "Icons drawn in $assets"
