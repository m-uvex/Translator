const { app, BrowserWindow, ipcMain, dialog, session } = require('electron');
const { spawn } = require('child_process');
const http = require('http'), path = require('path'), fs = require('fs'), os = require('os');
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

function killTree(c) {
  if (!c || !c.pid) return;
  try {
    if (IS_WIN) spawn('taskkill', ['/pid', String(c.pid), '/t', '/f'], { windowsHide: true, stdio: 'ignore' });
    else process.kill(-c.pid, 'SIGTERM');
  } catch { try { c.kill(); } catch {} }
}

function startOllama() {
  const req = http.get('http://127.0.0.1:11434', { timeout: 1500 }, res => res.resume());
  req.on('timeout', () => req.destroy());
  req.on('error', () => {
    try { ollama = runHidden(ollamaExe(), ['serve']); ollama.on('error', () => { ollama = null; }); } catch { ollama = null; }
  });
}
function startNllb() {
  try {
    const cfg = readCfg();
    const py = venvPython();
    if (cfg.nllbAutostart && py) {
      nllbWanted = true;
      nllb = runHidden(py, [resPath('nllb_server.py')], { cwd: __dirname });
      nllb.on('error', () => {});
    }
  } catch {}
}

ipcMain.handle('extract', async (_e, name, data) => {
  const buf = Buffer.from(data), ext = path.extname(name).toLowerCase();
  if (ext === '.docx') return (await require('mammoth').extractRawText({ buffer: buf })).value.trim();
  if (ext === '.pdf') {
    const { PDFParse } = require('pdf-parse'); const p = new PDFParse({ data: new Uint8Array(buf) });
    try { return (await p.getText()).text.replace(/\n*-- \d+ of \d+ --\n*/g, '\n\n').trim(); } finally { await p.destroy(); }
  }
  throw new Error('unsupported');
});

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
const spellers = {}; let personal = [];
const persFile = () => path.join(app.getPath('userData'), 'personal-dictionary.json');
function speller(lang) {
  if (spellers[lang]) return spellers[lang];
  let dir = path.join(__dirname, 'node_modules', 'dictionary-' + lang);
  if (!fs.existsSync(path.join(dir, 'index.aff'))) {
    const localDir = path.join(__dirname, 'dict', lang);
    if (fs.existsSync(path.join(localDir, 'index.aff'))) dir = localDir;
  }
  try {
    const sp = require('nspell')(fs.readFileSync(path.join(dir, 'index.aff')), fs.readFileSync(path.join(dir, 'index.dic')));
    personal.forEach(w => sp.add(w)); return (spellers[lang] = sp);
  } catch {
    return { correct: () => true, suggest: () => [], add: () => {} };
  }
}
const wordOk = (sp, w) => sp.correct(w) || (/[-'’]/.test(w) && w.split(/['’-]/).every(p => !p || sp.correct(p)));
// Fehler NICHT schlucken - der Renderer loggt sie
ipcMain.handle('spell:check', (_e, lang, words) => { const sp = speller(lang); return words.filter(w => !wordOk(sp, w)); });
ipcMain.handle('spell:suggest', (_e, lang, word) => { try { return speller(lang).suggest(word).slice(0, 5); } catch { return []; } });
ipcMain.handle('spell:add', (_e, word) => {
  if (!personal.includes(word)) { personal.push(word); try { fs.writeFileSync(persFile(), JSON.stringify(personal)); } catch {} Object.values(spellers).forEach(s => s.add(word)); }
  return true;
});
ipcMain.handle('spell:getPersonal', () => [...personal]);
ipcMain.handle('spell:remove', (_e, word) => {
  personal = personal.filter(w => w !== word);
  try { fs.writeFileSync(persFile(), JSON.stringify(personal)); } catch {}
  for (const k in spellers) delete spellers[k];
  return true;
});
ipcMain.handle('spell:edit', (_e, oldWord, newWord) => {
  const idx = personal.indexOf(oldWord);
  if (idx >= 0 && newWord.trim()) {
    personal[idx] = newWord.trim();
    try { fs.writeFileSync(persFile(), JSON.stringify(personal)); } catch {}
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
  try { fs.writeFileSync(r.filePath, JSON.stringify(personal, null, 2), 'utf8'); return personal.length; }
  catch { return -1; }
});
ipcMain.handle('dict:import', async () => {
  const r = await dialog.showOpenDialog(win, { properties: ['openFile'], filters: [{ name: 'JSON oder Text', extensions: ['json', 'txt'] }] });
  if (r.canceled || !r.filePaths.length) return null;
  try {
    const merged = mergeDictWords(personal, parseDictFile(fs.readFileSync(r.filePaths[0], 'utf8')));
    personal = merged.list;
    try { fs.writeFileSync(persFile(), JSON.stringify(personal)); } catch {}
    for (const k in spellers) delete spellers[k];
    return { added: merged.added, skipped: merged.skipped, total: personal.length };
  } catch { return { error: true }; }
});

ipcMain.handle('file:save', async (_e, name, data, kind) => {
  const names = { md:'Markdown', html:'HTML', txt:'Text', pdf:'PDF' };
  const r = await dialog.showSaveDialog(win, { defaultPath: name + '.' + kind, filters: [{ name: names[kind], extensions: [kind] }] });
  if (r.canceled || !r.filePath) return false;
  if (kind === 'pdf') fs.writeFileSync(r.filePath, await win.webContents.printToPDF({ pageSize: 'A4', printBackground: false }));
  else fs.writeFileSync(r.filePath, data, 'utf8');
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
        return { id: m.id, name: m.name, created: m.created, updated: m.updated, html };
      } catch { return null; }
    }).filter(Boolean).sort((a, b) => b.updated - a.updated);
  } catch { return []; }
});
ipcMain.handle('lib:read', (_e, id) => {
  try {
    const m = JSON.parse(fs.readFileSync(libMeta(id), 'utf8'));
    return { id: m.id, name: m.name, created: m.created, updated: m.updated, html: fs.readFileSync(libHtml(id), 'utf8') };
  } catch { return null; }
});
ipcMain.handle('lib:save', (_e, doc) => {
  const id = libId(doc && doc.id) || ('d' + Date.now().toString(36) + Math.random().toString(36).slice(2, 6));
  let created = Date.now();
  try { created = JSON.parse(fs.readFileSync(libMeta(id), 'utf8')).created || created; } catch {}
  const meta = { id, name: String((doc && doc.name) || 'Dokument').slice(0, 120), created, updated: Date.now() };
  fs.writeFileSync(libHtml(id), String((doc && doc.html) || ''), 'utf8');
  fs.writeFileSync(libMeta(id), JSON.stringify(meta), 'utf8');
  return meta;
});
ipcMain.handle('lib:delete', (_e, id) => {
  for (const f of [libHtml(id), libMeta(id)]) { try { fs.unlinkSync(f); } catch {} }
  return true;
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

let forceQuit = false;
ipcMain.handle('app:forceQuit', () => {
  forceQuit = true;
  if (win && !win.isDestroyed()) win.close();
});

app.whenReady().then(() => {
  try { personal = JSON.parse(fs.readFileSync(persFile(), 'utf8')); } catch {}
  session.defaultSession.setPermissionRequestHandler((_wc, permission, cb) => cb(permission === 'local-fonts'));
  startOllama(); startNllb();
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
app.on('will-quit', () => { killTree(nllb); killTree(ollama); });
app.on('window-all-closed', () => app.quit());