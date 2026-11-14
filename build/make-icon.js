#!/usr/bin/env node
// Erzeugt build/icon.png (512x512 RGBA) ohne Abhaengigkeiten: ein abgerundetes
// Quadrat in der Akzentfarbe mit zwei Pfeilen in der Tintenfarbe, also das
// Tausch-Symbol der App. Aufruf: node build/make-icon.js
'use strict';
const fs = require('fs'), path = require('path'), zlib = require('zlib');

const SIZE = 512, HALF = SIZE / 2, RADIUS = 112;
const BG = [156, 207, 245];  // --accent im hellen Theme
const INK = [14, 42, 68];    // --on-accent
const SS = 4;                // Kantenglaettung: 4x4 Abtastpunkte je Pixel

const inRoundRect = (x, y) => {
  const dx = Math.max(Math.abs(x) - (HALF - RADIUS), 0);
  const dy = Math.max(Math.abs(y) - (HALF - RADIUS), 0);
  return dx * dx + dy * dy <= RADIUS * RADIUS;
};
const inRect = (x, y, x0, x1, y0, y1) => x >= x0 && x <= x1 && y >= y0 && y <= y1;
const inTriangle = (x, y, base, tip, half) => {
  if (tip > base) { if (x < base || x > tip) return false; }
  else if (x > base || x < tip) return false;
  const t = Math.abs(tip - base) === 0 ? 1 : (x - base) / (tip - base);
  return Math.abs(y) <= half * (1 - t);
};
// Ein Pfeil: Schaft als Rechteck, Spitze als Dreieck, yc ist die Mittelachse.
const inArrow = (x, y, yc, dir) => {
  const shaft = dir > 0 ? inRect(x, y, -120, 34, yc - 17, yc + 17) : inRect(x, y, -34, 120, yc - 17, yc + 17);
  const head = inTriangle(x, y - yc, dir > 0 ? 34 : -34, dir > 0 ? 104 : -104, 38);
  return shaft || head;
};

const raw = Buffer.alloc((SIZE * 4 + 1) * SIZE);
let p = 0;
for (let py = 0; py < SIZE; py++) {
  raw[p++] = 0; // Filtertyp "None"
  for (let px = 0; px < SIZE; px++) {
    let covered = 0, ink = 0;
    for (let sy = 0; sy < SS; sy++) {
      for (let sx = 0; sx < SS; sx++) {
        const x = px + (sx + 0.5) / SS - HALF, y = py + (sy + 0.5) / SS - HALF;
        if (!inRoundRect(x, y)) continue;
        covered++;
        if (inArrow(x, y, -54, 1) || inArrow(x, y, 54, -1)) ink++;
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
ihdr.writeUInt32BE(SIZE, 0);
ihdr.writeUInt32BE(SIZE, 4);
ihdr[8] = 8;   // Bittiefe
ihdr[9] = 6;   // RGBA
const png = Buffer.concat([
  Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
  chunk('IHDR', ihdr),
  chunk('IDAT', zlib.deflateSync(raw, { level: 9 })),
  chunk('IEND', Buffer.alloc(0))
]);

const out = path.join(__dirname, 'icon.png');
fs.writeFileSync(out, png);
console.log(`${out} geschrieben (${SIZE}x${SIZE}, ${png.length} Bytes)`);
