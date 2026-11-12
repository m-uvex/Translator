# Einrichtungs-Assistent fuer die Rust-Fassung des Uebersetzers
#
# Gegenstueck zu install.ps1 im Wurzelordner: gleiche Oberflaeche, aber ohne
# Node.js und Electron, weil die Rust-App als eine .exe laeuft. Stattdessen
# wird die gebaute uebersetzer.exe kopiert, Ollama samt Modellen eingerichtet
# und auf Wunsch NLLB-200 in einer eigenen Python-Umgebung installiert.
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing
[Windows.Forms.Application]::EnableVisualStyles()

trap {
  try { [void][System.Windows.Forms.MessageBox]::Show($_.Exception.Message, 'Fehler bei der Einrichtung') } catch {}
  exit 1
}

$script:src = $PSScriptRoot
$script:cmdlog = Join-Path $env:TEMP 'uebersetzer-rust-setup.log'
$script:done = $false

# ---------- Sprachauswahl (Deutsch und Englisch, sonst Englisch) ----------
$sysLang = [System.Globalization.CultureInfo]::CurrentUICulture.TwoLetterISOLanguageName
if ($sysLang -notin @('de', 'en')) { $sysLang = 'en' }

$T = @{
  de = @{
    title='Uebersetzer - Einrichtung'; heading='Uebersetzer einrichten'
    intro='Die App wird in den gewaehlten Ordner kopiert. Fehlende Programme (Ollama, Python) kommen aus den offiziellen Quellen.'
    folder='Installationsordner'; ollama='Ollama installieren (falls nicht vorhanden)'
    models='Sprachmodelle fuer Ollama herunterladen:'; nllb='NLLB-200 von Meta einrichten (Python 3.12) - Versionen:'
    desk='Desktop-Verknuepfung erstellen'; start='Eintraege im Startmenue erstellen (inkl. Deinstallation)'
    install='Installieren'; update='Aktualisieren'; startBtn='Uebersetzer starten'; retry='Erneut versuchen'
    lang='Sprache'; errTitle='Fehler bei der Einrichtung'
    noExe='Die uebersetzer.exe wurde nicht gefunden. Bitte zuerst "cargo build --release" im Ordner rust ausfuehren.'
    cargoBuild='Baue die App mit cargo (das dauert einige Minuten) ...'
    exeOk='OK: uebersetzer.exe vorhanden'; copy='Kopiere App-Dateien ...'; updating='Aktualisiere vorhandene Installation ...'
    ollamaOk='OK: Ollama vorhanden'; ollamaInst='Installiere Ollama ...'; ollamaDl='Lade Ollama direkt von ollama.com ...'; ollamaStart='Starte Ollama ...'
    modelOk='OK: Modell {0} ist bereits installiert'; modelDl='Lade Sprachmodell {0} ...'
    pyOk='OK: Python 3.12 vorhanden'; pyInst='Installiere Python 3.12 ...'; pyDl='Lade Python direkt von python.org ...'
    venv='Erstelle Python-Umgebung ...'; pip='Installiere Pakete (torch, transformers ...) - das dauert einige Minuten ...'
    allModels='OK: Alle gewaehlten NLLB-Modelle sind bereits vorhanden'; hf='Lade {0} von Hugging Face ...'
    shortcuts='Lege Verknuepfungen an ...'; uninstallReg='Trage die Deinstallation in die Windows-Einstellungen ein ...'
    done='OK: Fertig! Die App ist eingerichtet.'; error='FEHLER: '
    noOllama='Ollama konnte nicht installiert werden.'; noOllamaStart='Der Ollama-Server laesst sich nicht starten.'
    noPy='Python 3.12 konnte nicht installiert werden.'; noVenv='Die Python-Umgebung konnte nicht erstellt werden.'; noPip='Die Python-Pakete konnten nicht installiert werden.'
  }
  en = @{
    title='Translator - Setup'; heading='Set up Translator'
    intro='The app is copied into the chosen folder. Missing programs (Ollama, Python) come from official sources.'
    folder='Installation folder'; ollama='Install Ollama (if missing)'
    models='Download language models for Ollama:'; nllb='Set up NLLB-200 by Meta (Python 3.12) - versions:'
    desk='Create desktop shortcut'; start='Create Start menu entries (including uninstall)'
    install='Install'; update='Update'; startBtn='Launch Translator'; retry='Try again'
    lang='Language'; errTitle='Setup error'
    noExe='uebersetzer.exe was not found. Please run "cargo build --release" in the rust folder first.'
    cargoBuild='Building the app with cargo (this may take a few minutes) ...'
    exeOk='OK: uebersetzer.exe present'; copy='Copying app files ...'; updating='Updating existing installation ...'
    ollamaOk='OK: Ollama present'; ollamaInst='Installing Ollama ...'; ollamaDl='Downloading Ollama directly from ollama.com ...'; ollamaStart='Starting Ollama ...'
    modelOk='OK: model {0} already installed'; modelDl='Downloading language model {0} ...'
    pyOk='OK: Python 3.12 present'; pyInst='Installing Python 3.12 ...'; pyDl='Downloading Python directly from python.org ...'
    venv='Creating Python environment ...'; pip='Installing packages (torch, transformers ...) - this may take a few minutes ...'
    allModels='OK: all selected NLLB models already present'; hf='Downloading {0} from Hugging Face ...'
    shortcuts='Creating shortcuts ...'; uninstallReg='Registering the uninstaller with Windows ...'
    done='OK: done! The app is set up.'; error='ERROR: '
    noOllama='Ollama could not be installed.'; noOllamaStart='Ollama server cannot be started.'
    noPy='Python 3.12 could not be installed.'; noVenv='Python environment could not be created.'; noPip='Python packages could not be installed.'
  }
}

