#!/usr/bin/env node
'use strict';
// Prueft die Dokumentuebersetzung ohne echten DeepL-Schluessel: ein kleiner
// Server auf 127.0.0.1 antwortet wie DeepL (Translatio-nen in derselben
// Reihenfolge, XML-Marken bleiben stehen) und praefixiert jeden Text mit "EN[".
// Damit laesst sich beweisen, dass nur Textinhalte getauscht werden und alles
// andere - Schrift, Farbe, Ausrichtung, Hyperlink, Tabelle, Bild, Kopfzeile -
// Byte fuer Byte gleich bleibt.
//
//   node tools/dokumenttest.js
//
// Grenze: Die echte DeepL-Antwort ist damit nicht geprueft (kein Schluessel
// vorhanden). Geprueft ist die eigene Verarbeitung: Anfrageformat, Marken,
// Rueckverteilung, ZIP-Aufbau, PDF-Positionen.

const http = require('http');
const fs = require('fs');
const os = require('os');
const path = require('path');
const crypto = require('crypto');
const JSZip = require('jszip');
const mammoth = require('mammoth');

const ROOT = path.join(__dirname, '..');
const doclat = require(path.join(ROOT, 'doclat.js'));

const zeilen = [];
const fehler = [];
const ok = (name, text) => zeilen.push('  ok    ' + name.padEnd(46) + (text == null ? '' : text));
const bad = (name, text) => { zeilen.push('  FEHLT ' + name.padEnd(46) + (text == null ? '' : text)); fehler.push(name); };
const near = (a, b, tol) => Math.abs(a - b) <= tol;

// ---------- Nachgebautes DeepL ----------
const anfragen = [];
function mockServer() {
  return new Promise(resolve => {
    const srv = http.createServer((req, res) => {
      let body = '';
      req.on('data', c => { body += c; });
      req.on('end', () => {
        const p = new URLSearchParams(body);
        const texts = p.getAll('text');
        const tgt = p.get('target_lang');
        anfragen.push({ texts: texts.length, tgt, tag: p.get('tag_handling'), chars: body.length });
        // Wie DeepL: Marken bleiben stehen, nur der Inhalt dazwischen wird
        // uebersetzt. Ohne Marken wird der ganze Text uebersetzt.
        const out = texts.map(t => {
          if (/<x id="\d+">/.test(t)) return t.replace(/<x id="(\d+)">([\s\S]*?)<\/x>/g, (m, id, inner) => '<x id="' + id + '">' + tgt.slice(0, 2) + '[' + inner + ']</x>');
          return tgt.slice(0, 2) + '[' + t + ']';
        });
        const payload = JSON.stringify({ translations: out.map((text, i) => ({ detected_source_language: 'DE', text })) });
        res.writeHead(200, { 'content-type': 'application/json' });
        res.end(payload);
      });
    });
    srv.listen(0, '127.0.0.1', () => resolve({ srv, port: srv.address().port }));
  });
}

// ---------- DOCX-Beispiel ----------
const NS = 'xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:wps="http://schemas.microsoft.com/office/word/2010/wordprocessingShape"';
const PNG = Buffer.from('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==', 'base64');

