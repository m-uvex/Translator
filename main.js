const { app, BrowserWindow, ipcMain, dialog, session, clipboard } = require('electron');
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
// Fehler NICHT schlucken - der Renderer loggt sie.
// Das persoenliche Woerterbuch entscheidet zuerst: nspell kann ein Wort nicht
// freischalten, das im Grundbestand nur als Teil eines Kompositums steht
// (Flag `o` = ONLYINCOMPOUND, etwa „ernaehren“, „macht“ oder „schloss“).
// `.add()` haengt dort nur weitere Flags an den gesperrten Eintrag, das Wort
// blieb also fuer immer falsch und die rote Welle kam nach dem naechsten Scan
// zurueck. Wer im Woerterbuch steht, gilt deshalb ohne Rueckfrage als richtig.
ipcMain.handle('spell:check', (_e, lang, words) => {
  const sp = speller(lang);
  return words.filter(w => !personalKeys.has(persKey(w)) && !wordOk(sp, w));
});
ipcMain.handle('spell:suggest', (_e, lang, word) => { try { return speller(lang).suggest(word).slice(0, 5); } catch { return []; } });
ipcMain.handle('spell:add', (_e, word) => {
  const w = String(word == null ? '' : word).trim();
  if (!w) return false;
  const key = persKey(w);
  if (!personalKeys.has(key)) {
    personal.push(w); personalKeys.add(key);
    try { fs.writeFileSync(persFile(), JSON.stringify(personal)); } catch {}
    Object.values(spellers).forEach(s => s.add(w));
  }
  return true;
});
ipcMain.handle('spell:getPersonal', () => [...personal]);
ipcMain.handle('spell:remove', (_e, word) => {
  const key = persKey(word);
  setPersonal(personal.filter(w => persKey(w) !== key));
  try { fs.writeFileSync(persFile(), JSON.stringify(personal)); } catch {}
  for (const k in spellers) delete spellers[k];
  return true;
});
ipcMain.handle('spell:edit', (_e, oldWord, newWord) => {
  const next = String(newWord == null ? '' : newWord).trim();
  const idx = personal.findIndex(w => persKey(w) === persKey(oldWord));
  if (idx >= 0 && next) {
    personal[idx] = next;
    setPersonal(personal);
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
    setPersonal(merged.list);
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
    if (curN > 0 && nextN >= 0 && nextN * 2 >= curN) fs.writeFileSync(b, cur, 'utf8');
    fs.writeFileSync(f, JSON.stringify(data), 'utf8');
    return true;
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
  try { fs.writeFileSync(r.filePath, data, 'utf8'); return true; } catch { return false; }
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
  try { setPersonal(JSON.parse(fs.readFileSync(persFile(), 'utf8'))); } catch {}
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