const { app, BrowserWindow, ipcMain, dialog, session, clipboard, Tray, Menu, globalShortcut, nativeImage } = require('electron');
const { spawn, spawnSync } = require('child_process');
const http = require('http'), path = require('path'), fs = require('fs'), os = require('os');
const { pathToFileURL } = require('url');
let nllb = null, ollama = null, nllbWanted = false, win = null;

// Erlaubt queryLocalFonts() im Renderer (fuer die Schriftartenliste)
app.commandLine.appendSwitch('enable-features', 'FontAccess');

// ---------- Plattform: Windows und Linux ----------
// Alle Unterschiede zwischen den Systemen stehen hier beieinander, damit eine
// Funktion nicht auf einem System nachgezogen werden muss: dieselben Dienste,
// dieselben Ports, nur andere Pfade und andere Mittel zum Beenden.
const IS_WIN = process.platform === 'win32';

// Im gepackten Betrieb liegt der Code in app.asar. Alles, was ein fremdes
// Programm oeffnen muss - der Python-Server und die Woerterbuecher -, liegt
// daneben in app.asar.unpacked (siehe asarUnpack in package.json).
const resPath = (...parts) => path.join(__dirname, ...parts).replace(/app\.asar([/\\])/, 'app.asar.unpacked$1');

// Kindprozesse bekommen unter Linux eine eigene Prozessgruppe, damit das
// Beenden der Gruppe auch die Enkel erwischt; unter Windows erledigt das
// taskkill /t.
const runHidden = (cmd, args, extra) => spawn(cmd, args, Object.assign({ windowsHide: true, stdio: 'ignore', detached: !IS_WIN }, extra || {}));

// Ollama liegt unter Windows im Benutzerprofil, unter Linux in einem der
// ueblichen Systemordner; fehlt beides, entscheidet der PATH.
function ollamaExe() {
  const list = IS_WIN
    ? [path.join(process.env.LOCALAPPDATA || '', 'Programs', 'Ollama', 'ollama.exe')]
    : ['/usr/local/bin/ollama', '/usr/bin/ollama', path.join(os.homedir(), '.local', 'bin', 'ollama')];
  return list.find(p => p && fs.existsSync(p)) || 'ollama';
}

// Die Umgebung heisst auf beiden Systemen nllb-venv. Unter Windows liegt der
// Interpreter in Scripts\pythonw.exe (ohne Konsolenfenster), unter Linux in
// bin/python3. Zusaetzlich wird der Ordner im Benutzerprofil geprueft, weil
// eine AppImage-Datei schreibgeschuetzt eingehaengt wird.
function venvPython() {
  const dirs = [path.join(__dirname, 'nllb-venv'), path.join(app.getPath('userData'), 'nllb-venv')];
  const rels = IS_WIN ? ['Scripts/pythonw.exe', 'Scripts/python.exe'] : ['bin/python3', 'bin/python'];
  for (const d of dirs) {
    for (const r of rels) { const f = path.join(d, r); if (fs.existsSync(f)) return f; }
  }
  return null;
}

// config.json steht im Programmordner; im Benutzerprofil hat sie Vorrang, weil
// sie dort auch bei einer schreibgeschuetzten Installation aenderbar ist.
function readCfg() {
  const files = [path.join(app.getPath('userData'), 'config.json'), path.join(__dirname, 'config.json')];
  for (const f of files) { try { return JSON.parse(fs.readFileSync(f, 'utf8')); } catch {} }
  return {};
}

// ---------- Schreiben mit Umweg ----------
// Erst in <datei>.part schreiben, dann umbenennen. Ein Absturz mitten im
// Schreiben laesst so nur die .part-Datei zurueck; die eigentliche Datei bleibt
// die alte, vollstaendige Fassung. Vorher konnte ein hartes Beenden eine halb
// geschriebene Datei hinterlassen, die beim naechsten Start nicht mehr lesbar
// war.
function safeWrite(file, data, enc) {
  const tmp = file + '.part';
  try {
    fs.mkdirSync(path.dirname(file), { recursive: true });
    const fd = fs.openSync(tmp, 'w');
    try {
      fs.writeFileSync(fd, data, enc);
      try { fs.fsyncSync(fd); } catch {}
    } finally { fs.closeSync(fd); }
    fs.renameSync(tmp, file);
    return true;
  } catch (e) {
    try { fs.unlinkSync(tmp); } catch {}
    logLine('Schreiben fehlgeschlagen: ' + file + ' — ' + (e && e.message || e));
    return false;
  }
}

// ---------- Protokoll ----------
// Die letzten 400 Zeilen liegen im Speicher, jede Zeile wandert zusaetzlich in
// %APPDATA%/<App>/protokoll.log. Wird die Datei groesser als 512 KB, bleibt die
// juengere Haelfte stehen. Nichts davon geht ins Netz; der Dialog „Verbindungen“
// kann die letzten Zeilen in die Zwischenablage legen.
const LOG_MAX = 400, LOG_BYTES = 512 * 1024;
const logFile = () => path.join(app.getPath('userData'), 'protokoll.log');
const logBuf = [];
function logLine(msg) {
  const d = new Date();
  const p = n => String(n).padStart(2, '0');
  const stamp = d.getFullYear() + '-' + p(d.getMonth() + 1) + '-' + p(d.getDate()) + ' ' + p(d.getHours()) + ':' + p(d.getMinutes()) + ':' + p(d.getSeconds());
  const line = stamp + '  ' + String(msg == null ? '' : msg).replace(/\s+$/, '');
  logBuf.push(line);
  if (logBuf.length > LOG_MAX) logBuf.splice(0, logBuf.length - LOG_MAX);
  try {
    const f = logFile();
    if (fs.existsSync(f) && fs.statSync(f).size > LOG_BYTES) fs.writeFileSync(f, fs.readFileSync(f, 'utf8').slice(-Math.floor(LOG_BYTES / 2)));
    fs.appendFileSync(f, line + '\n');
  } catch {}
  return line;
}
ipcMain.handle('log:write', (_e, msg) => { logLine(msg); return true; });
ipcMain.handle('log:tail', (_e, n) => logBuf.slice(-Math.max(1, Math.min(LOG_MAX, Number(n) || 100))).join('\n'));
ipcMain.handle('log:copy', () => { try { clipboard.writeText(logBuf.join('\n')); return logBuf.length; } catch { return 0; } });
ipcMain.handle('log:path', () => logFile());

// ---------- Datenbestand pruefen ----------
// Beim Start bleiben keine .part-Dateien liegen, und defekte JSON-Dateien
// werden gemeldet statt still uebersprungen. Die study-Dateien lesen sich aus
// ihrer .bak selbst wieder ein (siehe study:load); hier steht nur, was auffiel.
function dataCheck() {
  const root = app.getPath('userData');
  const out = { parts: [], broken: [] };
  for (const d of [root, path.join(root, 'study'), path.join(root, 'library')]) {
    let names = [];
    try { names = fs.readdirSync(d); } catch { continue; }
    for (const n of names) {
      const f = path.join(d, n);
      if (n.endsWith('.part')) { try { fs.unlinkSync(f); out.parts.push(n); } catch {} continue; }
      if (!n.endsWith('.json')) continue;
      try { JSON.parse(fs.readFileSync(f, 'utf8')); } catch { out.broken.push(path.relative(root, f).replace(/\\/g, '/')); }
    }
  }
  if (out.parts.length) logLine('Unfertige Dateien entfernt: ' + out.parts.join(', '));
  if (out.broken.length) logLine('Nicht lesbare Dateien: ' + out.broken.join(', '));
  return out;
}
ipcMain.handle('data:check', () => dataCheck());

process.on('uncaughtException', err => { logLine('Fehler im Hauptprozess: ' + (err && err.stack || err)); });
process.on('unhandledRejection', r => { logLine('Unbehandelte Zusage: ' + (r && r.stack || r)); });

// Ein Flag schreiben, ohne die uebrigen Werte der Datei zu verlieren. Das
// Benutzerprofil hat Vorrang vor dem Programmordner (readCfg), deshalb landet
// der Autostart dort und ueberlebt auch ein Update, das den Programmordner leert.
function writeCfg(patch) {
  const f = path.join(app.getPath('userData'), 'config.json');
  let cur = {};
  try { cur = JSON.parse(fs.readFileSync(f, 'utf8')) || {}; } catch {}
  try { safeWrite(f, JSON.stringify(Object.assign({}, cur, patch))); } catch {}
}

function killTree(c) {
  if (!c || !c.pid) return;
  try {
    if (IS_WIN) spawn('taskkill', ['/pid', String(c.pid), '/t', '/f'], { windowsHide: true, stdio: 'ignore' });
    else process.kill(-c.pid, 'SIGTERM');
  } catch { try { c.kill(); } catch {} }
}

