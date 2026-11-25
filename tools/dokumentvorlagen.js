'use strict';
// Erzeugt die Pruefdateien fuer die Dokumentuebersetzung: ein Word-Dokument
// und ein PDF mit gemischtem Layout, beide vollstaendig und ohne fremde
// Werkzeuge. Sie sind absichtlich gestaltungslastig: Ueberschriften, Fett,
// Kursiv, Unterstrichen, Farbe, Ausrichtung, Absatz- und Zeilenabstand,
// Aufzaehlung, Nummerierung, Hyperlink, Bild, Tabelle, Kopf-/Fusszeile,
// Seitenumbruch, Seitenraender und Seitenformat.
//
//   node -e "require('./tools/dokumentvorlagen').writeFixtures('testfiles')"

const fs = require('fs');
const path = require('path');
const zlib = require('zlib');
const JSZip = require('jszip');

// ---------- ein echtes kleines PNG ----------
function crc32(buf) {
  let c, crc = 0xffffffff;
  for (let n = 0; n < buf.length; n += 1) {
    c = (crc ^ buf[n]) & 0xff;
    for (let k = 0; k < 8; k += 1) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
    crc = (crc >>> 8) ^ c;
  }
  return (crc ^ 0xffffffff) >>> 0;
}
function chunk(type, data) {
  const len = Buffer.alloc(4); len.writeUInt32BE(data.length, 0);
  const body = Buffer.concat([Buffer.from(type, 'latin1'), data]);
  const crc = Buffer.alloc(4); crc.writeUInt32BE(crc32(body), 0);
  return Buffer.concat([len, body, crc]);
}
function png(w, h, rgb) {
  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(w, 0); ihdr.writeUInt32BE(h, 4);
  ihdr[8] = 8; ihdr[9] = 2; ihdr[10] = 0; ihdr[11] = 0; ihdr[12] = 0;
  const row = Buffer.alloc(1 + w * 3);
  for (let x = 0; x < w; x += 1) { row[1 + x * 3] = rgb[0]; row[2 + x * 3] = rgb[1]; row[3 + x * 3] = rgb[2]; }
  const raw = Buffer.concat(Array.from({ length: h }, () => row));
  return Buffer.concat([
    Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
    chunk('IHDR', ihdr), chunk('IDAT', zlib.deflateSync(raw)), chunk('IEND', Buffer.alloc(0))
  ]);
}

// ---------- Word ----------
const NS = 'xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" ' +
  'xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" ' +
  'xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" ' +
  'xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" ' +
  'xmlns:pic="http://schemas.openxmlformats.org/drawingml/2006/picture"';

const rPr = o => {
  const p = [];
  if (o.font) p.push(`<w:rFonts w:ascii="${o.font}" w:hAnsi="${o.font}" w:cs="${o.font}"/>`);
  if (o.b) p.push('<w:b/>');
  if (o.i) p.push('<w:i/>');
  if (o.u) p.push('<w:u w:val="single"/>');
  if (o.color) p.push(`<w:color w:val="${o.color}"/>`);
  if (o.sz) p.push(`<w:sz w:val="${o.sz}"/><w:szCs w:val="${o.sz}"/>`);
  if (o.style) p.push(`<w:rStyle w:val="${o.style}"/>`);
  return p.length ? '<w:rPr>' + p.join('') + '</w:rPr>' : '';
};
const run = (text, o = {}) => `<w:r>${rPr(o)}<w:t${/^\s|\s$/.test(text) ? ' xml:space="preserve"' : ''}>${text}</w:t></w:r>`;
const para = (runs, o = {}) => {
  const p = [];
  if (o.style) p.push(`<w:pStyle w:val="${o.style}"/>`);
  if (o.numId) p.push(`<w:numPr><w:ilvl w:val="0"/><w:numId w:val="${o.numId}"/></w:numPr>`);
  if (o.spacing) p.push(`<w:spacing ${o.spacing}/>`);
  if (o.jc) p.push(`<w:jc w:val="${o.jc}"/>`);
  if (o.ind) p.push(`<w:ind ${o.ind}/>`);
  return `<w:p>${p.length ? '<w:pPr>' + p.join('') + '</w:pPr>' : ''}${runs}</w:p>`;
};

