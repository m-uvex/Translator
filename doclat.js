'use strict';
// Dokumentuebersetzung ueber DeepL, ohne die Gestalt der Datei zu verlieren.
//
// Word: Die Datei wird als ZIP gelesen, in word/document.xml (und Kopf-/Fusszeilen)
// wird ausschliesslich der Inhalt von <w:t> ersetzt. Alles andere - Schriftart,
// Groesse, Farbe, Ausrichtung, Absaetze, Nummerierung, Tabellen, Hyperlinks,
// Bilder, Kopf- und Fusszeilen - bleibt Byte fuer Byte stehen. Die Absaetze
// gehen als Ganzes an DeepL, die einzelnen Textstuecke (runs) sind mit
// <x id="n"> markiert; damit bleibt der Satz zusammenhaengend und die
// Formatierung trotzdem an der richtigen Stelle.
//
// PDF: Die Textzeilen werden mit ihren Positionen gelesen, uebersetzt und an
// dieselbe Stelle geschrieben (weisse Flaeche darunter, gleiche Groesse,
// automatisch verkleinert, wenn die Uebersetzung laenger ist). Das erhaelt
// Layout und Seiten, aber nur bei PDFs mit Text - ein Scan hat keinen Text.
// Grenzen stehen in Doku.txt §225.

const fs = require('fs');
const path = require('path');
const os = require('os');
const JSZip = require('jszip');

const FREE = 'https://api-free.deepl.com/v2';
const PRO = 'https://api.deepl.com/v2';

// DeepL will Sprachcodes gross und mit Region, wo es sie gibt.
const LANG = {
  de: 'DE', en: 'EN-US', es: 'ES', fr: 'FR', it: 'IT', nl: 'NL', pt: 'PT-PT',
  pl: 'PL', ru: 'RU', tr: 'TR', ar: 'AR', ja: 'JA', zh: 'ZH', ko: 'KO', cs: 'CS',
  da: 'DA', fi: 'FI', el: 'EL', hu: 'HU', id: 'ID', nb: 'NB', ro: 'RO', sk: 'SK',
  sl: 'SL', sv: 'SV', uk: 'UK', bg: 'BG', et: 'ET', lt: 'LT', lv: 'LV'
};
const langCode = l => LANG[String(l || '').toLowerCase()] || String(l || '').toUpperCase();

const xmlEsc = s => String(s).replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
const xmlUnesc = s => String(s).replace(/&lt;/g, '<').replace(/&gt;/g, '>').replace(/&quot;/g, '"').replace(/&apos;/g, "'").replace(/&#(\d+);/g, (_, d) => String.fromCodePoint(+d)).replace(/&#x([0-9a-f]+);/gi, (_, h) => String.fromCodePoint(parseInt(h, 16))).replace(/&amp;/g, '&');
const hasLetters = s => /[\p{L}]/u.test(String(s || ''));

function endpointFor(key, override) {
  if (override) return String(override).replace(/\/$/, '');
  return /:fx\s*$/.test(String(key || '').trim()) ? FREE : PRO;
}

// ---------- DeepL ----------
async function deepl(texts, opt) {
  const key = String(opt.key || '').trim();
  if (!key) { const e = new Error('DeepL-Schluessel fehlt'); e.code = 'noKey'; throw e; }
  const body = new URLSearchParams();
  for (const t of texts) body.append('text', t);
  if (opt.src && opt.src !== 'auto') body.set('source_lang', langCode(opt.src));
  body.set('target_lang', langCode(opt.tgt));
  // XML-Marken nur fuer Word-Absaetze. PDF-Zeilen sind reiner Text; dort waeren
  // spitze Klammern aus dem Dokument sonst als Markup gelesen worden.
  if (opt.tags !== false) body.set('tag_handling', 'xml');
  body.set('preserve_formatting', '1');
  if (opt.splitSentences === false) body.set('split_sentences', '0');
  const url = endpointFor(key, opt.endpoint) + '/translate';
  let res;
  try {
    res = await fetch(url, { method: 'POST', headers: { authorization: 'DeepL-Auth-Key ' + key, 'content-type': 'application/x-www-form-urlencoded' }, body });
  } catch (e) {
    const err = new Error('DeepL nicht erreichbar: ' + (e && e.message || e));
    err.code = 'network';
    throw err;
  }
  if (!res.ok) {
    const map = { 403: 'DeepL-Schluessel abgelehnt (403)', 456: 'DeepL-Monatslimit erreicht (456)', 429: 'DeepL: zu viele Anfragen (429)', 413: 'Dokument zu gross fuer DeepL (413)' };
    const err = new Error(map[res.status] || 'DeepL antwortet mit ' + res.status);
    err.code = 'deepl' + res.status;
    throw err;
  }
  const data = await res.json();
  return (data.translations || []).map(t => t.text);
}

// ---------- Word ----------
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
      const s = spans[i];
      if (s.start < start && start < s.end && s.start > best) { best = s.start; owner = i; }
    }
    out.push({ start, attrs: m[1] || '', innerStart, innerEnd, text: xmlUnesc(m[2]), owner });
  }
  return out;
}

