#!/usr/bin/env node
// Prüft die deutschen Oberflächentexte gegen die Stilregel aus Doku §7 und
// meldet Lücken in den übrigen Sprachfassungen.
//
//   node tools/regeln.js          meldet nur Verstöße (Rückgabe 1, wenn welche)
//   node tools/regeln.js --alle   zeigt zusätzlich die Sprachabdeckung
//
// Die Regel ist bewusst eine Liste und keine Sprachverarbeitung: Ein Wort, das
// hier als Lehnwort steht, hat im Programm ein deutsches Gegenwort. Wer etwas
// bewusst anders lässt, trägt es unten in AUSNAHMEN mit Grund ein.

const fs = require('fs');
const path = require('path');

const ROOT = path.join(__dirname, '..');

// Erste Bestandteile, bei denen ein Bindestrich richtig ist: Marken, Kürzel und
// Fremdwörter, die ohne Bindestrich nicht lesbar wären.
const MARKEN = new Set([
  'Ollama', 'NLLB', 'Piper', 'Readlang', 'Hunspell', 'Word', 'Excel', 'TTS',
  'MiLMMT', 'TranslateGemma', 'Gemma', 'DIN', 'JSON', 'HTML', 'Markdown', 'PDF',
  'CSV', 'Sprach', 'Daten', 'Python', 'Ollama', 'Piper'
]);

// Lehnwort -> deutsches Gegenwort. Nur Wörter, die im Programm schon einmal
// deutsch stehen; alles andere wäre Geschmack.
const LEHNWORT = {
  'Standard': 'Vorgabe',
  'Editor': 'Textbearbeitung',
  'Engine': 'Sprachausgabe',
  'Infos': 'Angaben',
  'Amber': 'Bernstein',
  'Pink': 'Rosa',
  'Design': 'Darstellung',
  'Layout': 'Anordnung',
  'Session': 'Sitzung',
  'Button': 'Knopf',
  'Feature': 'Funktion',
  'Backup': 'Sicherung',
  'Prompt': 'Anweisung'
};

// Bewusst so gelassen (Entscheidungen aus 1.15.188 und §7). Schlüssel oder Text.
const AUSNAHMEN = new Set([
  's_csvSrs', 'glossColls', 's_sys', 'serverDown', 's_nllb', 'styleIcon',
  's_edAI', 'edAIdim', 's_start', 's_bootModel', 'st_pro', 'o_natural',
  'o_formal', 's_story', 's_helper', 'langGeneric', 's_edTbGroup'
]);

const PERSONEN = /\b(Absender|Empfänger|Nutzer|Benutzer|Anwender|Leser|Hörer|Sprecher|Schreiber)\b(?!\*)/;

function literale(src, start, ende) {
  // Alle Textstücke der Datei mit Position (für Zeilennummern).
  const out = [];
  let i = 0;
  while (i < src.length) {
    const c = src[i];
    if (c === '"' || c === "'" || c === '`') {
      const q = c;
      let j = i + 1;
      let buf = '';
      while (j < src.length) {
        if (src[j] === '\\') { buf += src[j] + (src[j + 1] || ''); j += 2; continue; }
        if (src[j] === q) break;
        buf += src[j];
        j += 1;
      }
      out.push({ text: buf, at: i });
      i = j + 1;
    } else i += 1;
  }
  return start == null ? out : out.filter(x => x.at >= start && x.at < ende);
}

function zeile(src, at) {
  return src.slice(0, at).split('\n').length;
}

function block(src, name) {
  const m = new RegExp('const\\s+' + name + '\\s*=\\s*\\{').exec(src);
  if (!m) return null;
  let i = m.index + m[0].length - 1;
  let depth = 0;
  for (; i < src.length; i += 1) {
    const c = src[i];
    if (c === "'" || c === '"' || c === '`') {
      const q = c;
      i += 1;
      while (i < src.length && src[i] !== q) { if (src[i] === '\\') i += 1; i += 1; }
      continue;
    }
    if (c === '{') depth += 1;
    else if (c === '}') { depth -= 1; if (depth === 0) return { start: m.index, end: i }; }
  }
  return null;
}