$script:L = $T[$sysLang]
function tr($k) { if ($script:L.ContainsKey($k)) { return $script:L[$k] } else { return $T['en'][$k] } }

# ---------- Oberflaeche ----------
$f = New-Object Windows.Forms.Form
$f.Text = tr 'title'; $f.ClientSize = New-Object Drawing.Size(580, 700)
$f.MinimumSize = New-Object Drawing.Size(560, 660)
$f.StartPosition = 'CenterScreen'
$f.FormBorderStyle = 'Sizable'; $f.MaximizeBox = $true
$f.Font = New-Object Drawing.Font('Segoe UI', 10)

function Ctl($c, $x, $y, $w, $h, $t) {
  $c.Location = New-Object Drawing.Point($x, $y); $c.Size = New-Object Drawing.Size($w, $h); if ($t) { $c.Text = $t }
  $f.Controls.Add($c); return $c
}

$h = Ctl (New-Object Windows.Forms.Label) 20 14 320 30 (tr 'heading')
$h.Font = New-Object Drawing.Font('Segoe UI', 15, [Drawing.FontStyle]::Bold)
$h.Anchor = 'Top,Left'

$lblLang = Ctl (New-Object Windows.Forms.Label) 330 20 60 22 ((tr 'lang') + ':')
$lblLang.Anchor = 'Top,Right'
$cmbLang = Ctl (New-Object Windows.Forms.ComboBox) 395 18 165 26 ''
$cmbLang.DropDownStyle = 'DropDownList'; $cmbLang.Anchor = 'Top,Right'
[void]$cmbLang.Items.AddRange(@('Deutsch', 'English'))
$cmbLang.SelectedIndex = if ($sysLang -eq 'de') { 0 } else { 1 }

$ctrlRefs = @{}
$ctrlRefs.intro = Ctl (New-Object Windows.Forms.Label) 20 52 540 40 (tr 'intro')
$ctrlRefs.intro.Anchor = 'Top,Left,Right'

