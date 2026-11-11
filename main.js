const { app, BrowserWindow, ipcMain, dialog, session } = require('electron');
const { spawn } = require('child_process');
const http = require('http'), path = require('path'), fs = require('fs');
let nllb = null, ollama = null, nllbWanted = false, win = null;

// Erlaubt queryLocalFonts() im Renderer (fuer die Schriftartenliste)
app.commandLine.appendSwitch('enable-features', 'FontAccess');

const killTree = c => { try { if (c && c.pid) spawn('taskkill', ['/pid', String(c.pid), '/t', '/f'], { windowsHide: true, stdio: 'ignore' }); } catch {} };

function startOllama() {
  const req = http.get('http://127.0.0.1:11434', { timeout: 1500 }, res => res.resume());
  req.on('timeout', () => req.destroy());
  req.on('error', () => {
    const exe = path.join(process.env.LOCALAPPDATA || '', 'Programs', 'Ollama', 'ollama.exe');
    try { ollama = spawn(fs.existsSync(exe) ? exe : 'ollama', ['serve'], { windowsHide: true, stdio: 'ignore' }); ollama.on('error', () => { ollama = null; }); } catch { ollama = null; }
  });
}
function startNllb() {
  try {
    const cfg = JSON.parse(fs.readFileSync(path.join(__dirname, 'config.json'), 'utf8'));
    const py = path.join(__dirname, 'nllb-venv', 'Scripts', 'pythonw.exe');
    if (cfg.nllbAutostart && fs.existsSync(py)) {
      nllbWanted = true;
      nllb = spawn(py, [path.join(__dirname, 'nllb_server.py')], { cwd: __dirname, windowsHide: true, stdio: 'ignore' });
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
ipcMain.handle('fonts:list', () => {
  const dirs = [
    path.join(process.env.WINDIR || 'C:\\Windows', 'Fonts'),
    path.join(process.env.LOCALAPPDATA || '', 'Microsoft', 'Windows', 'Fonts'),
  ];
  const names = new Set();
  for (const d of dirs) {
    try {
      for (const f of fs.readdirSync(d)) {
        if (!/\.(ttf|otf|ttc|woff2?)$/i.test(f)) continue;
        let n = path.basename(f, path.extname(f));
        n = n.replace(/[-_ ]?(bold|italic|regular|light|medium|semibold|black|thin|condensed|expanded|oblique|extralight|extrabold|heavy|ultralight)\b/gi, '').trim();
        if (n === n.toLowerCase() || n === n.toUpperCase()) n = n.replace(/\b\w/g, c => c.toUpperCase());
        if (n) names.add(n);
      }
    } catch {}
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

app.whenReady().then(() => {
  try { personal = JSON.parse(fs.readFileSync(persFile(), 'utf8')); } catch {}
  session.defaultSession.setPermissionRequestHandler((_wc, permission, cb) => cb(permission === 'local-fonts'));
  startOllama(); startNllb();
  const icon = path.join(__dirname, 'build', 'icon.ico');
  win = new BrowserWindow({ width: 1280, height: 820, minWidth: 980, minHeight: 600, autoHideMenuBar: true,
    ...(fs.existsSync(icon) ? { icon } : {}),
    webPreferences: { webSecurity: false, preload: path.join(__dirname, 'preload.js') } });
  win.loadFile('index.html', nllbWanted ? { query: { nllb: '1' } } : {});
});
app.on('will-quit', () => { killTree(nllb); killTree(ollama); });
app.on('window-all-closed', () => app.quit());