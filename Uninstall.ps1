$ErrorActionPreference = 'SilentlyContinue'
Add-Type -AssemblyName System.Windows.Forms
$dir = $PSScriptRoot
$r = [Windows.Forms.MessageBox]::Show("Übersetzer wirklich deinstallieren?`n`nDer Programmordner wird entfernt.", 'Deinstallieren', 'YesNo', 'Question')
if ($r -ne 'Yes') { exit }
Get-Process electron | Where-Object { $_.Path -and $_.Path.StartsWith($dir, [StringComparison]::OrdinalIgnoreCase) } | Stop-Process -Force
Start-Sleep -Milliseconds 800
$m = [Windows.Forms.MessageBox]::Show('Auch die heruntergeladenen NLLB-/Omnilingual-Modelle löschen? (mehrere GB)', 'Modelle', 'YesNo', 'Question')
if ($m -eq 'Yes') {
  $py = Join-Path $dir 'nllb-venv\Scripts\python.exe'
  if (Test-Path $py) { try { & $py (Join-Path $dir 'nllb_server.py') --purge } catch {} }
}
Remove-Item (Join-Path ([Environment]::GetFolderPath('Desktop')) 'Übersetzer.lnk')
Remove-Item (Join-Path ([Environment]::GetFolderPath('Programs')) 'Übersetzer.lnk')
Remove-Item (Join-Path ([Environment]::GetFolderPath('Programs')) 'Übersetzer deinstallieren.lnk')
Start-Process cmd.exe -ArgumentList '/c', "timeout /t 2 >nul & rmdir /s /q `"$dir`"" -WindowStyle Hidden