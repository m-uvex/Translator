'use strict';
// Dokumentuebersetzung mit dem lokalen Modell des Uebersetzers, ohne die
// Gestalt der Datei zu verlieren. Kein Cloud-Dienst, kein Schluessel, kein
// Wasserzeichen: uebersetzt wird mit TranslateGemma, NLLB oder einem anderen
// lokal eingerichteten Modell.
//
// Word: Die Datei wird als ZIP gelesen. Uebersetzt wird ausschliesslich der
// Inhalt von <w:t> in document.xml, den Kopf-/Fusszeilen und den Fussnoten.
// Jeder Absatz geht als Ganzes an das Modell, die Uebersetzung wird auf die
// Textstuecke verteilt; alles andere -
// Schriftart, Groesse, Farbe, Ausrichtung, Absaetze, Nummerierung, Tabellen,
// Hyperlinks, Bilder, Kopf- und Fusszeilen - bleibt unangetastet, weil die
// XML-Struktur nur an den Textstellen beschnitten wird.
//
// PDF: Der Seiteninhalt bleibt als Hintergrund stehen (Bilder, Flaechen,
// Linien, Tabellenstriche). Entfernt werden nur die Textoperatoren (Tj, TJ, ',
// "), also genau die alten Buchstaben. Der uebersetzte Text wird an dieselbe
// Grundlinie gesetzt, mit derselben Schriftfamilie, Groesse und Fuellfarbe und
// derselben Ausrichtung; reicht die Zeilenbreite nicht, wird zuerst umbrochen,
// erst danach leicht verkleinert. Seitenzahl, Seitengroesse, Link-Ziele und
// Bilder bleiben dadurch erhalten.
//
// Uebersetzungen werden zwischengespeichert und gebuendelt (40 Texte je
// Anfrage), damit grosse Dokumente nicht am Rate-Limit scheitern. Ein Fehler
// in einem Absatz kostet nur diesen Absatz: er bleibt in der Ausgangssprache
// stehen, das Dokument wird trotzdem geschrieben.
//
// Grenzen stehen in Doku.txt §225.

const fs = require('fs');
const path = require('path');
const os = require('os');
const zlib = require('zlib');
const JSZip = require('jszip');

// Sprachcodes der Oberflaeche auf die kurze Form bringen ("en-US" -> "en",
// "deu_Latn" -> "deu").
const baseLang = l => String(l || '').toLowerCase().split(/[-_]/)[0];

