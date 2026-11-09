# Einrichtungs-Assistent fuer den Uebersetzer (Installation, Aktualisierung, Deinstallation)
#
# HINWEIS: Omnilingual MT (OMT) von Meta ist derzeit (Stand 2026) nicht als
# oeffentliches Modell verfuegbar. Alle OMT-Optionen sind auskommentiert.
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing
[Windows.Forms.Application]::EnableVisualStyles()

trap {
  try { [void][System.Windows.Forms.MessageBox]::Show($_.Exception.Message, 'Fehler bei der Einrichtung') } catch {}
  exit 1
}

$script:src = $PSScriptRoot
if (-not $script:src -or -not (Test-Path (Join-Path $script:src 'main.js'))) {
  [void][System.Windows.Forms.MessageBox]::Show('Bitte die ZIP-Datei zuerst vollstaendig entpacken und Installieren.bat aus dem entpackten Ordner starten.', 'Einrichtung'); exit
}
$script:cmdlog = Join-Path $env:TEMP 'uebersetzer-setup.log'
$script:done = $false

# ---------- Sprachauswahl ----------
$sysLang = [System.Globalization.CultureInfo]::CurrentUICulture.TwoLetterISOLanguageName
if ($sysLang -notin @('de','en','nl','es','tr','ar')) { $sysLang = 'en' }