// Adressen kommen aus den Einstellungen der Oberflaeche und damit aus deren
// localStorage, den der Hauptprozess nicht lesen kann. Beide Dienste melden
// ihre Adresse deshalb ueber einen IPC-Kanal; hier wird sie zusaetzlich neben
// config.json abgelegt, damit die Vorabpruefung beim naechsten Hochfahren
// dieselbe Adresse fragt und nicht die fest eingebaute.
const cleanUrl = u => String(u || '').trim().replace(/\/+$/, '');
const isLoopback = h => /^(127\.0\.0\.1|localhost|\[::1\])$/i.test(String(h || ''));

const OLLAMA_DEFAULT = 'http://127.0.0.1:11434';
const ollamaUrlFile = () => path.join(app.getPath('userData'), 'ollama-address.json');
let ollamaUrl = OLLAMA_DEFAULT;
function readOllamaUrl() {
  try {
    const u = cleanUrl(JSON.parse(fs.readFileSync(ollamaUrlFile(), 'utf8')).url);
    if (/^https?:\/\/[^\s/]+$/i.test(u)) ollamaUrl = u;
  } catch {}
  return ollamaUrl;
}
function setOllamaUrl(u) {
  const s = cleanUrl(u);
  if (!/^https?:\/\/[^\s/]+$/i.test(s)) return;
  if (s === ollamaUrl && fs.existsSync(ollamaUrlFile())) return;
  ollamaUrl = s;
  try { safeWrite(ollamaUrlFile(), JSON.stringify({ url: s })); } catch {}
}
ipcMain.on('ollama:url', (_e, u) => setOllamaUrl(u));
// Ollama wird nur gestartet, wenn die eingestellte Adresse auf diesen Rechner
// zeigt. Eine fremde Adresse gehoert einem anderen Geraet; ein `ollama serve`
// auf diesem Rechner haette dort nichts zu bedienen. Bei einem anderen lokalen
// Port bekommt der Server ihn ueber OLLAMA_HOST, damit er auch dort lauscht.
function startOllama() {
  const base = readOllamaUrl();
  let u; try { u = new URL(base); } catch { return; }
  const lib = u.protocol === 'https:' ? require('https') : http;
  const req = lib.get(base + '/api/version', { timeout: 1500 }, res => res.resume());
  req.on('timeout', () => req.destroy());
  req.on('error', () => {
    if (!isLoopback(u.hostname)) return;
    const port = u.port || (u.protocol === 'https:' ? '443' : '80');
    const host = u.hostname === 'localhost' ? '127.0.0.1' : u.hostname;
    try {
      const env = Object.assign({}, process.env, { OLLAMA_HOST: `${host}:${port}` });
      ollama = runHidden(ollamaExe(), ['serve'], { env });
      ollama.on('error', () => { ollama = null; });
    } catch { ollama = null; }
  });
}
// Die NLLB-Adresse steht in den Einstellungen der Oberflaeche und damit im
// localStorage der Seite, das der Hauptprozess nicht selbst lesen kann. Die
// Seite meldet sie deshalb bei jeder Aenderung ueber 'nllb:url'; hier wird sie
// zusaetzlich neben config.json abgelegt, damit die Vorabpruefung schon beim
// naechsten Hochfahren dieselbe Adresse fragt und nicht fest 8765.
const NLLB_DEFAULT = 'http://127.0.0.1:8765';
const nllbUrlFile = () => path.join(app.getPath('userData'), 'nllb-address.json');
let nllbUrl = NLLB_DEFAULT;
function readNllbUrl() {
  try {
    const u = cleanUrl(JSON.parse(fs.readFileSync(nllbUrlFile(), 'utf8')).url);
    if (/^https?:\/\/[^\s/]+$/i.test(u)) nllbUrl = u;
  } catch {}
  return nllbUrl;
}
function setNllbUrl(u) {
  const s = cleanUrl(u);
  if (!/^https?:\/\/[^\s/]+$/i.test(s)) return;
  // Auch die Vorgabe wird einmal geschrieben, damit die Datei den Stand der
  // Einstellungen abbildet und nicht nur die Abweichungen davon.
  if (s === nllbUrl && fs.existsSync(nllbUrlFile())) return;
  nllbUrl = s;
  try { safeWrite(nllbUrlFile(), JSON.stringify({ url: s })); } catch {}
}
// Ein von Hand gestarteter Server darf nicht verdoppelt werden: erst fragen, ob
// auf der eingestellten Adresse schon jemand antwortet. Jede HTTP-Antwort
// zaehlt, nicht nur 200; erst wenn gar nichts antwortet oder die Verbindung
// haengt, startet die App selbst.
function nllbAnswers(cb) {
  const base = readNllbUrl();
  let lib;
  try { lib = new URL(base).protocol === 'https:' ? require('https') : http; } catch { return cb(false); }
  let done = false;
  const answer = v => { if (!done) { done = true; cb(v); } };
  const req = lib.get(base + '/health', { timeout: 1500 }, res => { res.resume(); answer(true); });
  req.on('timeout', () => req.destroy());
  req.on('error', () => answer(false));
}
// nllb_server.py hoert von sich aus auf 8765. Steht in den Einstellungen ein
// anderer Port auf diesem Rechner, bekommt der Server ihn ueber NLLB_PORT,
// damit die Oberflaeche denselben Endpunkt erreicht, den sie anspricht.
function nllbPort() {
  try {
    const u = new URL(readNllbUrl());
    return /^(127\.0\.0\.1|localhost|\[::1\])$/i.test(u.hostname) ? u.port : '';
  } catch { return ''; }
}
ipcMain.on('nllb:url', (_e, u) => setNllbUrl(u));
function startNllb(force) {
  try {
    const cfg = readCfg();
    const py = venvPython();
    if (py && (force || cfg.nllbAutostart)) {
      nllbWanted = true;
      nllbAnswers(up => {
        if (up) return;
        try {
          // Der Server liegt im gepackten Betrieb in app.asar.unpacked.
          // `__dirname` zeigt dagegen auf die Datei app.asar, und ein spawn mit
          // diesem `cwd` scheitert mit ENOENT - der Server startete in der
          // installierten Fassung deshalb nie. Genommen wird das Verzeichnis
          // der aufgeloesten Datei; im Entwicklungsbetrieb ist das der
          // Projektordner.
          const srv = resPath('nllb_server.py');
          // Zustand (installierte Modelle, Sprachliste) ins Benutzerprofil
          // legen: app.asar.unpacked wird bei jedem Update ersetzt, die
          // Modellgewichte liegen aber weiter im Hugging-Face-Cache und
          // gaelten sonst als fehlend.
          const port = nllbPort();
          const env = Object.assign({}, process.env, { NLLB_STATE_DIR: app.getPath('userData') }, port ? { NLLB_PORT: port } : {});
          nllb = runHidden(py, [srv], { cwd: path.dirname(srv), env });
          nllb.on('error', () => {});
        } catch {}
      });
    }
  } catch {}
}

// ---------- Python-Umgebung aus der App heraus einrichten ----------
// Der Set-up-Wizard legt nllb-venv an. Wer die App ueber den Installer bekommt
// oder die Umgebung spaeter verliert, soll dasselbe hier nachholen koennen: Die
// Einstellungen bieten es an, und ein Modell-Download loest es aus, wenn der
// Server nicht antwortet. Die Pakete stehen in requirements.txt und liegen im
// Paket neben nllb_server.py.
const venvDir = () => path.join(app.getPath('userData'), 'nllb-venv');

// Der System-Python ist unter Windows am haeufigsten im Benutzerprofil, sonst
// ueber den Launcher `py -3.12` oder den PATH; unter Linux heisst er python3.
function basePython() {
  const cands = IS_WIN
    ? [[path.join(process.env.LOCALAPPDATA || '', 'Programs', 'Python', 'Python312', 'python.exe'), []],
       ['py', ['-3.12']], ['python', []]]
    : [['python3', []], ['python', []]];
  for (const [cmd, pre] of cands) {
    try {
      const r = spawnSync(cmd, [...pre, '-c', 'import sys;print(sys.version_info[0])'], { encoding: 'utf8', windowsHide: true, timeout: 20000 });
      if (r && r.status === 0 && String(r.stdout).trim() === '3') return { cmd, pre };
    } catch {}
  }
  return null;
}

// Zeilenweise Ausgabe fuer pip und venv; das Ergebnis ist nur der Exit-Code.
function runStream(cmd, args, onLine) {
  return new Promise(resolve => {
    let c;
    try { c = spawn(cmd, args, { windowsHide: true, stdio: ['ignore', 'pipe', 'pipe'] }); }
    catch (e) { onLine(String(e.message || e)); return resolve(1); }
    const feed = b => String(b).split(/\r?\n/).forEach(l => { if (l.trim()) onLine(l.trim()); });
    c.stdout.on('data', feed); c.stderr.on('data', feed);
    c.on('error', e => { onLine(String(e.message || e)); resolve(1); });
    c.on('close', code => resolve(typeof code === 'number' ? code : 1));
  });
}