const DEEPL_CHUNK = 24;
const DEEPL_CHARS = 90000;

// Uebersetzt beliebig viele Stuecke in Bloecken; doppelte Texte nur einmal.
async function translateFlat(texts, opt) {
  const uniq = [...new Set(texts)];
  const map = new Map();
  for (let i = 0; i < uniq.length; i += 50) {
    const part = uniq.slice(i, i + 50);
    const out = await deepl(part, opt);
    part.forEach((t, k) => map.set(t, out[k] == null ? t : out[k]));
  }
  return texts.map(t => map.get(t) ?? t);
}

// Ein Absatz geht als Ganzes mit Run-Marken an DeepL. Kommen die Marken nicht
// zurueck (anderer Dienst, Fehler), werden die Stuecke einzeln uebersetzt.
async function translateParagraph(items, opt) {
  const idx = [];
  items.forEach((t, i) => { if (hasLetters(t)) idx.push(i); });
  if (!idx.length) return { texts: items.slice(), requests: 0, chars: 0 };
  const payload = items.map((t, i) => (idx.includes(i) ? `<x id="${i}">${xmlEsc(t)}</x>` : '')).join('');
  // Ein einzelner Absatz kann laenger sein als eine DeepL-Anfrage erlaubt.
  // Dann werden die Stuecke einzeln uebersetzt; der Absatz verliert dabei den
  // Zusammenhang, aber es bricht nichts ab.
  if (payload.length > 30000) {
    const flat = await translateFlat(idx.map(i => items[i]), opt);
    return { texts: items.map((t, i) => (idx.includes(i) ? flat[idx.indexOf(i)] : t)), requests: Math.ceil(new Set(idx.map(i => items[i])).size / 50) || 1, chars: payload.length };
  }
  let translated = null, requests = 0;
  try {
    const out = await deepl([payload], opt);
    requests += 1;
    translated = out[0] || '';
  } catch (e) { if (e.code === 'noKey' || e.code === 'network') throw e; }
  const byId = new Map();
  if (translated) {
    const re = /<x id="(\d+)">([\s\S]*?)<\/x>/g;
    let m;
    while ((m = re.exec(translated))) byId.set(Number(m[1]), xmlUnesc(m[2]));
  }
  const missing = idx.filter(i => !byId.has(i));
  if (missing.length) {
    const flat = await translateFlat(missing.map(i => items[i]), opt);
    requests += Math.ceil(new Set(missing.map(i => items[i])).size / 50) || 1;
    missing.forEach((i, k) => byId.set(i, flat[k]));
  }
  return { texts: items.map((t, i) => (byId.has(i) ? byId.get(i) : t)), requests, chars: payload.length };
}

async function translateDocxBuffer(buf, opt) {
  const zip = await JSZip.loadAsync(buf);
  const names = Object.keys(zip.files).filter(n => !zip.files[n].dir);
  const parts = names.filter(n => /^word\/(document|header\d*|footer\d*|footnotes|endnotes)\.xml$/.test(n));
  if (!parts.length) { const e = new Error('Kein Word-Text gefunden'); e.code = 'docx'; throw e; }
  const stats = { parts: parts.length, paragraphs: 0, segments: 0, requests: 0, chars: 0, unchanged: 0 };
  for (const name of parts) {
    const xml = await zip.file(name).async('string');
    const spans = paraSpans(xml);
    const els = textElements(xml, spans);
    if (!els.length) continue;
    const groups = new Map();
    for (const el of els) {
      if (el.owner < 0) continue;
      if (!groups.has(el.owner)) groups.set(el.owner, []);
      groups.get(el.owner).push(el);
    }
    const edits = [];
    for (const list of groups.values()) {
      const r = await translateParagraph(list.map(e => e.text), opt);
      stats.paragraphs += 1;
      stats.requests += r.requests;
      stats.chars += r.chars;
      list.forEach((el, i) => {
        stats.segments += 1;
        if (r.texts[i] === el.text) { stats.unchanged += 1; return; }
        edits.push({ el, text: r.texts[i] });
      });
    }
    edits.sort((a, b) => b.el.innerStart - a.el.innerStart);
    let out = xml;
    for (const { el, text } of edits) {
      const esc = xmlEsc(text);
      out = out.slice(0, el.innerStart) + esc + out.slice(el.innerEnd);
      // Fuehrende/abschliessende Leerzeichen brauchen xml:space, sonst entfernt
      // Word sie beim naechsten Oeffnen.
      if (/^\s|\s$/.test(text) && !/xml:space/.test(el.attrs)) {
        const tagStart = out.lastIndexOf('<w:t', el.innerStart);
        const tagEnd = out.indexOf('>', tagStart);
        out = out.slice(0, tagEnd) + ' xml:space="preserve"' + out.slice(tagEnd);
      }
    }
    if (out !== xml) zip.file(name, out);
  }
  const done = await zip.generateAsync({ type: 'nodebuffer', compression: 'DEFLATE', compressionOptions: { level: 6 } });
  return { buffer: done, stats };
}