function documentXml() {
  const body = [
    para(run('Quartalsbericht 2026', { font: 'Georgia', b: 1, color: '1F4E79', sz: 32 }), { style: 'Heading1', jc: 'center', spacing: 'w:after="240"' }),
    para(
      run('Der Umsatz ist ', { font: 'Arial', sz: 22 }) +
      run('deutlich gestiegen', { font: 'Arial', sz: 22, b: 1 }) +
      run(', die Kosten sanken um ', { font: 'Arial', sz: 22 }) +
      run('acht Prozent', { font: 'Arial', sz: 22, i: 1, color: 'C00000' }) +
      run('.', { font: 'Arial', sz: 22 }),
      { jc: 'both', spacing: 'w:before="120" w:after="120" w:line="360" w:lineRule="auto"' }
    ),
    para(run('Stand: 30. September 2026', { font: 'Arial', sz: 20, color: '595959' }), { jc: 'right', spacing: 'w:after="240"' }),
    para(run('Erster Punkt mit ', { font: 'Arial', sz: 22 }) + run('wichtigem Hinweis', { font: 'Arial', sz: 22, b: 1 }), { numId: 1 }),
    para(run('Zweiter Punkt', { font: 'Arial', sz: 22, i: 1 }), { numId: 1 }),
    para(run('Dritter Punkt', { font: 'Arial', sz: 22 }), { numId: 1 }),
    para(run('Planung abschließen', { font: 'Arial', sz: 22 }), { numId: 2 }),
    para(run('Bericht versenden', { font: 'Arial', sz: 22 }), { numId: 2 }),
    para(
      run('Mehr dazu auf ', { font: 'Arial', sz: 22 }) +
      '<w:hyperlink r:id="rId10"><w:r><w:rPr><w:rStyle w:val="Hyperlink"/><w:b/><w:u w:val="single"/><w:color w:val="0563C1"/></w:rPr><w:t>unserer Website</w:t></w:r></w:hyperlink>' +
      run('.', { font: 'Arial', sz: 22 }),
      { jc: 'left', spacing: 'w:before="120" w:after="120"' }
    ),
    '<w:tbl><w:tblPr><w:tblStyle w:val="TableGrid"/><w:tblW w:w="0" w:type="auto"/></w:tblPr>' +
      '<w:tblGrid><w:gridCol w:w="2835"/><w:gridCol w:w="2835"/><w:gridCol w:w="2835"/></w:tblGrid>' +
      '<w:tr><w:trPr><w:tblHeader/></w:trPr>' +
        ['Kennzahl', 'Wert', 'Veränderung'].map(t => `<w:tc><w:tcPr><w:shd w:val="clear" w:color="auto" w:fill="D9E2F3"/></w:tcPr>${para(run(t, { font: 'Arial', sz: 22, b: 1, color: '1F4E79' }))}</w:tc>`).join('') +
      '</w:tr>' +
      '<w:tr>' +
        [`<w:tc>${para(run('Umsatz', { font: 'Arial', sz: 22 }))}</w:tc>`,
         `<w:tc>${para(run('1,2 Millionen Euro', { font: 'Arial', sz: 22 }))}</w:tc>`,
         `<w:tc>${para(run('plus acht Prozent', { font: 'Arial', sz: 22, color: '1E7B34' }))}</w:tc>`].join('') +
      '</w:tr></w:tbl>',
    para(run('', { font: 'Arial', sz: 22 }), { jc: 'center' }),
    '<w:p><w:pPr><w:jc w:val="center"/></w:pPr><w:r><w:drawing><wp:inline distT="0" distB="0" distL="0" distR="0">' +
      '<wp:extent cx="914400" cy="457200"/><wp:docPr id="1" name="Bild 1"/>' +
      '<a:graphic><a:graphicData uri="http://schemas.openxmlformats.org/drawingml/2006/picture">' +
      '<pic:pic><pic:nvPicPr><pic:cNvPr id="1" name="bild.png"/><pic:cNvPicPr/></pic:nvPicPr>' +
      '<pic:blipFill><a:blip r:embed="rId11"/><a:stretch><a:fillRect/></a:stretch></pic:blipFill>' +
      '<pic:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="914400" cy="457200"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></pic:spPr>' +
      '</pic:pic></a:graphicData></a:graphic></wp:inline></w:drawing></w:r></w:p>',
    '<w:p><w:r><w:br w:type="page"/></w:r></w:p>',
    para(run('Anhang', { font: 'Cambria', b: 1, color: '2E74B5', sz: 28 }), { jc: 'left', spacing: 'w:after="160"' }),
    para(run('Die Zahlen stammen aus der internen Auswertung.', { font: 'Arial', sz: 22 }), { jc: 'left', spacing: 'w:line="240" w:lineRule="auto"' }),
    para(run('Rückfragen an das Controlling.', { font: 'Arial', sz: 22 }), { jc: 'left', spacing: 'w:before="80"' }),
    '<w:sectPr><w:headerReference w:type="default" r:id="rId6"/><w:footerReference w:type="default" r:id="rId7"/>' +
      '<w:pgSz w:w="11906" w:h="16838"/><w:pgMar w:top="1417" w:right="1417" w:bottom="1417" w:left="1417" w:header="708" w:footer="708" w:gutter="0"/>' +
      '<w:cols w:space="708"/><w:docGrid w:linePitch="360"/></w:sectPr>'
  ].join('');
  return '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>\n<w:document ' + NS + '><w:body>' + body + '</w:body></w:document>';
}