$T = @{
  de = @{
    title='Uebersetzer - Einrichtung'; heading='Uebersetzer einrichten'
    intro='Fehlende Programme werden automatisch aus den offiziellen Quellen (winget, nodejs.org, ollama.com, python.org, Hugging Face) geladen.'
    folder='Installationsordner'; ollama='Ollama installieren (falls nicht vorhanden)'
    models='Sprachmodelle fuer Ollama herunterladen:'; nllb='NLLB-200 von Meta einrichten (Python 3.12) - Versionen:'
    auto='Uebersetzungs-Server automatisch mit der App starten'
    desk='Desktop-Verknuepfung erstellen'; start='Eintraege im Startmenue erstellen (inkl. Deinstallation)'
    install='Installieren'; update='Aktualisieren'; startBtn='Uebersetzer starten'; retry='Erneut versuchen'
    lang='Sprache'; errTitle='Fehler bei der Einrichtung'
    needUnzip='Bitte die ZIP-Datei zuerst vollstaendig entpacken und Installieren.bat aus dem entpackten Ordner starten.'
    nodeOk='OK: Node.js vorhanden'; nodeInst='Installiere Node.js (LTS) ...'; nodeDl='Lade Node.js direkt von nodejs.org ...'
    elecOk='OK: Electron und Pakete vorhanden'; elecInst='Installiere Electron ...'; pkgInst='Installiere npm-Pakete ...'
    elecMirror='Erneuter Versuch ueber einen Mirror ...'; elecDl='Lade Electron direkt von GitHub ...'
    updating='Aktualisiere vorhandene Installation ...'; copy='Kopiere App-Dateien ...'
    ollamaOk='OK: Ollama vorhanden'; ollamaInst='Installiere Ollama ...'; ollamaDl='Lade Ollama direkt von ollama.com ...'; ollamaStart='Starte Ollama ...'
    pyOk='OK: Python 3.12 vorhanden'; pyInst='Installiere Python 3.12 ...'; pyDl='Lade Python direkt von python.org ...'
    venv='Erstelle Python-Umgebung ...'; pip='Installiere Pakete (torch, transformers ...) - das dauert einige Minuten ...'
    modelOk='OK: Modell {0} ist bereits installiert'; modelDl='Lade Sprachmodell {0} ...'; hf='Lade {0} von Hugging Face ...'
    allModels='OK: Alle gewaehlten Uebersetzungsmodelle sind bereits vorhanden'
    done='OK: Fertig! Die App ist eingerichtet.'; error='FEHLER: '
    noNode='Node.js konnte nicht installiert werden.'; noElectron='Electron konnte nicht installiert werden (GitHub blockiert?).'
    noPackages='npm-Pakete konnten nicht installiert werden (nspell, Woerterbuecher).'; noOllama='Ollama konnte nicht installiert werden.'
    noPy='Python 3.12 konnte nicht installiert werden.'; noVenv='Python-Umgebung konnte nicht erstellt werden.'; noPip='Python-Pakete konnten nicht installiert werden.'
    noOllamaStart='Ollama-Server laesst sich nicht starten.'; epkgMissing='Electron-Paket fehlt (npm install fehlgeschlagen).'
  }
  en = @{
    title='Translator - Setup'; heading='Set up Translator'
    intro='Missing programs are downloaded automatically from official sources (winget, nodejs.org, ollama.com, python.org, Hugging Face).'
    folder='Installation folder'; ollama='Install Ollama (if missing)'
    models='Download language models for Ollama:'; nllb='Set up NLLB-200 by Meta (Python 3.12) - versions:'
    auto='Start translation server automatically with the app'
    desk='Create desktop shortcut'; start='Create Start menu entries (including uninstall)'
    install='Install'; update='Update'; startBtn='Launch Translator'; retry='Try again'
    lang='Language'; errTitle='Setup error'
    needUnzip='Please unpack the ZIP file completely first and run Installieren.bat from the unpacked folder.'
    nodeOk='OK: Node.js present'; nodeInst='Installing Node.js (LTS) ...'; nodeDl='Downloading Node.js directly from nodejs.org ...'
    elecOk='OK: Electron and packages present'; elecInst='Installing Electron ...'; pkgInst='Installing npm packages ...'
    elecMirror='Retrying via mirror ...'; elecDl='Downloading Electron directly from GitHub ...'
    updating='Updating existing installation ...'; copy='Copying app files ...'
    ollamaOk='OK: Ollama present'; ollamaInst='Installing Ollama ...'; ollamaDl='Downloading Ollama directly from ollama.com ...'; ollamaStart='Starting Ollama ...'
    pyOk='OK: Python 3.12 present'; pyInst='Installing Python 3.12 ...'; pyDl='Downloading Python directly from python.org ...'
    venv='Creating Python environment ...'; pip='Installing packages (torch, transformers ...) - this may take a few minutes ...'
    modelOk='OK: model {0} already installed'; modelDl='Downloading language model {0} ...'; hf='Downloading {0} from Hugging Face ...'
    allModels='OK: all selected translation models already present'
    done='OK: done! The app is set up.'; error='ERROR: '
    noNode='Node.js could not be installed.'; noElectron='Electron could not be installed (GitHub blocked?).'
    noPackages='npm packages could not be installed (nspell, dictionaries).'; noOllama='Ollama could not be installed.'
    noPy='Python 3.12 could not be installed.'; noVenv='Python environment could not be created.'; noPip='Python packages could not be installed.'
    noOllamaStart='Ollama server cannot be started.'; epkgMissing='Electron package missing (npm install failed).'
  }
  nl = @{
    title='Vertaler - Installatie'; heading='Vertaler instellen'
    intro='Ontbrekende programma''s worden automatisch gedownload van officiele bronnen (winget, nodejs.org, ollama.com, python.org, Hugging Face).'
    folder='Installatiemap'; ollama='Ollama installeren (indien niet aanwezig)'
    models='Taalmodellen voor Ollama downloaden:'; nllb='NLLB-200 van Meta instellen (Python 3.12) - versies:'
    auto='Vertaalserver automatisch met de app starten'
    desk='Snelkoppeling op bureaublad maken'; start='Startmenu-items maken (inclusief verwijderen)'
    install='Installeren'; update='Bijwerken'; startBtn='Vertaler starten'; retry='Opnieuw proberen'
    lang='Taal'; errTitle='Fout bij installatie'
    needUnzip='Pak het ZIP-bestand eerst volledig uit en start Installieren.bat uit de uitgepakte map.'
    nodeOk='OK: Node.js aanwezig'; nodeInst='Node.js (LTS) installeren ...'; nodeDl='Node.js rechtstreeks van nodejs.org downloaden ...'
    elecOk='OK: Electron en pakketten aanwezig'; elecInst='Electron installeren ...'; pkgInst='npm-pakketten installeren ...'
    elecMirror='Opnieuw via mirror ...'; elecDl='Electron rechtstreeks van GitHub downloaden ...'
    updating='Bestaande installatie bijwerken ...'; copy='App-bestanden kopieren ...'
    ollamaOk='OK: Ollama aanwezig'; ollamaInst='Ollama installeren ...'; ollamaDl='Ollama rechtstreeks van ollama.com downloaden ...'; ollamaStart='Ollama starten ...'
    pyOk='OK: Python 3.12 aanwezig'; pyInst='Python 3.12 installeren ...'; pyDl='Python rechtstreeks van python.org downloaden ...'
    venv='Python-omgeving aanmaken ...'; pip='Pakketten installeren (torch, transformers ...) - dit duurt een paar minuten ...'
    modelOk='OK: model {0} al geinstalleerd'; modelDl='Taalmodel {0} downloaden ...'; hf='{0} van Hugging Face downloaden ...'
    allModels='OK: alle gekozen vertaalmodellen zijn al aanwezig'
    done='OK: klaar! De app is ingesteld.'; error='FOUT: '
    noNode='Node.js kon niet worden geinstalleerd.'; noElectron='Electron kon niet worden geinstalleerd (GitHub geblokkeerd?).'
    noPackages='npm-pakketten konden niet worden geinstalleerd (nspell, woordenboeken).'; noOllama='Ollama kon niet worden geinstalleerd.'
    noPy='Python 3.12 kon niet worden geinstalleerd.'; noVenv='Python-omgeving kon niet worden aangemaakt.'; noPip='Python-pakketten konden niet worden geinstalleerd.'
    noOllamaStart='Ollama-server kan niet worden gestart.'; epkgMissing='Electron-pakket ontbreekt (npm install mislukt).'
  }
  es = @{
    title='Traductor - Configuracion'; heading='Configurar Traductor'
    intro='Los programas que falten se descargaran automaticamente de las fuentes oficiales (winget, nodejs.org, ollama.com, python.org, Hugging Face).'
    folder='Carpeta de instalacion'; ollama='Instalar Ollama (si no esta presente)'
    models='Descargar modelos de lenguaje para Ollama:'; nllb='Configurar NLLB-200 de Meta (Python 3.12) - versiones:'
    auto='Iniciar servidor de traduccion automaticamente con la app'
    desk='Crear acceso directo en el escritorio'; start='Crear accesos en el menu Inicio (incluye desinstalacion)'
    install='Instalar'; update='Actualizar'; startBtn='Iniciar Traductor'; retry='Reintentar'
    lang='Idioma'; errTitle='Error de instalacion'
    needUnzip='Por favor descomprima el archivo ZIP por completo primero y ejecute Installieren.bat desde la carpeta descomprimida.'
    nodeOk='OK: Node.js presente'; nodeInst='Instalando Node.js (LTS) ...'; nodeDl='Descargando Node.js directamente de nodejs.org ...'
    elecOk='OK: Electron y paquetes presentes'; elecInst='Instalando Electron ...'; pkgInst='Instalando paquetes npm ...'
    elecMirror='Reintentando mediante mirror ...'; elecDl='Descargando Electron directamente de GitHub ...'
    updating='Actualizando instalacion existente ...'; copy='Copiando archivos de la app ...'
    ollamaOk='OK: Ollama presente'; ollamaInst='Instalando Ollama ...'; ollamaDl='Descargando Ollama directamente de ollama.com ...'; ollamaStart='Iniciando Ollama ...'
    pyOk='OK: Python 3.12 presente'; pyInst='Instalando Python 3.12 ...'; pyDl='Descargando Python directamente de python.org ...'
    venv='Creando entorno Python ...'; pip='Instalando paquetes (torch, transformers ...) - esto puede tardar unos minutos ...'
    modelOk='OK: modelo {0} ya instalado'; modelDl='Descargando modelo de lenguaje {0} ...'; hf='Descargando {0} de Hugging Face ...'
    allModels='OK: todos los modelos de traduccion seleccionados ya estan presentes'
    done='OK: listo! La aplicacion esta configurada.'; error='ERROR: '
    noNode='No se pudo instalar Node.js.'; noElectron='No se pudo instalar Electron (GitHub bloqueado?).'
    noPackages='No se pudieron instalar los paquetes npm (nspell, diccionarios).'; noOllama='No se pudo instalar Ollama.'
    noPy='No se pudo instalar Python 3.12.'; noVenv='No se pudo crear el entorno Python.'; noPip='No se pudieron instalar los paquetes Python.'
    noOllamaStart='No se puede iniciar el servidor Ollama.'; epkgMissing='Falta el paquete Electron (npm install fallido).'
  }
  tr = @{
    title='Cevirmen - Kurulum'; heading='Cevirmen Kurulumu'
    intro='Eksik programlar resmi kaynaklardan (winget, nodejs.org, ollama.com, python.org, Hugging Face) otomatik indirilir.'
    folder='Kurulum klasoru'; ollama='Ollama yukle (mevcut degilse)'
    models='Ollama icin dil modellerini indir:'; nllb='Meta NLLB-200 kurulumu (Python 3.12) - surumler:'
    auto='Ceviri sunucusunu uygulama ile otomatik baslat'
    desk='Masaustu kisayolu olustur'; start='Baslat menusu ogeleri olustur (kaldirma dahil)'
    install='Yukle'; update='Guncelle'; startBtn='Cevirmeni Baslat'; retry='Yeniden dene'
    lang='Dil'; errTitle='Kurulum hatasi'
    needUnzip='Lutfen once ZIP dosyasini tamamen cikarin ve cikarilan klasorden Installieren.bat calistirin.'
    nodeOk='TAMAM: Node.js mevcut'; nodeInst='Node.js (LTS) yukleniyor ...'; nodeDl='Node.js dogrudan nodejs.org uzerinden indiriliyor ...'
    elecOk='TAMAM: Electron ve paketler mevcut'; elecInst='Electron yukleniyor ...'; pkgInst='npm paketleri yukleniyor ...'
    elecMirror='Mirror uzerinden tekrar deneniyor ...'; elecDl='Electron dogrudan GitHub uzerinden indiriliyor ...'
    updating='Mevcut kurulum guncelleniyor ...'; copy='Uygulama dosyalari kopyalaniyor ...'
    ollamaOk='TAMAM: Ollama mevcut'; ollamaInst='Ollama yukleniyor ...'; ollamaDl='Ollama dogrudan ollama.com uzerinden indiriliyor ...'; ollamaStart='Ollama baslatiliyor ...'
    pyOk='TAMAM: Python 3.12 mevcut'; pyInst='Python 3.12 yukleniyor ...'; pyDl='Python dogrudan python.org uzerinden indiriliyor ...'
    venv='Python ortami olusturuluyor ...'; pip='Paketler yukleniyor (torch, transformers ...) - birkac dakika surebilir ...'
    modelOk='TAMAM: {0} modeli zaten yuklu'; modelDl='Dil modeli {0} indiriliyor ...'; hf='{0} Hugging Face uzerinden indiriliyor ...'
    allModels='TAMAM: Secilen tum ceviri modelleri zaten mevcut'
    done='TAMAM: bitti! Uygulama kuruldu.'; error='HATA: '
    noNode='Node.js yuklenemedi.'; noElectron='Electron yuklenemedi (GitHub engellendi mi?).'
    noPackages='npm paketleri yuklenemedi (nspell, sozlukler).'; noOllama='Ollama yuklenemedi.'
    noPy='Python 3.12 yuklenemedi.'; noVenv='Python ortami olusturulamadi.'; noPip='Python paketleri yuklenemedi.'
    noOllamaStart='Ollama sunucusu baslatilamiyor.'; epkgMissing='Electron paketi eksik (npm install basarisiz).'
  }
  ar = @{
    title='Almotargem - Al Eidad'; heading='Eidad Almotargem'
    intro='Al barameg al mafqoda yatam tahmiloha talqaiyan men al masader al rasmiya (winget, nodejs.org, ollama.com, python.org, Hugging Face).'
    folder='Mogallad al tathbeet'; ollama='Tathbeet Ollama (etha lam yakon mawgoodan)'
    models='Tahmil namatheg al logha li Ollama:'; nllb='Eidad NLLB-200 men Meta (Python 3.12) - al esdarat:'
    auto='Tashgheel server al targama talqaiyan maa al tatbeeq'
    desk='Inshaa ekhtesar ala sath al maktab'; start='Inshaa qawaem fi qaemat al bedaya'
    install='Tathbeet'; update='Tahdeeth'; startBtn='Tashgheel Almotargem'; retry='Iadat al mohawala'
    lang='Al Logha'; errTitle='Khata fi al eidad'
    needUnzip='Yorga fak al daght an milaf ZIP awalan thoma tashgheel Installieren.bat men al mogallad.'
    nodeOk='Tamam: Node.js mawgood'; nodeInst='Gari tathbeet Node.js (LTS) ...'; nodeDl='Tahmil Node.js mobasharatan men nodejs.org ...'
    elecOk='Tamam: Electron wa al hozam mawgooda'; elecInst='Gari tathbeet Electron ...'; pkgInst='Gari tathbeet hozam npm ...'
    elecMirror='Iadat al mohawala abr mirror ...'; elecDl='Tahmil Electron mobasharatan men GitHub ...'
    updating='Tahdeeth al tathbeet al haly ...'; copy='Naskh milafat al tatbeeq ...'
    ollamaOk='Tamam: Ollama mawgood'; ollamaInst='Gari tathbeet Ollama ...'; ollamaDl='Tahmil Ollama mobasharatan men ollama.com ...'; ollamaStart='Gari tashgheel Ollama ...'
    pyOk='Tamam: Python 3.12 mawgood'; pyInst='Gari tathbeet Python 3.12 ...'; pyDl='Tahmil Python mobasharatan men python.org ...'
    venv='Inshaa beeat Python ...'; pip='Tathbeet al hozam (torch, transformers ...) - qad yastaghreq daqaiq ...'
    modelOk='Tamam: al namothag {0} mothabat belfeel'; modelDl='Gari tahmil namothag al logha {0} ...'; hf='Gari tahmil {0} men Hugging Face ...'
    allModels='Tamam: gamee namatheg al targama al mokhtara mawgooda'
    done='Tamam: jahiz! Al tatbeeq moadd belkamel.'; error='KHATA: '
    noNode='Taathara tathbeet Node.js.'; noElectron='Taathara tathbeet Electron (hal GitHub mahgoob?).'
    noPackages='Taathara tathbeet hozam npm (nspell, al maagem).'; noOllama='Taathara tathbeet Ollama.'
    noPy='Taathara tathbeet Python 3.12.'; noVenv='Taathara inshaa beeat Python.'; noPip='Taathara tathbeet hozam Python.'
    noOllamaStart='La yumken tashgheel khadem Ollama.'; epkgMissing='Hozmat Electron mafqoda (fashila npm install).'
  }
}

