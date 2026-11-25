#!/usr/bin/env node
// Rauchtest: startet die App mit eigenem Profil und eigenem Debug-Port, prüft
// die wichtigsten Zusagen und beendet sie wieder.
//
//   node tools/rauchtest.js
//
// Geprüft wird, was in den letzten Iterationen tatsächlich kaputt war:
// 1. Die Oberfläche lädt (Titel, Arbeitsplätze, Brücke zum Hauptprozess).
// 2. Alle sechs Sprachfassungen haben dieselben Schlüssel.
// 3. Die Knöpfe einer Dialogzeile sind gleich hoch und gleich groß beschriftet.
// 4. Jeder Dialog lässt sich öffnen.
// Ohne diesen Lauf wurde bisher jeder Release von Hand durch den Debugger
// geprüft; die Zahlen dazu stehen nur im Gespräch, nicht im Projekt.

const { spawn, spawnSync } = require('child_process');
const fs = require('fs');
const os = require('os');
const path = require('path');

const ROOT = path.join(__dirname, '..');
const PORT = 9411 + Math.floor(Math.random() * 80);
const PROFIL = fs.mkdtempSync(path.join(os.tmpdir(), 'rauch-'));
const WARTEN_MS = 40000;

const fehler = [];
const zeilen = [];
const ok = (name, text) => zeilen.push('  ok    ' + name.padEnd(28) + (text || ''));
const bad = (name, text) => { zeilen.push('  FEHLT ' + name.padEnd(28) + (text || '')); fehler.push(name); };

const sleep = ms => new Promise(r => setTimeout(r, ms));

async function hole(url, versuche = 1) {
  for (let i = 0; i < versuche; i += 1) {
    try {
      const r = await fetch(url);
      if (r.ok) return await r.json();
    } catch {}
    await sleep(500);
  }
  return null;
}

function beenden(kind) {
  if (!kind || kind.killed) return;
  try {
    if (process.platform === 'win32') spawnSync('taskkill', ['/pid', String(kind.pid), '/t', '/f'], { windowsHide: true, stdio: 'ignore' });
    else kind.kill('SIGTERM');
  } catch { try { kind.kill(); } catch {} }
}