function waitNllb(ms) {
  return new Promise(resolve => {
    const t0 = Date.now();
    const tick = () => nllbAnswers(up => {
      if (up) return resolve(true);
      if (Date.now() - t0 > ms) return resolve(false);
      setTimeout(tick, 600);
    });
    tick();
  });
}

let nllbSetupJob = null;
function setupNllb(sender) {
  if (nllbSetupJob) return nllbSetupJob;
  const say = line => { try { if (sender && !sender.isDestroyed()) sender.send('nllb:setup', { line }); } catch {} };
  nllbSetupJob = (async () => {
    if (await waitNllb(800)) { say('Der Server antwortet bereits.'); return { ok: true, already: true }; }
    let py = venvPython();
    if (!py) {
      const base = basePython();
      if (!base) { say('Kein Python 3 gefunden. Bitte Python 3.12 installieren.'); return { ok: false, error: 'no python' }; }
      say('Lege die Python-Umgebung an ...');
      const code = await runStream(base.cmd, [...base.pre, '-m', 'venv', venvDir()], say);
      if (code !== 0) return { ok: false, error: 'venv' };
      py = venvPython();
      if (!py) return { ok: false, error: 'venv' };
    }
    // pythonw.exe kann kein pip; unter Windows also der Interpreter mit Konsole.
    const pip = IS_WIN ? py.replace(/pythonw?\.exe$/i, 'python.exe') : py;
    say('Installiere die Pakete; das dauert einige Minuten ...');
    const code = await runStream(pip, ['-m', 'pip', 'install', '--progress-bar', 'off', '-r', resPath('requirements.txt')], say);
    if (code !== 0) return { ok: false, error: 'pip' };
    say('Starte den Server ...');
    startNllb(true);
    if (!(await waitNllb(60000))) { say('Der Server antwortet nicht.'); return { ok: false, error: 'server' }; }
    // Ohne dieses Flag liefe der Server nur bis zum naechsten Programmstart:
    // startNllb(true) erzwingt ihn nur fuer diesen Lauf.
    writeCfg({ nllbAutostart: true });
    say('Fertig.');
    return { ok: true };
  })().finally(() => { nllbSetupJob = null; });
  return nllbSetupJob;
}
ipcMain.handle('nllb:setup', e => setupNllb(e.sender));

// ---------- Einrichtung beim ersten Start ----------
// install.ps1 liegt als extraResource in der Setup-EXE (build.extraResources)
// und laeuft hier mit -Headless: dieselben Schritte wie im Assistenten - Ollama
// samt Modellen, Python 3.12, nllb-venv, die Uebersetzungsmodelle -, nur ohne
// eigenes Fenster und ohne Node/Electron/Kopieren, weil der Installer das schon
// erledigt hat. Zielordner ist die Installation, die Umgebung liegt wie beim
// Serverstart im Benutzerprofil (venvDir), der Zustand daneben (NLLB_STATE_DIR).
const setupScript = () => app.isPackaged
  ? path.join(process.resourcesPath, 'install.ps1')
  : path.join(__dirname, 'install.ps1');
// Die Modelle, die der Assistent vorschlaegt: reine Uebersetzung, Stil/Kontext
// und die kleinste NLLB-Fassung. Wer mehr will, nimmt sie in den Einstellungen
// unter "Uebersetzungsmodelle" dazu.
const SETUP_OLLAMA = ['translategemma:4b', 'gemma3:4b'];
const SETUP_NLLB = ['nllb:600M'];

// Gefragt wird nur, solange die Einrichtung nicht durch ist und auch nicht
// bewusst uebersprungen wurde; eine vorhandene Umgebung gilt ebenfalls als
// eingerichtet, damit eine bestehende Installation nicht erneut gefragt wird.
function setupState() {
  const cfg = readCfg();
  return { needed: !cfg.setupDone && !cfg.setupSkipped && !venvPython(), done: !!cfg.setupDone };
}
ipcMain.handle('setup:needed', () => setupState());
ipcMain.handle('setup:skip', () => { writeCfg({ setupSkipped: true }); return { ok: true }; });

let setupJob = null;
function runSetup(sender) {
  if (setupJob) return setupJob;
  const send = (kind, text) => { try { if (sender && !sender.isDestroyed()) sender.send('setup:line', { kind, text }); } catch {} };
  const script = setupScript();
  if (!fs.existsSync(script)) return Promise.resolve({ ok: false, error: 'install.ps1 fehlt' });
  const args = ['-NoProfile', '-ExecutionPolicy', 'Bypass', '-WindowStyle', 'Hidden', '-File', script,
    '-Headless', '-AppDir', path.dirname(app.getPath('exe')), '-VenvDir', venvDir(),
    '-ServerDir', path.dirname(resPath('nllb_server.py')), '-CfgDir', app.getPath('userData'),
    '-OllamaModels', SETUP_OLLAMA.join(','), '-NllbModels', SETUP_NLLB.join(',')];
  setupJob = new Promise(resolve => {
    let lastErr = '';
    let child;
    try { child = spawn('powershell', args, { windowsHide: true, stdio: ['ignore', 'pipe', 'pipe'] }); }
    catch (e) { send('err', String(e.message || e)); return resolve({ ok: false, error: String(e.message || e) }); }
    // Das Skript spricht in vier Sorten: LOG (Zeile der Komponenten), STEP
    // (laufender Schritt), OK und ERR. Alles andere (etwa die Ausgabe von pip
    // oder curl) geht als gewoehnliche Zeile mit.
    const feed = b => String(b).split(/\r?\n/).forEach(raw => {
      const s = raw.trim();
      if (!s) return;
      const m = s.match(/^(LOG|STEP|OK|ERR)\|(.*)$/);
      if (!m) { if (s.length < 300) send('log', s); return; }
      if (m[1] === 'ERR') { lastErr = m[2]; send('err', m[2]); return; }
      send(m[1] === 'STEP' ? 'step' : 'log', m[2]);
    });
    child.stdout.on('data', feed); child.stderr.on('data', feed);
    child.on('error', e => { lastErr = String(e.message || e); send('err', lastErr); });
    child.on('close', code => {
      if (code === 0) {
        writeCfg({ setupDone: true, setupSkipped: false });
        startNllb(true);
        return resolve({ ok: true });
      }
      resolve({ ok: false, error: lastErr || ('Exit ' + code) });
    });
  }).finally(() => { setupJob = null; });
  return setupJob;
}
ipcMain.handle('setup:run', e => runSetup(e.sender));