$script:L = $T[$sysLang]
$script:langCode = $sysLang
function tr($k) { if ($script:L.ContainsKey($k)) { return $script:L[$k] } else { return $T['en'][$k] } }

# ---------- Oberflaeche (resizable) ----------
$f = New-Object Windows.Forms.Form
$f.Text = tr 'title'; $f.ClientSize = New-Object Drawing.Size(580, 740)
$f.MinimumSize = New-Object Drawing.Size(560, 700)
$f.StartPosition = 'CenterScreen'
$f.FormBorderStyle = 'Sizable'; $f.MaximizeBox = $true
$f.Font = New-Object Drawing.Font('Segoe UI', 10)

function Ctl($c, $x, $y, $w, $h, $t) {
  $c.Location = New-Object Drawing.Point($x, $y); $c.Size = New-Object Drawing.Size($w, $h); if ($t) { $c.Text = $t }
  $f.Controls.Add($c); return $c
}

# Header-Zeile: Titel links, Sprachwahl rechts
$h = Ctl (New-Object Windows.Forms.Label) 20 14 320 30 (tr 'heading')
$h.Font = New-Object Drawing.Font('Segoe UI', 15, [Drawing.FontStyle]::Bold)
$h.Anchor = 'Top,Left'

$lblLang = Ctl (New-Object Windows.Forms.Label) 330 20 60 22 ((tr 'lang') + ':')
$lblLang.Anchor = 'Top,Right'
$cmbLang = Ctl (New-Object Windows.Forms.ComboBox) 395 18 165 26 ''
$cmbLang.DropDownStyle = 'DropDownList'
$cmbLang.Anchor = 'Top,Right'
[void]$cmbLang.Items.AddRange(@('Deutsch','English','Nederlands','Espanol','Turkce','Arabic'))
$cmbLang.SelectedIndex = switch ($sysLang) { 'de' { 0 } 'en' { 1 } 'nl' { 2 } 'es' { 3 } 'tr' { 4 } 'ar' { 5 } default { 1 } }

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
[void]$clbModels.Items.AddRange(@('translategemma:4b - beste reine Uebersetzung (ca. 3,3 GB)', 'gemma3:4b - Stil, Kontext, Glossar, Wortalternativen (ca. 3,3 GB)', 'qwen3.5:2b - klein, mit Bildverstaendnis (ca. 2,7 GB)'))
$clbModels.SetItemChecked(0, $true); $clbModels.SetItemChecked(1, $true)