function docXml() {
  return '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>\n' +
    '<w:document ' + NS + '><w:body>' +
    '<w:p><w:pPr><w:pStyle w:val="Heading1"/><w:jc w:val="center"/></w:pPr><w:r><w:rPr><w:rFonts w:ascii="Georgia" w:hAnsi="Georgia"/><w:b/><w:color w:val="1F4E79"/><w:sz w:val="32"/><w:szCs w:val="32"/></w:rPr><w:t>Quartalsbericht</w:t></w:r></w:p>' +
    '<w:p><w:r><w:t xml:space="preserve">Der Umsatz ist </w:t></w:r><w:r><w:rPr><w:b/></w:rPr><w:t>gestiegen</w:t></w:r><w:r><w:t>, die Kosten sanken.</w:t></w:r></w:p>' +
    '<w:p><w:pPr><w:numPr><w:ilvl w:val="0"/><w:numId w:val="1"/></w:numPr></w:pPr><w:r><w:rPr><w:i/></w:rPr><w:t>Erster Punkt</w:t></w:r></w:p>' +
    '<w:p><w:hyperlink r:id="rId5"><w:r><w:rPr><w:u w:val="single"/><w:color w:val="0563C1"/></w:rPr><w:t>Website</w:t></w:r></w:hyperlink></w:p>' +
    '<w:tbl><w:tr><w:tc><w:p><w:r><w:t>Umsatz</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>2026</w:t></w:r></w:p></w:tc></w:tr></w:tbl>' +
    '<w:p><w:r><w:drawing><wp:inline><a:graphic><a:graphicData><wps:wsp><wps:txbx><w:txbxContent><w:p><w:r><w:t>Im Kasten</w:t></w:r></w:p></w:txbxContent></wps:txbx></wps:wsp></a:graphicData></a:graphic></wp:inline></w:drawing></w:r></w:p>' +
    '</w:body></w:document>';
}

async function buildDocx() {
  const zip = new JSZip();
  zip.file('[Content_Types].xml', '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>\n<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Default Extension="png" ContentType="image/png"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/header1.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.header+xml"/><Override PartName="/word/footer1.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.footer+xml"/></Types>');
  zip.file('_rels/.rels', '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>\n<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>');
  zip.file('word/document.xml', docXml());
  zip.file('word/header1.xml', '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>\n<w:hdr ' + NS + '><w:p><w:r><w:rPr><w:color w:val="7F7F7F"/><w:sz w:val="18"/></w:rPr><w:t>Vertraulich</w:t></w:r></w:p></w:hdr>');
  zip.file('word/footer1.xml', '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>\n<w:ftr ' + NS + '><w:p><w:pPr><w:jc w:val="right"/></w:pPr><w:r><w:t>Seite 1</w:t></w:r></w:p></w:ftr>');
  zip.file('word/styles.xml', '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>\n<w:styles ' + NS + '><w:style w:type="paragraph" w:styleId="Heading1"><w:name w:val="heading 1"/><w:rPr><w:b/><w:sz w:val="32"/></w:rPr></w:style></w:styles>');
  zip.file('word/_rels/document.xml.rels', '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>\n<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId5" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink" Target="https://example.com" TargetMode="External"/><Relationship Id="rId6" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="media/image1.png"/></Relationships>');
  zip.file('word/media/image1.png', PNG);
  return zip.generateAsync({ type: 'nodebuffer' });
}

const textOnly = xml => String(xml).replace(/(<w:t(\s[^>]*)?>)[\s\S]*?(<\/w:t>)/g, '$1§$3').replace(/\s+xml:space="preserve"/g, '');
const sha = b => crypto.createHash('sha256').update(b).digest('hex');