// ---------- EPUB ----------
// Ein EPUB ist ein ZIP mit XHTML-Dateien. container.xml nennt die Paketdatei
// (OPF), dort stehen der Titel und die Lesereihenfolge (spine). Fuer Lesetexte
// genuegt der reine Text, die Reihenfolge muss aber stimmen, sonst geraten die
// Kapitel durcheinander.
const xmlDec = s => String(s || '')
  .replace(/&#x([0-9a-f]+);/gi, (_, h) => String.fromCodePoint(parseInt(h, 16)))
  .replace(/&#(\d+);/g, (_, d) => String.fromCodePoint(parseInt(d, 10)))
  .replace(/&lt;/g, '<').replace(/&gt;/g, '>').replace(/&quot;/g, '"')
  .replace(/&apos;/g, "'").replace(/&nbsp;/g, ' ').replace(/&amp;/g, '&');
function htmlText(s) {
  return xmlDec(String(s || '')
    .replace(/<(script|style)[\s\S]*?<\/\1>/gi, ' ')
    .replace(/<\/(p|div|h\d|li|tr|blockquote|section|article)>/gi, '\n')
    .replace(/<br\s*\/?>/gi, '\n')
    .replace(/<[^>]+>/g, ' '))
    .replace(/[ \t\u00a0]+/g, ' ').replace(/ *\n */g, '\n').replace(/\n{3,}/g, '\n\n').trim();
}
async function epubText(buf) {
  const JSZip = require('jszip');
  const zip = await JSZip.loadAsync(buf);
  const read = async n => { const f = n && zip.file(n); return f ? f.async('string') : ''; };
  const cont = await read('META-INF/container.xml');
  let opfPath = (cont.match(/full-path="([^"]+)"/i) || [])[1] || '';
  if (!opfPath) opfPath = Object.keys(zip.files).find(n => /\.opf$/i.test(n)) || '';
  const opf = await read(opfPath);
  const title = htmlText((opf.match(/<dc:title[^>]*>([\s\S]*?)<\/dc:title>/i) || [])[1] || '');
  const base = opfPath.replace(/[^/]+$/, '');
  const href = {};
  for (const tag of opf.match(/<item\b[^>]*>/gi) || []) {
    const id = (tag.match(/\bid="([^"]+)"/) || [])[1];
    const h = (tag.match(/\bhref="([^"]+)"/) || [])[1];
    if (id && h) href[id] = decodeURIComponent(h);
  }
  const spine = (opf.match(/<itemref\b[^>]*>/gi) || [])
    .map(t => (t.match(/\bidref="([^"]+)"/) || [])[1]).filter(Boolean);
  const files = spine.length
    ? spine.map(id => href[id]).filter(Boolean)
    : Object.keys(zip.files).filter(n => /\.x?html?$/i.test(n)).sort();
  const parts = [];
  for (const h of files) {
    const src = await read(base + h) || await read(h);
    const t = htmlText(src);
    if (t) parts.push(t);
  }
  return { title, text: parts.join('\n\n') };
}

// Text aus einer Datei holen. Word, PDF und EPUB brauchen die Fremdbibliotheken,
// alles andere (txt, md, csv, html, srt) ist bereits reiner Text.
async function extractText(name, buf) {
  const ext = path.extname(String(name || '')).toLowerCase();
  if (ext === '.epub') return (await epubText(buf)).text;
  if (ext === '.docx') return (await require('mammoth').extractRawText({ buffer: buf })).value.trim();
  if (ext === '.pdf') {
    const { PDFParse } = require('pdf-parse'); const p = new PDFParse({ data: new Uint8Array(buf) });
    try { return (await p.getText()).text.replace(/\n*-- \d+ of \d+ --\n*/g, '\n\n').trim(); } finally { await p.destroy(); }
  }
  if (['.txt', '.md', '.csv', '.srt', '.json', '.html', '.htm'].includes(ext)) return buf.toString('utf8').replace(/^\uFEFF/, '');
  throw new Error('unsupported');
}
ipcMain.handle('extract', async (_e, name, data) => extractText(name, Buffer.from(data)));

// ---------- Schriftarten des Systems (Fallback, wenn queryLocalFonts nicht geht) ----------
// Systemschriften: Unter Linux liefert fc-list die Familiennamen, unter Windows
// werden die Dateinamen der Schriftordner ausgewertet. Fehlt fc-list, laeuft
// derselbe Weg wie unter Windows - dann rekursiv, weil Linux die Schriften in
// Unterordnern ablegt.
const FONT_DIRS = IS_WIN
  ? [path.join(process.env.WINDIR || 'C:\\Windows', 'Fonts'), path.join(process.env.LOCALAPPDATA || '', 'Microsoft', 'Windows', 'Fonts')]
  : [path.join(os.homedir(), '.local', 'share', 'fonts'), path.join(os.homedir(), '.fonts'), '/usr/local/share/fonts', '/usr/share/fonts'];
const FONT_EXT = /\.(ttf|otf|ttc|woff2?)$/i;
const FONT_CUT = /[-_ ]?(bold|italic|regular|light|medium|semibold|black|thin|condensed|expanded|oblique|extralight|extrabold|heavy|ultralight)\b/gi;
function fontFiles(dir, depth) {
  const out = [];
  let list = [];
  try { list = fs.readdirSync(dir, { withFileTypes: true }); } catch { return out; }
  for (const e of list) {
    if (e.isDirectory()) { if (depth > 0) out.push(...fontFiles(path.join(dir, e.name), depth - 1)); continue; }
    if (FONT_EXT.test(e.name)) out.push(e.name);
  }
  return out;
}
function fcFamilies() {
  try {
    const out = require('child_process').execFileSync('fc-list', ['--format', '%{family}\n'], { encoding: 'utf8', timeout: 4000, maxBuffer: 8 << 20 });
    return out.split('\n').flatMap(l => l.split(',')).map(s => s.trim()).filter(Boolean);
  } catch { return []; }
}
ipcMain.handle('fonts:list', () => {
  const fc = IS_WIN ? [] : fcFamilies();
  if (fc.length) return [...new Set(fc)].sort((a, b) => a.localeCompare(b));
  const names = new Set();
  for (const d of FONT_DIRS) {
    for (const f of fontFiles(d, IS_WIN ? 0 : 3)) {
      let n = path.basename(f, path.extname(f));
      n = n.replace(FONT_CUT, '').trim();
      if (n === n.toLowerCase() || n === n.toUpperCase()) n = n.replace(/\b\w/g, c => c.toUpperCase());
      if (n) names.add(n);
    }
  }
  return [...names].sort((a, b) => a.localeCompare(b));
});

// ---------- Rechtschreibpruefung ----------
const spellers = {}; let personal = [], personalKeys = new Set();
const persFile = () => path.join(app.getPath('userData'), 'personal-dictionary.json');
// Vergleichsschluessel des persoenlichen Woerterbuchs: Wortlaut ohne
// Gross-/Kleinschreibung, damit „Schloss“ im Text denselben Eintrag meint wie
// „schloss“ in der Liste.
const persKey = w => String(w == null ? '' : w).trim().toLocaleLowerCase();
// Setzt Liste und Vergleichsindex gemeinsam. Jede Stelle, die `personal`
// aendert, geht hier durch, damit der Index nicht auseinanderlaeuft.
function setPersonal(list) {
  personal = Array.isArray(list) ? list : [];
  personalKeys = new Set(personal.map(persKey));
}
// Wo die Woerterbuecher liegen: `dict/<sprache>` ist die ausgelieferte Fassung
// (auch fuer Arabisch, das kein npm-Paket hat), das npm-Paket ist der Rueckfall.
function spellDir(lang) {
  const local = path.join(__dirname, 'dict', lang);
  if (fs.existsSync(path.join(local, 'index.aff')) && fs.existsSync(path.join(local, 'index.dic'))) return local;
  return path.join(__dirname, 'node_modules', 'dictionary-' + lang);
}
// Die Engine ist echtes Hunspell als WebAssembly (`hunspell-wasm`) mit denselben
// Woerterbuchdateien. `nspell` hat nur einen Teil der Hunspell-Regeln umgesetzt;
// damit galten Komposita („Bahnhof“, „Kindergarten“), Formen wie „macht“ und
// Bindestrichwoerter wie „E-Mail“ als falsch - gemessen 66 von 134 Woertern,
// echtes Hunspell 6. Das Paket ist ESM, main.js ist CommonJS, deshalb das
// dynamische import(); danach ist die API synchron. Ein Speller je Sprache
// bleibt im Speicher, damit nicht jeder Scan das Woerterbuch neu baut.
async function speller(lang) {
  if (spellers[lang]) return spellers[lang];
  const dir = spellDir(lang);
  try {
    const { createHunspellFromFiles } = await import('hunspell-wasm');
    const sp = await createHunspellFromFiles(path.join(dir, 'index.aff'), path.join(dir, 'index.dic'));
    personal.forEach(w => sp.addWord(w));
    return (spellers[lang] = sp);
  } catch (e) {
    // Fehler NICHT schlucken - der Renderer loggt sie.
    console.error('Rechtschreibpruefung: Woerterbuch ' + lang + ' nicht ladbar:', (e && e.message) || e);
    return { testSpelling: () => true, getSpellingSuggestions: () => [], addWord: () => {} };
  }
}
// Umlaut-Schreibweise ohne Umlaute („fuer“, „ueber“, „Groesse“): Wer so tippt,
// soll keine Welle bekommen. Geprueft wird nur als Rueckfall, wenn das Wort
// selbst nicht im Woerterbuch steht, und ohne ss/ß-Umformung - sonst wuerde die
// Schweizer Fassung deutsche ß-Schreibweisen schlucken (und umgekehrt).
const UMLAUT_PAIRS = [['ae', 'ä'], ['oe', 'ö'], ['ue', 'ü'], ['Ae', 'Ä'], ['Oe', 'Ö'], ['Ue', 'Ü'], ['AE', 'Ä'], ['OE', 'Ö'], ['UE', 'Ü']];
function umlautForm(w) {
  let out = w;
  for (const [from, to] of UMLAUT_PAIRS) out = out.split(from).join(to);
  return out;
}
// Nur im Deutschen ist „ue/oe/ae“ eine Umschrift fuer Umlaute. Im
// Niederlaendischen, Spanischen und Tuerkischen sind das gewoehnliche
// Buchstabenfolgen; dort liesse der Rueckfall falsche Schreibweisen durch
// („mueduer“ fuer „müdür“, „agueero“ fuer „agüero“, „reuenie“ fuer „reünie“).
const umlautLang = lang => /^de\b/i.test(String(lang || ''));
// „ss“ gilt in der Umlaut-Schreibweise als „ß“ (Groesse -> Größe), aber nur in
// Woertern, die auch ae/oe/ue enthalten: sonst wuerde die deutsche Fassung
// „Strasse“ schlucken, obwohl dort „Straße“ gilt - dafuer gibt es de-ch.
function sharpForm(w, u) {
  if (u === w || !/ss/.test(u)) return null;
  return u.replace(/ss/g, 'ß');
}
const formsOf = w => {
  const u = umlautForm(w);
  return u === w ? [w] : [u, sharpForm(w, u)].filter(Boolean);
};
const wordOk = (sp, w, lang) => {
  if (sp.testSpelling(w)) return true;
  const um = umlautLang(lang);
  const okPart = p => !p || sp.testSpelling(p) || (um && formsOf(p).some(f => sp.testSpelling(f)));
  const parts = /[-'’]/.test(w) ? w.split(/['’-]/) : null;
  if (parts && parts.every(okPart)) return true;
  return um && formsOf(w).some(f => sp.testSpelling(f));
};
// Das persoenliche Woerterbuch entscheidet zuerst: Wer ein Wort dort eintraegt,
// will es ohne Rueckfrage als richtig sehen - auch dann, wenn die Engine es
// anders sieht und ein spaeterer Scan die Welle zurueckbringen wuerde.
ipcMain.handle('spell:check', async (_e, lang, words) => {
  const sp = await speller(lang);
  return words.filter(w => !personalKeys.has(persKey(w)) && !wordOk(sp, w, lang));
});
ipcMain.handle('spell:suggest', async (_e, lang, word) => {
  try { return (await speller(lang)).getSpellingSuggestions(word).slice(0, 5); } catch { return []; }
});
ipcMain.handle('spell:add', (_e, word) => {
  const w = String(word == null ? '' : word).trim();
  if (!w) return false;
  const key = persKey(w);
  if (!personalKeys.has(key)) {
    personal.push(w); personalKeys.add(key);
    try { safeWrite(persFile(), JSON.stringify(personal)); } catch {}
    Object.values(spellers).forEach(s => s.addWord(w));
  }
  return true;
});
ipcMain.handle('spell:getPersonal', () => [...personal]);
ipcMain.handle('spell:remove', (_e, word) => {
  const key = persKey(word);
  setPersonal(personal.filter(w => persKey(w) !== key));
  try { safeWrite(persFile(), JSON.stringify(personal)); } catch {}
  for (const k in spellers) delete spellers[k];
  return true;
});
ipcMain.handle('spell:edit', (_e, oldWord, newWord) => {
  const next = String(newWord == null ? '' : newWord).trim();
  const idx = personal.findIndex(w => persKey(w) === persKey(oldWord));
  if (idx >= 0 && next) {
    personal[idx] = next;
    setPersonal(personal);
    try { safeWrite(persFile(), JSON.stringify(personal)); } catch {}
    for (const k in spellers) delete spellers[k];
  }
  return true;
});

// Export und Import des persoenlichen Woerterbuchs. Der Export schreibt eine
// einfache JSON-Liste. Der Import liest auch Textlisten (eine Zeile pro Wort)
// und vergleicht ohne Ruecksicht auf Gross-/Kleinschreibung, damit dasselbe
// Wort nicht in zwei Schreibweisen doppelt in der Liste landet - weder gegen
// den Bestand noch innerhalb der importierten Datei.
const dictKey = w => String(w).trim().toLocaleLowerCase();
function dictWord(raw) {
  if (typeof raw === 'string') return raw.trim();
  if (raw && typeof raw.word === 'string') return raw.word.trim();
  return '';
}
function mergeDictWords(existing, incoming) {
  const seen = new Set(), list = [];
  for (const raw of existing) {
    const word = dictWord(raw);
    if (!word || seen.has(dictKey(word))) continue;
    seen.add(dictKey(word)); list.push(word);
  }
  let added = 0, skipped = 0;
  for (const raw of incoming) {
    const word = dictWord(raw);
    if (!word || seen.has(dictKey(word))) { skipped++; continue; }
    seen.add(dictKey(word)); list.push(word); added++;
  }
  return { list, added, skipped };
}
// JSON-Liste, JSON mit words-Array oder reine Textliste. Ein BOM am
// Dateianfang (Windows-Editor) wuerde sonst jeden JSON.parse scheitern lassen.
function parseDictFile(text) {
  const raw = String(text).replace(/^\uFEFF/, '').trim();
  if (!raw) return [];
  try {
    const data = JSON.parse(raw);
    if (Array.isArray(data)) return data;
    if (data && Array.isArray(data.words)) return data.words;
    if (typeof data === 'string') return [data];
    if (data && typeof data === 'object') return Object.keys(data);
    return [];
  } catch { return raw.split(/[\r\n;,]+/); }
}
ipcMain.handle('dict:export', async () => {
  const r = await dialog.showSaveDialog(win, { defaultPath: 'woerterbuch.json', filters: [{ name: 'JSON', extensions: ['json'] }] });
  if (r.canceled || !r.filePath) return -1;
  try { safeWrite(r.filePath, JSON.stringify(personal, null, 2), 'utf8'); return personal.length; }
  catch { return -1; }
});
ipcMain.handle('dict:import', async () => {
  const r = await dialog.showOpenDialog(win, { properties: ['openFile'], filters: [{ name: 'JSON oder Text', extensions: ['json', 'txt'] }] });
  if (r.canceled || !r.filePaths.length) return null;
  try {
    const merged = mergeDictWords(personal, parseDictFile(fs.readFileSync(r.filePaths[0], 'utf8')));
    setPersonal(merged.list);
    try { safeWrite(persFile(), JSON.stringify(personal)); } catch {}
    for (const k in spellers) delete spellers[k];
    return { added: merged.added, skipped: merged.skipped, total: personal.length };
  } catch { return { error: true }; }
});

ipcMain.handle('file:save', async (_e, name, data, kind, opts) => {
  const names = { md:'Markdown', html:'HTML', txt:'Text', pdf:'PDF' };
  const r = await dialog.showSaveDialog(win, { defaultPath: name + '.' + kind, filters: [{ name: names[kind], extensions: [kind] }] });
  if (r.canceled || !r.filePath) return false;
  if (kind === 'pdf') {
    // A4-Blaetter bringen ihre Seiten selbst mit: randlos drucken und die
    // Seitengroesse aus der Druck-CSS uebernehmen, damit jede Seite genau ein
    // Blatt fuellt. Die durchlaufende Seite behaelt die Standardraender.
    const pdf = { pageSize: 'A4', printBackground: true };
    if (opts && opts.a4) { pdf.margins = { marginType: 'none' }; pdf.preferCSSPageSize = true; }
    safeWrite(r.filePath, await win.webContents.printToPDF(pdf));
  }
  else safeWrite(r.filePath, data, 'utf8');
  return true;
});

// ---------- Datei-Bibliothek des Texteditors ----------
// Jedes Dokument liegt als echte HTML-Datei in %APPDATA%/<App>/library,
// daneben eine kleine JSON-Datei mit Name und Zeitstempeln. So bleiben die
// Dateien auch ausserhalb der App lesbar, und die Vorschau kann den Inhalt
// direkt laden. Die IDs werden auf unkritische Zeichen beschraenkt, damit
// kein Pfad aus dem Renderer in fremde Ordner zeigen kann.
const libDir = () => {
  const d = path.join(app.getPath('userData'), 'library');
  fs.mkdirSync(d, { recursive: true });
  return d;
};
const libId = id => String(id || '').replace(/[^a-zA-Z0-9_-]/g, '');
const libHtml = id => path.join(libDir(), libId(id) + '.html');
const libMeta = id => path.join(libDir(), libId(id) + '.json');
ipcMain.handle('lib:list', () => {
  try {
    const d = libDir();
    return fs.readdirSync(d).filter(f => f.endsWith('.json')).map(f => {
      try {
        const m = JSON.parse(fs.readFileSync(path.join(d, f), 'utf8'));
        const html = fs.readFileSync(libHtml(m.id), 'utf8');
        return { id: m.id, name: m.name, created: m.created, updated: m.updated, mode: m.mode === 'a4' ? 'a4' : 'flow', html };
      } catch { return null; }
    }).filter(Boolean).sort((a, b) => b.updated - a.updated);
  } catch { return []; }
});
ipcMain.handle('lib:read', (_e, id) => {
  try {
    const m = JSON.parse(fs.readFileSync(libMeta(id), 'utf8'));
    return { id: m.id, name: m.name, created: m.created, updated: m.updated, mode: m.mode === 'a4' ? 'a4' : 'flow', html: fs.readFileSync(libHtml(id), 'utf8') };
  } catch { return null; }
});
ipcMain.handle('lib:save', (_e, doc) => {
  const id = libId(doc && doc.id) || ('d' + Date.now().toString(36) + Math.random().toString(36).slice(2, 6));
  let created = Date.now();
  try { created = JSON.parse(fs.readFileSync(libMeta(id), 'utf8')).created || created; } catch {}
  const meta = { id, name: String((doc && doc.name) || 'Dokument').slice(0, 120), created, updated: Date.now(), mode: doc && doc.mode === 'a4' ? 'a4' : 'flow' };
  safeWrite(libHtml(id), String((doc && doc.html) || ''), 'utf8');
  safeWrite(libMeta(id), JSON.stringify(meta), 'utf8');
  return meta;
});
ipcMain.handle('lib:delete', (_e, id) => {
  for (const f of [libHtml(id), libMeta(id)]) { try { fs.unlinkSync(f); } catch {} }
  return true;
});

// ---------- Lesetexte und Vokabeln ----------
// Beides liegt als je eine JSON-Datei im Benutzerprofil. Der Renderer haelt die
// Liste im Speicher und schreibt sie als Ganzes zurueck; so bleibt die Ablage
// unabhaengig von der Groesse und laesst sich mit jedem Editor ansehen.
const studyNames = { texts: 'texts.json', vocab: 'vocab.json' };
const studyDir = () => {
  const d = path.join(app.getPath('userData'), 'study');
  fs.mkdirSync(d, { recursive: true });
  return d;
};
const studyFile = name => path.join(studyDir(), studyNames[name] || 'unbekannt.json');
// Sicherung: vor jedem Schreiben liegt die vorherige Fassung als .bak daneben.
// Ein einziger falscher Schreibvorgang hat sonst die ganze Liste geloescht
// (passiert beim Testen), und ohne Sicherung war sie nicht zurueckzuholen.
// Die Sicherung wird NICHT aufgefrischt, wenn der neue Stand deutlich kleiner
// ist als der alte: sonst wuerde ein zweiter Fehlschlag (etwa eine leere Liste,
// nachdem schon ein Testeintrag in der Hauptdatei steht) die letzte gute
// Sicherung mit dem bereits kaputten Stand ueberschreiben.
const studyBak = name => studyFile(name) + '.bak';
const studyCount = txt => { try { const d = JSON.parse(txt); return Array.isArray(d) ? d.length : -1; } catch { return -1; } };
ipcMain.handle('study:load', (_e, name) => {
  if (!studyNames[name]) return null;
  for (const f of [studyFile(name), studyBak(name)]) {
    try {
      const d = JSON.parse(fs.readFileSync(f, 'utf8'));
      if (Array.isArray(d)) return d;
    } catch { }
  }
  return null;
});
ipcMain.handle('study:save', (_e, name, data) => {
  if (!studyNames[name]) return false;
  const f = studyFile(name), b = studyBak(name);
  try {
    let cur = null;
    try { cur = fs.readFileSync(f, 'utf8'); } catch { }
    const curN = cur == null ? -1 : studyCount(cur);
    const nextN = Array.isArray(data) ? data.length : -1;
    // Geschrieben wird immer, gesichert nur bei einem glaubwuerdigen Umfang:
    // leere oder auf weniger als die Haelfte geschrumpfte Listen sind der
    // Fehlerfall, den die Sicherung ueberleben muss.
    if (curN > 0 && nextN >= 0 && nextN * 2 >= curN) safeWrite(b, cur, 'utf8');
    return safeWrite(f, JSON.stringify(data), 'utf8');
  } catch { return false; }
});
// Eine Datei auswaehlen und ihren Text zurueckgeben (eigene Texte hochladen).
ipcMain.handle('study:importText', async () => {
  const r = await dialog.showOpenDialog(win, {
    properties: ['openFile'],
    filters: [{ name: 'Text, PDF, EPUB', extensions: ['txt', 'md', 'html', 'htm', 'docx', 'pdf', 'epub'] }]
  });
  if (r.canceled || !r.filePaths.length) return null;
  const file = r.filePaths[0];
  const name = path.basename(file).replace(/\.[^.]+$/, '');
  try {
    const buf = fs.readFileSync(file);
    // EPUB liefert den Titel aus seinen Metadaten mit; sonst taugt der Dateiname.
    if (path.extname(file).toLowerCase() === '.epub') {
      const e = await epubText(buf);
      return { name, title: e.title || name, text: e.text };
    }
    return { name, title: name, text: await extractText(file, buf) };
  } catch (e) { return { error: e.message || 'read' }; }
});
// Zwischenablage fuer den Textdialog. Der Renderer liest sie nicht selbst, weil
// file:// kein sicherer Kontext ist und navigator.clipboard dort schweigt.
ipcMain.handle('clip:readText', () => clipboard.readText());
// Schreiben geht denselben Weg: unter file:// fehlt navigator.clipboard, ein
// Aufruf im Renderer lief dort still ins Leere.
ipcMain.handle('clip:writeText', (_e, text) => {
  clipboard.writeText(String(text == null ? '' : text));
  return true;
});
ipcMain.handle('study:importFile', async (_e, ext) => {
  const exts = String(ext || 'csv').split(',');
  const r = await dialog.showOpenDialog(win, { properties: ['openFile'], filters: [{ name: exts.map(e => e.toUpperCase()).join(' / '), extensions: exts }] });
  if (r.canceled || !r.filePaths.length) return null;
  try { return { name: path.basename(r.filePaths[0]), text: fs.readFileSync(r.filePaths[0], 'utf8') }; } catch { return null; }
});
ipcMain.handle('study:exportFile', async (_e, name, data, ext) => {
  const r = await dialog.showSaveDialog(win, { defaultPath: name + '.' + ext, filters: [{ name: String(ext).toUpperCase(), extensions: [String(ext)] }] });
  if (r.canceled || !r.filePath) return false;
  return safeWrite(r.filePath, data, 'utf8');
});

// Eigene Fensterleiste (frame:false). Die drei Knoepfe im Renderer rufen diese
// Kanaele auf; Schliessen laeuft bewusst ueber win.close(), damit die bestehende
// Rueckfrage aus dem close-Handler erhalten bleibt.
ipcMain.handle('win:minimize', () => { if (win && !win.isDestroyed()) win.minimize(); });
ipcMain.handle('win:toggleMax', () => {
  if (!win || win.isDestroyed()) return false;
  win.isMaximized() ? win.unmaximize() : win.maximize();
  return win.isMaximized();
});
ipcMain.handle('win:close', () => { if (win && !win.isDestroyed()) win.close(); });

// ---------- Infobereich und systemweiter Kurzbefehl ----------
// Das Fenster ist der einzige Weg in die App; wer es hinter anderen Fenstern
// verliert, holt es mit dem Kurzbefehl oder ueber das Symbol im Infobereich
// nach vorn. Der Kurzbefehl zeigt immer, er versteckt nicht: ein verstecktes
// Fenster ohne Tray-Symbol waere nicht mehr zu erreichen.
const HOTKEY = 'CommandOrControl+Shift+Space';
// Ist die Kombination schon von einem anderen Programm belegt, liefert
// globalShortcut.register() false, ohne einen Fehler zu werfen. Der Renderer
// fragt den Wert ueber `app:hotkey` ab und zeigt in den Einstellungen dann
// keinen Tastenkuerzel mehr an, sondern einen Hinweis.
let hotkeyOk = false;
let tray = null;
// Der Infobereich ist der Ort, an dem das Symbol ohnehin angeklickt wird;
// steht der Kurzbefehl zur Verfuegung, nennt der Tooltip ihn gleich mit. Das
// Menu bleibt deutsch, weil das Tray dem Betriebssystem gehoert.
const trayTip = () => hotkeyOk ? 'Übersetzer – Strg+Shift+Leertaste' : 'Übersetzer';
function showWindow() {
  if (!win || win.isDestroyed()) return;
  if (win.isMinimized()) win.restore();
  win.show(); win.focus();
}
// ---------- Sprachausgabe ----------
// Chromium bringt in Electron keine Web-Speech-Stimmen mit: getVoices() ist
// leer, und speechSynthesis bleibt stumm. Gesprochen wird deshalb ueber die
// Ausgabe des Systems - unter Windows SAPI, unter Linux speech-dispatcher
// (spd-say) oder espeak-ng. Beides laeuft lokal, es wird nichts gesendet und
// nichts nachgeladen.
let ttsProc = null;
let ttsBackend = null; // unter Linux: 'spd-say', 'espeak-ng', 'espeak' oder 'keine'
// Das PowerShell-Skript geht als Base64 (UTF-16LE) hinueber, damit der Text
// nicht durch Anfuehrungszeichen muss: er steht in einer Datei und wird von
// dort gelesen. Stimme nach Sprache, sonst bleibt die Standardstimme.
const TTS_PS = [
  'Add-Type -AssemblyName System.Speech',
  '$s = New-Object System.Speech.Synthesis.SpeechSynthesizer',
  '$s.Rate = -1',
  '$txt = [IO.File]::ReadAllText($env:TTS_FILE, [Text.Encoding]::UTF8)',
  "$v = $s.GetInstalledVoices() | Where-Object { $_.Enabled -and $_.VoiceInfo.Culture.Name -like ($env:TTS_LANG + '-*') } | Select-Object -First 1",
  'if ($v) { $s.SelectVoice($v.VoiceInfo.Name) }',
  // In eine Datei statt direkt auf das Geraet: nur eine Datei laesst sich im
  // Renderer anhalten und an derselben Stelle fortsetzen.
  '$s.SetOutputToWaveFile($env:TTS_WAV)',
  '$s.Speak($txt)',
  '$s.Dispose()'
].join('; ');
const TTS_PS_B64 = Buffer.from(TTS_PS, 'utf16le').toString('base64');
// ---------- Aussprache: herunterladbare Stimmen (Piper) ----------
// SAPI klingt mechanisch. Piper ist ein kleines neuronales Modell, das auf der
// CPU schneller als Echtzeit rechnet und eine WAV-Datei liefert; die Datei kann
// der Renderer anhalten und fortsetzen, ein laufender Sprechprozess nicht.
// Engine und Stimmen werden bei Bedarf in den Benutzerordner geladen, damit
// weder Installer noch Programmordner wachsen. Gemessene Groessen stehen in
// der Liste und gehen so in die Oberflaeche (Einstellungen, Modellwaehler).
const PIPER_ZIP = 'https://github.com/rhasspy/piper/releases/download/2023.11.14-2/piper_windows_amd64.zip';
const PIPER_HF = 'https://huggingface.co/rhasspy/piper-voices/resolve/main/';
// `mb` ist die Groesse der ONNX-Datei, `rtf` der gemessene Echtzeitfaktor auf
// diesem Rechner (0,41 heisst: 2,5-mal so schnell wie die Sprachausgabe dauert).
// Nur Stimmen, die mit der mitgelieferten Piper-Fassung (2023.11.14-2) wirklich
// laufen: Neuere Stimmen (en_US-hfc_female, es_ES-davefx, es_ES-sharvard,
// fr_FR-siwis) brechen dort mit `"aɪ" is not a single codepoint` ab, weil sie
// Mehrzeichen-Phoneme verwenden, die diese Fassung nicht kennt (Abschnitt 171).
const TTS_VOICES = [
  { id:'de_DE-thorsten-high', lang:'de', name:'Thorsten', size:'hoch', mb:108.6, rtf:0.41, path:'de/de_DE/thorsten/high/' },
  { id:'en_US-lessac-medium', lang:'en', name:'Lessac', size:'mittel', mb:60.3, rtf:0.099, path:'en/en_US/lessac/medium/' },
  { id:'es_MX-claude-high', lang:'es', name:'Claude', size:'hoch', mb:63.1, rtf:0.116, path:'es/es_MX/claude/high/' },
  // Tuerkisch: schon in 1.15.146 mit dieser Engine gemessen (dort 0,1158);
  // nachgemessen beim Aufnehmen 0,0932 (0,42 s Rechnen fuer 4,54 s Ton).
  { id:'tr_TR-dfki-medium', lang:'tr', name:'DFKI', size:'mittel', mb:63.2, rtf:0.0932, path:'tr/tr_TR/dfki/medium/' },
  // Französisch: fr_FR-tom-medium spricht (0,169, 44,1 kHz); fr_FR-upmc-medium
  // spricht ebenfalls (0,078, 76,7 MB) - im Angebot ist die kleinere Stimme.
  { id:'fr_FR-tom-medium', lang:'fr', name:'Tom', size:'mittel', mb:63.5, rtf:0.169, path:'fr/fr_FR/tom/medium/' },
  // Italienisch: paola-medium und serena-medium brechen mit dieser Engine ab
  // („aɪ“ nicht auf ein Zeichen abbildbar); nur riccardo-x_low spricht.
  { id:'it_IT-riccardo-x_low', lang:'it', name:'Riccardo', size:'niedrig', mb:28.1, rtf:0.066, path:'it/it_IT/riccardo/x_low/' },
  // Portugiesisch: faber, cadu und jeff (alle medium) brechen ab, edresson-low spricht.
  { id:'pt_BR-edresson-low', lang:'pt', name:'Edresson', size:'niedrig', mb:63.1, rtf:0.066, path:'pt/pt_BR/edresson/low/' },
  // Arabisch: kareem-medium (0,094) und -low (0,066) sprechen; die medium ist im Angebot.
  { id:'ar_JO-kareem-medium', lang:'ar', name:'Kareem', size:'mittel', mb:63.2, rtf:0.0944, path:'ar/ar_JO/kareem/medium/' }
];
const ttsRoot = () => path.join(app.getPath('userData'), 'stimmen');
const piperExe = () => path.join(ttsRoot(), 'piper', 'piper.exe');
const voiceFile = (v, ext) => path.join(ttsRoot(), v.id + ext);
const voiceReady = v => fs.existsSync(voiceFile(v, '.onnx')) && fs.existsSync(voiceFile(v, '.onnx.json'));
// Fertig eingerichtete Stimme zur Sprache. Eine ausdrueckliche Wahl gilt nur,
// wenn ihre Sprache zum Text passt: sonst spraeche ein deutscher Satz mit einer
// englischen Stimme. Passt sie nicht, gewinnt die Stimme der Textsprache.
function ttsVoiceFor(sel, lang) {
  const want = String(lang || '').slice(0, 2).toLowerCase();
  return TTS_VOICES.find(v => v.id === sel && v.lang === want && voiceReady(v)) ||
    TTS_VOICES.find(v => v.lang === want && voiceReady(v)) || null;
}
// Die Stimmenliste fuer die Oberflaeche: Groesse, Zustand und gemessener Faktor.
const ttsList = () => ({ engine: fs.existsSync(piperExe()), dir: ttsRoot(),
  voices: TTS_VOICES.map(v => ({ id:v.id, lang:v.lang, name:v.name, quality:v.size, mb:v.mb, rtf:v.rtf, installed: voiceReady(v) })) });
// Ein Download mit Fortschritt. Hugging Face und GitHub leiten beide um, deshalb
// wird der Weiterleitung gefolgt; geschrieben wird erst in eine .part-Datei,
// damit ein Abbruch keine halbe Stimme als fertig hinterlaesst.
function ttsDownload(url, dest, onPct, hops = 0) {
  return new Promise((resolve, reject) => {
    const lib = url.startsWith('https:') ? require('https') : http;
    const req = lib.get(url, { headers:{ 'User-Agent':'Uebersetzer' } }, res => {
      if (res.statusCode >= 300 && res.statusCode < 400 && res.headers.location && hops < 6) {
        res.resume();
        return ttsDownload(new URL(res.headers.location, url).toString(), dest, onPct, hops + 1).then(resolve, reject);
      }
      if (res.statusCode !== 200) { res.resume(); return reject(new Error('HTTP ' + res.statusCode)); }
      const total = Number(res.headers['content-length'] || 0), out = fs.createWriteStream(dest);
      let got = 0;
      res.on('data', c => { got += c.length; if (total && onPct) onPct(got / total); });
      res.on('error', reject);
      out.on('error', reject);
      out.on('finish', () => out.close(() => resolve()));
      res.pipe(out);
    });
    req.on('error', reject);
  });
}
// Entpacken uebernimmt PowerShell; unter Linux gibt es Piper in dieser App
// nicht, dort sprechen weiter spd-say und espeak-ng.
function ttsUnzip(zip, dir) {
  fs.mkdirSync(dir, { recursive: true });
  const q = s => String(s).replace(/'/g, "''");
  const r = spawnSync('powershell', ['-NoProfile', '-Command', `Expand-Archive -LiteralPath '${q(zip)}' -DestinationPath '${q(dir)}' -Force`], { windowsHide:true, stdio:'ignore' });
  return r.status === 0;
}
const ttsSend = d => { try { if (win && !win.isDestroyed()) win.webContents.send('tts:progress', d); } catch {} };
let ttsBusy = '';
async function ttsInstall(id) {
  const v = TTS_VOICES.find(x => x.id === id);
  if (!v) return { ok:false, error:'unbekannte Stimme' };
  if (!IS_WIN) return { ok:false, error:'nur unter Windows' };
  if (ttsBusy) return { ok:false, error:'laeuft' };
  ttsBusy = id;
  try {
    fs.mkdirSync(ttsRoot(), { recursive:true });
    if (!fs.existsSync(piperExe())) {
      ttsSend({ id, step:'engine', pct:0 });
      const zip = path.join(ttsRoot(), 'piper.zip');
      await ttsDownload(PIPER_ZIP, zip, p => ttsSend({ id, step:'engine', pct:p }));
      if (!ttsUnzip(zip, ttsRoot())) throw new Error('entpacken');
      try { fs.unlinkSync(zip); } catch {}
    }
    for (const ext of ['.onnx', '.onnx.json']) {
      const f = voiceFile(v, ext);
      if (fs.existsSync(f)) continue;
      ttsSend({ id, step:'voice', pct:0 });
      await ttsDownload(PIPER_HF + v.path + v.id + ext, f + '.part', p => ttsSend({ id, step:'voice', pct:p }));
      fs.renameSync(f + '.part', f);
    }
    return { ok:true };
  } catch (e) {
    return { ok:false, error:String((e && e.message) || e) };
  } finally { ttsBusy = ''; ttsSend({ id, done:true }); }
}
function ttsDelete(id) {
  const v = TTS_VOICES.find(x => x.id === id);
  if (!v) return { ok:false };
  ttsStop();
  for (const ext of ['.onnx', '.onnx.json']) { try { fs.unlinkSync(voiceFile(v, ext)); } catch {} }
  return { ok:true };
}
function ttsStop() {
  if (!ttsProc) return;
  try { ttsProc.kill(); } catch {}
  ttsProc = null;
}
// Jede Ausgabe bekommt eine eigene WAV-Datei: beim Vorlesen eines Textes laeuft
// ein Absatz, waehrend der naechste schon erzeugt wird - eine gemeinsame Datei
// wuerde ihm den Ton unter den Fuessen wegziehen. Aufgeraeumt wird nach Alter
// und nicht sofort: eine Datei, die gerade abgespielt wird, liesse sich unter
// Windows ohnehin nicht loeschen.
let ttsLastFile = '';
const ttsTempWav = () => path.join(app.getPath('temp'),
  `uebersetzer-tts-${Date.now()}-${Math.random().toString(36).slice(2, 7)}.wav`);
function ttsPrune() {
  const dir = app.getPath('temp'), now = Date.now();
  let names = [];
  try { names = fs.readdirSync(dir); } catch { return; }
  for (const f of names) {
    if (!/^uebersetzer-tts.*\.wav$/.test(f)) continue;
    const p = path.join(dir, f);
    if (p === ttsLastFile) continue;
    try { if (now - fs.statSync(p).mtimeMs > 5 * 60 * 1000) fs.unlinkSync(p); } catch {}
  }
}
// WAV erzeugen und ihren Pfad liefern; null, wenn Piper nicht sprechen konnte.
function piperSay(text, v) {
  return new Promise(resolve => {
    ttsPrune();
    const out = ttsTempWav();
    let p;
    try {
      p = spawn(piperExe(), ['--model', voiceFile(v, '.onnx'), '--output_file', out],
        { windowsHide:true, stdio:['pipe', 'ignore', 'ignore'], cwd:path.dirname(piperExe()) });
    } catch { return resolve(null); }
    ttsProc = p;
    p.on('error', () => { ttsProc = null; resolve(null); });
    p.on('close', code => { ttsProc = null; resolve(code === 0 && fs.existsSync(out) ? out : null); });
    try { p.stdin.write(String(text) + '\n', 'utf8'); p.stdin.end(); } catch { resolve(null); }
  });
}
async function ttsSpeak(text, lang, voice) {
  const txt = String(text || '').trim();
  if (!txt) return { ok: false, reason: 'leer' };
  const l = String(lang || 'en').slice(0, 2).toLowerCase();
  ttsStop();
  // Eine heruntergeladene Stimme hat Vorrang: sie klingt natuerlicher und
  // liefert eine Datei, die der Renderer anhalten und fortsetzen kann.
  const v = ttsVoiceFor(voice, l);
  if (v) {
    const out = await piperSay(txt, v);
    if (out) { ttsLastFile = out; return { ok:true, backend:'piper', voice:v.id, lang:l, file:pathToFileURL(out).href }; }
  }
  if (IS_WIN) {
    const file = path.join(app.getPath('temp'), 'uebersetzer-tts.txt');
    ttsPrune();
    const wav = ttsTempWav();
    try { fs.writeFileSync(file, txt, 'utf8'); } catch { return { ok: false, reason: 'datei' }; }
    const env = Object.assign({}, process.env, { TTS_FILE: file, TTS_LANG: l, TTS_WAV: wav });
    const p = spawn('powershell', ['-NoProfile', '-ExecutionPolicy', 'Bypass', '-EncodedCommand', TTS_PS_B64],
      { windowsHide: true, stdio: 'ignore', env });
    ttsProc = p;
    return await new Promise(resolve => {
      const done = ok => { ttsProc = null; try { fs.unlinkSync(file); } catch {}; if (ok && fs.existsSync(wav)) ttsLastFile = wav; resolve(ok && fs.existsSync(wav) ? { ok:true, backend:'sapi', lang:l, file:pathToFileURL(wav).href } : { ok:false, reason:'sprachausgabe' }); };
      p.on('error', () => done(false));
      p.on('close', code => done(code === 0));
    });
  }
  const has = cmd => { try { return spawnSync('sh', ['-c', `command -v ${cmd}`]).status === 0; } catch { return false; } };
  if (!ttsBackend) ttsBackend = has('spd-say') ? 'spd-say' : has('espeak-ng') ? 'espeak-ng' : has('espeak') ? 'espeak' : 'keine';
  if (ttsBackend === 'keine') return { ok: false, reason: 'keine Sprachausgabe auf diesem System' };
  // spd-say nimmt die Sprache als Kurzform, espeak-ng ebenso; der Text ist ein
  // eigenes Argument, deshalb braucht er keine Anfuehrungszeichen.
  const args = ttsBackend === 'spd-say' ? ['-l', l, '-r', '-10', txt] : ['-v', l, txt];
  ttsProc = spawn(ttsBackend, args, { stdio: 'ignore' });
  ttsProc.on('error', () => { ttsProc = null; });
  return { ok: true, backend: ttsBackend, lang: l };
}
function createTray() {
  // Auf beiden Systemen das PNG: Das ICO liegt nur im Archiv (app.asar), und
  // resPath zeigt im gepackten Betrieb auf app.asar.unpacked, wo nur icon.png
  // liegt (siehe asarUnpack). Mit dem ICO brach die Existenzpruefung ab, und
  // die App lief ohne Symbol im Infobereich. Windows skaliert das PNG selbst
  // auf die Groesse des Infobereichs.
  const icon = resPath('build', 'icon.png');
  if (!fs.existsSync(icon)) return;
  const img = nativeImage.createFromPath(icon);
  if (img.isEmpty()) return;
  try {
    tray = new Tray(img);
    tray.setToolTip(trayTip());
    tray.setContextMenu(Menu.buildFromTemplate([
      { label: 'Öffnen', click: showWindow },
      { type: 'separator' },
      // Beenden laeuft ueber win.close(), damit die Rueckfrage des Editors bei
      // ungesicherten Dokumenten greift und der Merker forceQuit erst dort
      // gesetzt wird; ein app.quit() an dieser Stelle uebersprang sie.
      { label: 'Beenden', click: () => { if (win && !win.isDestroyed()) win.close(); else { forceQuit = true; app.quit(); } } }
    ]));
    tray.on('click', showWindow);
  } catch { tray = null; }
}

let forceQuit = false;
ipcMain.handle('app:hotkey', () => hotkeyOk);
ipcMain.handle('tts:speak', (_e, text, lang, voice) => ttsSpeak(text, lang, voice));
ipcMain.handle('tts:list', () => ttsList());
ipcMain.handle('tts:install', (_e, id) => ttsInstall(id));
ipcMain.handle('tts:delete', (_e, id) => ttsDelete(id));
ipcMain.handle('tts:stop', () => { ttsStop(); return true; });
ipcMain.handle('app:forceQuit', () => {
  forceQuit = true;
  if (win && !win.isDestroyed()) win.close();
});

app.whenReady().then(() => {
  logLine('Start, Version ' + app.getVersion());
  dataCheck();
  try { setPersonal(JSON.parse(fs.readFileSync(persFile(), 'utf8'))); } catch {}
  session.defaultSession.setPermissionRequestHandler((_wc, permission, cb) => cb(permission === 'local-fonts'));
  startOllama(); startNllb(); createTray();
  // Der Kurzbefehl ist eine Zusatzleistung: Ist er schon von einem anderen
  // Programm belegt, laeuft die App ohne ihn weiter.
  try { hotkeyOk = globalShortcut.register(HOTKEY, showWindow); } catch { hotkeyOk = false; }
  // createTray() lief vor der Registrierung, der Tooltip kennt den Kurzbefehl
  // deshalb erst jetzt.
  if (tray) tray.setToolTip(trayTip());
  const icon = resPath('build', IS_WIN ? 'icon.ico' : 'icon.png');
  win = new BrowserWindow({ width: 1280, height: 820, minWidth: 980, minHeight: 600, autoHideMenuBar: true, frame: false,
    ...(fs.existsSync(icon) ? { icon } : {}),
    webPreferences: { webSecurity: false, preload: path.join(__dirname, 'preload.js') } });
  const sendWinState = () => { if (win && !win.isDestroyed()) win.webContents.send('win:state', win.isMaximized()); };
  win.on('maximize', sendWinState); win.on('unmaximize', sendWinState);
  win.on('close', e => {
    if (forceQuit) return;
    e.preventDefault();
    if (win && !win.isDestroyed()) win.webContents.send('app:request-close');
  });
  win.loadFile('index.html', nllbWanted ? { query: { nllb: '1' } } : {});
});
app.on('will-quit', () => {
  try { globalShortcut.unregisterAll(); } catch {}
  try { if (tray) { tray.destroy(); tray = null; } } catch {}
  ttsStop();
  killTree(nllb); killTree(ollama);
});
app.on('window-all-closed', () => app.quit());
app.on('render-process-gone', (_e, _wc, d) => logLine('Oberflaeche beendet: ' + JSON.stringify(d)));
app.on('child-process-gone', (_e, d) => logLine('Kindprozess beendet: ' + JSON.stringify(d)));