$chkNllb = Ctl (New-Object Windows.Forms.CheckBox) 20 312 540 26 (tr 'nllb')
$chkNllb.Anchor = 'Top,Left,Right'
$clbNllb = Ctl (New-Object Windows.Forms.CheckedListBox) 44 340 516 78 ''
$clbNllb.CheckOnClick = $true; $clbNllb.Anchor = 'Top,Left,Right'; $clbNllb.Enabled = $false
[void]$clbNllb.Items.AddRange(@('nllb:600M - schnell (ca. 2,5 GB)', 'nllb:1.3B - genauer (ca. 5,5 GB)', 'nllb:3.3B - am genauesten (ca. 17,6 GB, ~8 GB RAM, langsam)'))
$clbNllb.SetItemChecked(0, $true)

$chkAuto = Ctl (New-Object Windows.Forms.CheckBox) 20 430 540 26 (tr 'auto')
$chkAuto.Anchor = 'Top,Left,Right'
$chkDesk = Ctl (New-Object Windows.Forms.CheckBox) 20 468 540 26 (tr 'desk')
$chkDesk.Anchor = 'Top,Left,Right'
$chkStart = Ctl (New-Object Windows.Forms.CheckBox) 20 496 540 26 (tr 'start')
$chkStart.Anchor = 'Top,Left,Right'
foreach ($c in $chkOllama, $chkAuto, $chkDesk, $chkStart) { $c.Checked = $true }
$chkAuto.Enabled = $false
$chkNllb.Add_CheckedChanged({ $clbNllb.Enabled = $chkNllb.Checked; $chkAuto.Enabled = $chkNllb.Checked })