async function testDocx(port) {
  const src = await buildDocx();
  const before = await JSZip.loadAsync(src);
  const { buffer, stats } = await doclat.translateDocxBuffer(src, { key: 'test:fx', src: 'de', tgt: 'en', endpoint: 'http://127.0.0.1:' + port });
  const after = await JSZip.loadAsync(buffer);
  const namesBefore = Object.keys(before.files).filter(n => !before.files[n].dir).sort();
  const namesAfter = Object.keys(after.files).filter(n => !after.files[n].dir).sort();
  if (JSON.stringify(namesBefore) === JSON.stringify(namesAfter)) ok('DOCX: gleiche Teile im ZIP', namesAfter.length + ' Teile');
  else bad('DOCX: gleiche Teile im ZIP', JSON.stringify({ namesBefore, namesAfter }));

  const docBefore = await before.file('word/document.xml').async('string');
  const docAfter = await after.file('word/document.xml').async('string');
  if (textOnly(docBefore) === textOnly(docAfter)) ok('DOCX: Aufbau ohne Text unveraendert');
  else bad('DOCX: Aufbau ohne Text unveraendert', 'XML unterscheidet sich ausserhalb der Textstellen');

  for (const teil of ['word/styles.xml', 'word/_rels/document.xml.rels', '[Content_Types].xml', '_rels/.rels', 'word/media/image1.png']) {
    const a = await before.file(teil).async('nodebuffer');
    const b = await after.file(teil).async('nodebuffer');
    if (sha(a) === sha(b)) ok('DOCX: ' + teil + ' byte-gleich');
    else bad('DOCX: ' + teil + ' byte-gleich', 'Inhalt geaendert');
  }
  for (const [was, muss] of [
    ['w:ascii="Georgia"', 'Schrift Georgia'],
    ['w:color w:val="1F4E79"', 'Farbe 1F4E79'],
    ['w:sz w:val="32"', 'Groesse 32 halbe Punkte'],
    ['w:jc w:val="center"', 'Ausrichtung zentriert'],
    ['<w:numPr>', 'Nummerierung'],
    ['r:id="rId5"', 'Hyperlink-Verweis'],
    ['<w:tbl>', 'Tabelle'],
    ['<w:drawing>', 'Zeichnung/Textfeld']
  ]) {
    if (docAfter.includes(was)) ok('DOCX: erhalten - ' + muss);
    else bad('DOCX: erhalten - ' + muss, was + ' fehlt');
  }

  const tn = docAfter.match(/<w:t(\s[^>]*)?>([\s\S]*?)<\/w:t>/g) || [];
  const uebersetzt = tn.filter(t => /EN\[/.test(t)).length;
  if (uebersetzt >= 7) ok('DOCX: Textstellen uebersetzt', uebersetzt + ' von ' + tn.length);
  else bad('DOCX: Textstellen uebersetzt', uebersetzt + ' von ' + tn.length);

  if (/<w:t[^>]*>2026<\/w:t>/.test(docAfter)) ok('DOCX: Zahl ohne Buchstaben bleibt stehen');
  else bad('DOCX: Zahl ohne Buchstaben bleibt stehen');

  const header = await after.file('word/header1.xml').async('string');
  if (/EN\[Vertraulich\]/.test(header) && /w:color w:val="7F7F7F"/.test(header)) ok('DOCX: Kopfzeile uebersetzt, Farbe bleibt');
  else bad('DOCX: Kopfzeile uebersetzt, Farbe bleibt', header.slice(0, 160));

  const footer = await after.file('word/footer1.xml').async('string');
  if (/EN\[Seite 1\]/.test(footer)) ok('DOCX: Fusszeile uebersetzt');
  else bad('DOCX: Fusszeile uebersetzt', footer.slice(0, 160));

  const kasten = /<w:txbxContent><w:p><w:r><w:t[^>]*>([^<]*)<\/w:t>/.exec(docAfter);
  if (kasten && /EN\[Im Kasten\]/.test(kasten[1])) ok('DOCX: Text im Textfeld uebersetzt');
  else bad('DOCX: Text im Textfeld uebersetzt', kasten ? kasten[1] : 'nicht gefunden');

  // Zeilenumbruch im Absatz: drei Stuecke muessen einzeln uebersetzt sein.
  const absatz = /<w:t[^>]*>EN\[Der Umsatz ist \]<\/w:t>/.test(docAfter);
  if (absatz) ok('DOCX: Absatzstuecke einzeln verteilt');
  else bad('DOCX: Absatzstuecke einzeln verteilt', docAfter.slice(docAfter.indexOf('Umsatz') - 200, docAfter.indexOf('Umsatz') + 200));

  const plain = (await mammoth.extractRawText({ buffer })).value;
  if (plain.includes('EN[Quartalsbericht]') && plain.includes('EN[Website]')) ok('DOCX: Datei bleibt lesbar (mammoth)', plain.replace(/\s+/g, ' ').slice(0, 60));
  else bad('DOCX: Datei bleibt lesbar (mammoth)', plain.slice(0, 120));

  const mitMarken = anfragen.filter(a => a.tag === 'xml').length;
  if (mitMarken >= 1) ok('DOCX: Anfrage mit XML-Marken', mitMarken + ' Anfrage(n)');
  else bad('DOCX: Anfrage mit XML-Marken');

  // Datei-Ebene: Ergebnis liegt neben dem Original, ein zweiter Lauf
  // ueberschreibt nichts, sondern zaehlt hoch.
  const ordner = fs.mkdtempSync(path.join(os.tmpdir(), 'doclat-out-'));
  const quelle = path.join(ordner, 'probe.docx');
  fs.writeFileSync(quelle, src);
  const r1 = await doclat.translateFile(quelle, { key: 'test:fx', src: 'de', tgt: 'en', endpoint: 'http://127.0.0.1:' + port });
  const r2 = await doclat.translateFile(quelle, { key: 'test:fx', src: 'de', tgt: 'en', endpoint: 'http://127.0.0.1:' + port });
  if (fs.existsSync(r1.out) && path.basename(r1.out) === 'probe.en.docx' && fs.statSync(r1.out).size > 1000) ok('Datei: schreibt probe.en.docx daneben', fs.statSync(r1.out).size + ' Bytes');
  else bad('Datei: schreibt probe.en.docx daneben', JSON.stringify(r1));
  if (path.basename(r2.out) === 'probe.en (2).docx' && fs.existsSync(r2.out)) ok('Datei: zweiter Lauf ueberschreibt nichts', path.basename(r2.out));
  else bad('Datei: zweiter Lauf ueberschreibt nichts', JSON.stringify(r2));
  const zurueck = await JSZip.loadAsync(fs.readFileSync(r1.out));
  const xmlDatei = await zurueck.file('word/document.xml').async('string');
  if (/EN\[Quartalsbericht\]/.test(xmlDatei)) ok('Datei: Inhalt der geschriebenen Datei stimmt');
  else bad('Datei: Inhalt der geschriebenen Datei stimmt');
  fs.rmSync(ordner, { recursive: true, force: true });

  return { stats, plain };
}

// ---------- PDF-Beispiel ----------
async function buildPdf() {
  const { PDFDocument, StandardFonts, rgb } = require('pdf-lib');
  const pdf = await PDFDocument.create();
  const font = await pdf.embedFont(StandardFonts.Helvetica);
  const bold = await pdf.embedFont(StandardFonts.HelveticaBold);
  const seite1 = pdf.addPage([595, 842]);
  seite1.drawText('Quartalsbericht', { x: 72, y: 760, size: 18, font: bold, color: rgb(0.12, 0.31, 0.47) });
  seite1.drawText('Der Umsatz ist gestiegen.', { x: 72, y: 700, size: 11, font, color: rgb(0, 0, 0) });
  seite1.drawText('Die Kosten sanken im Vergleich zum Vorjahr.', { x: 72, y: 682, size: 11, font, color: rgb(0, 0, 0) });
  const seite2 = pdf.addPage([595, 842]);
  seite2.drawText('Website', { x: 72, y: 760, size: 11, font, color: rgb(0, 0, 1) });
  const annot = pdf.context.obj({
    Type: 'Annot', Subtype: 'Link', Rect: [72, 755, 120, 768], Border: [0, 0, 0],
    A: { Type: 'Action', S: 'URI', URI: 'https://example.com' }
  });
  seite2.node.addAnnot(pdf.context.register(annot));
  return Buffer.from(await pdf.save());
}

async function pdfInfo(buf) {
  const pdfjs = await import('pdfjs-dist/legacy/build/pdf.mjs');
  const doc = await pdfjs.getDocument({ data: new Uint8Array(buf), useSystemFonts: false, isEvalSupported: false }).promise;
  const pages = [];
  const colors = [];
  for (let i = 1; i <= doc.numPages; i += 1) {
    const p = await doc.getPage(i);
    const tc = await p.getTextContent();
    const vp = p.getViewport({ scale: 1 });
    pages.push({ width: vp.width, height: vp.height, items: tc.items.filter(it => String(it.str || '').trim()).map(it => ({ str: it.str, x: it.transform[4], y: it.transform[5], h: it.height })) });
    const ops = await p.getOperatorList();
    let cur = null;
    for (let k = 0; k < ops.fnArray.length; k += 1) {
      if (ops.fnArray[k] === pdfjs.OPS.setFillRGBColor) cur = ops.argsArray[k];
      else if (ops.fnArray[k] === pdfjs.OPS.showText) colors.push(cur);
    }
  }
  return { pages, colors };
}

async function testPdf(port) {
  const src = await buildPdf();
  const beforeInfo = await pdfInfo(src);
  const { buffer, stats } = await doclat.translatePdfBuffer(src, { key: 'test:fx', src: 'de', tgt: 'en', endpoint: 'http://127.0.0.1:' + port });
  const after = await pdfInfo(buffer);
  if (buffer.slice(0, 5).toString() === '%PDF-' && buffer.length > 500) ok('PDF: gueltige Datei', buffer.length + ' Bytes');
  else bad('PDF: gueltige Datei', buffer.length + ' Bytes');
  if (after.pages.length === beforeInfo.pages.length) ok('PDF: gleiche Seitenzahl', after.pages.length);
  else bad('PDF: gleiche Seitenzahl', after.pages.length + ' statt ' + beforeInfo.pages.length);
  const sizeSame = after.pages.every((p, i) => near(p.width, beforeInfo.pages[i].width, 0.6) && near(p.height, beforeInfo.pages[i].height, 0.6));
  if (sizeSame) ok('PDF: gleiche Seitengroesse', after.pages.map(p => Math.round(p.width) + 'x' + Math.round(p.height)).join(', '));
  else bad('PDF: gleiche Seitengroesse');

  const texte = after.pages.flatMap(p => p.items.map(i => i.str));
  const uebersetzt = texte.filter(t => /EN\[/.test(t));
  if (uebersetzt.length >= 4) ok('PDF: Textzeilen uebersetzt', uebersetzt.length + ' Zeilen');
  else bad('PDF: Textzeilen uebersetzt', JSON.stringify(texte));

  // Grundlinien: die Uebersetzung steht auf derselben Hoehe wie das Original.
  const ysBefore = beforeInfo.pages.map(p => p.items.map(i => i.y));
  const ysAfter = after.pages.map(p => p.items.map(i => i.y));
  let maxDelta = 0;
  for (let i = 0; i < ysBefore.length; i += 1) {
    for (let k = 0; k < Math.min(ysBefore[i].length, ysAfter[i].length); k += 1) maxDelta = Math.max(maxDelta, Math.abs(ysBefore[i][k] - ysAfter[i][k]));
  }
  if (maxDelta <= 1.5) ok('PDF: Grundlinien bleiben', 'max. ' + maxDelta.toFixed(2) + ' pt Abweichung');
  else bad('PDF: Grundlinien bleiben', maxDelta.toFixed(2) + ' pt');

  const xBefore = beforeInfo.pages.map(p => p.items.map(i => i.x));
  const xAfter = after.pages.map(p => p.items.map(i => i.x));
  let maxX = 0;
  for (let i = 0; i < xBefore.length; i += 1) for (let k = 0; k < Math.min(xBefore[i].length, xAfter[i].length); k += 1) maxX = Math.max(maxX, Math.abs(xBefore[i][k] - xAfter[i][k]));
  if (maxX <= 1.5) ok('PDF: linker Rand bleibt', 'max. ' + maxX.toFixed(2) + ' pt');
  else bad('PDF: linker Rand bleibt', maxX.toFixed(2) + ' pt');

  // Farbe der Ueberschrift: 31/79/120. pdf.js liefert "#1f4f78".
  const hexRgb = c => {
    const raw = Array.isArray(c) && typeof c[0] === 'string' ? c[0] : c;
    if (typeof raw !== 'string') return null;
    const h = raw.replace(/^#/, '');
    return h.length === 6 ? [parseInt(h.slice(0, 2), 16), parseInt(h.slice(2, 4), 16), parseInt(h.slice(4, 6), 16)] : null;
  };
  const colored = after.colors.some(c => { const v = hexRgb(c); return v && Math.abs(v[0] - 31) < 6 && Math.abs(v[1] - 79) < 6 && Math.abs(v[2] - 120) < 6; });
  if (colored) ok('PDF: Farbe der Ueberschrift bleibt', '1f4f78 in ' + after.colors.length + ' Farbeintraegen');
  else bad('PDF: Farbe der Ueberschrift bleibt', JSON.stringify(after.colors.slice(0, 6)));

  // Link bleibt Link.
  const pdfjs = await import('pdfjs-dist/legacy/build/pdf.mjs');
  const d = await pdfjs.getDocument({ data: new Uint8Array(buffer), isEvalSupported: false }).promise;
  const p2 = await d.getPage(2);
  const anns = await p2.getAnnotations();
  const link = anns.find(a => a.subtype === 'Link' && String(a.unsafeUrl || a.url || '').includes('example.com'));
  if (link) ok('PDF: Link-Ziel bleibt', link.subtype + ' -> ' + (link.url || link.unsafeUrl));
  else bad('PDF: Link-Ziel bleibt', JSON.stringify(anns.map(a => ({ s: a.subtype, u: a.url, ru: a.unsafeUrl }))));

  return { stats };
}

// ---------- Fehlerwege ----------
async function testErrors(port) {
  const src = await buildDocx();
  try {
    await doclat.translateDocxBuffer(src, { key: '', tgt: 'en', endpoint: 'http://127.0.0.1:' + port });
    bad('Fehler: ohne Schluessel', 'kein Fehler geworfen');
  } catch (e) {
    if (e.code === 'noKey') ok('Fehler: ohne Schluessel', e.message);
    else bad('Fehler: ohne Schluessel', e.code + ': ' + e.message);
  }
  try {
    await doclat.translateFile(path.join(os.tmpdir(), 'x.txt'), { key: 'test:fx', tgt: 'en' });
    bad('Fehler: unbekannte Endung', 'kein Fehler geworfen');
  } catch (e) {
    if (e.code === 'unsupported') ok('Fehler: unbekannte Endung', e.message);
    else bad('Fehler: unbekannte Endung', e.code + ': ' + e.message);
  }
  // 403 vom Dienst wird als solcher gemeldet.
  const srv = http.createServer((req, res) => { res.writeHead(403, { 'content-type': 'application/json' }); res.end('{"message":"wrong key"}'); });
  await new Promise(r => srv.listen(0, '127.0.0.1', r));
  try {
    await doclat.deepl(['Hallo'], { key: 'falsch', tgt: 'en', endpoint: 'http://127.0.0.1:' + srv.address().port });
    bad('Fehler: 403 wird gemeldet', 'kein Fehler');
  } catch (e) {
    if (e.code === 'deepl403') ok('Fehler: 403 wird gemeldet', e.message);
    else bad('Fehler: 403 wird gemeldet', e.code);
  }
  srv.close();
}

(async () => {
  const { srv, port } = await mockServer();
  try {
    const { stats } = await testDocx(port);
    ok('DOCX: Kennzahlen', JSON.stringify(stats));
    const ps = await testPdf(port);
    ok('PDF: Kennzahlen', JSON.stringify(ps.stats));
    await testErrors(port);
  } catch (e) {
    bad('Lauf', (e && e.stack || e).toString().split('\n').slice(0, 4).join(' | '));
  }
  srv.close();
  console.log(zeilen.join('\n'));
  console.log('');
  console.log(fehler.length ? 'FEHLGESCHLAGEN: ' + fehler.join(', ') : 'Dokumentpruefung bestanden (' + zeilen.length + ' Pruefungen).');
  process.exit(fehler.length ? 1 : 0);
})();