const xmlEsc = s => String(s).replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
const xmlUnesc = s => String(s).replace(/&lt;/g, '<').replace(/&gt;/g, '>').replace(/&quot;/g, '"').replace(/&apos;/g, "'").replace(/&#(\d+);/g, (_, d) => String.fromCodePoint(+d)).replace(/&#x([0-9a-f]+);/gi, (_, h) => String.fromCodePoint(parseInt(h, 16))).replace(/&amp;/g, '&');
const hasLetters = s => /[\p{L}]/u.test(String(s || ''));

// Fuehrende und abschliessende Leerzeichen des Originals erhalten: lokale
// Modelle schneiden sie ab, in Word trennen sie aber Runs voneinander.
function keepEdge(orig, trans) {
  const o = String(orig || '');
  const t = String(trans == null ? '' : trans);
  if (!t.trim()) return o;
  const lead = /^\s/.test(o) && !/^\s/.test(t) ? (o.match(/^\s+/) || [''])[0] : '';
  const trail = /\s$/.test(o) && !/\s$/.test(t) ? (o.match(/\s+$/) || [''])[0] : '';
  return lead + t.trim() + trail;
}

const sleep = ms => new Promise(r => setTimeout(r, ms));

function fail(code, message) { const e = new Error(message); e.code = code; return e; }
function progress(opt, payload) {
  if (opt && typeof opt.onProgress === 'function') {
    try { opt.onProgress(payload); } catch { /* Die Anzeige darf den Lauf nie stoppen. */ }
  }
}

// ---------- Zwischenspeicher ----------
// Gleiche Absaetze werden beim zweiten Lauf nicht erneut berechnet. Die Datei
// liegt im Benutzerprofil, nicht im Projekt; Pruefungen schalten sie ab.
const MAX_CACHE = 20000;
let cachePath = process.env.DOCLAT_CACHE
  ? path.resolve(process.env.DOCLAT_CACHE)
  : path.join(process.env.APPDATA || path.join(os.homedir(), 'AppData', 'Roaming'), 'uebersetzer', 'doclat-cache.json');
let cache = null;
let cacheDirty = 0;
const cacheOff = opt => (opt && opt.cache === false) || process.env.DOCLAT_NO_CACHE === '1';

function getCache(opt) {
  if (cacheOff(opt)) return {};
  if (!cache) {
    try { const j = JSON.parse(fs.readFileSync(cachePath, 'utf8')); cache = j && typeof j === 'object' ? j : {}; }
    catch { cache = {}; }
  }
  return cache;
}
function cacheGet(opt, key) {
  const c = getCache(opt);
  return Object.prototype.hasOwnProperty.call(c, key) ? c[key] : undefined;
}
function cacheSet(opt, key, value) {
  if (cacheOff(opt) || value == null) return;
  const c = getCache(opt);
  c[key] = value;
  cacheDirty += 1;
}
function cacheKey(tgt, src, text) { return baseLang(tgt) + '\u0001' + (baseLang(src) || 'auto') + '\u0001' + text; }
function saveCache(opt) {
  if (cacheOff(opt) || !cacheDirty || !cache) return;
  try {
    fs.mkdirSync(path.dirname(cachePath), { recursive: true });
    const keys = Object.keys(cache);
    if (keys.length > MAX_CACHE) for (const k of keys.slice(0, keys.length - MAX_CACHE)) delete cache[k];
    const tmp = cachePath + '.tmp';
    fs.writeFileSync(tmp, JSON.stringify(cache));
    fs.renameSync(tmp, cachePath);
    cacheDirty = 0;
  } catch { /* Ohne Schreibrecht bleibt der Speicher nur im Prozess. */ }
}
function setCacheFile(file) { cachePath = path.resolve(String(file)); cache = null; cacheDirty = 0; }

// ---------- Uebersetzer (lokal) ----------
// Kein Cloud-Dienst: uebersetzt wird mit dem Modell, das im Uebersetzer
// eingestellt ist. Ollama-Modelle (TranslateGemma, gemma3) und die lokalen
// NLLB-Server-Modelle benutzen dieselben Adressen wie der Hauptprozess.

const OLLAMA_LANG = {
  de: 'German', en: 'English', fr: 'French', es: 'Spanish', it: 'Italian',
  nl: 'Dutch', pl: 'Polish', pt: 'Portuguese', tr: 'Turkish', ru: 'Russian',
  uk: 'Ukrainian', cs: 'Czech', da: 'Danish', fi: 'Finnish', sv: 'Swedish',
  nb: 'Norwegian', no: 'Norwegian', el: 'Greek', hu: 'Hungarian', ro: 'Romanian',
  bg: 'Bulgarian', sk: 'Slovak', sl: 'Slovenian', et: 'Estonian', lt: 'Lithuanian',
  lv: 'Latvian', ar: 'Arabic', ja: 'Japanese', ko: 'Korean', zh: 'Chinese',
  id: 'Indonesian'
};
const langWord = c => OLLAMA_LANG[baseLang(c)] || String(c || '');

// Obergrenzen je Block: kleine Modelle verlieren bei sehr langen Anfragen den
// Faden, und der Fortschritt soll regelmaessig sichtbar werden.
const MAX_TEXTS = 8;
const MAX_CHARS = 12000;

function promptFor(text, opt) {
  const a = baseLang(opt.src) === 'auto' ? 'source-language' : langWord(opt.src);
  const b = langWord(opt.tgt);
  return `You are a professional ${a} (${opt.src || 'auto'}) to ${b} (${opt.tgt}) translator. ` +
    `Your goal is to accurately convey the meaning and nuances of the original ${a} text while adhering to ${b} grammar, vocabulary, and cultural sensitivities.\n` +
    `Produce only the ${b} translation, without any additional explanations or commentary. ` +
    `Please translate the following ${a} text into ${b}:\n\n\n${text}`;
}

async function ollamaTranslate(texts, opt) {
  const base = String(opt.url || 'http://127.0.0.1:11434').replace(/\/+$/, '');
  const model = String(opt.model || '').trim();
  if (!model) throw fail('engine', 'Kein Uebersetzungsmodell gewaehlt');
  const out = [];
  for (const text of texts) {
    let last = null;
    for (let attempt = 0; attempt < 2; attempt += 1) {
      try {
        const res = await fetch(base + '/api/chat', {
          method: 'POST',
          headers: { 'content-type': 'application/json' },
          body: JSON.stringify({
            model, stream: false,
            messages: [{ role: 'user', content: promptFor(text, opt) }],
            options: { temperature: 0.1, num_ctx: 8192 }
          })
        });
        if (!res.ok) throw fail('engine', 'Ollama antwortet mit ' + res.status);
        const j = await res.json();
        let t = String((j.message && j.message.content) || '').trim();
        t = t.replace(/^```[a-z]*\s*/i, '').replace(/```$/, '').trim();
        if (!t) throw fail('empty', 'Das Modell liefert keinen Text');
        out.push(t);
        last = null;
        break;
      } catch (e) { last = e; await sleep(400); }
    }
    if (last) throw last;
  }
  return out;
}

async function nllbTranslate(texts, opt) {
  const base = String(opt.nllb || 'http://127.0.0.1:8765').replace(/\/+$/, '');
  const res = await fetch(base + '/translate', {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ model: opt.model, src: opt.src, tgt: opt.tgt, text: texts.join('\n') })
  });
  if (!res.ok) throw fail('engine', 'Der Uebersetzungsserver antwortet mit ' + res.status);
  const j = await res.json();
  const parts = String(j.translation == null ? '' : j.translation).split('\n');
  if (parts.length !== texts.length) throw fail('engine', 'Der Server liefert ' + parts.length + ' statt ' + texts.length + ' Texte');
  return parts;
}

async function localTranslate(texts, opt) {
  if (!texts.length) return [];
  if (String(opt.engine || '') === 'nllb') return nllbTranslate(texts, opt);
  return ollamaTranslate(texts, opt);
}

// Eine Anfrage je Stueck: lokale Modelle arbeiten der Reihe nach, damit sie
// sich nicht gegenseitig den Speicher wegnehmen. Fehler werden halbiert, bis
// der einzelne Text scheitert; der bleibt dann in der Ausgangssprache.
async function translateChunk(texts, opt, stats) {
  if (!texts.length) return [];
  const call = typeof opt.translate === 'function' ? opt.translate : localTranslate;
  try {
    const out = await call(texts, opt);
    if (!Array.isArray(out) || out.length !== texts.length) throw fail('engine', 'Uebersetzer liefert ' + (out && out.length) + ' statt ' + texts.length + ' Texte');
    stats.requests += 1;
    stats.chars += texts.reduce((n, t) => n + t.length, 0) + texts.length;
    return out;
  } catch (e) {
    const code = String((e && e.code) || '');
    if (code === 'engine' || code === 'network' || code === 'noKey') throw e;
    if (texts.length > 1) {
      const mid = Math.ceil(texts.length / 2);
      const a = await translateChunk(texts.slice(0, mid), opt, stats);
      const b = await translateChunk(texts.slice(mid), opt, stats);
      return a.concat(b);
    }
    stats.failed += 1;
    if (stats.errors.length < 5) stats.errors.push(((e && e.message) || String(e)).slice(0, 160));
    return [null];
  }
}

