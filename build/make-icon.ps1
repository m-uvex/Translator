# Erzeugt das Logo der App: build/icon.png (512 px) und build/icon.ico (256 bis 16 px).
# Aufruf: powershell -NoProfile -ExecutionPolicy Bypass -File build/make-icon.ps1
#
# Das Logo ist ein weisses Blatt mit umgeknickter Ecke auf einem abgerundeten
# Feld in Babyblau. Oben links steht ẞ (Latein, U+1E9E) in Tintenfarbe, unten
# rechts Ж (Kyrillisch, U+0416) im Akzentblau - die Entsprechung zu den
# Buchstaben-Symbolen anderer Uebersetzer, ohne weitere Zutaten.
param([string]$Root = (Split-Path -Parent $PSScriptRoot))

Add-Type -AssemblyName System.Drawing

$fontFamily = 'Segoe UI'
$accentTop = [System.Drawing.Color]::FromArgb(255, 156, 207, 245)
$accentBottom = [System.Drawing.Color]::FromArgb(255, 122, 185, 234)
$ink = [System.Drawing.Color]::FromArgb(255, 14, 42, 68)
$accentInk = [System.Drawing.Color]::FromArgb(255, 42, 116, 181)
$foldTint = [System.Drawing.Color]::FromArgb(255, 228, 241, 253)
$foldLine = [System.Drawing.Color]::FromArgb(255, 205, 226, 245)
$white = [System.Drawing.Color]::White

function Draw-Glyph($g, $brush, [string]$ch, [single]$cx, [single]$cy, [single]$h) {
  $path = New-Object System.Drawing.Drawing2D.GraphicsPath
  $family = New-Object System.Drawing.FontFamily($fontFamily)
  $path.AddString($ch, $family, [int][System.Drawing.FontStyle]::Bold, 200, [System.Drawing.PointF]::new(0, 0), [System.Drawing.StringFormat]::GenericTypographic)
  $family.Dispose()
  $box = $path.GetBounds()
  $scale = $h / $box.Height
  $m = New-Object System.Drawing.Drawing2D.Matrix
  $m.Translate(-$box.X, -$box.Y, [System.Drawing.Drawing2D.MatrixOrder]::Append)
  $m.Scale($scale, $scale, [System.Drawing.Drawing2D.MatrixOrder]::Append)
  $m.Translate(($cx - $box.Width * $scale / 2), ($cy - $h / 2), [System.Drawing.Drawing2D.MatrixOrder]::Append)
  $path.Transform($m)
  $g.FillPath($brush, $path)
  $m.Dispose(); $path.Dispose()
}

# Feld mit 23 Prozent Rundung.
function FieldPath([single]$s) {
  $p = New-Object System.Drawing.Drawing2D.GraphicsPath
  $d = 2 * $s * 0.23
  $p.AddArc(0, 0, $d, $d, 180, 90)
  $p.AddArc(($s - $d), 0, $d, $d, 270, 90)
  $p.AddArc(($s - $d), ($s - $d), $d, $d, 0, 90)
  $p.AddArc(0, ($s - $d), $d, $d, 90, 90)
  $p.CloseFigure()
  return $p
}

# Blatt mit abgeschnittener Ecke oben rechts.
function DocumentPath([single]$x, [single]$y, [single]$w, [single]$h, [single]$r, [single]$fold) {
  $p = New-Object System.Drawing.Drawing2D.GraphicsPath
  $d = 2 * $r
  $p.AddLine(($x + $r), $y, ($x + $w - $fold), $y)
  $p.AddLine(($x + $w - $fold), $y, ($x + $w), ($y + $fold))
  $p.AddLine(($x + $w), ($y + $fold), ($x + $w), ($y + $h - $r))
  $p.AddArc(($x + $w - $d), ($y + $h - $d), $d, $d, 0, 90)
  $p.AddArc($x, ($y + $h - $d), $d, $d, 90, 90)
  $p.AddArc($x, $y, $d, $d, 180, 90)
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

  $field = FieldPath $s
  $gradient = New-Object System.Drawing.Drawing2D.LinearGradientBrush([System.Drawing.RectangleF]::new(0, 0, $s, $s), $accentTop, $accentBottom, 90.0)
  $g.FillPath($gradient, $field)

  $x = $s * 0.185; $y = $s * 0.115; $w = $s * 0.63; $h = $s * 0.77
  $r = $s * 0.08; $fold = $s * 0.155
  $card = DocumentPath $x $y $w $h $r $fold

  # Weicher Schatten in drei Lagen, damit das Blatt auf dem Feld liegt.
  for ($i = 3; $i -ge 1; $i--) {
    $m = New-Object System.Drawing.Drawing2D.Matrix
    $m.Translate(0, ($s * 0.007 * $i))
    $clone = $card.Clone()
    $clone.Transform($m)
    $sb = New-Object System.Drawing.SolidBrush([System.Drawing.Color]::FromArgb([int](18 / $i), 12, 48, 84))
    $g.FillPath($sb, $clone)
    $sb.Dispose(); $clone.Dispose(); $m.Dispose()
  }

  $whiteBrush = New-Object System.Drawing.SolidBrush($white)
  $g.FillPath($whiteBrush, $card)

  # Eselsohr: eingeklappte Ecke mit zarter Kante.
  $foldPoints = @(
    [System.Drawing.PointF]::new(($x + $w - $fold), $y),
    [System.Drawing.PointF]::new(($x + $w), ($y + $fold)),
    [System.Drawing.PointF]::new(($x + $w - $fold), ($y + $fold))
  )
  $tri = New-Object System.Drawing.Drawing2D.GraphicsPath
  $tri.AddPolygon($foldPoints)
  $tintBrush = New-Object System.Drawing.SolidBrush($foldTint)
  $g.FillPath($tintBrush, $tri)
  $pen = New-Object System.Drawing.Pen($foldLine, [single]($s * 0.006))
  $g.DrawPolygon($pen, $foldPoints)

  $inkBrush = New-Object System.Drawing.SolidBrush($ink)
  $blueBrush = New-Object System.Drawing.SolidBrush($accentInk)
  Draw-Glyph $g $inkBrush ([string][char]0x1E9E) ($s * 0.345) ($s * 0.335) ($s * 0.26)
  Draw-Glyph $g $blueBrush ([string][char]0x0416) ($s * 0.655) ($s * 0.665) ($s * 0.26)

  $blueBrush.Dispose(); $inkBrush.Dispose(); $pen.Dispose(); $tintBrush.Dispose(); $tri.Dispose()
  $whiteBrush.Dispose(); $card.Dispose(); $gradient.Dispose(); $field.Dispose(); $g.Dispose()
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
