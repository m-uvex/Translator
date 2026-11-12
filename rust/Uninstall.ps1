# Deinstallation der Rust-Fassung des Uebersetzers
#
# Entfernt Verknuepfungen und den Eintrag in den Windows-Einstellungen und
# loescht auf Wunsch den Programmordner samt NLLB-Umgebung. Die Einstellungen
# der App (%APPDATA%\Uebersetzer\rust-settings.json) bleiben erhalten, solange
# die Frage nach den Daten mit "Nein" beantwortet wird.
param([switch]$quiet)

$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Windows.Forms
$src = $PSScriptRoot

function Ask($text, $title) {
  return [Windows.Forms.DialogResult]::Yes -eq [Windows.Forms.MessageBox]::Show($text, $title, 'YesNo', 'Question')
}

if (-not $quiet) {
  if (-not (Ask "Der Uebersetzer wird entfernt. Fortfahren?" 'Uebersetzer deinstallieren')) { exit 0 }
}

# Laufende App beenden
Get-Process uebersetzer -ErrorAction SilentlyContinue |
  Where-Object { $_.Path -and $_.Path.StartsWith($src, [StringComparison]::OrdinalIgnoreCase) } |
  Stop-Process -Force -ErrorAction SilentlyContinue
Start-Sleep -Milliseconds 500

# Verknuepfungen
foreach ($dir in @([Environment]::GetFolderPath('Desktop'), [Environment]::GetFolderPath('Programs'))) {
  foreach ($name in 'Uebersetzer.lnk', 'Uebersetzer deinstallieren.lnk', 'Uebersetzer NLLB-Server.lnk') {
    $p = Join-Path $dir $name
    if (Test-Path $p) { Remove-Item $p -Force -ErrorAction SilentlyContinue }
  }
}

# Eintrag in den Windows-Einstellungen
$key = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\UebersetzerRust'
if (Test-Path $key) { Remove-Item $key -Recurse -Force -ErrorAction SilentlyContinue }

# Programmordner
if (Test-Path $src) {
  $remove = $true
  if (-not $quiet) {
    $remove = Ask "Auch der Programmordner`n$src`nsamt NLLB-Umgebung und Modellen wird geloescht.`n`nLoeschen?" 'Ordner entfernen'
  }
  if ($remove) {
    try { Remove-Item $src -Recurse -Force -ErrorAction Stop }
    catch { [void][Windows.Forms.MessageBox]::Show("Der Ordner konnte nicht vollstaendig geloescht werden:`n$($_.Exception.Message)", 'Deinstallation') }
  }
}

# Einstellungen und Bibliothek der App
$data = Join-Path $env:APPDATA 'Uebersetzer'
if ((Test-Path $data) -and -not $quiet) {
  if (Ask "Auch die Einstellungen und die Bibliothek loeschen?`n$data" 'Daten entfernen') {
    Remove-Item $data -Recurse -Force -ErrorAction SilentlyContinue
  }
}

if (-not $quiet) { [void][Windows.Forms.MessageBox]::Show('Der Uebersetzer wurde entfernt.', 'Deinstallation') }
