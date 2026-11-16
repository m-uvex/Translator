# Erzeugt das Logo der App: build/icon.png (512 px) und build/icon.ico (256 bis 16 px).
# Aufruf: powershell -NoProfile -ExecutionPolicy Bypass -File build/make-icon.ps1
#
# Das Logo ist ein abgerundetes Quadrat in Babyblau mit einem großen Ü in
# Tintenfarbe: der Anfangsbuchstabe des Namens, wie ihn auch die Verknüpfung
# traegt. Bewusst keine Buchstabenpaare und keine Symbole aus der Oberflaeche —
# die stehen dort fuer einzelne Befehle und wuerden als App-Zeichen verwirren.
# Das Ü bleibt bis 16 px lesbar, weil es nur aus wenigen dicken Strichen besteht.
param([string]$Root = (Split-Path -Parent $PSScriptRoot))

Add-Type -AssemblyName System.Drawing

$fontFamily = 'Segoe UI'
$accentTop = [System.Drawing.Color]::FromArgb(255, 156, 207, 245)
$accentBottom = [System.Drawing.Color]::FromArgb(255, 126, 189, 238)
$ink = [System.Drawing.Color]::FromArgb(255, 14, 42, 68)

function New-LogoBitmap([int]$size) {
  $bmp = New-Object System.Drawing.Bitmap -ArgumentList $size, $size
  $g = [System.Drawing.Graphics]::FromImage($bmp)
  $g.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::AntiAlias
  $g.PixelOffsetMode = [System.Drawing.Drawing2D.PixelOffsetMode]::HighQuality
  $g.Clear([System.Drawing.Color]::Transparent)

  # Abgerundetes Quadrat: 22 Prozent der Kante, wie die Karten der App.
  $r = $size * 0.22
  $d = 2 * $r
  $rect = New-Object System.Drawing.RectangleF -ArgumentList 0, 0, $size, $size
  $shape = New-Object System.Drawing.Drawing2D.GraphicsPath
  $shape.AddArc($rect.X, $rect.Y, $d, $d, 180, 90)
  $shape.AddArc($rect.Right - $d, $rect.Y, $d, $d, 270, 90)
  $shape.AddArc($rect.Right - $d, $rect.Bottom - $d, $d, $d, 0, 90)
  $shape.AddArc($rect.X, $rect.Bottom - $d, $d, $d, 90, 90)
  $shape.CloseFigure()
  $brush = New-Object System.Drawing.Drawing2D.LinearGradientBrush -ArgumentList $rect, $accentTop, $accentBottom, 90.0
  $g.FillPath($brush, $shape)

  # Das Ü als Vektorpfad, auf 54 Prozent der Kante gebracht und mittig gesetzt.
  # Die Punkte zaehlen zum Pfad, deshalb sitzt das Zeichen damit optisch etwas
  # ueber der Mitte — das gleicht der Blick aus.
  $path = New-Object System.Drawing.Drawing2D.GraphicsPath
  $family = New-Object System.Drawing.FontFamily($fontFamily)
  $path.AddString([string][char]0x00DC, $family, [int][System.Drawing.FontStyle]::Bold, 200,
    (New-Object System.Drawing.PointF -ArgumentList 0, 0), [System.Drawing.StringFormat]::GenericTypographic)
  $family.Dispose()
  $b = $path.GetBounds()
  if ($b.Height -le 0) { throw 'Kein Glyph gezeichnet - Schrift pruefen.' }
  $scale = ($size * 0.54) / $b.Height
  $m = New-Object System.Drawing.Drawing2D.Matrix
  $m.Translate(-$b.X, -$b.Y, [System.Drawing.Drawing2D.MatrixOrder]::Append)
  $m.Scale($scale, $scale, [System.Drawing.Drawing2D.MatrixOrder]::Append)
  $m.Translate(($size - $b.Width * $scale) / 2, ($size - $b.Height * $scale) / 2,
    [System.Drawing.Drawing2D.MatrixOrder]::Append)
  $path.Transform($m)
  $inkBrush = New-Object System.Drawing.SolidBrush -ArgumentList $ink
  $g.FillPath($inkBrush, $path)

  $inkBrush.Dispose(); $m.Dispose(); $path.Dispose(); $brush.Dispose(); $shape.Dispose(); $g.Dispose()
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