function emptyStats() {
  return { requests: 0, chars: 0, cached: 0, failed: 0, kept: 0, translated: 0, errors: [] };
}

// Uebersetzt eine Liste Texte: doppelte nur einmal, Treffer aus dem Speicher,
// der Rest in Bloecken.
async function translateMany(texts, opt, stats, phase) {
  const tgt = baseLang(opt.tgt);
  const src = baseLang(opt.src) || 'auto';
  const uniq = [...new Set(texts.filter(hasLetters))];
  const map = new Map();
  const todo = [];
  for (const t of uniq) {
    const hit = cacheGet(opt, cacheKey(tgt, src, t));
    if (hit !== undefined) { map.set(t, hit); stats.cached += 1; } else todo.push(t);
  }
  if (todo.length && phase) progress(opt, { phase, done: 0, total: todo.length, percent: 0 });
  let i = 0, sent = 0;
  while (i < todo.length) {
    const part = [];
    let chars = 0;
    while (i < todo.length && part.length < MAX_TEXTS && (chars + todo[i].length <= MAX_CHARS || !part.length)) {
      part.push(todo[i]);
      chars += todo[i].length;
      i += 1;
    }
    const out = await translateChunk(part, opt, stats);
    part.forEach((t, k) => {
      const v = out[k];
      if (v == null) { stats.kept += 1; return; }
      map.set(t, v);
      cacheSet(opt, cacheKey(tgt, src, t), v);
    });
    sent += part.length;
    if (phase) progress(opt, { phase, done: sent, total: todo.length, percent: todo.length ? Math.round(sent / todo.length * 100) : 100 });
  }
  return map;
}

// ---------- Word ----------
const DOCX_PARTS = /^word\/(document|header\d*|footer\d*|footnotes|endnotes)\.xml$/;

function paraSpans(xml) {
  const re = /<w:p(\s[^>]*)?(\/?)>|<\/w:p>/g;
  const stack = [];
  const spans = [];
  let m;
  while ((m = re.exec(xml))) {
    if (m[0] === '</w:p>') { const s = stack.pop(); if (s != null) spans.push({ start: s, end: re.lastIndex }); }
    else if (!m[2]) stack.push(m.index);
  }
  return spans;
}

function textElements(xml, spans) {
  const re = /<w:t(\s[^>]*)?>([\s\S]*?)<\/w:t>/g;
  const out = [];
  let m;
  while ((m = re.exec(xml))) {
    const start = m.index;
    const innerStart = start + m[0].indexOf('>') + 1;
    const innerEnd = start + m[0].length - '</w:t>'.length;
    // Der naechstgelegene Absatz (bei verschachtelten Textfeldern der innere).
    let owner = -1, best = -1;
    for (let i = 0; i < spans.length; i += 1) {
      const sp = spans[i];
      if (sp.start < start && start < sp.end && sp.start > best) { best = sp.start; owner = i; }
    }
    out.push({ start, attrs: m[1] || '', innerStart, innerEnd, text: xmlUnesc(m[2]), owner });
  }
  return out;
}

// Verteilt eine Absatzuebersetzung auf die Textstuecke. Die Stuecke tragen die
// Formatierung; ihre Laenge bestimmt, wie viel der Uebersetzung sie bekommen.
// Zahlen und Zeichen ohne Buchstaben bleiben unangetastet.
function distributeTranslation(translation, texts) {
  const out = texts.slice();
  const idx = texts.map((t, i) => (hasLetters(t) ? i : -1)).filter(i => i >= 0);
  if (!idx.length) return out;
  if (idx.length === 1) { out[idx[0]] = keepEdge(texts[idx[0]], translation); return out; }
  const parts = splitByWeights(translation, idx.map(i => Math.max(1, texts[i].length)));
  idx.forEach((i, k) => { out[i] = keepEdge(texts[i], parts[k]); });
  return out;
}

async function translateDocxBuffer(buf, opt) {
  const zip = await JSZip.loadAsync(buf);
  const names = Object.keys(zip.files).filter(n => !zip.files[n].dir);
  const parts = names.filter(n => DOCX_PARTS.test(n));
  if (!parts.length) throw fail('docx', 'Kein Word-Text gefunden');

  // Erst alles einlesen, damit der Fortschritt eine echte Gesamtzahl hat.
  const docs = [];
  for (const name of parts) {
    const xml = await zip.file(name).async('string');
    const els = textElements(xml, paraSpans(xml));
    if (!els.length) continue;
    const groups = new Map();
    for (const el of els) {
      if (el.owner < 0) continue;
      if (!groups.has(el.owner)) groups.set(el.owner, []);
      groups.get(el.owner).push(el);
    }
    docs.push({ name, xml, groups: [...groups.values()] });
  }

  const stats = Object.assign(emptyStats(), {
    kind: 'docx', parts: docs.length, paragraphs: 0, segments: 0, unchanged: 0,
    engine: String(opt.engine || 'ollama'), model: String(opt.model || '')
  });

  // 1. Absaetze als Ganzes uebersetzen.
  const plains = [];
  for (const d of docs) for (const g of d.groups) plains.push(g.map(e => e.text).join(''));
  const texts = plains.filter(t => hasLetters(t));
  progress(opt, { phase: 'docx', done: 0, total: texts.length, percent: 0 });
  const map = await translateMany(texts, opt, stats, 'docx');
  if (texts.length) progress(opt, { phase: 'docx', done: texts.length, total: texts.length, percent: 100 });

  // 2. Was fehlt, je Textstueck nachholen; scheitert es, bleibt es stehen.
  const need = [];
  const plan = docs.map(d => d.groups.map(g => {
    const plain = g.map(e => e.text).join('');
    const tr = map.get(plain);
    if (tr == null) { if (hasLetters(plain)) g.forEach(e => { if (hasLetters(e.text)) need.push(e.text); }); return { g, parts: null }; }
    return { g, parts: distributeTranslation(tr, g.map(e => e.text)) };
  }));
  const extra = need.length ? await translateMany(need, opt, stats, 'docx-items') : new Map();

  // 3. Zurueckschreiben: nur die Textstellen, sonst nichts.
  for (let di = 0; di < docs.length; di += 1) {
    const d = docs[di];
    const edits = [];
    for (const { g, parts } of plan[di]) {
      stats.paragraphs += 1;
      g.forEach((el, i) => {
        if (!hasLetters(el.text)) { stats.segments += 1; return; }
        stats.segments += 1;
        let text = parts ? parts[i] : null;
        if (text == null) text = extra.has(el.text) ? keepEdge(el.text, extra.get(el.text)) : null;
        if (text == null || !String(text).trim()) { stats.unchanged += 1; return; }
        if (text === el.text) { stats.unchanged += 1; return; }
        stats.translated += 1;
        edits.push({ el, text });
      });
    }
    let out = d.xml;
    edits.sort((a, b) => b.el.innerStart - a.el.innerStart);
    for (const { el, text } of edits) {
      out = out.slice(0, el.innerStart) + xmlEsc(text) + out.slice(el.innerEnd);
      // Fuehrende/abschliessende Leerzeichen brauchen xml:space, sonst entfernt
      // Word sie beim naechsten Oeffnen.
      if (/^\s|\s$/.test(text) && !/xml:space/.test(el.attrs)) {
        const tagStart = out.lastIndexOf('<w:t', el.innerStart);
        const tagEnd = out.indexOf('>', tagStart);
        out = out.slice(0, tagEnd) + ' xml:space="preserve"' + out.slice(tagEnd);
      }
    }
    if (out !== d.xml) zip.file(d.name, out);
  }

  saveCache(opt);
  progress(opt, { phase: 'write', done: 1, total: 1, percent: 100 });
  const done = await zip.generateAsync({ type: 'nodebuffer', compression: 'DEFLATE', compressionOptions: { level: 6 } });
  return { buffer: done, stats };
}