const HEAD = '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>\n';

function stylesXml() {
  return HEAD + `<w:styles ${NS}>` +
    '<w:docDefaults><w:rPrDefault><w:rPr><w:rFonts w:ascii="Calibri" w:hAnsi="Calibri" w:cs="Calibri"/><w:sz w:val="22"/></w:rPr></w:rPrDefault>' +
    '<w:pPrDefault><w:pPr><w:spacing w:after="160" w:line="259" w:lineRule="auto"/></w:pPr></w:pPrDefault></w:docDefaults>' +
    '<w:style w:type="paragraph" w:default="1" w:styleId="Normal"><w:name w:val="Normal"/></w:style>' +
    '<w:style w:type="paragraph" w:styleId="Heading1"><w:name w:val="heading 1"/><w:basedOn w:val="Normal"/><w:pPr><w:keepNext/><w:spacing w:before="480" w:after="120"/><w:outlineLvl w:val="0"/></w:pPr><w:rPr><w:b/><w:sz w:val="32"/></w:rPr></w:style>' +
    '<w:style w:type="character" w:styleId="Hyperlink"><w:name w:val="Hyperlink"/><w:rPr><w:color w:val="0563C1"/><w:u w:val="single"/></w:rPr></w:style>' +
    '<w:style w:type="table" w:styleId="TableGrid"><w:name w:val="Table Grid"/><w:tblPr><w:tblBorders>' +
    '<w:top w:val="single" w:sz="4" w:space="0" w:color="auto"/><w:left w:val="single" w:sz="4" w:space="0" w:color="auto"/>' +
    '<w:bottom w:val="single" w:sz="4" w:space="0" w:color="auto"/><w:right w:val="single" w:sz="4" w:space="0" w:color="auto"/>' +
    '<w:insideH w:val="single" w:sz="4" w:space="0" w:color="auto"/><w:insideV w:val="single" w:sz="4" w:space="0" w:color="auto"/>' +
    '</w:tblBorders></w:tblPr></w:style></w:styles>';
}