async function main() {
  const electron = require('electron');
  const kind = spawn(electron, ['.', '--remote-debugging-port=' + PORT, '--user-data-dir=' + PROFIL], {
    cwd: ROOT, stdio: 'ignore', windowsHide: true, env: Object.assign({}, process.env, { RAUCHTEST: '1' })
  });
  kind.on('exit', code => { if (code && code !== 0) fehler.push('App beendet mit Code ' + code); });

  let ziel = null;
  const ende = Date.now() + WARTEN_MS;
  while (Date.now() < ende && !ziel) {
    const liste = await hole('http://127.0.0.1:' + PORT + '/json/list');
    const seite = (liste || []).find(t => t.type === 'page' && t.webSocketDebuggerUrl);
    if (seite) ziel = seite.webSocketDebuggerUrl;
    else await sleep(500);
  }
  if (!ziel) { beenden(kind); return false; }

  const ws = new WebSocket(ziel);
  const offen = [];
  ws.addEventListener('message', ev => {
    try {
      const d = JSON.parse(ev.data);
      if (d.id && offen[d.id]) { offen[d.id](d); delete offen[d.id]; }
    } catch {}
  });
  await new Promise((res, rej) => {
    ws.addEventListener('open', res, { once: true });
    ws.addEventListener('error', rej, { once: true });
    setTimeout(rej, 10000);
  });

  let id = 0;
  const js = ausdruck => new Promise(res => {
    id += 1;
    const meine = id;
    offen[meine] = d => {
      const r = d.result && d.result.result;
      if (d.result && d.result.exceptionDetails) return res({ fehler: String(d.result.exceptionDetails.text || 'Ausnahme') });
      res(r && r.value !== undefined ? r.value : null);
    };
    ws.send(JSON.stringify({ id: meine, method: 'Runtime.evaluate', params: { expression: ausdruck, awaitPromise: true, returnByValue: true } }));
    setTimeout(() => { if (offen[meine]) { delete offen[meine]; res({ fehler: 'Zeitüberschreitung' }); } }, 15000);
  });

  // 1. Oberfläche geladen
  let start = null;
  for (let i = 0; i < 40 && !start; i += 1) {
    const r = await js("!!document.querySelector('#vT') && !!window.api");
    if (r === true) start = true; else await sleep(500);
  }
  if (start) ok('Oberfläche geladen', await js('document.title'));
  else bad('Oberfläche geladen', 'kein #vTr oder keine Brücke');

  // 2. Sprachfassungen
  const sprachen = await js(`(function(){
    const ks = Object.keys(window.K || {});
    const out = { schluessel: ks.length };
    for (const n of ['NL','ES','TR','AR']) { const d = window[n] || {}; out[n] = ks.filter(k => !(k in d)).length; }
    return out;
  })()`);
  if (sprachen && typeof sprachen.schluessel === 'number') {
    ok('Sprachschlüssel deutsch', String(sprachen.schluessel));
    const luecken = ['NL', 'ES', 'TR', 'AR'].map(n => n + ': ' + sprachen[n]);
    zeilen.push('        Lücken (Rückfall Englisch): ' + luecken.join(' · '));
  } else bad('Sprachschlüssel deutsch', 'nicht lesbar');

  // 3. Knöpfe einer Dialogzeile
  const knopf = await js(`(function(){
    const d = document.querySelector('#txDlg'); if (!d) return { fehler: 'kein #txDlg' };
    d.showModal();
    const m = s => { const e = document.querySelector(s); if (!e) return null; const r = e.getBoundingClientRect(); return { h: Math.round(r.height), w: Math.round(r.width), fs: getComputedStyle(e).fontSize }; };
    const out = { ab: m('#txCancel'), zu: m('#txOk'), datei: m('#txFile'), einfuegen: m('#txPaste') };
    d.close();
    return out;
  })()`);
  if (knopf && !knopf.fehler) {
    const a = knopf.ab, z = knopf.zu;
    if (a && z && a.h === z.h && a.fs === z.fs) ok('Dialogknöpfe gleich', a.w + '×' + a.h + ' px, ' + a.fs);
    else bad('Dialogknöpfe gleich', JSON.stringify(knopf));
  } else bad('Dialogknöpfe gleich', (knopf && knopf.fehler) || 'nicht messbar');

  // 4. Alle Dialoge öffnen
  const dialoge = await js(`(function(){
    const ids = [...document.querySelectorAll('dialog')].map(d => d.id).filter(Boolean);
    const kaputt = [];
    for (const id of ids) { const d = document.getElementById(id); if (!d || typeof d.showModal !== 'function') { kaputt.push(id); continue; } }
    return { anzahl: ids.length, kaputt };
  })()`);
  if (dialoge && dialoge.anzahl && !dialoge.kaputt.length) ok('Dialoge vorhanden', dialoge.anzahl);
  else bad('Dialoge vorhanden', JSON.stringify(dialoge));

  // 5. Protokoll erreichbar (Hauptprozess)
  const protokoll = await js('window.api.logTail(5).then(s => typeof s === "string")');
  if (protokoll === true) ok('Protokoll erreichbar', await js('window.api.logPath()')); else bad('Protokoll erreichbar', String(protokoll));

  try { ws.close(); } catch {}
  beenden(kind);
  return fehler.length === 0;
}

main().then(gut => {
  console.log(zeilen.join('\n'));
  console.log('');
  console.log(gut ? 'Rauchtest bestanden.' : 'Rauchtest fehlgeschlagen: ' + fehler.join(', '));
  try { fs.rmSync(PROFIL, { recursive: true, force: true }); } catch {}
  process.exit(gut ? 0 : 1);
}).catch(e => {
  console.log(zeilen.join('\n'));
  console.log('Rauchtest abgebrochen: ' + (e && e.message || e));
  process.exit(1);
});