// ---------- PDF ----------
const PDF_MIN_SHRINK = 0.55;

function groupLines(items) {
  const rows = new Map();
  for (const it of items) {
    if (!String(it.str || '').trim()) continue;
    const y = it.transform[5];
    const key = Math.round(y / 2.5);
    if (!rows.has(key)) rows.set(key, []);
    rows.get(key).push(it);
  }
  const lines = [];
  for (const list of rows.values()) {
    list.sort((a, b) => a.transform[4] - b.transform[4]);
    const first = list[0];
    const last = list[list.length - 1];
    const h = Math.max(1, first.height || Math.hypot(first.transform[2], first.transform[3]) || 10);
    let text = '';
    let prevEnd = null;
    for (const it of list) {
      const x = it.transform[4];
      if (prevEnd != null && x - prevEnd > h * 0.22) text += ' ';
      text += it.str;
      prevEnd = x + it.width;
    }
    lines.push({ y: first.transform[5], x: first.transform[4], height: h, width: (last.transform[4] + last.width) - first.transform[4], text: text.trim(), items: list });
  }
  lines.sort((a, b) => (b.y - a.y) || (a.x - b.x));
  return lines;
}

async function loadPdfjs() {
  const mod = await import('pdfjs-dist/legacy/build/pdf.mjs');
  return mod;
}

// pdf.js liefert die Fuellfarbe je nach Fassung als Hex-Zeichenkette
// ("#1f4f78"), als Feld [r,g,b] oder als Float32Array. Alle drei Formen
// werden hier auf 0..255 gebracht; alles Unbekannte ergibt null (schwarz).
function parseColor(args) {
  if (!args) return null;
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
    // Manche Fassungen liefern 0..1 statt 0..255.
    const scale = v.every(x => x <= 1.0001) ? 255 : 1;
    return v.map(x => Math.round(x * scale));
  }
  return null;
}