// ---------- PDF: Text aus dem Seiteninhalt entfernen ----------
// Der Inhalt bleibt als Hintergrund stehen; nur die Textoperatoren fallen weg.
const TEXT_OPS = new Set(['Tj', 'TJ', "'", '"']);

function tokenizePdf(src) {
  const toks = [];
  const n = src.length;
  const ws = c => c === ' ' || c === '\n' || c === '\r' || c === '\t' || c === '\f' || c === '\0';
  const delim = c => '()<>[]{}/%'.includes(c);
  let i = 0;
  while (i < n) {
    const c = src[i];
    if (ws(c)) { i += 1; continue; }
    if (c === '%') { while (i < n && src[i] !== '\n' && src[i] !== '\r') i += 1; continue; }
    if (c === '(') {
      let j = i + 1, depth = 1;
      while (j < n && depth) {
        const ch = src[j];
        if (ch === '\\') { j += 2; continue; }
        if (ch === '(') depth += 1; else if (ch === ')') depth -= 1;
        j += 1;
      }
      toks.push({ t: 'string', raw: src.slice(i, j) }); i = j; continue;
    }
    if (c === '<' && src[i + 1] === '<') {
      let j = i + 2, depth = 1;
      while (j < n && depth) {
        if (src[j] === '<' && src[j + 1] === '<') { depth += 1; j += 2; continue; }
        if (src[j] === '>' && src[j + 1] === '>') { depth -= 1; j += 2; continue; }
        if (src[j] === '(') { let d = 1; j += 1; while (j < n && d) { const ch = src[j]; if (ch === '\\') { j += 2; continue; } if (ch === '(') d += 1; else if (ch === ')') d -= 1; j += 1; } continue; }
        j += 1;
      }
      toks.push({ t: 'dict', raw: src.slice(i, j) }); i = j; continue;
    }
    if (c === '<') {
      let j = src.indexOf('>', i);
      if (j < 0) j = n - 1;
      toks.push({ t: 'hex', raw: src.slice(i, j + 1) }); i = j + 1; continue;
    }
    if (c === '[') {
      let j = i + 1, depth = 1;
      while (j < n && depth) {
        const ch = src[j];
        if (ch === '\\') { j += 2; continue; }
        if (ch === '(') { let d = 1; j += 1; while (j < n && d) { const x = src[j]; if (x === '\\') { j += 2; continue; } if (x === '(') d += 1; else if (x === ')') d -= 1; j += 1; } continue; }
        if (ch === '[') depth += 1; else if (ch === ']') depth -= 1;
        j += 1;
      }
      toks.push({ t: 'array', raw: src.slice(i, j) }); i = j; continue;
    }
    if (c === '/') {
      let j = i + 1;
      while (j < n && !ws(src[j]) && !delim(src[j])) j += 1;
      toks.push({ t: 'name', raw: src.slice(i, j) }); i = j; continue;
    }
    let j = i;
    while (j < n && !ws(src[j]) && !delim(src[j])) j += 1;
    if (j === i) { i += 1; continue; }
    const word = src.slice(i, j);
    i = j;
    if (/^[+-]?(\d|\.\d)/.test(word)) { toks.push({ t: 'num', raw: word }); continue; }
    if (word === 'BI') {
      // Inline-Bild: der binaere Teil darf nicht als Operatoren gelesen werden.
      const id = src.indexOf('ID', i);
      if (id < 0) { toks.push({ t: 'inline', raw: src.slice(j) }); break; }
      let k = id + 2;
      while (k < n - 1) {
        if ((src[k] === 'E' && src[k + 1] === 'I') && (ws(src[k - 1]) || src[k - 1] === undefined) && (k + 2 >= n || ws(src[k + 2]))) break;
        k += 1;
      }
      toks.push({ t: 'inline', raw: src.slice(j, Math.min(n, k + 2)) });
      i = Math.min(n, k + 2);
      continue;
    }
    toks.push({ t: 'op', raw: word });
  }
  return toks;
}

