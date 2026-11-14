# Erzeugt das Logo der App: build/icon.png (512 px) und build/icon.ico (256 bis 16 px).
# Aufruf: powershell -NoProfile -ExecutionPolicy Bypass -File build/make-icon.ps1
#
# Die beiden Zeichen stehen fuer den Namen der App und ersetzen die Buchstaben
# anderer Uebersetzer-Symbole: ẞ (Latein, U+1E9E) und Ж (Kyrillisch, U+0416).
# Sie werden als Vektorpfade gezeichnet und auf gleiche Hoehe gebracht, damit
# beide optisch gleich schwer wirken.
param([string]$Root = (Split-Path -Parent $PSScriptRoot))

Add-Type -AssemblyName System.Drawing

$fontFamily = 'Segoe UI'
$accentTop = [System.Drawing.Color]::FromArgb(255, 156, 207, 245)
$accentBottom = [System.Drawing.Color]::FromArgb(255, 126, 189, 238)
$ink = [System.Drawing.Color]::FromArgb(255, 14, 42, 68)

function Get-GlyphPath([string]$ch) {
  $path = New-Object System.Drawing.Drawing2D.GraphicsPath
  $family = New-Object System.Drawing.FontFamily($fontFamily)
  $fmt = [System.Drawing.StringFormat]::GenericTypographic
  $path.AddString($ch, $family, [int][System.Drawing.FontStyle]::Bold, 200, (New-Object System.Drawing.PointF(0, 0)), $fmt)
  $family.Dispose()
  return $path
}

function New-LogoBitmap([int]$size) {
  $bmp = New-Object System.Drawing.Bitmap($size, $size, [System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
  $g = [System.Drawing.Graphics]::FromImage($bmp)
  $g.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::AntiAlias
  $g.PixelOffsetMode = [System.Drawing.Drawing2D.PixelOffsetMode]::HighQuality
  $g.Clear([System.Drawing.Color]::Transparent)

  # Abgerundetes Quadrat: 22 Prozent der Kante, wie die Karten der App.
  $r = $size * 0.22
  $d = 2 * $r
  $rect = New-Object System.Drawing.RectangleF(0, 0, $size, $size)
  $shape = New-Object System.Drawing.Drawing2D.GraphicsPath
  $shape.AddArc($rect.X, $rect.Y, $d, $d, 180, 90)
  $shape.AddArc($rect.Right - $d, $rect.Y, $d, $d, 270, 90)
  $shape.AddArc($rect.Right - $d, $rect.Bottom - $d, $d, $d, 0, 90)
  $shape.AddArc($rect.X, $rect.Bottom - $d, $d, $d, 90, 90)
  $shape.CloseFigure()
  $brush = New-Object System.Drawing.Drawing2D.LinearGradientBrush($rect, $accentTop, $accentBottom, 90.0)
  $g.FillPath($brush, $shape)

  # Beide Zeichen auf dieselbe Hoehe bringen und zusammen zentrieren.
  $paths = @()
  foreach ($ch in @([string][char]0x1E9E, [string][char]0x0416)) {
    $p = Get-GlyphPath $ch
    $b = $p.GetBounds()
    if ($b.Height -le 0) { continue }
    $paths += , @{ Path = $p; Bounds = $b }
  }
  if ($paths.Count -eq 0) { throw 'Keine Glyphen gezeichnet - Schrift pruefen.' }

  # Hoehe so waehlen, dass beide Zeichen zusammen in die Flaeche passen; die
  # Breite ist hier der Engpass, nicht die Hoehe.
  $gap = $size * 0.06
  $aspect = 0
  foreach ($item in $paths) { $aspect += $item.Bounds.Width / $item.Bounds.Height }
  $maxWidth = $size * 0.74
  $target = [Math]::Min($size * 0.42, ($maxWidth - $gap) / $aspect)
  $scales = @()
  foreach ($item in $paths) { $scales += , ($target / $item.Bounds.Height) }
  $total = 0
  for ($i = 0; $i -lt $paths.Count; $i++) {
    $total += $paths[$i].Bounds.Width * $scales[$i]
    if ($i -gt 0) { $total += $gap }
  }

  $inkBrush = New-Object System.Drawing.SolidBrush($ink)
  $x = ($size - $total) / 2
  for ($i = 0; $i -lt $paths.Count; $i++) {
    $p = $paths[$i].Path
    $b = $paths[$i].Bounds
    $s = $scales[$i]
    $y = ($size - $b.Height * $s) / 2
    $m = New-Object System.Drawing.Drawing2D.Matrix
    $m.Translate(-$b.X, -$b.Y, [System.Drawing.Drawing2D.MatrixOrder]::Append)
    $m.Scale($s, $s, [System.Drawing.Drawing2D.MatrixOrder]::Append)
    $m.Translate($x, $y, [System.Drawing.Drawing2D.MatrixOrder]::Append)
    $p.Transform($m)
    $g.FillPath($inkBrush, $p)
    $m.Dispose(); $p.Dispose()
    $x += $b.Width * $s + $gap
  }

  $inkBrush.Dispose(); $brush.Dispose(); $shape.Dispose(); $g.Dispose()
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
  foreach ($s in $sizes) {
    $b = New-LogoBitmap $s
    $png = Join-Path $tmp "$s.png"
    $b.Save($png, [System.Drawing.Imaging.ImageFormat]::Png)
    $b.Dispose()
    $images += , @{ Size = $s; Bytes = [System.IO.File]::ReadAllBytes($png) }
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
