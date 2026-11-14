#!/usr/bin/env node
// Erzeugt build/icon.png (512x512) und build/icon.ico (16 bis 256 px) ohne
// Abhaengigkeiten: ein abgerundetes Quadrat in der Akzentfarbe mit zwei Pfeilen
// in der Tintenfarbe, also das Tausch-Symbol der App.
// Aufruf: node build/make-icon.js
'use strict';
const fs = require('fs'), path = require('path'), zlib = require('zlib');

const BG = [156, 207, 245];  // --accent im hellen Theme
const INK = [14, 42, 68];    // --on-accent
const SS = 4;                // Kantenglaettung: 4x4 Abtastpunkte je Pixel
const ICO_SIZES = [256, 128, 64, 48, 32, 16];

// Zeichnet ein Icon der Kantenlaenge size und liefert die PNG-Datei.
function renderPng(size) {
  const half = size / 2, radius = size * (112 / 512), unit = size / 512;

  const inRoundRect = (x, y) => {
    const dx = Math.max(Math.abs(x) - (half - radius), 0);
    const dy = Math.max(Math.abs(y) - (half - radius), 0);
    return dx * dx + dy * dy <= radius * radius;
  };
  const inRect = (x, y, x0, x1, y0, y1) => x >= x0 && x <= x1 && y >= y0 && y <= y1;
  const inTriangle = (x, y, base, tip, h) => {
    if (tip > base) { if (x < base || x > tip) return false; }
    else if (x > base || x < tip) return false;
    const t = Math.abs(tip - base) === 0 ? 1 : (x - base) / (tip - base);
    return Math.abs(y) <= h * (1 - t);
  };
  // Pfeil: Schaft als Rechteck, Spitze als Dreieck, yc ist die Mittelachse.
  const inArrow = (x, y, yc, dir) => {
    const s = unit, shaft = dir > 0
      ? inRect(x, y, -120 * s, 34 * s, yc - 17 * s, yc + 17 * s)
      : inRect(x, y, -34 * s, 120 * s, yc - 17 * s, yc + 17 * s);
    const head = inTriangle(x, y - yc, dir > 0 ? 34 * s : -34 * s, dir > 0 ? 104 * s : -104 * s, 38 * s);
    return shaft || head;
  };

  const raw = Buffer.alloc((size * 4 + 1) * size);
  let p = 0;
  for (let py = 0; py < size; py++) {
    raw[p++] = 0; // Filtertyp "None"
    for (let px = 0; px < size; px++) {
      let covered = 0, ink = 0;
      for (let sy = 0; sy < SS; sy++) {
        for (let sx = 0; sx < SS; sx++) {
          const x = px + (sx + 0.5) / SS - half, y = py + (sy + 0.5) / SS - half;
          if (!inRoundRect(x, y)) continue;
          covered++;
          if (inArrow(x, y, -54 * unit, 1) || inArrow(x, y, 54 * unit, -1)) ink++;
        }
      }
      const share = covered ? ink / covered : 0;
      raw[p++] = Math.round(BG[0] * (1 - share) + INK[0] * share);
      raw[p++] = Math.round(BG[1] * (1 - share) + INK[1] * share);
      raw[p++] = Math.round(BG[2] * (1 - share) + INK[2] * share);
      raw[p++] = Math.round((covered / (SS * SS)) * 255);
    }
  }

  const TABLE = (() => {
    const t = new Int32Array(256);
    for (let n = 0; n < 256; n++) {
      let c = n;
      for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
      t[n] = c;
    }
    return t;
  })();
  const crc32 = buf => {
    let c = -1;
    for (const b of buf) c = TABLE[(c ^ b) & 0xff] ^ (c >>> 8);
    return (c ^ -1) >>> 0;
  };
  const chunk = (type, data) => {
    const len = Buffer.alloc(4);
    len.writeUInt32BE(data.length, 0);
    const body = Buffer.concat([Buffer.from(type, 'ascii'), data]);
    const crc = Buffer.alloc(4);
    crc.writeUInt32BE(crc32(body), 0);
    return Buffer.concat([len, body, crc]);
  };
  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(size, 0);
  ihdr.writeUInt32BE(size, 4);
  ihdr[8] = 8;   // Bittiefe
  ihdr[9] = 6;   // RGBA
  return Buffer.concat([
    Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
    chunk('IHDR', ihdr),
    chunk('IDAT', zlib.deflateSync(raw, { level: 9 })),
    chunk('IEND', Buffer.alloc(0))
  ]);
}

// ICO mit eingebetteten PNG-Daten (ab Windows Vista lesbar).
function buildIco(images) {
  const head = Buffer.alloc(6);
  head.writeUInt16LE(0, 0);
  head.writeUInt16LE(1, 2);
  head.writeUInt16LE(images.length, 4);
  const entries = [];
  let offset = 6 + images.length * 16;
  for (const { size, png } of images) {
    const e = Buffer.alloc(16);
    e[0] = size >= 256 ? 0 : size;
    e[1] = size >= 256 ? 0 : size;
    e[2] = 0;
    e[3] = 0;
    e.writeUInt16LE(1, 4);
    e.writeUInt16LE(32, 6);
    e.writeUInt32LE(png.length, 8);
    e.writeUInt32LE(offset, 12);
    offset += png.length;
    entries.push(e);
  }
  return Buffer.concat([head, ...entries, ...images.map(i => i.png)]);
}

const png = renderPng(512);
const pngFile = path.join(__dirname, 'icon.png');
fs.writeFileSync(pngFile, png);
console.log(`${pngFile} geschrieben (512x512, ${png.length} Bytes)`);

const ico = buildIco(ICO_SIZES.map(size => ({ size, png: renderPng(size) })));
const icoFile = path.join(__dirname, 'icon.ico');
fs.writeFileSync(icoFile, ico);
console.log(`${icoFile} geschrieben (${ICO_SIZES.join(', ')} px, ${ico.length} Bytes)`);