$btn = Ctl (New-Object Windows.Forms.Button) 20 534 540 38 (tr 'install')
$btn.Anchor = 'Top,Left,Right'
$bar = Ctl (New-Object Windows.Forms.ProgressBar) 20 580 540 8 ''
$bar.Style = 'Marquee'; $bar.Visible = $false; $bar.Anchor = 'Top,Left,Right'
$lblStatus = Ctl (New-Object Windows.Forms.Label) 20 592 540 22 ''
$lblStatus.ForeColor = [Drawing.Color]::Gray; $lblStatus.Anchor = 'Top,Left,Right'
$txtLog = Ctl (New-Object Windows.Forms.TextBox) 20 618 540 100 ''
$txtLog.Multiline = $true; $txtLog.ReadOnly = $true; $txtLog.ScrollBars = 'Vertical'; $txtLog.Anchor = 'Top,Bottom,Left,Right'

# ---------- Hilfsfunktionen ----------
function Pump { [Windows.Forms.Application]::DoEvents() }
function Log($t) { $txtLog.AppendText($t + "`r`n"); Pump }
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
function Stop-App { Get-Process electron -ErrorAction SilentlyContinue | Where-Object { $_.Path -and $_.Path.StartsWith($script:dir, [StringComparison]::OrdinalIgnoreCase) } | Stop-Process -Force -ErrorAction SilentlyContinue; Start-Sleep -Milliseconds 700 }