async function pageColors(page, pdfjs) {
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

async function translatePdfBuffer(buf, opt) {
  const { PDFDocument, StandardFonts, rgb } = require('pdf-lib');
  const pdfjs = await loadPdfjs();
  const src = await pdfjs.getDocument({ data: new Uint8Array(buf), useSystemFonts: false, isEvalSupported: false }).promise;
  const doc = await PDFDocument.load(buf, { ignoreEncryption: true });

  // Schrift: Arial kann die Zeichen fast aller hier ueblichen Zielsprachen.
  // Fehlt sie, bleibt Helvetica (Westeuropa). Arabisch und CJK gehen nicht.
  let font = null, bold = null, fontName = 'Helvetica';
  try {
    const file = path.join(process.env.WINDIR || 'C:\\Windows', 'Fonts', 'arial.ttf');
    const fileB = path.join(process.env.WINDIR || 'C:\\Windows', 'Fonts', 'arialbd.ttf');
    if (fs.existsSync(file)) {
      doc.registerFontkit(require('@pdf-lib/fontkit'));
      font = await doc.embedFont(fs.readFileSync(file), { subset: true });
      bold = fs.existsSync(fileB) ? await doc.embedFont(fs.readFileSync(fileB), { subset: true }) : font;
      fontName = 'Arial';
    }
  } catch { font = null; }
  if (!font) {
    font = await doc.embedFont(StandardFonts.Helvetica);
    bold = await doc.embedFont(StandardFonts.HelveticaBold);
  }

  const stats = { pages: 0, lines: 0, drawn: 0, skipped: 0, requests: 0, chars: 0, font: fontName };
  const pages = doc.getPages();
  const toTranslate = [];
  const perPage = [];
  for (let i = 0; i < pages.length; i += 1) {
    const page = await src.getPage(i + 1);
    const content = await page.getTextContent();
    const lines = groupLines(content.items);
    perPage.push(lines);
    for (const l of lines) if (hasLetters(l.text)) toTranslate.push(l.text);
  }
  const translated = new Map();
  if (toTranslate.length) {
    const uniq = [...new Set(toTranslate)];
    stats.requests += Math.ceil(uniq.length / 50);
    for (let i = 0; i < uniq.length; i += 50) {
      const part = uniq.slice(i, i + 50);
      const out = await deepl(part, Object.assign({}, opt, { splitSentences: false, tags: false }));
      part.forEach((t, k) => translated.set(t, out[k] == null ? t : out[k]));
      stats.chars += part.join(' ').length;
    }
  }
  for (let i = 0; i < pages.length; i += 1) {
    const page = pages[i];
    const srcPage = await src.getPage(i + 1);
    const colors = await pageColors(srcPage, pdfjs);
    const lines = perPage[i];
    stats.pages += 1;
    const allItems = lines.flatMap(l => l.items);
    const colorFor = (line) => {
      // Farbe nur uebernehmen, wenn die Reihenfolge eindeutig ist: gleich viele
      // Farbwechsel wie Textstuecke. Sonst schwarz, statt zu raten.
      if (colors.length === allItems.length) {
        const idx = allItems.indexOf(line.items[0]);
        const c = colors[idx];
        if (c) return rgb(c[0] / 255, c[1] / 255, c[2] / 255);
      }
      return rgb(0, 0, 0);
    };
    for (const line of lines) {
      if (!hasLetters(line.text)) { stats.skipped += 1; continue; }
      const text = translated.get(line.text);
      if (!text) { stats.skipped += 1; continue; }
      // Alten Text abdecken: nur die Flaeche der Textstuecke, nicht die Zeile.
      for (const it of line.items) {
        const h = Math.max(1, it.height || line.height);
        const x = it.transform[4];
        const y = it.transform[5];
        page.drawRectangle({ x: x - 0.6, y: y - h * 0.28, width: Math.max(1, it.width) + 1.2, height: h * 1.06, color: rgb(1, 1, 1) });
      }
      const size0 = Math.max(4, line.height || 10);
      let size = size0;
      const boldText = /bold|black|heavy/i.test(line.items[0].fontName || '') || (line.items[0].fontName || '').includes('Bold');
      const useFont = boldText ? bold : font;
      let w = useFont.widthOfTextAtSize(text, size);
      const max = Math.max(6, line.width);
      while (w > max && size > size0 * PDF_MIN_SHRINK) {
        size -= 0.25;
        w = useFont.widthOfTextAtSize(text, size);
      }
      page.drawText(text, { x: line.x, y: line.y, size, font: useFont, color: colorFor(line), maxWidth: undefined });
      stats.lines += 1;
      stats.drawn += 1;
    }
  }
  const out = await doc.save();
  return { buffer: Buffer.from(out), stats };
}

// ---------- Datei ----------
function outputPath(file, tgt) {
  const dir = path.dirname(file);
  const ext = path.extname(file);
  const base = path.basename(file, ext);
  let out = path.join(dir, base + '.' + String(tgt || 'xx') + ext);
  let n = 2;
  while (fs.existsSync(out)) { out = path.join(dir, base + '.' + String(tgt || 'xx') + ' (' + n + ').' + ext.replace('.', '') ); n += 1; }
  return out;
}

async function translateFile(file, opt) {
  const ext = path.extname(file).toLowerCase();
  // Endung zuerst pruefen: eine unbekannte Datei soll "nicht unterstuetzt"
  // melden, nicht "Datei nicht gefunden", auch wenn sie nicht existiert.
  if (ext !== '.docx' && ext !== '.pdf') { const e = new Error('Nur .docx und .pdf'); e.code = 'unsupported'; throw e; }
  const buf = fs.readFileSync(file);
  let res;
  if (ext === '.docx') res = await translateDocxBuffer(buf, opt);
  else res = await translatePdfBuffer(buf, opt);
  const out = outputPath(file, opt.tgt);
  fs.writeFileSync(out, res.buffer);
  return { ok: true, file, out, stats: res.stats, bytes: res.buffer.length };
}

module.exports = { translateFile, translateDocxBuffer, translatePdfBuffer, deepl, outputPath, langCode, endpointFor, parseColor };