$ctrlRefs.folderLbl = Ctl (New-Object Windows.Forms.Label) 20 100 540 22 (tr 'folder')
$ctrlRefs.folderLbl.Anchor = 'Top,Left,Right'
$txtDir = Ctl (New-Object Windows.Forms.TextBox) 20 124 540 26 (Join-Path $env:LOCALAPPDATA 'Programs\Uebersetzer')
$txtDir.Anchor = 'Top,Left,Right'

$chkOllama = Ctl (New-Object Windows.Forms.CheckBox) 20 164 540 26 (tr 'ollama')
$chkOllama.Anchor = 'Top,Left,Right'
$ctrlRefs.modelsLbl = Ctl (New-Object Windows.Forms.Label) 20 196 540 22 (tr 'models')
$ctrlRefs.modelsLbl.Anchor = 'Top,Left,Right'
$clbModels = Ctl (New-Object Windows.Forms.CheckedListBox) 20 220 540 78 ''
$clbModels.CheckOnClick = $true; $clbModels.Anchor = 'Top,Left,Right'
[void]$clbModels.Items.AddRange(@(
  'translategemma:4b - beste reine Uebersetzung (ca. 3,3 GB)',
  'gemma3:4b - Stil, Kontext, Glossar, Wortalternativen (ca. 3,3 GB)',
  'qwen3.5:2b - klein, mit Bildverstaendnis (ca. 2,7 GB)'))
$clbModels.SetItemChecked(0, $true); $clbModels.SetItemChecked(1, $true)

$chkNllb = Ctl (New-Object Windows.Forms.CheckBox) 20 312 540 26 (tr 'nllb')
$chkNllb.Anchor = 'Top,Left,Right'
$clbNllb = Ctl (New-Object Windows.Forms.CheckedListBox) 44 340 516 78 ''
$clbNllb.CheckOnClick = $true; $clbNllb.Anchor = 'Top,Left,Right'; $clbNllb.Enabled = $false
[void]$clbNllb.Items.AddRange(@(
  'nllb:600M - schnell (ca. 2,5 GB)',
  'nllb:1.3B - genauer (ca. 5,5 GB)',
  'nllb:3.3B - am genauesten (ca. 17,6 GB, ~8 GB RAM, langsam)'))
$clbNllb.SetItemChecked(0, $true)

$chkDesk = Ctl (New-Object Windows.Forms.CheckBox) 20 432 540 26 (tr 'desk')
$chkDesk.Anchor = 'Top,Left,Right'
$chkStart = Ctl (New-Object Windows.Forms.CheckBox) 20 460 540 26 (tr 'start')
$chkStart.Anchor = 'Top,Left,Right'
foreach ($c in $chkOllama, $chkDesk, $chkStart) { $c.Checked = $true }
$chkNllb.Add_CheckedChanged({ $clbNllb.Enabled = $chkNllb.Checked })

$btn = Ctl (New-Object Windows.Forms.Button) 20 500 540 38 (tr 'install')
$btn.Anchor = 'Top,Left,Right'
$bar = Ctl (New-Object Windows.Forms.ProgressBar) 20 546 540 8 ''
$bar.Style = 'Marquee'; $bar.Visible = $false; $bar.Anchor = 'Top,Left,Right'
$lblStatus = Ctl (New-Object Windows.Forms.Label) 20 558 540 22 ''
$lblStatus.ForeColor = [Drawing.Color]::Gray; $lblStatus.Anchor = 'Top,Left,Right'
$txtLog = Ctl (New-Object Windows.Forms.TextBox) 20 584 540 96 ''
$txtLog.Multiline = $true; $txtLog.ReadOnly = $true; $txtLog.ScrollBars = 'Vertical'; $txtLog.Anchor = 'Top,Bottom,Left,Right'

