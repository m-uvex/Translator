#!/usr/bin/env node
'use strict';
// Vergleicht Original und uebersetzte Datei maschinell: Schriftarten,
// Schriftgroessen, Farben, Ausrichtungen, Absatz- und Seitenformatierung,
// Link-Ziele, Bild- und Tabellenanzahl, Seitenzahl. Der Text darf sich
// unterscheiden - alles andere nicht.
//
//   node tools/dokumentvergleich.js            (lokales Modell ueber Ollama)
//   node tools/dokumentvergleich.js --fake     (nachgebauter Uebersetzer, schnell)
//
// Ergebnisdateien liegen in testfiles/.

const fs = require('fs');
const os = require('os');
const path = require('path');
const crypto = require('crypto');
const JSZip = require('jszip');

const ROOT = path.join(__dirname, '..');
const doclat = require(path.join(ROOT, 'doclat.js'));
const { buildDocx, buildPdf } = require(path.join(__dirname, 'dokumentvorlagen.js'));

const zeilen = [];
const fehler = [];
const ok = (name, text) => zeilen.push('  ok    ' + name.padEnd(52) + (text == null ? '' : text));
const bad = (name, text) => { zeilen.push('  FEHLT ' + name.padEnd(52) + (text == null ? '' : text)); fehler.push(name); };
const sha = b => crypto.createHash('sha256').update(b).digest('hex').slice(0, 12);
const num = v => (Number.isFinite(Number(v)) ? Number(v) : 0);
const near = (a, b, tol) => Math.abs(a - b) <= tol;

// ---------- Uebersetzer ----------
// Nachbau fuer den schnellen Lauf: gleiche Schnittstelle wie das lokale Modell.
function fakeTranslate(texts) {
  return texts.map(t => t
    .replace(/<x id="(\d+)">([\s\S]*?)<\/x>/g, (m, id, inner) => `<x id="${id}">EN[${inner}]</x>`)
    .replace(/^(?!.*<x id=)([\s\S]+)$/, 'EN[$1]'));
}

async function ollamaTranslate(texts, opt) {
  const out = [];
  const base = String(opt.url || 'http://127.0.0.1:11434').replace(/\/$/, '');
  const names = langNames(opt.src, opt.tgt);
  for (const t of texts) {
    // Die Marken sind nur fuer unser Backend; das Modell bekommt reinen Text.
    const plain = t.replace(/<x id="(\d+)">([\s\S]*?)<\/x>/g, (m, id, inner) => `\u0001${id}\u0001${inner}\u0002`);
    const prompt = `You are a professional ${names.src} (${opt.src || 'auto'}) to ${names.tgt} (${opt.tgt}) translator. ` +
      `Produce only the ${names.tgt} translation, without any additional explanations or commentary. ` +
      `Keep the markers \u0001n\u0001 ... \u0002 exactly as they are and translate only the text between them. ` +
      `Please translate the following ${names.src} text into ${names.tgt}:\n\n${plain}`;
    const r = await fetch(base + '/api/chat', {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({
        model: opt.model, stream: false, think: false,
        messages: [{ role: 'user', content: prompt }],
        options: { temperature: 0.1, num_ctx: 8192, num_predict: 2048 }
      })
    });
    if (!r.ok) throw Object.assign(new Error('Ollama antwortet mit ' + r.status), { code: 'engine' });
    const j = await r.json();
    let text = String((j.message && j.message.content) || '').trim();
    text = text.replace(/^```[a-z]*\n?|```$/g, '').trim();
    // Marken zurueck in die Form, die doclat erwartet.
    text = text.replace(/\u0001(\d+)\u0001([\s\S]*?)\u0002/g, (m, id, inner) => `<x id="${id}">${inner.trim()}</x>`);
    out.push(text);
  }
  return out;
}

