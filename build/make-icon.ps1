# Erzeugt das Logo der App: build/icon.png (512 px) und build/icon.ico (256 bis 16 px).
# Aufruf: powershell -NoProfile -ExecutionPolicy Bypass -File build/make-icon.ps1
#
# Das Logo ist eine weisse Sprechblase auf einem abgerundeten Feld in Babyblau.
# Darin stehen die Zeichen ẞ (Latein, U+1E9E) und Ж (Kyrillisch, U+0416) - die
# Entsprechung zu den Buchstaben-Symbolen anderer Uebersetzer. Die Zeichen
# werden als Vektorpfade gezeichnet und auf gleiche Hoehe gebracht, damit beide
# optisch gleich schwer wirken.
param([string]$Root = (Split-Path -Parent $PSScriptRoot))

Add-Type -AssemblyName System.Drawing

$fontFamily = 'Segoe UI'
$accentTop = [System.Drawing.Color]::FromArgb(255, 156, 207, 245)
$accentBottom = [System.Drawing.Color]::FromArgb(255, 126, 189, 238)
$ink = [System.Drawing.Color]::FromArgb(255, 14, 42, 68)
$white = [System.Drawing.Color]::White

function Get-Glyph([string]$ch) {
  $path = New-Object System.Drawing.Drawing2D.GraphicsPath
  $family = New-Object System.Drawing.FontFamily($fontFamily)
  $fmt = [System.Drawing.StringFormat]::GenericTypographic
  $path.AddString($ch, $family, [int][System.Drawing.FontStyle]::Bold, 200, [System.Drawing.PointF]::new(0, 0), $fmt)
  $family.Dispose()
  return @{ Path = $path; Box = $path.GetBounds() }
}

function RoundRect([single]$x, [single]$y, [single]$w, [single]$h, [single]$r) {
  $p = New-Object System.Drawing.Drawing2D.GraphicsPath
  $d = 2 * $r
  $p.AddArc($x, $y, $d, $d, 180, 90)
  $p.AddArc($x + $w - $d, $y, $d, $d, 270, 90)
  $p.AddArc($x + $w - $d, $y + $h - $d, $d, $d, 0, 90)
  $p.AddArc($x, $y + $h - $d, $d, $d, 90, 90)
  $p.CloseFigure()
  return $p
}