function numberingXml() {
  const abstract = (id, fmt, text) => `<w:abstractNum w:abstractNumId="${id}"><w:lvl w:ilvl="0"><w:start w:val="1"/><w:numFmt w:val="${fmt}"/><w:lvlText w:val="${text}"/><w:lvlJc w:val="left"/><w:pPr><w:ind w:left="720" w:hanging="360"/></w:pPr><w:rPr><w:rFonts w:ascii="Arial" w:hAnsi="Arial"/></w:rPr></w:lvl></w:abstractNum>`;
  return HEAD + `<w:numbering ${NS}>` + abstract(0, 'bullet', '•') + abstract(1, 'decimal', '%1.') +
    '<w:num w:numId="1"><w:abstractNumId w:val="0"/></w:num><w:num w:numId="2"><w:abstractNumId w:val="1"/></w:num></w:numbering>';
}

async function buildDocx() {
  const zip = new JSZip();
  zip.file('[Content_Types].xml', HEAD + '<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">' +
    '<Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>' +
    '<Default Extension="xml" ContentType="application/xml"/><Default Extension="png" ContentType="image/png"/>' +
    '<Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>' +
    '<Override PartName="/word/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml"/>' +
    '<Override PartName="/word/numbering.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.numbering+xml"/>' +
    '<Override PartName="/word/settings.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.settings+xml"/>' +
    '<Override PartName="/word/header1.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.header+xml"/>' +
    '<Override PartName="/word/footer1.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.footer+xml"/>' +
    '</Types>');
  zip.file('_rels/.rels', HEAD + '<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">' +
    '<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>');
  zip.file('word/document.xml', documentXml());
  zip.file('word/styles.xml', stylesXml());
  zip.file('word/numbering.xml', numberingXml());
  zip.file('word/settings.xml', HEAD + `<w:settings ${NS}><w:defaultTabStop w:val="708"/><w:compat><w:compatSetting w:name="compatibilityMode" w:uri="http://schemas.microsoft.com/office/word" w:val="15"/></w:compat></w:settings>`);
  zip.file('word/header1.xml', HEAD + `<w:hdr ${NS}>` +
    para(run('Vertraulich – Quartalsbericht', { font: 'Arial', sz: 18, color: '7F7F7F' }), { jc: 'right' }) + '</w:hdr>');
  zip.file('word/footer1.xml', HEAD + `<w:ftr ${NS}>` +
    para(run('Seite 1 von 2', { font: 'Arial', sz: 18 }), { jc: 'center' }) + '</w:ftr>');
  zip.file('word/_rels/document.xml.rels', HEAD + '<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">' +
    '<Relationship Id="rId5" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/>' +
    '<Relationship Id="rId6" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/header" Target="header1.xml"/>' +
    '<Relationship Id="rId7" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/footer" Target="footer1.xml"/>' +
    '<Relationship Id="rId8" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/numbering" Target="numbering.xml"/>' +
    '<Relationship Id="rId9" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/settings" Target="settings.xml"/>' +
    '<Relationship Id="rId10" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink" Target="https://example.com/quartalsbericht" TargetMode="External"/>' +
    '<Relationship Id="rId11" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="media/bild.png"/>' +
    '</Relationships>');
  zip.file('word/media/bild.png', png(8, 8, [46, 116, 181]));
  return zip.generateAsync({ type: 'nodebuffer', compression: 'DEFLATE' });
}