function langNames(src, tgt) {
  const en = { de: 'German', en: 'English', fr: 'French', es: 'Spanish', it: 'Italian', nl: 'Dutch', pl: 'Polish', pt: 'Portuguese', tr: 'Turkish', ru: 'Russian' };
  return { src: en[String(src || '').slice(0, 2)] || src || 'German', tgt: en[String(tgt || '').slice(0, 2)] || tgt || 'English' };
}

// ---------- Word: Gestalt lesen ----------
const runProps = rpr => {
  const get = re => { const m = re.exec(rpr || ''); return m ? m[1] : ''; };
  return {
    font: get(/w:rFonts[^>]*w:ascii="([^"]+)"/),
    bold: /<w:b(?:\s+w:val="(?:1|true)")?\s*\/>/.test(rpr || ''),
    italic: /<w:i(?:\s+w:val="(?:1|true)")?\s*\/>/.test(rpr || ''),
    underline: /<w:u\s+w:val="([^"]+)"/.test(rpr || '') ? get(/<w:u\s+w:val="([^"]+)"/) : '',
    color: get(/w:color\s+w:val="([^"]+)"/),
    size: get(/w:sz\s+w:val="(\d+)"/),
    style: get(/w:rStyle\s+w:val="([^"]+)"/)
  };
};
const paraProps = ppr => {
  const get = re => { const m = re.exec(ppr || ''); return m ? m[1] : ''; };
  const spacing = get(/<w:spacing([^>]*)\/>/);
  const sp = {};
  for (const k of ['before', 'after', 'line', 'lineRule']) {
    const m = new RegExp('w:' + k + '="([^"]+)"').exec(spacing || '');
    if (m) sp[k] = m[1];
  }
  return {
    style: get(/w:pStyle\s+w:val="([^"]+)"/),
    align: get(/<w:jc\s+w:val="([^"]+)"/),
    spacing: sp,
    indent: get(/<w:ind([^>]*)\/>/),
    numbering: get(/<w:numId\s+w:val="(\d+)"/),
    level: get(/<w:ilvl\s+w:val="(\d+)"/)
  };
};

function paragraphs(xml) {
  const out = [];
  const re = /<w:p(?:\s[^>]*)?>[\s\S]*?<\/w:p>/g;
  let m;
  while ((m = re.exec(xml))) {
    const block = m[0];
    // Verschachtelte Absaetze (Textfelder) gehoeren zum inneren Block; hier
    // gibt es keine, deshalb genuegt der erste pPr-Block.
    const ppr = (/<w:pPr>([\s\S]*?)<\/w:pPr>/.exec(block) || [])[1] || '';
    const runs = [];
    const rr = /<w:r>([\s\S]*?)<\/w:r>/g;
    let r;
    while ((r = rr.exec(block))) {
      const rpr = (/<w:rPr>([\s\S]*?)<\/w:rPr>/.exec(r[1]) || [])[1] || '';
      const ts = [];
      const tt = /<w:t(?:\s[^>]*)?>([\s\S]*?)<\/w:t>/g;
      let t;
      while ((t = tt.exec(r[1]))) ts.push(t[1]);
      runs.push({ text: ts.join(''), props: runProps(rpr) });
    }
    out.push({ props: paraProps(ppr), runs, hyperlink: /<w:hyperlink\s/.test(block), breakPage: /<w:br\s+w:type="page"\s*\/>/.test(block) });
  }
  return out;
}

function docxTexts(xml) {
  return [...xml.matchAll(/<w:t(?:\s[^>]*)?>([\s\S]*?)<\/w:t>/g)].map(m => m[1]);
}