function stripTextOps(src) {
  const toks = tokenizePdf(src);
  const out = [];
  let operands = [];
  for (const tk of toks) {
    if (tk.t === 'op') {
      if (TEXT_OPS.has(tk.raw)) { operands = []; continue; }
      for (const o of operands) out.push(o.raw);
      out.push(tk.raw);
      operands = [];
    } else if (tk.t === 'num' || tk.t === 'name' || tk.t === 'string' || tk.t === 'hex' || tk.t === 'array' || tk.t === 'dict' || tk.t === 'inline') {
      operands.push(tk);
    }
  }
  for (const o of operands) out.push(o.raw);
  return out.join(' ');
}

// ---------- PDF: Zeilen, Farben, Schriften ----------
async function loadPdfjs() { return import('pdfjs-dist/legacy/build/pdf.mjs'); }

function pdfjsStdUrl() {
  try {
    const pkg = require.resolve('pdfjs-dist/package.json');
    const dir = path.join(path.dirname(pkg), 'standard_fonts').split(path.sep).join('/');
    return 'file:///' + (dir.startsWith('/') ? dir.slice(1) : dir) + '/';
  } catch { return null; }
}

// pdf.js liefert die Fuellfarbe je nach Fassung als Hex-Zeichenkette
// ("#1f4f78"), als Feld [r,g,b] oder als Float32Array.
function parseColor(args) {
  if (args == null) return null;
  const hex = raw => {
    const h = String(raw).replace(/^#/, '');
    if (h.length !== 6) return null;
    return [parseInt(h.slice(0, 2), 16), parseInt(h.slice(2, 4), 16), parseInt(h.slice(4, 6), 16)];
  };
  if (typeof args === 'string') return hex(args);
  if (typeof args[0] === 'string') return hex(args[0]);
  if (args.length >= 3) {
    const v = [Number(args[0]), Number(args[1]), Number(args[2])];
    if (v.some(x => !Number.isFinite(x))) return null;
    const scale = v.every(x => x <= 1.0001) ? 255 : 1;
    return v.map(x => Math.round(x * scale));
  }
  return null;
}

// Farbe je Textausgabe in der Reihenfolge des Inhalts. Ohne diesen Durchlauf
// waere jede Zeile schwarz, weil getTextContent keine Farbe kennt.
async function pageTextColors(page, pdfjs) {
  try {
    const ops = await page.getOperatorList();
    const OPS = pdfjs.OPS;
    let cur = [0, 0, 0];
    const out = [];
    for (let i = 0; i < ops.fnArray.length; i += 1) {
      const fn = ops.fnArray[i];
      const args = ops.argsArray[i];
      if (fn === OPS.setFillRGBColor || fn === OPS.setFillColor) { const c = parseColor(args); if (c) cur = c; }
      else if (fn === OPS.setFillGray) { const g = Math.round(Number(args[0]) * 255); if (Number.isFinite(g)) cur = [g, g, g]; }
      else if (fn === OPS.showText || fn === OPS.showSpacedText) out.push(cur);
    }
    return out;
  } catch { return []; }
}

function fontOf(page, item) {
  let o = null;
  try { o = page.commonObjs.get(item.fontName); } catch { o = null; }
  const name = String((o && o.name) || item.fontName || '');
  const bold = !!(o && o.bold) || /bold|black|heavy|semibold|demi/i.test(name);
  const italic = !!(o && o.italic) || /italic|oblique/i.test(name);
  const mono = /courier|mono|consol|fixed|typewriter/i.test(name);
  const serif = !mono && /times|serif|georgia|garamond|cambria|book|roman|palatino|minion/i.test(name);
  const family = mono ? 'mono' : (serif ? 'serif' : 'sans');
  return { name, family, bold, italic };
}

const num = v => (Number.isFinite(Number(v)) ? Number(v) : 0);

function makeLine(items) {
  const first = items[0];
  const last = items[items.length - 1];
  const h = Math.max(1, ...items.map(it => num(it.height) || Math.hypot(num(it.transform[2]), num(it.transform[3])) || 10));
  let text = '';
  let prevEnd = null;
  for (const it of items) {
    const x = num(it.transform[4]);
    if (prevEnd != null && x - prevEnd > h * 0.22) text += ' ';
    text += it.str;
    prevEnd = x + num(it.width);
  }
  const right = num(last.transform[4]) + num(last.width);
  return {
    y: num(first.transform[5]),
    x: num(first.transform[4]),
    right,
    width: Math.max(1, right - num(first.transform[4])),
    size: Math.max(...items.map(it => num(it.size) || num(it.height) || 10)),
    text: text.replace(/\s+/g, ' ').trim(),
    items
  };
}

function groupLines(items) {
  const rows = new Map();
  for (const it of items) {
    if (!String(it.str || '').trim()) continue;
    const key = Math.round(it.transform[5] / 2.5);
    if (!rows.has(key)) rows.set(key, []);
    rows.get(key).push(it);
  }
  const lines = [];
  for (const list of rows.values()) {
    list.sort((a, b) => a.transform[4] - b.transform[4]);
    let seg = [];
    const flush = () => { if (seg.length) lines.push(makeLine(seg)); seg = []; };
    for (const it of list) {
      if (seg.length) {
        const prev = seg[seg.length - 1];
        const gap = num(it.transform[4]) - (num(prev.transform[4]) + num(prev.width));
        const h = Math.max(1, num(prev.height) || 10, num(it.height) || 10);
        // Grosse Luecke = zweispaltiger Satz oder Tabelle, keine Zeile.
        if (gap > h * 2.5) flush();
      }
      seg.push(it);
    }
    flush();
  }
  lines.sort((a, b) => (b.y - a.y) || (a.x - b.x));
  return lines;
}

// Ausrichtung aus der Lage im Satzspiegel: linksbuendig, zentriert oder rechts.
function pageBlock(lines) {
  let left = Infinity, right = -Infinity;
  for (const l of lines) { left = Math.min(left, l.x); right = Math.max(right, l.right); }
  if (!Number.isFinite(left)) return { left: 0, right: 0, width: 0 };
  return { left, right, width: Math.max(1, right - left) };
}

function alignOf(line, block) {
  if (!block.width) return 'left';
  const l = line.x - block.left;
  const r = block.right - line.right;
  if (r <= 2.5 && l > 3) return 'right';
  if (l > 3 && r > 3 && Math.abs(l - r) <= Math.max(6, line.size * 0.7)) return 'center';
  return 'left';
}

// ---------- PDF: Schriften einbetten ----------
const FONT_DIR = () => path.join(process.env.WINDIR || 'C:\\Windows', 'Fonts');
const FONT_FILES = {
  arial: ['arial.ttf', 'arialbd.ttf', 'ariali.ttf', 'arialbi.ttf'],
  times: ['times.ttf', 'timesbd.ttf', 'timesi.ttf', 'timesbi.ttf'],
  courier: ['cour.ttf', 'courbd.ttf', 'couri.ttf', 'courbi.ttf'],
  calibri: ['calibri.ttf', 'calibrib.ttf', 'calibrii.ttf', 'calibriz.ttf'],
  cambria: ['cambria.ttc', 'cambriab.ttf', 'cambriai.ttf', 'cambriaz.ttf'],
  georgia: ['georgia.ttf', 'georgiab.ttf', 'georgiai.ttf', 'georgiaz.ttf'],
  verdana: ['verdana.ttf', 'verdanab.ttf', 'verdanai.ttf', 'verdanaz.ttf'],
  tahoma: ['tahoma.ttf', 'tahomabd.ttf', 'tahomabd.ttf', 'tahomabd.ttf'],
  consolas: ['consola.ttf', 'consolab.ttf', 'consolai.ttf', 'consolaz.ttf']
};

function familyKey(font) {
  const n = String(font.name || '').toLowerCase();
  if (/courier|mono|consol/.test(n) || font.family === 'mono') return 'courier';
  if (/times|roman|georgia|garamond|cambria|palatino|minion|book/.test(n) || font.family === 'serif') return 'times';
  if (/calibri/.test(n)) return 'calibri';
  if (/verdana/.test(n)) return 'verdana';
  if (/tahoma/.test(n)) return 'tahoma';
  if (/segoe/.test(n)) return 'segoe';
  return 'arial';
}

const standardName = (font) => {
  const key = familyKey(font);
  if (key === 'times') return 'Times';
  if (key === 'courier') return 'Courier';
  return 'Helvetica';
};

function standardVariant(StandardFonts, base, bold, italic) {
  if (base === 'Times') {
    if (bold && italic) return StandardFonts.TimesRomanBoldItalic;
    if (bold) return StandardFonts.TimesRomanBold;
    if (italic) return StandardFonts.TimesRomanItalic;
    return StandardFonts.TimesRoman;
  }
  if (base === 'Courier') {
    if (bold && italic) return StandardFonts.CourierBoldOblique;
    if (bold) return StandardFonts.CourierBold;
    if (italic) return StandardFonts.CourierOblique;
    return StandardFonts.Courier;
  }
  if (bold && italic) return StandardFonts.HelveticaBoldOblique;
  if (bold) return StandardFonts.HelveticaBold;
  if (italic) return StandardFonts.HelveticaOblique;
  return StandardFonts.Helvetica;
}

function ttfBytes(file) {
  try {
    if (!fs.existsSync(file)) return null;
    const data = fs.readFileSync(file);
    // .ttc ist eine Sammlung; pdf-lib/fontkit erwartet eine einzelne Schrift.
    if (data.slice(0, 4).toString() === 'ttcf') return null;
    return data;
  } catch { return null; }
}

async function makeFontResolver(doc, pdfjsLib) {
  const { StandardFonts } = pdfjsLib;
  const cacheMap = new Map();
  let kit = null;
  return async function resolve(font, sample) {
    const key = familyKey(font) + '|' + (font.bold ? 1 : 0) + (font.italic ? 1 : 0);
    if (cacheMap.has(key)) return cacheMap.get(key);
    const needUnicode = /[^\u0000-\u00ff]/.test(String(sample || ''));
    // Die 14 Standardschriften sind am Namen erkennbar; wer mit ihnen arbeitet,
    // bekommt denselben Namen zurueck (Helvetica bleibt Helvetica).
    const isStandard = /^helvetica|^times|^courier|^symbol|^zapf/i.test(String(font.name || ''));
    let out = null;
    if (isStandard && !needUnicode) {
      out = await doc.embedFont(standardVariant(StandardFonts, standardName(font), font.bold, font.italic));
    } else {
      const fam = familyKey(font);
      const files = FONT_FILES[fam] || FONT_FILES.arial;
      const pick = font.bold && font.italic ? 3 : font.bold ? 1 : font.italic ? 2 : 0;
      const tries = [files[pick], files[0]];
      for (const f of tries) {
        const bytes = ttfBytes(path.join(FONT_DIR(), f));
        if (!bytes) continue;
        try {
          if (!kit) { kit = require('@pdf-lib/fontkit'); doc.registerFontkit(kit); }
          out = await doc.embedFont(bytes, { subset: true });
          break;
        } catch { out = null; }
      }
      if (!out) {
        // Ohne passende Datei bleibt die Standardschrift; fuer Zeichen ausserhalb
        // von WinAnsi gibt es dann keine saubere Loesung (siehe Doku).
        out = await doc.embedFont(standardVariant(StandardFonts, standardName(font), font.bold, font.italic));
      }
    }
    cacheMap.set(key, out);
    return out;
  };
}

// ---------- PDF: Schreiben ----------
const MIN_SHRINK = 0.82;

function wrapWords(text, font, size, maxWidth) {
  const words = String(text).split(/\s+/).filter(Boolean);
  const lines = [];
  let cur = '';
  for (const w of words) {
    const test = cur ? cur + ' ' + w : w;
    if (font.widthOfTextAtSize(test, size) <= maxWidth || !cur) cur = test;
    else { lines.push(cur); cur = w; }
  }
  if (cur) lines.push(cur);
  return lines.length ? lines : [String(text)];
}

// Teilt die Uebersetzung im Verhaeltnis der Originalstuecke auf, damit Farbe,
// Fett und Kursiv innerhalb einer Zeile an der richtigen Stelle bleiben.
function splitByWeights(text, weights) {
  const total = weights.reduce((a, b) => a + b, 0) || 1;
  const words = String(text).split(/(\s+)/).filter(s => s !== '');
  const out = new Array(weights.length).fill('');
  let acc = 0;
  let wi = 0;
  let target = weights[0] / total;
  for (const chunk of words) {
    while (wi < weights.length - 1 && acc >= target - 1e-9) { wi += 1; target += weights[wi] / total; }
    out[wi] += chunk;
    acc += chunk.length / Math.max(1, text.length);
  }
  return out.map(s => s.replace(/\s+/g, ' ').trim());
}

async function translatePdfBuffer(buf, opt) {
  const { PDFDocument, PDFName, PDFDict, PDFArray, PDFRawStream, rgb } = require('pdf-lib');
  const pdfjs = await loadPdfjs();
  const std = pdfjsStdUrl();
  const src = await pdfjs.getDocument({ data: new Uint8Array(buf), isEvalSupported: false, verbosity: 0, standardFontDataUrl: std || undefined }).promise;
  const doc = await PDFDocument.load(buf, { ignoreEncryption: true });
  const resolveFont = await makeFontResolver(doc, { StandardFonts: require('pdf-lib').StandardFonts });

  const stats = Object.assign(emptyStats(), {
    kind: 'pdf', pages: 0, lines: 0, drawn: 0, skipped: 0, wrapped: 0, shrunk: 0,
    colorFallback: 0, fonts: []
  });

  // 1. Text und Gestalt lesen.
  const pages = [];
  for (let i = 0; i < doc.getPageCount(); i += 1) {
    const page = await src.getPage(i + 1);
    const content = await page.getTextContent();
    const colors = await pageTextColors(page, pdfjs);
    const items = content.items.filter(it => String(it.str || '').trim());
    const all = content.items;
    items.forEach((it, k) => {
      const c = colors.length === items.length ? colors[k] : (colors.length === 1 ? colors[0] : null);
      if (!c && colors.length) stats.colorFallback += 1;
      it.color = c || [0, 0, 0];
      it.font = fontOf(page, it);
      it.size = Math.max(1, num(it.height) || Math.hypot(num(it.transform[2]), num(it.transform[3])) || 10);
    });
    const lines = groupLines(items);
    const block = pageBlock(lines);
    // Rand aus der Lage des Satzspiegels schaetzen (Word/LaTeX: 2-3 cm).
    const pw = doc.getPage(i).getWidth();
    const margin = Math.max(12, Math.min(block.left || 0, Math.max(0, pw - (block.right || pw))));
    lines.forEach(l => {
      l.align = alignOf(l, block);
      l.fonts = l.items.map(it => it.font || fontOf(page, it));
      l.sizes = l.items.map(it => it.size);
      l.colors = l.items.map(it => it.color);
    });
    pages.push({ lines, block, margin, empty: !items.length, allItems: all.length });
  }

  // 2. Uebersetzen (gebueindelt, mit Speicher).
  const texts = pages.flatMap(p => p.lines.filter(l => hasLetters(l.text)).map(l => l.text));
  const translated = await translateMany(texts, opt, stats, 'pdf');

  // 3. Alten Text aus dem Seiteninhalt nehmen; der Rest bleibt Hintergrund.
  const stripStream = (stream) => {
    const filterRef = stream.dict.lookup(PDFName.of('Filter'));
    const first = filterRef instanceof PDFArray ? filterRef.lookup(0) : filterRef;
    const fname = first ? String(first.asString ? first.asString() : first) : '';
    const raw = Buffer.from(stream.contents);
    let plain;
    if (/FlateDecode/.test(fname)) { try { plain = zlib.inflateSync(raw); } catch { return null; } }
    else if (!fname) plain = raw;
    else return null;
    return Buffer.from(stripTextOps(plain.toString('latin1')), 'latin1');
  };
  const replaceStream = (holder, key, stream) => {
    const plain = stripStream(stream);
    if (!plain) return false;
    holder.set(key, doc.context.register(doc.context.flateStream(plain)));
    return true;
  };
  const seen = new Set();
  const stripContainer = (holder, dictKey) => {
    const node = holder.lookup(dictKey);
    if (!node) return;
    if (node instanceof PDFArray) {
      for (let k = 0; k < node.size(); k += 1) {
        const ref = node.get(k);
        const key = ref && ref.toString ? ref.toString() : '';
        if (key && seen.has(key)) continue;
        if (key) seen.add(key);
        const st = doc.context.lookup(ref);
        if (st instanceof PDFRawStream) replaceStream(node, k, st);
      }
      return;
    }
    const refKey = String(holder.get(dictKey) || '');
    if (refKey && seen.has(refKey)) return;
    if (refKey) seen.add(refKey);
    const st = doc.context.lookup(holder.get(dictKey));
    if (st instanceof PDFRawStream) replaceStream(holder, dictKey, st);
  };
  const stripForms = (dict, depth) => {
    if (depth > 4) return;
    const res = dict.lookup(PDFName.of('Resources'), PDFDict);
    if (!res) return;
    const xo = res.lookup(PDFName.of('XObject'), PDFDict);
    if (!xo) return;
    for (const [k, v] of xo.entries()) {
      const st = doc.context.lookup(v);
      if (!(st instanceof PDFRawStream)) continue;
      const sub = st.dict.lookup(PDFName.of('Subtype'));
      if (!sub || String(sub.asString ? sub.asString() : sub) !== '/Form') continue;
      replaceStream(xo, k, st);
      stripForms(st.dict, depth + 1);
    }
  };
  for (const page of doc.getPages()) {
    stripContainer(page.node, PDFName.of('Contents'));
    stripForms(page.node, 0);
  }

  // 4. Uebersetzten Text setzen.
  const drawOne = (page, font, text, x, y, size, color) => {
    page.drawText(String(text), { x, y, size, font, color: rgb(color[0] / 255, color[1] / 255, color[2] / 255) });
  };
  const fontNames = new Set();
  for (let pi = 0; pi < pages.length; pi += 1) {
    const info = pages[pi];
    const page = doc.getPage(pi);
    if (info.empty) { stats.pages += 1; continue; }
    const sorted = info.lines.slice().sort((a, b) => b.y - a.y);
    for (let idx = 0; idx < sorted.length; idx += 1) {
      const line = sorted[idx];
      stats.lines += 1;
      if (!hasLetters(line.text)) { stats.skipped += 1; continue; }
      let text = translated.get(line.text);
      if (!text) { text = line.text; stats.kept += 1; } else stats.translated += 1;
      // Verfuegbarer Platz reicht bis zum Satzspiegel der Seite, nicht nur bis
      // zum Ende der laengsten Zeile: sonst schrumpfte schon ein Titel, dessen
      // Uebersetzung ein paar Zeichen laenger ist.
      const margin = info.margin;
      const pageW = page.getWidth();
      const center = line.x + line.width / 2;
      const avail = Math.max(line.width, line.align === 'right'
        ? line.right - margin
        : line.align === 'center'
          ? 2 * Math.min(center - margin, pageW - margin - center)
          : pageW - margin - line.x);
      const size0 = line.size;
      let size = size0;
      const font0 = line.fonts[0];
      let font = null;
      try { font = await resolveFont(font0, text); } catch { font = null; }
      if (!font) { stats.skipped += 1; continue; }
      fontNames.add(String(font0.name || '') + '|' + font0.family + (font0.bold ? '|b' : '') + (font0.italic ? '|i' : ''));
      // Naechste Grundlinie darunter begrenzt den Platz fuer Umbruchzeilen.
      const below = sorted.slice(idx + 1).find(l => l.y < line.y - 0.5);
      const gapBelow = below ? line.y - below.y : Math.max(24, line.y - 24);
      let rows = [text];
      if (font.widthOfTextAtSize(text, size) > avail) {
        const room = Math.max(1, Math.floor(gapBelow / (size * 1.16)));
        const wrapped = wrapWords(text, font, size, avail);
        if (wrapped.length > 1 && wrapped.length <= Math.min(3, room)) { rows = wrapped; stats.wrapped += 1; }
        else {
          // Lieber etwas kleiner als ueber den Rand: harte Untergrenze 82 %.
          while (font.widthOfTextAtSize(text, size) > avail && size > size0 * MIN_SHRINK) size -= 0.25;
          stats.shrunk += 1;
        }
      }
      const useSize = size;
      for (let ri = 0; ri < rows.length; ri += 1) {
        const row = rows[ri];
        const w = font.widthOfTextAtSize(row, useSize);
        const y = line.y - ri * useSize * 1.16;
        const x = line.align === 'right' ? line.right - w : line.align === 'center' ? (line.x + line.width / 2) - w / 2 : line.x;
        const multi = line.items.length > 1 && rows.length === 1;
        if (multi) {
          const parts = splitByWeights(row, line.items.map(it => String(it.str || '').length));
          let cx = x;
          for (let k = 0; k < parts.length; k += 1) {
            if (!parts[k]) continue;
            let f = null;
            try { f = await resolveFont(line.fonts[k], parts[k]); } catch { f = font; }
            if (!f) f = font;
            drawOne(page, f, parts[k], cx, y, useSize, line.colors[k] || [0, 0, 0]);
            cx += f.widthOfTextAtSize(parts[k] + ' ', useSize);
          }
        } else {
          drawOne(page, font, row, x, y, useSize, line.colors[0] || [0, 0, 0]);
        }
      }
      stats.drawn += 1;
    }
    stats.pages += 1;
  }

  saveCache(opt);
  const out = await doc.save();
  stats.fonts = [...fontNames];
  return { buffer: Buffer.from(out), stats };
}

// ---------- Datei ----------
function outputPath(file, tgt) {
  const dir = path.dirname(file);
  const ext = path.extname(file);
  const base = path.basename(file, ext);
  let out = path.join(dir, base + '.' + String(tgt || 'xx') + ext);
  let n = 2;
  while (fs.existsSync(out)) { out = path.join(dir, base + '.' + String(tgt || 'xx') + ' (' + n + ')' + ext); n += 1; }
  return out;
}

async function translateFile(file, opt) {
  const ext = path.extname(file).toLowerCase();
  // Endung zuerst pruefen: eine unbekannte Datei soll "nicht unterstuetzt"
  // melden, nicht "Datei nicht gefunden", auch wenn sie nicht existiert.
  if (ext !== '.docx' && ext !== '.pdf') throw fail('unsupported', 'Nur .docx und .pdf');
  const buf = fs.readFileSync(file);
  const res = ext === '.docx' ? await translateDocxBuffer(buf, opt) : await translatePdfBuffer(buf, opt);
  const out = outputPath(file, baseLang(opt.tgt) || 'xx');
  fs.writeFileSync(out, res.buffer);
  return { ok: true, file, out, stats: res.stats, bytes: res.buffer.length };
}

module.exports = {
  translateFile, translateDocxBuffer, translatePdfBuffer, outputPath,
  baseLang, parseColor, setCacheFile, _localTranslate: localTranslate,
  // Fuer die Pruefungen sichtbar: Inhalt eines Seitenstroms ohne Textoperatoren.
  _stripTextOps: stripTextOps, _tokenizePdf: tokenizePdf
};