function New-LogoBitmap([int]$size) {
  $bmp = New-Object System.Drawing.Bitmap($size, $size, [System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
  $g = [System.Drawing.Graphics]::FromImage($bmp)
  $g.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::AntiAlias
  $g.PixelOffsetMode = [System.Drawing.Drawing2D.PixelOffsetMode]::HighQuality
  $g.Clear([System.Drawing.Color]::Transparent)
  $s = [single]$size

  # Feld: abgerundetes Quadrat mit 22 Prozent Rundung, wie die Karten der App.
  $field = RoundRect 0 0 $s $s ($s * 0.22)
  $gradient = New-Object System.Drawing.Drawing2D.LinearGradientBrush([System.Drawing.RectangleF]::new(0, 0, $s, $s), $accentTop, $accentBottom, 90.0)
  $g.FillPath($gradient, $field)

  # Sprechblase mit Schwanz unten links.
  $bubble = RoundRect ($s * 0.10) ($s * 0.12) ($s * 0.80) ($s * 0.58) ($s * 0.17)
  $tail = New-Object System.Drawing.Drawing2D.GraphicsPath
  $tail.AddPolygon(@(
      [System.Drawing.PointF]::new(($s * 0.27), ($s * 0.68)),
      [System.Drawing.PointF]::new(($s * 0.45), ($s * 0.68)),
      [System.Drawing.PointF]::new(($s * 0.26), ($s * 0.87))
    ))
  $whiteBrush = New-Object System.Drawing.SolidBrush($white)
  $g.FillPath($whiteBrush, $bubble)
  $g.FillPath($whiteBrush, $tail)

  # Beide Zeichen auf gleiche Hoehe bringen und in der Blase zentrieren.
  $left = Get-Glyph ([string][char]0x1E9E)
  $right = Get-Glyph ([string][char]0x0416)
  $h = $s * 0.29
  $gap = $s * 0.075
  $w1 = $left.Box.Width * ($h / $left.Box.Height)
  $w2 = $right.Box.Width * ($h / $right.Box.Height)
  $x = ($s - ($w1 + $gap + $w2)) / 2
  $y = ($s * 0.415) - ($h / 2)
  $inkBrush = New-Object System.Drawing.SolidBrush($ink)
  foreach ($item in @(@($left, $x), @($right, ($x + $w1 + $gap)))) {
    $path = $item[0].Path
    $box = $item[0].Box
    $scale = $h / $box.Height
    $m = New-Object System.Drawing.Drawing2D.Matrix
    $m.Translate(-$box.X, -$box.Y, [System.Drawing.Drawing2D.MatrixOrder]::Append)
    $m.Scale($scale, $scale, [System.Drawing.Drawing2D.MatrixOrder]::Append)
    $m.Translate($item[1], $y, [System.Drawing.Drawing2D.MatrixOrder]::Append)
    $path.Transform($m)
    $g.FillPath($inkBrush, $path)
    $m.Dispose(); $path.Dispose()
  }

  $inkBrush.Dispose(); $whiteBrush.Dispose()
  $bubble.Dispose(); $tail.Dispose(); $field.Dispose(); $gradient.Dispose(); $g.Dispose()
  return $bmp
}

function Save-Png([System.Drawing.Bitmap]$bmp, [string]$file) {
  $dir = Split-Path -Parent $file
  if (-not (Test-Path $dir)) { New-Item -ItemType Directory -Force -Path $dir | Out-Null }
  $bmp.Save($file, [System.Drawing.Imaging.ImageFormat]::Png)
  Write-Host "  $file"
}

function Save-Ico([int[]]$sizes, [string]$file) {
  $tmp = Join-Path ([System.IO.Path]::GetTempPath()) ('ico-' + [Guid]::NewGuid().ToString('N'))
  New-Item -ItemType Directory -Force -Path $tmp | Out-Null
  $images = @()
  foreach ($size in $sizes) {
    $b = New-LogoBitmap $size
    $png = Join-Path $tmp "$size.png"
    $b.Save($png, [System.Drawing.Imaging.ImageFormat]::Png)
    $b.Dispose()
    $images += , @{ Size = $size; Bytes = [System.IO.File]::ReadAllBytes($png) }
  }
  Remove-Item $tmp -Recurse -Force -ErrorAction SilentlyContinue

  $ms = New-Object System.IO.MemoryStream
  $bw = New-Object System.IO.BinaryWriter($ms)
  $bw.Write([UInt16]0); $bw.Write([UInt16]1); $bw.Write([UInt16]$images.Count)
  $offset = 6 + 16 * $images.Count
  foreach ($img in $images) {
    $dim = if ($img.Size -ge 256) { 0 } else { $img.Size }
    $bw.Write([Byte]$dim); $bw.Write([Byte]$dim)
    $bw.Write([Byte]0); $bw.Write([Byte]0)
    $bw.Write([UInt16]1); $bw.Write([UInt16]32)
    $bw.Write([UInt32]$img.Bytes.Length); $bw.Write([UInt32]$offset)
    $offset += $img.Bytes.Length
  }
  foreach ($img in $images) { $bw.Write($img.Bytes) }
  $bw.Flush()
  [System.IO.File]::WriteAllBytes($file, $ms.ToArray())
  $bw.Dispose(); $ms.Dispose()
  Write-Host "  $file"
}

Write-Host 'Logo:'
$b = New-LogoBitmap 512
Save-Png $b (Join-Path $Root 'build/icon.png')
$b.Dispose()
Save-Ico @(256, 128, 64, 48, 32, 16) (Join-Path $Root 'build/icon.ico')
Write-Host 'Fertig.'