// Deutsche Texte einsammeln: Einträge der Form `key: ['deutsch', …]` und
// Aufrufe `X('key','deutsch', …)`.
function sammle() {
  const funde = [];
  const dateien = ['renderer.js', 'editor.js', 'study.js'];
  for (const datei of dateien) {
    // Auskommentierte Zeilen zählen nicht: ein Eintrag, der bewusst stillgelegt
    // wurde (etwa eine nicht mehr benutzte Farbe), ist keine Wortwahl mehr.
    const roh = fs.readFileSync(path.join(ROOT, datei), 'utf8');
    const src = roh.replace(/\/\*[\s\S]*?\*\//g, m => m.replace(/[^\n]/g, ' '));
    const re = /([A-Za-z_$][A-Za-z0-9_$]*)\s*:\s*\[\s*(['"`])((?:[^'"`\\]|\\.)*)\2/g;
    let m;
    while ((m = re.exec(src))) {
      const key = m[1];
      const text = m[3];
      if (!/^[\x20-\x7e\u00a0-\uffff]*$/.test(text)) continue;
      funde.push({ datei, key, text, zeile: zeile(src, m.index) });
    }
    const rx = /X\(\s*'([^']+)'\s*,\s*'((?:[^'\\]|\\.)*)'/g;
    while ((m = rx.exec(src))) {
      funde.push({ datei, key: m[1], text: m[2].replace(/\\'/g, "'"), zeile: zeile(src, m.index) });
    }
  }
  return funde.filter(f => /[A-Za-z]/.test(f.text));
}

function pruefe() {
  const funde = sammle();
  const treffer = [];
  for (const f of funde) {
    if (AUSNAHMEN.has(f.key)) continue;
    const text = f.text;
    // 1. Bindestrich in einem nativen Kompositum
    const hy = /(^|[^\w-])([A-ZÄÖÜ][a-zäöüß]{2,})-([A-ZÄÖÜ][a-zäöüß]{2,})/g;
    let h;
    while ((h = hy.exec(text))) {
      if (MARKEN.has(h[2])) continue;
      treffer.push({ ...f, regel: 'Bindestrich', wort: h[2] + '-' + h[3], tipp: h[2] + h[3].toLowerCase() });
    }
    // 2. Lehnwort, obwohl ein deutsches Gegenwort im Programm steht
    for (const [lehn, de] of Object.entries(LEHNWORT)) {
      if (new RegExp('\\b' + lehn + '\\b').test(text)) {
        treffer.push({ ...f, regel: 'Lehnwort', wort: lehn, tipp: de });
      }
    }
    // 3. Personenbezeichnung ohne Sternchen
    const g = PERSONEN.exec(text);
    if (g && !text.includes(g[1] + '*')) {
      treffer.push({ ...f, regel: 'Geschlecht', wort: g[1], tipp: g[1] + '*in' });
    }
  }
  return { treffer, funde };
}

function abdeckung() {
  const src = fs.readFileSync(path.join(ROOT, 'renderer.js'), 'utf8');
  const k = block(src, 'K');
  if (!k) return null;
  const schluessel = name => {
    const b = block(src, name);
    if (!b) return null;
    const set = new Set();
    const teil = src.slice(b.start, b.end);
    const re = /(?:^|[{,\s])([A-Za-z_$][A-Za-z0-9_$]*)\s*:/g;
    let m;
    while ((m = re.exec(teil))) set.add(m[1]);
    return set;
  };
  const alle = schluessel('K');
  const out = { schluessel: alle.size, fehlt: {}, fremd: {} };
  for (const name of ['NL', 'ES', 'TR', 'AR']) {
    const s = schluessel(name);
    if (!s) continue;
    out.fehlt[name] = [...alle].filter(x => !s.has(x)).length;
    out.fremd[name] = [];
  }
  return out;
}

const alle = process.argv.includes('--alle');
const { treffer, funde } = pruefe();
const a = abdeckung();

if (alle && a) {
  console.log('Sprachschlüssel: ' + a.schluessel + ' deutsche Einträge');
  for (const name of Object.keys(a.fehlt)) {
    console.log('  ' + name + ': ' + a.fehlt[name] + ' ohne eigenen Text (Rückfall auf Englisch)');
  }
  console.log('');
}

if (!treffer.length) {
  console.log('Keine Verstöße gegen die Wortwahlregel (' + funde.length + ' deutsche Texte geprüft).');
  process.exit(0);
}

treffer.sort((x, y) => x.datei.localeCompare(y.datei) || x.zeile - y.zeile);
let letzte = '';
for (const t of treffer) {
  const ort = t.datei + ':' + t.zeile;
  if (ort !== letzte) { console.log(''); console.log(ort); letzte = ort; }
  console.log('  ' + t.regel.padEnd(12) + ' ' + t.key.padEnd(20) + ' „' + t.wort + '“ → ' + t.tipp);
}
console.log('');
console.log(treffer.length + ' Fundstellen in ' + new Set(treffer.map(t => t.datei + ':' + t.zeile)).size + ' Zeilen. Bewusste Ausnahmen stehen in AUSNAHMEN.');
process.exit(1);