// ---------- PDF ----------
async function buildPdf() {
  const { PDFDocument, StandardFonts, rgb } = require('pdf-lib');
  const pdf = await PDFDocument.create();
  const helv = await pdf.embedFont(StandardFonts.Helvetica);
  const helvB = await pdf.embedFont(StandardFonts.HelveticaBold);
  const times = await pdf.embedFont(StandardFonts.TimesRoman);
  const courier = await pdf.embedFont(StandardFonts.Courier);
  const img = await pdf.embedPng(png(24, 12, [204, 122, 0]));
  const blue = rgb(31 / 255, 79 / 255, 120 / 255);
  const grey = rgb(89 / 255, 89 / 255, 89 / 255);
  const red = rgb(192 / 255, 0, 0);

  const p1 = pdf.addPage([595, 842]);
  p1.drawText('Quartalsbericht 2026', { x: 72, y: 760, size: 20, font: helvB, color: blue });
  const sub = 'Interner Bericht';
  p1.drawText(sub, { x: 523 - helv.widthOfTextAtSize(sub, 11), y: 735, size: 11, font: helv, color: grey });
  p1.drawText('Der Umsatz ist deutlich gestiegen.', { x: 72, y: 700, size: 11, font: helv });
  p1.drawText('Die Kosten sanken um acht Prozent.', { x: 72, y: 684, size: 11, font: helv });
  p1.drawText('Wichtige Kennzahlen im Ueberblick:', { x: 72, y: 668, size: 11, font: helv });
  p1.drawText('Umsatz und Ertrag', { x: 72, y: 630, size: 13, font: helvB, color: blue });
  p1.drawText('Der Gewinn lag ueber dem Plan.', { x: 72, y: 608, size: 11, font: times, color: red });
  p1.drawText('Kennung Q-2026-0042', { x: 72, y: 588, size: 10, font: courier });
  p1.drawRectangle({ x: 72, y: 520, width: 451, height: 26, color: rgb(240 / 255, 235 / 255, 214 / 255) });
  p1.drawText('Hinweis zum Bericht', { x: 80, y: 528, size: 11, font: helvB, color: rgb(0.2, 0.2, 0.2) });
  p1.drawImage(img, { x: 72, y: 440, width: 96, height: 48 });
  p1.drawText('Mehr auf unserer Website', { x: 72, y: 400, size: 11, font: helv, color: rgb(0, 0, 0.6) });
  const annot = pdf.context.obj({
    Type: 'Annot', Subtype: 'Link', Rect: [72, 395, 240, 412], Border: [0, 0, 0],
    A: { Type: 'Action', S: 'URI', URI: 'https://example.com/quartalsbericht' }
  });
  p1.node.addAnnot(pdf.context.register(annot));

  const p2 = pdf.addPage([595, 842]);
  p2.drawText('Anhang', { x: 72, y: 760, size: 16, font: helvB, color: blue });
  const mid = 'Zusammenfassung der Zahlen';
  p2.drawText(mid, { x: 297.5 - helv.widthOfTextAtSize(mid, 11) / 2, y: 726, size: 11, font: helv });
  p2.drawText('Die Auswertung stammt aus dem Controlling.', { x: 72, y: 700, size: 11, font: helv });
  p2.drawText('Rueckfragen bitte an das Team.', { x: 72, y: 684, size: 11, font: helv });
  // kleine Tabelle als Flaechen mit Text
  const rows = [['Kennzahl', 'Wert'], ['Umsatz', '1,2 Millionen'], ['Kosten', 'acht Prozent']];
  rows.forEach((r, i) => {
    const y = 620 - i * 22;
    p2.drawRectangle({ x: 72, y: y - 4, width: 300, height: 20, borderColor: rgb(0.5, 0.5, 0.5), borderWidth: 0.7, color: i === 0 ? rgb(0.85, 0.89, 0.95) : rgb(1, 1, 1) });
    p2.drawLine({ start: { x: 250, y: y - 4 }, end: { x: 250, y: y + 16 }, thickness: 0.7, color: rgb(0.5, 0.5, 0.5) });
    p2.drawText(r[0], { x: 78, y, size: 10, font: i === 0 ? helvB : helv });
    p2.drawText(r[1], { x: 258, y, size: 10, font: helv });
  });
  return Buffer.from(await pdf.save());
}

async function writeFixtures(dir) {
  fs.mkdirSync(dir, { recursive: true });
  const docx = path.join(dir, 'original.docx');
  const pdf = path.join(dir, 'original.pdf');
  fs.writeFileSync(docx, await buildDocx());
  fs.writeFileSync(pdf, await buildPdf());
  return { docx, pdf };
}

module.exports = { buildDocx, buildPdf, writeFixtures, png };

if (require.main === module) {
  writeFixtures(path.join(__dirname, '..', 'testfiles')).then(r => {
    console.log('geschrieben:', r.docx, r.pdf);
  }).catch(e => { console.error(e); process.exit(1); });
}