# ---------- Sprachwechsel ----------
function Apply-Lang($code) {
  $script:langCode = $code
  $script:L = $T[$code]
  $f.Text = tr 'title'
  $h.Text = tr 'heading'
  $lblLang.Text = (tr 'lang') + ':'
  $ctrlRefs.intro.Text = tr 'intro'
  $ctrlRefs.folderLbl.Text = tr 'folder'
  $chkOllama.Text = tr 'ollama'
  $ctrlRefs.modelsLbl.Text = tr 'models'
  $chkNllb.Text = tr 'nllb'
  $chkAuto.Text = tr 'auto'
  $chkDesk.Text = tr 'desk'
  $chkStart.Text = tr 'start'
  if ($script:done) { $btn.Text = tr 'startBtn' } else {
    $btn.Text = if (Test-Path (Join-Path $txtDir.Text.Trim() 'package.json')) { tr 'update' } else { tr 'install' }
  }
}
$cmbLang.Add_SelectedIndexChanged({
  $code = switch ($cmbLang.SelectedIndex) { 0 { 'de' } 1 { 'en' } 2 { 'nl' } 3 { 'es' } 4 { 'tr' } 5 { 'ar' } default { 'en' } }
  Apply-Lang $code
})

# ---------- Komponenten ----------
function Ensure-Node {
  Refresh-Path
  if ((Find-Cmd 'node.exe') -and (Find-Cmd 'npm.cmd')) { Log (tr 'nodeOk'); return }
  Log (tr 'nodeInst'); Winget 'OpenJS.NodeJS.LTS'
  if (-not ((Find-Cmd 'node.exe') -and (Find-Cmd 'npm.cmd'))) {
    Log (tr 'nodeDl')
    $v = (Invoke-RestMethod 'https://nodejs.org/dist/index.json' | Where-Object { $_.lts } | Select-Object -First 1).version
    $msi = Join-Path $env:TEMP "node-$v-x64.msi"
    Download "https://nodejs.org/dist/$v/node-$v-x64.msi" $msi
    Start-Process 'msiexec.exe' -ArgumentList "/i `"$msi`" /qn /norestart" -Verb RunAs -Wait; Refresh-Path
  }
  if (-not (Find-Cmd 'npm.cmd')) { throw (tr 'noNode') }
}
function Test-Electron { Test-Path (Join-Path $script:dir 'node_modules\electron\dist\electron.exe') }
function Test-NodeModules {
  (Test-Path (Join-Path $script:dir 'node_modules\nspell\package.json')) -and
  (Test-Path (Join-Path $script:dir 'node_modules\dictionary-de\index.dic')) -and
  (Test-Path (Join-Path $script:dir 'node_modules\dictionary-en\index.dic')) -and
  (Test-Path (Join-Path $script:dir 'node_modules\dictionary-nl\index.dic')) -and
  (Test-Path (Join-Path $script:dir 'node_modules\mammoth\package.json')) -and
  (Test-Path (Join-Path $script:dir 'node_modules\pdf-parse\package.json'))
}
function Ensure-Electron {
  if ((Test-Electron) -and (Test-NodeModules)) { Log (tr 'elecOk'); return }
  if (-not (Test-NodeModules)) { Log (tr 'pkgInst') } else { Log (tr 'elecInst') }
  $env:ELECTRON_MIRROR = ''
  [void](Run 'npm.cmd install --no-audit --no-fund --omit=dev' $script:dir)
  if ((Test-Electron) -and (Test-NodeModules)) { Log (tr 'elecOk'); return }
  if (-not (Test-Electron)) {
    Log (tr 'elecMirror')
    $env:ELECTRON_MIRROR = 'https://npmmirror.com/mirrors/electron/'
    [void](Run 'node node_modules\electron\install.js' $script:dir)
    if ((Test-Electron) -and (Test-NodeModules)) { Log (tr 'elecOk'); return }
    Log (tr 'elecDl')
    $epkg = Join-Path $script:dir 'node_modules\electron\package.json'
    if (-not (Test-Path $epkg)) { throw (tr 'epkgMissing') }
    $v = (Get-Content $epkg -Raw | ConvertFrom-Json).version
    $zip = Join-Path $env:TEMP "electron-v$v-win32-x64.zip"
    Download "https://github.com/electron/electron/releases/download/v$v/electron-v$v-win32-x64.zip" $zip
    $dist = Join-Path $script:dir 'node_modules\electron\dist'
    New-Item -ItemType Directory -Force $dist | Out-Null
    if (Find-Cmd 'tar.exe') { [void](Run "tar.exe -xf `"$zip`" -C `"$dist`"") }
    if (-not (Test-Path (Join-Path $dist 'electron.exe'))) { Expand-Archive -Path $zip -DestinationPath $dist -Force }
    [IO.File]::WriteAllText((Join-Path $script:dir 'node_modules\electron\path.txt'), 'electron.exe')
    if (-not (Test-Electron)) { throw (tr 'noElectron') }
    Log (tr 'elecOk')
  }
  if (-not (Test-NodeModules)) { throw (tr 'noPackages') }
}
function Find-Ollama { $c = Find-Cmd 'ollama.exe'; if ($c) { return $c.Source }; $p = Join-Path $env:LOCALAPPDATA 'Programs\Ollama\ollama.exe'; if (Test-Path $p) { return $p }; return $null }
function Ollama-Up { try { [void](Invoke-WebRequest 'http://127.0.0.1:11434' -UseBasicParsing -TimeoutSec 2); return $true } catch { return $false } }
function Ensure-Ollama {
  $o = Find-Ollama; if ($o) { Log (tr 'ollamaOk'); return $o }
  Log (tr 'ollamaInst'); Winget 'Ollama.Ollama'; $o = Find-Ollama
  if (-not $o) { Log (tr 'ollamaDl'); $exe = Join-Path $env:TEMP 'OllamaSetup.exe'; Download 'https://ollama.com/download/OllamaSetup.exe' $exe; [void](Run "`"$exe`" /SILENT"); $o = Find-Ollama }
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
  foreach ($c in @("$env:LOCALAPPDATA\Programs\Python\Python312\python.exe", "$env:LOCALAPPDATA\Python\pythoncore-3.12-64\python.exe", "$env:ProgramFiles\Python312\python.exe")) { if (Test-Path $c) { return $c } }
  try { if (Find-Cmd 'py.exe') { $o = (& py.exe -3.12 -c "import sys;print(sys.executable)" 2>$null); if ($LASTEXITCODE -eq 0 -and $o -and (Test-Path "$o")) { return "$o" } } } catch {}
  return $null
}
function Setup-Mt($ids, $auto) {
  $py = Find-Py
  if (-not $py) {
    Log (tr 'pyInst'); Winget 'Python.Python.3.12'; $py = Find-Py
    if (-not $py) { Log (tr 'pyDl'); $exe = Join-Path $env:TEMP 'python-3.12.10-amd64.exe'; Download 'https://www.python.org/ftp/python/3.12.10/python-3.12.10-amd64.exe' $exe; [void](Run "`"$exe`" /quiet InstallAllUsers=0 PrependPath=1 Include_launcher=0"); $py = Find-Py }
  } else { Log (tr 'pyOk') }
  if (-not $py) { throw (tr 'noPy') }
  $venv = Join-Path $script:dir 'nllb-venv'; $vpy = Join-Path $venv 'Scripts\python.exe'
  if (-not (Test-Path $vpy)) { Log (tr 'venv'); if ((Run "`"$py`" -m venv `"$venv`"") -ne 0) { throw (tr 'noVenv') } }
  Log (tr 'pip')
  if ((Run "`"$vpy`" -m pip install --progress-bar off torch transformers sentencepiece langcodes language-data huggingface_hub") -ne 0) { throw (tr 'noPip') }
  $inst = @()
  try { $out = & $vpy -u nllb_server.py --installed $script:dir 2>$null; if ($out) { $inst = @($out | ConvertFrom-Json) } } catch {}
  $missing = @($ids | Where-Object { $inst -notcontains $_ })
  if ($missing.Count -eq 0) { Log (tr 'allModels') }
  else { foreach ($id in $missing) { Log ((tr 'hf') -f $id); if ((Run "`"$vpy`" -u nllb_server.py --install $id" $script:dir) -ne 0) { throw "$id" } } }
  @{ nllbAutostart = [bool]$auto } | ConvertTo-Json | Set-Content (Join-Path $script:dir 'config.json') -Encoding ASCII
}
function New-Shortcut($lnk) {
  $s = (New-Object -ComObject WScript.Shell).CreateShortcut($lnk)
  $s.TargetPath = $script:electron; $s.Arguments = '"' + $script:dir + '"'; $s.WorkingDirectory = $script:dir; $s.Description = 'Uebersetzer'
  $ico = Join-Path $script:dir 'build\icon.ico'
  if (Test-Path $ico) { $s.IconLocation = $ico } else { $s.IconLocation = $script:electron }
  $s.Save()
}
function New-UninstallShortcut($lnk) {
  $s = (New-Object -ComObject WScript.Shell).CreateShortcut($lnk)
  $s.TargetPath = 'powershell.exe'
  $s.Arguments = '-NoProfile -ExecutionPolicy Bypass -WindowStyle Hidden -File "' + (Join-Path $script:dir 'Uninstall.ps1') + '"'
  $s.WorkingDirectory = $script:dir; $s.Description = 'Uebersetzer deinstallieren'
  $ico = Join-Path $script:dir 'build\icon.ico'
  if (Test-Path $ico) { $s.IconLocation = $ico }
  $s.Save()
}

# ---------- Ablauf ----------
$txtDir.Add_TextChanged({ if (-not $script:done) { $btn.Text = if (Test-Path (Join-Path $txtDir.Text.Trim() 'package.json')) { tr 'update' } else { tr 'install' } } })
$btn.Add_Click({
  if ($script:done) { Start-Process $script:electron -ArgumentList ('"' + $script:dir + '"'); $f.Close(); return }
  $script:dir = $txtDir.Text.Trim()
  foreach ($c in $f.Controls) { if ($c -ne $txtLog -and $c -ne $lblStatus -and $c -ne $bar -and $c -ne $cmbLang -and $c -ne $lblLang) { $c.Enabled = $false } }
  $bar.Visible = $true
  try {
    Ensure-Node
    $isUpdate = Test-Path (Join-Path $script:dir 'package.json')
    if ($isUpdate) { Log (tr 'updating'); Stop-App }
    Log (tr 'copy')
    [void](New-Item -ItemType Directory -Force $script:dir)
    if ((Resolve-Path $script:dir).Path -ne (Resolve-Path $script:src).Path) {
      foreach ($n in 'main.js','preload.js','index.html','style.css','renderer.js','editor.js','nllb_server.py','package.json','requirements.txt','Uninstall.ps1') { Copy-Item (Join-Path $script:src $n) $script:dir -Force }
      if (Test-Path (Join-Path $script:src 'build')) { Copy-Item (Join-Path $script:src 'build') $script:dir -Recurse -Force }
    }
    Ensure-Electron
    $script:electron = Join-Path $script:dir 'node_modules\electron\dist\electron.exe'

    $tags = @(); for ($i = 0; $i -lt $clbModels.Items.Count; $i++) { if ($clbModels.GetItemChecked($i)) { $tags += ($clbModels.Items[$i] -split ' ')[0] } }
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
    if ($ids.Count -gt 0) { Setup-Mt $ids ($chkAuto.Checked) }

    if ($chkDesk.Checked) { New-Shortcut (Join-Path ([Environment]::GetFolderPath('Desktop')) 'Uebersetzer.lnk') }
    if ($chkStart.Checked) {
      New-Shortcut (Join-Path ([Environment]::GetFolderPath('Programs')) 'Uebersetzer.lnk')
      New-UninstallShortcut (Join-Path ([Environment]::GetFolderPath('Programs')) 'Uebersetzer deinstallieren.lnk')
    }
    Log (tr 'done')
    $script:done = $true; $btn.Text = tr 'startBtn'; $btn.Enabled = $true
  } catch {
    Log ((tr 'error') + $_.Exception.Message)
    if (Test-Path $script:cmdlog) { Get-Content $script:cmdlog -Tail 8 -ErrorAction SilentlyContinue | ForEach-Object { Log ('  ' + $_) } }
    foreach ($c in $f.Controls) { $c.Enabled = $true }
    $clbNllb.Enabled = $chkNllb.Checked
    $chkAuto.Enabled = $chkNllb.Checked
    $btn.Text = tr 'retry'
  } finally { $bar.Visible = $false; Status '' }
})
if (Test-Path (Join-Path $txtDir.Text 'package.json')) { $btn.Text = tr 'update' }
[void]$f.ShowDialog()