# ---------- Hilfsfunktionen ----------
function Pump { [Windows.Forms.Application]::DoEvents() }
function Log($t) { $txtLog.AppendText($t + "`r`n"); $txtLog.SelectionStart = $txtLog.TextLength; $txtLog.ScrollToCaret(); Pump }
function Status($t) { $lblStatus.Text = $t; Pump }
function Wait-Ms($ms) { $sw = [Diagnostics.Stopwatch]::StartNew(); while ($sw.ElapsedMilliseconds -lt $ms) { Start-Sleep -Milliseconds 100; Pump } }
function Refresh-Path { $env:Path = [Environment]::GetEnvironmentVariable('Path', 'Machine') + ';' + [Environment]::GetEnvironmentVariable('Path', 'User') }
function Find-Cmd($n) { return (Get-Command $n -ErrorAction SilentlyContinue) }
function Run($cmd, $cwd) {
  if (-not $cwd) { $cwd = $script:src }
  $psi = New-Object Diagnostics.ProcessStartInfo
  $psi.FileName = 'cmd.exe'
  $psi.Arguments = '/d /s /c "' + $cmd + ' > "' + $script:cmdlog + '" 2>&1"'
  $psi.WorkingDirectory = $cwd; $psi.UseShellExecute = $false; $psi.CreateNoWindow = $true
  Remove-Item $script:cmdlog -ErrorAction SilentlyContinue
  $p = [Diagnostics.Process]::Start($psi)
  while (-not $p.HasExited) {
    Wait-Ms 300
    try {
      $fs = [IO.File]::Open($script:cmdlog, 'Open', 'Read', 'ReadWrite'); $sr = New-Object IO.StreamReader($fs)
      $all = $sr.ReadToEnd(); $sr.Close()
      $last = $all -split "[\r\n]+" | Where-Object { $_.Trim() } | Select-Object -Last 1
      if ($last) { Status ($last.Substring(0, [Math]::Min(90, $last.Length))) }
    } catch {}
  }
  $p.WaitForExit()
  return [int]$p.ExitCode
}
function Download($url, $out) { if ((Run "curl.exe -L --fail -o `"$out`" `"$url`"") -ne 0) { throw "Download fehlgeschlagen: $url" } }
function Winget($id) { if (Find-Cmd 'winget.exe') { [void](Run "winget install -e --id $id --silent --accept-package-agreements --accept-source-agreements"); Refresh-Path } }
function Stop-App { Get-Process uebersetzer -ErrorAction SilentlyContinue | Where-Object { $_.Path -and $_.Path.StartsWith($script:dir, [StringComparison]::OrdinalIgnoreCase) } | Stop-Process -Force -ErrorAction SilentlyContinue; Start-Sleep -Milliseconds 700 }

# ---------- Sprachwechsel ----------
function Apply-Lang($code) {
  $script:L = $T[$code]
  $f.Text = tr 'title'
  $h.Text = tr 'heading'
  $lblLang.Text = (tr 'lang') + ':'
  $ctrlRefs.intro.Text = tr 'intro'
  $ctrlRefs.folderLbl.Text = tr 'folder'
  $ctrlRefs.modelsLbl.Text = tr 'models'
  $chkOllama.Text = tr 'ollama'
  $chkNllb.Text = tr 'nllb'
  $chkDesk.Text = tr 'desk'
  $chkStart.Text = tr 'start'
  if ($script:done) { $btn.Text = tr 'startBtn' } else {
    $btn.Text = if (Test-Path (Join-Path $txtDir.Text.Trim() 'uebersetzer.exe')) { tr 'update' } else { tr 'install' }
  }
}
$cmbLang.Add_SelectedIndexChanged({ Apply-Lang $(if ($cmbLang.SelectedIndex -eq 0) { 'de' } else { 'en' }) })

# ---------- Komponenten ----------
function Find-Exe {
  foreach ($p in @(
      (Join-Path $script:src 'target\release\uebersetzer.exe'),
      (Join-Path $script:src 'target\debug\uebersetzer.exe'),
      (Join-Path $script:src 'uebersetzer.exe'))) {
    if (Test-Path $p) { return $p }
  }
  return $null
}
function Ensure-Exe {
  $exe = Find-Exe
  if ($exe -and $exe -like '*\release\*') { Log (tr 'exeOk'); return $exe }
  $cargo = Find-Cmd 'cargo.exe'
  if (-not $cargo) { $cargo = Find-Cmd 'cargo' }
  if ($cargo) {
    Log (tr 'cargoBuild')
    if ((Run 'cargo build --release' $script:src) -ne 0) { throw (tr 'noExe') }
    Refresh-Path
    $exe = Find-Exe
  }
  if (-not $exe) { throw (tr 'noExe') }
  Log (tr 'exeOk')
  return $exe
}
function Find-Ollama {
  $c = Find-Cmd 'ollama.exe'; if ($c) { return $c.Source }
  $p = Join-Path $env:LOCALAPPDATA 'Programs\Ollama\ollama.exe'; if (Test-Path $p) { return $p }
  return $null
}
function Ollama-Up { try { [void](Invoke-WebRequest 'http://127.0.0.1:11434' -UseBasicParsing -TimeoutSec 2); return $true } catch { return $false } }
function Ensure-Ollama {
  $o = Find-Ollama; if ($o) { Log (tr 'ollamaOk'); return $o }
  Log (tr 'ollamaInst'); Winget 'Ollama.Ollama'; $o = Find-Ollama
  if (-not $o) {
    Log (tr 'ollamaDl'); $exe = Join-Path $env:TEMP 'OllamaSetup.exe'
    Download 'https://ollama.com/download/OllamaSetup.exe' $exe
    [void](Run "`"$exe`" /SILENT"); $o = Find-Ollama
  }
  if (-not $o) { throw (tr 'noOllama') }
  return $o
}
function Start-OllamaServer($o) {
  if (Ollama-Up) { return }
  Log (tr 'ollamaStart'); Start-Process $o -ArgumentList 'serve' -WindowStyle Hidden
  for ($i = 0; $i -lt 30 -and -not (Ollama-Up); $i++) { Wait-Ms 2000 }
  if (-not (Ollama-Up)) { throw (tr 'noOllamaStart') }
}
function Find-Py {
  foreach ($c in @(
      "$env:LOCALAPPDATA\Programs\Python\Python312\python.exe",
      "$env:LOCALAPPDATA\Python\pythoncore-3.12-64\python.exe",
      "$env:ProgramFiles\Python312\python.exe")) { if (Test-Path $c) { return $c } }
  try {
    if (Find-Cmd 'py.exe') {
      $o = (& py.exe -3.12 -c "import sys;print(sys.executable)" 2>$null)
      if ($LASTEXITCODE -eq 0 -and $o -and (Test-Path "$o")) { return "$o" }
    }
  } catch {}
  return $null
}
function Setup-Nllb($ids) {
  $py = Find-Py
  if (-not $py) {
    Log (tr 'pyInst'); Winget 'Python.Python.3.12'; $py = Find-Py
    if (-not $py) {
      Log (tr 'pyDl'); $exe = Join-Path $env:TEMP 'python-3.12.10-amd64.exe'
      Download 'https://www.python.org/ftp/python/3.12.10/python-3.12.10-amd64.exe' $exe
      [void](Run "`"$exe`" /quiet InstallAllUsers=0 PrependPath=1 Include_launcher=0"); $py = Find-Py
    }
  } else { Log (tr 'pyOk') }
  if (-not $py) { throw (tr 'noPy') }
  $venv = Join-Path $script:dir 'nllb-venv'; $vpy = Join-Path $venv 'Scripts\python.exe'
  if (-not (Test-Path $vpy)) {
    Log (tr 'venv')
    if ((Run "`"$py`" -m venv `"$venv`"") -ne 0) { throw (tr 'noVenv') }
  }
  Log (tr 'pip')
  if ((Run "`"$vpy`" -m pip install --progress-bar off torch transformers sentencepiece langcodes language-data huggingface_hub") -ne 0) { throw (tr 'noPip') }
  $inst = @()
  try { $out = & $vpy -u nllb_server.py --installed $script:dir 2>$null; if ($out) { $inst = @($out | ConvertFrom-Json) } } catch {}
  $missing = @($ids | Where-Object { $inst -notcontains $_ })
  if ($missing.Count -eq 0) { Log (tr 'allModels') }
  else {
    foreach ($id in $missing) {
      Log ((tr 'hf') -f $id)
      if ((Run "`"$vpy`" -u nllb_server.py --install $id" $script:dir) -ne 0) { throw "$id" }
    }
  }
}
function New-Shortcut($lnk) {
  $s = (New-Object -ComObject WScript.Shell).CreateShortcut($lnk)
  $s.TargetPath = $script:run; $s.WorkingDirectory = $script:dir; $s.Description = 'Uebersetzer'
  $s.IconLocation = $script:run
  $s.Save()
}
function New-UninstallShortcut($lnk) {
  $s = (New-Object -ComObject WScript.Shell).CreateShortcut($lnk)
  $s.TargetPath = 'powershell.exe'
  $s.Arguments = '-NoProfile -ExecutionPolicy Bypass -WindowStyle Hidden -File "' + (Join-Path $script:dir 'Uninstall.ps1') + '"'
  $s.WorkingDirectory = $script:dir; $s.Description = 'Uebersetzer deinstallieren'
  $s.IconLocation = $script:run
  $s.Save()
}
function New-NllbShortcut($lnk) {
  $s = (New-Object -ComObject WScript.Shell).CreateShortcut($lnk)
  $s.TargetPath = (Join-Path $script:dir 'nllb-venv\Scripts\pythonw.exe')
  $s.Arguments = '"' + (Join-Path $script:dir 'nllb_server.py') + '"'
  $s.WorkingDirectory = $script:dir; $s.Description = 'NLLB-200-Uebersetzungsserver'
  $s.Save()
}
function Set-UninstallEntry {
  $key = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\UebersetzerRust'
  New-Item -Path $key -Force | Out-Null
  $size = 0
  try { $size = [int](((Get-ChildItem $script:dir -Recurse -File -ErrorAction SilentlyContinue | Measure-Object Length -Sum).Sum) / 1KB) } catch {}
  $exe = Join-Path $script:dir 'uebersetzer.exe'
  $vals = @{
    DisplayName = 'Uebersetzer'; DisplayIcon = $exe; DisplayVersion = '1.0'
    Publisher = 'Uebersetzer'; InstallLocation = $script:dir
    UninstallString = 'powershell.exe -NoProfile -ExecutionPolicy Bypass -WindowStyle Hidden -File "' + (Join-Path $script:dir 'Uninstall.ps1') + '"'
    QuietUninstallString = 'powershell.exe -NoProfile -ExecutionPolicy Bypass -WindowStyle Hidden -File "' + (Join-Path $script:dir 'Uninstall.ps1') + '" -quiet'
    NoModify = 1; NoRepair = 1; EstimatedSize = $size
  }
  foreach ($k in $vals.Keys) { New-ItemProperty -Path $key -Name $k -Value $vals[$k] -PropertyType ($(if ($vals[$k] -is [int]) { 'DWord' } else { 'String' })) -Force | Out-Null }
}

# ---------- Ablauf ----------
$txtDir.Add_TextChanged({ if (-not $script:done) { $btn.Text = if (Test-Path (Join-Path $txtDir.Text.Trim() 'uebersetzer.exe')) { tr 'update' } else { tr 'install' } } })
$btn.Add_Click({
  if ($script:done) { Start-Process $script:run; $f.Close(); return }
  $script:dir = $txtDir.Text.Trim()
  foreach ($c in $f.Controls) {
    if ($c -ne $txtLog -and $c -ne $lblStatus -and $c -ne $bar -and $c -ne $cmbLang -and $c -ne $lblLang) { $c.Enabled = $false }
  }
  $bar.Visible = $true
  try {
    $exe = Ensure-Exe
    $isUpdate = Test-Path (Join-Path $script:dir 'uebersetzer.exe')
    if ($isUpdate) { Log (tr 'updating'); Stop-App }
    Log (tr 'copy')
    [void](New-Item -ItemType Directory -Force $script:dir)
    $targetExe = Join-Path $script:dir 'uebersetzer.exe'
    if ((Resolve-Path $exe).Path -ne (Resolve-Path $targetExe -ErrorAction SilentlyContinue).Path) { Copy-Item $exe $targetExe -Force }
    $script:run = $targetExe
    $script:py = $null
    foreach ($n in 'nllb_server.py', 'requirements.txt', 'Uninstall.ps1', 'Installieren.bat') {
      foreach ($base in $script:src, (Split-Path $script:src -Parent)) {
        $from = Join-Path $base $n
        if (Test-Path $from) { Copy-Item $from $script:dir -Force; if ($n -eq 'nllb_server.py') { $script:py = Join-Path $script:dir $n }; break }
      }
    }
    foreach ($d in 'dict', 'assets') {
      if (Test-Path (Join-Path $script:src $d)) { Copy-Item (Join-Path $script:src $d) $script:dir -Recurse -Force }
    }

    $tags = @()
    for ($i = 0; $i -lt $clbModels.Items.Count; $i++) { if ($clbModels.GetItemChecked($i)) { $tags += ($clbModels.Items[$i] -split ' ')[0] } }
    if ($chkOllama.Checked -or $tags.Count -gt 0) {
      $o = Ensure-Ollama
      if ($tags.Count -gt 0) {
        Start-OllamaServer $o
        $have = @()
        try { $have = (& $o list) -split "`r?`n" | Select-Object -Skip 1 | ForEach-Object { ($_ -split '\s+')[0] } } catch {}
        foreach ($tag in $tags) {
          if ($have -contains $tag) { Log ((tr 'modelOk') -f $tag) }
          else { Log ((tr 'modelDl') -f $tag); if ((Run "`"$o`" pull $tag") -ne 0) { throw "$tag" } }
        }
      }
    }

    $ids = @()
    for ($i = 0; $i -lt $clbNllb.Items.Count; $i++) { if ($clbNllb.GetItemChecked($i)) { $ids += ($clbNllb.Items[$i] -split ' ')[0] } }
    if ($chkNllb.Checked -and $ids.Count -gt 0) { Setup-Nllb $ids }

    if ($chkDesk.Checked -or $chkStart.Checked) { Log (tr 'shortcuts') }
    if ($chkDesk.Checked) { New-Shortcut (Join-Path ([Environment]::GetFolderPath('Desktop')) 'Uebersetzer.lnk') }
    if ($chkStart.Checked) {
      New-Shortcut (Join-Path ([Environment]::GetFolderPath('Programs')) 'Uebersetzer.lnk')
      New-UninstallShortcut (Join-Path ([Environment]::GetFolderPath('Programs')) 'Uebersetzer deinstallieren.lnk')
      $vpy = Join-Path $script:dir 'nllb-venv\Scripts\pythonw.exe'
      if ($chkNllb.Checked -and (Test-Path $vpy)) { New-NllbShortcut (Join-Path ([Environment]::GetFolderPath('Programs')) 'Uebersetzer NLLB-Server.lnk') }
    }
    Log (tr 'uninstallReg')
    Set-UninstallEntry
    Log (tr 'done')
    $script:done = $true; $btn.Text = tr 'startBtn'; $btn.Enabled = $true
  } catch {
    Log ((tr 'error') + $_.Exception.Message)
    if (Test-Path $script:cmdlog) { Get-Content $script:cmdlog -Tail 8 -ErrorAction SilentlyContinue | ForEach-Object { Log ('  ' + $_) } }
    foreach ($c in $f.Controls) { $c.Enabled = $true }
    $clbNllb.Enabled = $chkNllb.Checked
    $btn.Text = tr 'retry'
  } finally { $bar.Visible = $false; Status '' }
})
if (Test-Path (Join-Path $txtDir.Text 'uebersetzer.exe')) { $btn.Text = tr 'update' }
[void]$f.ShowDialog()