async function docxProfile(buf) {
  const zip = await JSZip.loadAsync(buf);
  const names = Object.keys(zip.files).filter(n => !zip.files[n].dir).sort();
  const xml = {};
  for (const n of names) if (/^word\/(document|header\d*|footer\d*|footnotes|endnotes)\.xml$/.test(n)) xml[n] = await zip.file(n).async('string');
  const rels = await zip.file('word/_rels/document.xml.rels').async('string');
  const hyperlinks = [...rels.matchAll(/<Relationship[^>]*Type="[^"]*\/hyperlink"[^>]*Target="([^"]+)"/g)].map(m => m[1]).sort();
  const media = {};
  for (const n of names) if (/^word\/(media|embeddings)\//.test(n)) media[n] = sha(await zip.file(n).async('nodebuffer'));
  const bytes = {};
  for (const n of names) if (n !== 'word/document.xml' && !/^word\/(header|footer)\d*\.xml$/.test(n)) bytes[n] = sha(await zip.file(n).async('nodebuffer'));
  const doc = xml['word/document.xml'] || '';
  const sect = (/<w:sectPr[\s\S]*?<\/w:sectPr>/.exec(doc) || [''])[0];
  return {
    names, xml, bytes, media, hyperlinks, sect,
    tables: (doc.match(/<w:tbl>/g) || []).length,
    rows: (doc.match(/<w:tr>/g) || []).length,
    cells: (doc.match(/<w:tc>/g) || []).length,
    images: (doc.match(/<a:blip /g) || []).length,
    breaks: (doc.match(/<w:br w:type="page"\/>/g) || []).length,
    paragraphs: Object.fromEntries(Object.entries(xml).map(([n, x]) => [n, paragraphs(x)]))
  };
}

const stripText = x => String(x)
  .replace(/(<w:t(?:\s[^>]*)?>)[\s\S]*?(<\/w:t>)/g, '$1$2')
  .replace(/\s+xml:space="preserve"/g, '');

// ---------- PDF: Gestalt lesen ----------
async function pdfjsModule() { return import('pdfjs-dist/legacy/build/pdf.mjs'); }

function compactLines(items) {
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
    const flush = () => {
      if (!seg.length) return;
      const first = seg[0], last = seg[seg.length - 1];
      let text = '', prev = null;
      for (const it of seg) {
        if (prev != null && num(it.transform[4]) - prev > num(it.height) * 0.22) text += ' ';
        text += it.str;
        prev = num(it.transform[4]) + num(it.width);
      }
      const size = Math.max(...seg.map(it => num(it.height) || 10));
      lines.push({
        x: num(first.transform[4]), y: num(first.transform[5]),
        right: num(last.transform[4]) + num(last.width),
        size, text: text.trim(), fontName: first.fontName,
        color: first.color || [0, 0, 0]
      });
      seg = [];
    };
    for (const it of list) {
      if (seg.length) {
        const prev = seg[seg.length - 1];
        const gap = num(it.transform[4]) - (num(prev.transform[4]) + num(prev.width));
        if (gap > Math.max(3, num(prev.height) * 2.5)) flush();
      }
      seg.push(it);
    }
    flush();
  }
  lines.sort((a, b) => (b.y - a.y) || (a.x - b.x));
  return lines;
}

async function pdfProfile(buf) {
  const pdfjs = await pdfjsModule();
  const dir = path.join(ROOT, 'node_modules', 'pdfjs-dist', 'standard_fonts').split(path.sep).join('/');
  const std = 'file:///' + (dir.startsWith('/') ? dir.slice(1) : dir) + '/';
  const doc = await pdfjs.getDocument({ data: new Uint8Array(buf), isEvalSupported: false, verbosity: 0, standardFontDataUrl: std }).promise;
  const pages = [];
  for (let i = 1; i <= doc.numPages; i += 1) {
    const page = await doc.getPage(i);
    const vp = page.getViewport({ scale: 1 });
    const tc = await page.getTextContent();
    const ops = await page.getOperatorList();
    const OPS = pdfjs.OPS;
    let cur = [0, 0, 0];
    const cols = [];
    const images = [];
    for (let k = 0; k < ops.fnArray.length; k += 1) {
      const fn = ops.fnArray[k], args = ops.argsArray[k];
      if (fn === OPS.setFillRGBColor) { const h = String(args[0] || '').replace('#', ''); if (h.length === 6) cur = [parseInt(h.slice(0, 2), 16), parseInt(h.slice(2, 4), 16), parseInt(h.slice(4, 6), 16)]; }
      else if (fn === OPS.setFillGray) { const g = Math.round(num(args[0]) * 255); cur = [g, g, g]; }
      else if (fn === OPS.showText || fn === OPS.showSpacedText) cols.push(cur);
      else if (fn === OPS.paintImageXObject || fn === OPS.paintImageMaskXObject || fn === OPS.paintInlineImageXObject) images.push(num(args && args[0]));
    }
    const items = tc.items.filter(it => String(it.str || '').trim());
    items.forEach((it, k) => {
      it.color = cols.length === items.length ? cols[k] : (cols.length === 1 ? cols[0] : [0, 0, 0]);
      let o = null;
      try { o = page.commonObjs.get(it.fontName); } catch { o = null; }
      it.fontName = (o && o.name) || it.fontName;
      it.fontBold = !!(o && o.bold) || /bold|black|heavy/i.test(it.fontName);
      it.fontItalic = !!(o && o.italic) || /italic|oblique/i.test(it.fontName);
    });
    const anns = await page.getAnnotations();
    pages.push({
      width: vp.width, height: vp.height,
      lines: compactLines(items),
      links: anns.filter(a => String(a.subtype || '').includes('Link')).map(a => String(a.url || a.unsafeUrl || '')).sort(),
      imageOps: images.length
    });
  }
  return { pages };
}

async function pdfImageHashes(buf) {
  const { PDFDocument, PDFName, PDFDict, PDFArray, PDFRawStream } = require('pdf-lib');
  const doc = await PDFDocument.load(buf);
  const out = [];
  for (const page of doc.getPages()) {
    const res = page.node.Resources();
    if (!res) { out.push([]); continue; }
    const xo = res.lookup(PDFName.of('XObject'), PDFDict);
    if (!xo) { out.push([]); continue; }
    const list = [];
    for (const [, v] of xo.entries()) {
      const st = doc.context.lookup(v);
      if (!(st instanceof PDFRawStream)) continue;
      const sub = st.dict.lookup(PDFName.of('Subtype'));
      if (sub && String(sub.asString ? sub.asString() : sub) === '/Image') list.push(sha(Buffer.from(st.contents)));
    }
    out.push(list.sort());
  }
  return out;
}

// ---------- Vergleich ----------
const norm = s => String(s || '').toLowerCase().replace(/\s+/g, ' ').trim();

function compareDocx(a, b) {
  if (JSON.stringify(a.names) === JSON.stringify(b.names)) ok('DOCX: gleiche Teile im ZIP', a.names.length + ' Teile');
  else bad('DOCX: gleiche Teile im ZIP', a.names.join(',') + ' -> ' + b.names.join(','));

  const sameBytes = Object.keys(a.bytes).filter(n => b.bytes[n] === a.bytes[n]);
  const diffBytes = Object.keys(a.bytes).filter(n => b.bytes[n] !== a.bytes[n]);
  if (!diffBytes.length) ok('DOCX: alle Nicht-Text-Teile byte-gleich', sameBytes.length + ' Teile');
  else bad('DOCX: alle Nicht-Text-Teile byte-gleich', diffBytes.join(', '));

  if (JSON.stringify(a.media) === JSON.stringify(b.media)) ok('DOCX: Bilder unveraendert', Object.keys(a.media).length + ' Datei(en)');
  else bad('DOCX: Bilder unveraendert', JSON.stringify(a.media) + ' -> ' + JSON.stringify(b.media));

  const partsA = Object.keys(a.xml).sort();
  let struktOk = true;
  for (const n of partsA) if (stripText(a.xml[n]) !== stripText(b.xml[n] || '')) { struktOk = false; bad('DOCX: Aufbau ohne Text gleich — ' + n); }
  if (struktOk) ok('DOCX: Aufbau ohne Text gleich', partsA.length + ' Teile');

  let propsOk = true;
  for (const n of partsA) {
    const pa = a.paragraphs[n] || [], pb = b.paragraphs[n] || [];
    if (pa.length !== pb.length) { propsOk = false; bad('DOCX: gleiche Absatzanzahl — ' + n, pa.length + ' -> ' + pb.length); continue; }
    for (let i = 0; i < pa.length; i += 1) {
      if (JSON.stringify(pa[i].props) !== JSON.stringify(pb[i].props)) { propsOk = false; bad(`DOCX: Absatzformat ${n} #${i + 1}`, JSON.stringify(pa[i].props) + ' -> ' + JSON.stringify(pb[i].props)); }
      if (pa[i].runs.length !== pb[i].runs.length) { propsOk = false; bad(`DOCX: Textstueckanzahl ${n} #${i + 1}`, pa[i].runs.length + ' -> ' + pb[i].runs.length); continue; }
      for (let k = 0; k < pa[i].runs.length; k += 1) {
        if (JSON.stringify(pa[i].runs[k].props) !== JSON.stringify(pb[i].runs[k].props)) { propsOk = false; bad(`DOCX: Zeichenformat ${n} #${i + 1}.${k + 1}`, JSON.stringify(pa[i].runs[k].props) + ' -> ' + JSON.stringify(pb[i].runs[k].props)); }
        if (a.paragraphs[n][i].hyperlink !== b.paragraphs[n][i].hyperlink) { propsOk = false; bad(`DOCX: Hyperlink ${n} #${i + 1}`); }
      }
    }
  }
  if (propsOk) ok('DOCX: Schrift, Groesse, Farbe, Ausrichtung, Abstaende, Nummerierung', Object.values(a.paragraphs).flat().length + ' Absaetze');

  if (a.sect === b.sect) ok('DOCX: Seitenformat und Seitenraender gleich', (a.sect.match(/w:w="\d+"/g) || []).join(' '));
  else bad('DOCX: Seitenformat und Seitenraender gleich', a.sect.slice(0, 90) + ' -> ' + b.sect.slice(0, 90));

  for (const [name, av, bv] of [['Tabellen', a.tables, b.tables], ['Tabellenzeilen', a.rows, b.rows], ['Tabellenzellen', a.cells, b.cells], ['Bilder', a.images, b.images], ['Seitenumbrueche', a.breaks, b.breaks]]) {
    if (av === bv) ok('DOCX: ' + name + ' gleich', av);
    else bad('DOCX: ' + name + ' gleich', av + ' -> ' + bv);
  }
  if (JSON.stringify(a.hyperlinks) === JSON.stringify(b.hyperlinks)) ok('DOCX: Link-Ziele gleich', a.hyperlinks.join(', ') || 'keine');
  else bad('DOCX: Link-Ziele gleich', a.hyperlinks + ' -> ' + b.hyperlinks);

  // Text: jede Textstelle mit Buchstaben muss uebersetzt sein, Zahlen bleiben.
  let total = 0, translated = 0, kept = [];
  for (const n of partsA) {
    const ta = docxTexts(a.xml[n]), tb = docxTexts(b.xml[n]);
    for (let i = 0; i < ta.length; i += 1) {
      if (!/\p{L}/u.test(ta[i])) continue;
      total += 1;
      if (tb[i] != null && tb[i] !== ta[i] && tb[i].trim()) translated += 1;
      else kept.push(ta[i].slice(0, 30));
    }
  }
  if (total && translated === total) ok('DOCX: alle Textstellen uebersetzt', translated + ' von ' + total);
  else bad('DOCX: alle Textstellen uebersetzt', translated + ' von ' + total + (kept.length ? ' | stehen geblieben: ' + kept.slice(0, 4).join(' / ') : ''));
  return { total, translated };
}

function comparePdf(a, b) {
  if (a.pages.length === b.pages.length) ok('PDF: gleiche Seitenzahl', b.pages.length);
  else bad('PDF: gleiche Seitenzahl', a.pages.length + ' -> ' + b.pages.length);

  let sizeOk = a.pages.length === b.pages.length;
  for (let i = 0; i < Math.min(a.pages.length, b.pages.length); i += 1) {
    if (!near(a.pages[i].width, b.pages[i].width, 0.6) || !near(a.pages[i].height, b.pages[i].height, 0.6)) sizeOk = false;
  }
  if (sizeOk) ok('PDF: gleiche Seitengroesse', b.pages.map(p => Math.round(p.width) + 'x' + Math.round(p.height)).join(', '));
  else bad('PDF: gleiche Seitengroesse');

  let matched = 0, totalLines = 0, styleBad = 0, notTranslated = [];
  for (let i = 0; i < Math.min(a.pages.length, b.pages.length); i += 1) {
    for (const la of a.pages[i].lines) {
      if (!/\p{L}/u.test(la.text)) continue;
      totalLines += 1;
      const mid = (la.x + la.right) / 2;
      const cand = b.pages[i].lines.filter(lb => near(lb.y, la.y, 0.9) &&
        (near(lb.x, la.x, 0.9) || near(lb.right, la.right, 0.9) || near((lb.x + lb.right) / 2, mid, 0.9) ||
         (lb.x <= la.x + 1 && lb.right >= la.right - 6)));
      if (!cand.length) { styleBad += 1; continue; }
      const lb = cand[0];
      const styleSame = near(lb.size, la.size, 0.2) &&
        lb.fontName === la.fontName && lb.fontBold === la.fontBold && lb.fontItalic === la.fontItalic &&
        near(lb.color[0], la.color[0], 3) && near(lb.color[1], la.color[1], 3) && near(lb.color[2], la.color[2], 3);
      if (!styleSame) {
        styleBad += 1;
        if (styleBad <= 4) bad('PDF: Schrift/Groesse/Farbe an "' + la.text.slice(0, 24) + '"',
          `${la.fontName} ${la.size} ${JSON.stringify(la.color)} -> ${lb.fontName} ${lb.size} ${JSON.stringify(lb.color)}`);
        continue;
      }
      if (norm(lb.text) === norm(la.text)) { notTranslated.push(la.text.slice(0, 28)); continue; }
      matched += 1;
    }
  }
  if (!styleBad) ok('PDF: Schrift, Groesse, Farbe und Lage aller Zeilen gleich', matched + ' Zeilen');
  else bad('PDF: Schrift, Groesse, Farbe und Lage aller Zeilen gleich', styleBad + ' Abweichung(en)');
  if (totalLines && matched === totalLines) ok('PDF: alle Zeilen uebersetzt', matched + ' von ' + totalLines);
  else bad('PDF: alle Zeilen uebersetzt', matched + ' von ' + totalLines + (notTranslated.length ? ' | unveraendert: ' + notTranslated.slice(0, 3).join(' / ') : ''));

  const linksA = JSON.stringify(a.pages.map(p => p.links));
  const linksB = JSON.stringify(b.pages.map(p => p.links));
  if (linksA === linksB) ok('PDF: Link-Ziele gleich', a.pages.flatMap(p => p.links).join(', ') || 'keine');
  else bad('PDF: Link-Ziele gleich', linksA + ' -> ' + linksB);
  return { matched, totalLines };
}

(async () => {
  const args = process.argv.slice(2);
  const fake = args.includes('--fake');
  const model = (args.find(a => a.startsWith('--model=')) || '').split('=')[1] || process.env.DOCLAT_MODEL || 'translategemma:4b';
  const url = process.env.OLLAMA_URL || 'http://127.0.0.1:11434';
  const dir = path.join(ROOT, 'testfiles');
  fs.mkdirSync(dir, { recursive: true });

  const srcDocx = await buildDocx();
  const srcPdf = await buildPdf();
  fs.writeFileSync(path.join(dir, 'original.docx'), srcDocx);
  fs.writeFileSync(path.join(dir, 'original.pdf'), srcPdf);

  const base = fake
    ? { src: 'de', tgt: 'en', cache: false, translate: fakeTranslate }
    : { src: 'de', tgt: 'en', cache: false, engine: 'ollama', model, url };
  console.log('Uebersetzer: ' + (fake ? 'Nachbau (--fake)' : 'lokales Modell ' + model + ' @ ' + url));
  console.log('');

  try {
    const t0 = Date.now();
    const docx = await doclat.translateDocxBuffer(srcDocx, Object.assign({}, base, { onProgress: p => process.stdout.write(`\rdocx ${p.percent || 0} %   `) }));
    console.log('');
    const t1 = Date.now();
    const pdf = await doclat.translatePdfBuffer(srcPdf, Object.assign({}, base, { onProgress: p => process.stdout.write(`\rpdf  ${p.percent || 0} %   `) }));
    console.log('');

    const outDocx = path.join(dir, 'ergebnis.en.docx');
    const outPdf = path.join(dir, 'ergebnis.en.pdf');
    fs.writeFileSync(outDocx, docx.buffer);
    fs.writeFileSync(outPdf, pdf.buffer);

    console.log(`DOCX: ${docx.buffer.length} Bytes, ${JSON.stringify(docx.stats)}`);
    console.log(`PDF:  ${pdf.buffer.length} Bytes, ${JSON.stringify(pdf.stats)}`);
    console.log(`Dauer: ${Math.round((t1 - t0) / 1000)} s DOCX, ${Math.round((Date.now() - t1) / 1000)} s PDF`);
    console.log('');

    const A = await docxProfile(srcDocx);
    const B = await docxProfile(docx.buffer);
    compareDocx(A, B);

    const PA = await pdfProfile(srcPdf);
    const PB = await pdfProfile(pdf.buffer);
    comparePdf(PA, PB);
    const IA = await pdfImageHashes(srcPdf);
    const IB = await pdfImageHashes(pdf.buffer);
    if (JSON.stringify(IA) === JSON.stringify(IB)) ok('PDF: Bilder unveraendert', IA.flat().length + ' Bild(er)');
    else bad('PDF: Bilder unveraendert', JSON.stringify(IA) + ' -> ' + JSON.stringify(IB));

    // Gegenprobe: die deutschen Woerter duerfen nicht mehr im Ergebnis stehen.
    const wordDocx = Object.values(B.xml).join(' ');
    const wordPdf = PB.pages.map(p => p.lines.map(l => l.text).join(' ')).join(' ');
    const leftovers = ['Quartalsbericht', 'Umsatz', 'Kosten', 'Anhang', 'Website'].filter(w => (wordDocx + ' ' + wordPdf).includes(w));
    if (fake) ok('Gegenprobe entfaellt beim Nachbau (EN[...]-Praefix)', '');
    else if (!leftovers.length) ok('Gegenprobe: keine deutschen Woerter mehr');
    else if (!fake) bad('Gegenprobe: keine deutschen Woerter mehr', leftovers.join(', '));
  } catch (e) {
    bad('Lauf', (e && e.stack || e).toString().split('\n').slice(0, 4).join(' | '));
  }

  console.log(zeilen.join('\n'));
  console.log('');
  console.log(fehler.length ? 'FEHLGESCHLAGEN (' + fehler.length + '): ' + fehler.join(' | ') : 'Vergleich bestanden (' + zeilen.length + ' Pruefungen).');
  process.exit(fehler.length ? 1 : 0);
})();
