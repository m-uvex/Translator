# Release in einem Lauf: Version, Bau, Icon, Installer, stille Installation,
# Start und /health. Die Reihenfolge ist nicht beliebig, sondern die, die sich
# in den letzten Iterationen als noetig erwiesen hat:
#   - electron steht fuer den Bau unter devDependencies (sonst packt
#     electron-builder das Modul in die App),
#   - der Installer bricht mit Exitcode 2 ab, solange die App laeuft,
#   - das Icon muss vor dem NSIS-Lauf gesetzt werden, weil --prepackaged den
#     Ordner unveraendert uebernimmt.
# Aufruf:  powershell -NoProfile -ExecutionPolicy Bypass -File tools/release.ps1
#          -SkipInstall   nur bauen
#          -SkipHealth    installieren, aber nicht starten und messen
#          -Version 1.15.190  feste Version statt naechster Patchstand
param(
  [switch]$SkipInstall,
  [switch]$SkipHealth,
  [string]$Version
)

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$appName = [char]0x00DC + 'bersetzer'   # "Uebersetzer" mit Umlaut, ASCII-Datei
$fail = { param($m) Write-Host ('ABBRUCH: ' + $m) -ForegroundColor Red; exit 1 }

Push-Location $root
try {
  $pkgPath = Join-Path $root 'package.json'
  $pkg = Get-Content $pkgPath -Raw | ConvertFrom-Json
  $old = $pkg.version
  if (-not $Version) {
    $parts = $old.Split('.')
    $parts[2] = [string]([int]$parts[2] + 1)
    $Version = $parts -join '.'
  }
  Write-Host ('Version ' + $old + ' -> ' + $Version)

  Copy-Item $pkgPath (Join-Path $root '.tmp-package.json') -Force
  & node -e "const f='package.json',fs=require('fs');const d=JSON.parse(fs.readFileSync(f,'utf8'));d.version=process.argv[1];d.devDependencies=d.devDependencies||{};if(d.dependencies&&d.dependencies.electron){d.devDependencies.electron=d.dependencies.electron;delete d.dependencies.electron;}fs.writeFileSync(f,JSON.stringify(d,null,1)+'\n');" $Version
  if ($LASTEXITCODE -ne 0) { & $fail 'package.json konnte nicht umgestellt werden' }

  Write-Host 'Baue Anwendung ...'
  if (Test-Path (Join-Path $root 'dist')) { Remove-Item (Join-Path $root 'dist') -Recurse -Force }
  # Die Schalter muessen in Anfuehrungszeichen stehen: PowerShell liest
  # "-c.win.signAndEditExecutable=false" sonst als eigenen Parameter und sucht
  # eine Datei dieses Namens.
  & npx electron-builder --win dir '-c.win.signAndEditExecutable=false' | Out-Host
  if ($LASTEXITCODE -ne 0) { & $fail 'electron-builder (dir) fehlgeschlagen' }

  $rcedit = Get-ChildItem (Join-Path $env:LOCALAPPDATA 'electron-builder\Cache\winCodeSign') -Recurse -Filter 'rcedit-x64.exe' -ErrorAction SilentlyContinue | Select-Object -First 1
  if ($rcedit) {
    & $rcedit.FullName (Join-Path $root ('dist/win-unpacked/' + $appName + '.exe')) --set-icon (Join-Path $root 'build/icon.ico') | Out-Host
  } else { Write-Host 'Hinweis: rcedit nicht gefunden, Icon bleibt das von electron-builder' }

  Write-Host 'Baue Installer ...'
  & npx electron-builder --win nsis --prepackaged (Join-Path $root 'dist/win-unpacked') '-c.win.signAndEditExecutable=false' | Out-Host
  if ($LASTEXITCODE -ne 0) { & $fail 'electron-builder (nsis) fehlgeschlagen' }

  # Zurueck auf die Fassung im Repository (electron gehoert dort zu den
  # Abhaengigkeiten), aber mit der neuen Versionsnummer: ein blosses
  # Zurueckkopieren haette den Versionssprung wieder aufgehoben.
  Move-Item (Join-Path $root '.tmp-package.json') $pkgPath -Force
  & node -e "const f='package.json',fs=require('fs');const d=JSON.parse(fs.readFileSync(f,'utf8'));d.version=process.argv[1];fs.writeFileSync(f,JSON.stringify(d,null,1)+'\n');" $Version
  if ($LASTEXITCODE -ne 0) { & $fail 'Versionsnummer konnte nicht gesetzt werden' }

  $setup = Join-Path $root ('dist/' + $appName + ' Setup ' + $Version + '.exe')
  if (-not (Test-Path $setup)) { & $fail ('Installer fehlt: ' + $setup) }
  Write-Host ('Fertig: ' + $setup)

  if ($SkipInstall) { Pop-Location; exit 0 }

  # Eine laufende App sperrt die zu ersetzenden Dateien; der Installer endet
  # dann mit Exitcode 2 und aendert nichts.
  Get-Process | Where-Object { $_.ProcessName -like '*ersetzer*' } | ForEach-Object { try { $_.Kill() } catch {} }
  Start-Sleep -Seconds 3

  $p = Start-Process -FilePath $setup -ArgumentList '/S' -PassThru
  $p.WaitForExit(240000) | Out-Null
  if (-not $p.HasExited) { & $fail 'Installer reagiert nicht' }
  if ($p.ExitCode -ne 0) { & $fail ('Installer-Exitcode ' + $p.ExitCode) }

  $asis = Join-Path $env:LOCALAPPDATA ('Programs/Uebersetzer/resources/app.asar')
  if (-not (Test-Path $asis)) { & $fail 'app.asar fehlt nach der Installation' }
  Write-Host ('Installiert: ' + (Get-Item $asis).LastWriteTime)

  if ($SkipHealth) { Pop-Location; exit 0 }

  Start-Process (Join-Path $env:LOCALAPPDATA ('Programs/Uebersetzer/' + $appName + '.exe'))
  $health = $null
  for ($i = 0; $i -lt 40; $i++) {
    Start-Sleep -Seconds 1
    try { $health = (Invoke-WebRequest -UseBasicParsing -TimeoutSec 2 'http://127.0.0.1:8765/health').Content; break } catch {}
  }
  if (-not $health) { & $fail '/health antwortet nicht' }
  Write-Host ('health: ' + $health)
  Write-Host 'Release fertig.'
} finally {
  if (Test-Path (Join-Path $root '.tmp-package.json')) { Copy-Item (Join-Path $root '.tmp-package.json') (Join-Path $root 'package.json') -Force }
  Pop-Location
}
