// =============================================================================
// Lesen und Vokabeln
// -----------------------------------------------------------------------------
// Drei Arbeitsplaetze teilen sich diesen Aufbau:
//   Texte/Lesen  - eigene Texte mit Textart und Titel, Leseoberflaeche mit
//                  antippbaren Woertern (Arbeitsplatz des Texteditors, also
//                  dieselbe Ablage: #vL und #vR).
//   Vokabeln     - Liste, Suche/Eintrag ohne Vorlage, CSV im Readlang-Format.
//   Ueben        - Abfrage mit Kontextsatz, gesuchtes Wort ausgeblendet, mit
//                  Spaced Repetition.
// Eine angetippte Stelle im Text fragt das eingestellte Ollama-Modell nach der
// Uebersetzung im Satzzusammenhang und legt die Vokabel mit Kontextsatz an.
// =============================================================================

const ST = { texts: [], vocab: [], cur: null, pick: false, chip: null, chipTimer: 0, para: 0 };
// Suche, Sortierung, Filter und Mehrfachauswahl der Vokabelliste. Nur fuer die
// Sitzung: nach einem Neustart steht die Liste wieder vollstaendig da.
ST.vocFind = ''; ST.vocSort = 'due'; ST.vocFilter = 'all'; ST.selMode = false; ST.sel = new Set();
const V = () => $('#vocList');
const todayISO = () => { const d = new Date(); return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`; };
const nrm = s => String(s || '').toLocaleLowerCase().trim();
const uid = p => p + Date.now().toString(36) + Math.random().toString(36).slice(2, 6);

// ---------- Vorlesen ----------
// Gesprochen wird ueber die Sprachausgabe des Systems, nicht ueber
// speechSynthesis: Chromium bringt in Electron keine Web-Speech-Stimmen mit,
// getVoices() ist dort leer und jeder Aufruf bliebe stumm (gemessen in der
// laufenden App: 0 Stimmen). Der Hauptprozess spricht deshalb ueber SAPI
// (Windows) bzw. spd-say oder espeak-ng (Linux), lokal und ohne Netz. Die
// Stimme wird nach der Sprache des Wortes gewaehlt, damit ein deutsches Wort
// nicht englisch klingt; fehlt sie, bleibt es bei der Standardstimme statt bei
// Stille. speechSynthesis bleibt nur als Rueckfall, falls eine Plattform doch
// Stimmen mitbringt.
const SPEECH_ISO = { deu:'de', eng:'en', fra:'fr', spa:'es', ita:'it', por:'pt', nld:'nl', pol:'pl', ces:'cs', swe:'sv', dan:'da', nob:'no', nor:'no', fin:'fi', tur:'tr', rus:'ru', ukr:'uk', ell:'el', ron:'ro', hun:'hu', zho:'zh', jpn:'ja', kor:'ko', ara:'ar', hin:'hi', ind:'id', vie:'vi' };
const speechLang = code => {
  const c = String(code || '').split('_')[0].toLowerCase();
  return SPEECH_ISO[c] || c || 'en';
};
// Alter Weg ueber die Web-Speech-Schnittstelle; nur noch Rueckfall.
function legacySpeak(s, lang) {
  if (!window.speechSynthesis) return false;
  try {
    speechSynthesis.cancel();
    const u = new SpeechSynthesisUtterance(s);
    u.lang = lang;
    const v = speechSynthesis.getVoices().find(x => String(x.lang || '').toLowerCase().startsWith(lang));
    if (v) u.voice = v;
    u.rate = 0.95;
    speechSynthesis.speak(u);
    return true;
  } catch { return false; }
}
// Der Hauptprozess liefert eine WAV-Datei (Piper-Stimme oder SAPI). Gespielt
// wird sie hier, damit sich dieselbe Ausgabe anhalten und an derselben Stelle
// fortsetzen laesst; ein laufender Sprechprozess koennte das nicht.
const TTS_PLAY = { el:null, key:'', paused:false };
// Zustand fuer die Knoepfe: spielt gerade, oder steht sie angehalten da?
window.ttsState = () => ({ playing: !!(TTS_PLAY.el && !TTS_PLAY.paused), paused: !!(TTS_PLAY.el && TTS_PLAY.paused) });
const ttsAnnounce = () => { try { window.dispatchEvent(new Event('tts-state')); } catch {} };
// Alles anhalten und die Wiedergabe vergessen; der naechste Klick faengt vorn an.
function ttsHalt() {
  if (TTS_PLAY.el) { try { TTS_PLAY.el.pause(); TTS_PLAY.el.src = ''; } catch {} }
  TTS_PLAY.el = null; TTS_PLAY.key = ''; TTS_PLAY.paused = false;
  try { if (window.api && window.api.ttsStop) window.api.ttsStop(); } catch {}
  ttsAnnounce();
}
window.ttsHalt = ttsHalt;
// Die eine Stelle, die ueber Systemweg und Rueckfall entscheidet. Liefert
// Promise<boolean>: true, wenn wirklich gesprochen wurde. Derselbe Text noch
// einmal geklickt haelt an, ein weiteres Mal setzt er fort.
async function speakText(text, code) {
  const s = String(text || '').trim();
  if (!s) return false;
  const lang = speechLang(code), key = lang + '|' + s;
  if (TTS_PLAY.el && TTS_PLAY.key === key) {
    if (TTS_PLAY.paused) { try { await TTS_PLAY.el.play(); } catch {} TTS_PLAY.paused = false; }
    else { TTS_PLAY.el.pause(); TTS_PLAY.paused = true; }
    ttsAnnounce();
    return true;
  }
  let r = null;
  try { if (window.api && window.api.ttsSpeak) r = await window.api.ttsSpeak(s, lang, S.ttsVoice); } catch {}
  if (r && r.ok && r.file) {
    ttsHalt();
    const a = new Audio(r.file);
    a.playbackRate = ttsRate();
    TTS_PLAY.el = a; TTS_PLAY.key = key; TTS_PLAY.paused = false;
    a.onended = () => { if (TTS_PLAY.el === a) { TTS_PLAY.el = null; TTS_PLAY.key = ''; TTS_PLAY.paused = false; ttsAnnounce(); } };
    a.onerror = () => { if (TTS_PLAY.el === a) { TTS_PLAY.el = null; TTS_PLAY.key = ''; ttsAnnounce(); } };
    try { await a.play(); } catch {}
    ttsAnnounce();
    return true;
  }
  // Rueckfall nur, wenn er wirklich Stimmen hat; sonst waere "gesprochen" eine
  // Luege (getVoices() ist in Electron leer, siehe oben).
  const voices = window.speechSynthesis ? speechSynthesis.getVoices() : [];
  return voices.length ? legacySpeak(s, lang) : false;
}
const speakVocab = e => !!e && speakText(e.term, e.src || S.studySrc);

// ---------- Ganzen Lesetext vorlesen ----------
// Gelesen wird absatzweise: ein langer Text in einem Stueck liesse lange auf den
// ersten Ton warten und ergaebe eine riesige WAV-Datei. Anhalten wirkt sofort
// auf den laufenden Absatz, Fortsetzen spielt denselben weiter.
const READ = { on:false, paused:false, stop:false, el:null, wake:null };

function readIcon() {
  const b = $('#rSpeak'); if (!b) return;
  const i = READ.on && READ.paused ? 'play' : READ.on ? 'pause' : 'volume';
  b.dataset.i = i;
  b.dataset.tt = READ.on && READ.paused ? 'resumeT' : READ.on ? 'pauseT' : 'speak';
  b.innerHTML = ic(i);
  b.title = READ.on ? t(b.dataset.tt) : (b.dataset.vn ? `${t('speak')} · ${b.dataset.vn}` : t('speak'));
}
// Verlassen des Lesens, neuer Text, anderes Vorlesen: alles anhalten. Der
// Hauptprozess wird mit abgebrochen, sonst rechnet Piper weiter.
function readHalt() {
  const was = READ.on || !!READ.el || !!READ.wake;
  READ.stop = true; READ.on = false; READ.paused = false;
  const a = READ.el; READ.el = null;
  if (a) { try { a.pause(); a.src = ''; } catch {} }
  const w = READ.wake; READ.wake = null;
  if (w) w();
  if (was) { try { if (window.api && window.api.ttsStop) window.api.ttsStop(); } catch {} }
  readIcon();
}
window.ttsReadHalt = readHalt;
// Das Warten auf das Fortsetzen; geloest wird es beim Fortsetzen oder Abbruch.
const readHold = () => new Promise(res => { READ.wake = res; });
// Ein Tempowechsel am Regler gilt sofort, auch fuer das, was gerade laeuft.
window.ttsRateChanged = () => {
  if (TTS_PLAY.el) TTS_PLAY.el.playbackRate = ttsRate();
  if (READ.el) READ.el.playbackRate = ttsRate();
};

async function readPart(txt, lang) {
  let r = null;
  try { if (window.api && window.api.ttsSpeak) r = await window.api.ttsSpeak(txt, lang, S.ttsVoice); } catch {}
  if (READ.stop) return false;
  if (!r || !r.ok || !r.file) return false;
  while (READ.paused && !READ.stop) await readHold();
  if (READ.stop) return false;
  const a = new Audio(r.file);
  a.playbackRate = ttsRate();
  READ.el = a;
  await new Promise(res => { a.onended = res; a.onerror = res; a.play().catch(res); });
  if (READ.el === a) READ.el = null;
  return !READ.stop;
}

// Von der Stelle an, die gerade oben steht: wer mitten im Text ist, will nicht
// jedes Mal von vorn hoeren.
async function readStart(tx) {
  const art = $('#reader'), wrap = $('#readerWrap');
  if (!art || !tx) return;
  const ps = [...art.querySelectorAll('p')];
  const parts = ps.map(p => String(p.dataset.txt || p.textContent || '').trim());
  if (!parts.length) return;
  const wt = wrap ? wrap.getBoundingClientRect().top : 0;
  let from = ps.findIndex(p => p.getBoundingClientRect().bottom > wt);
  if (from < 0) from = 0;
  const lang = speechLang(tx.src || S.studySrc);
  // Eine laufende Einzelausgabe (Uebersetzung, Vokabel) darf nicht dazwischenreden.
  ttsHalt();
  READ.on = true; READ.paused = false; READ.stop = false;
  readIcon();
  for (let i = from; i < parts.length && !READ.stop; i++) {
    if (!parts[i]) continue;
    if (!await readPart(parts[i], lang)) break;
  }
  if (!READ.stop) readHalt();
}

function readToggle() {
  READ.paused = !READ.paused;
  if (READ.paused) { if (READ.el) { try { READ.el.pause(); } catch {} } }
  else if (READ.el) { try { READ.el.play(); } catch {} }
  else { const w = READ.wake; READ.wake = null; if (w) w(); }
  readIcon();
}

// Der Knopf erscheint nur, wenn ein eingerichtetes Modell die Sprache dieses
// Textes spricht; ohne Modell gibt es nichts zu hoeren und deshalb keinen Knopf.
// Der Tooltip nennt die Stimme, die dann sprechen wuerde (main.js: ttsVoiceFor).
async function updRSpeak(tx) {
  const b = $('#rSpeak'); if (!b || !tx) return;
  const id = tx.id, lang = speechLang(tx.src || S.studySrc);
  let vs = [];
  try { vs = (await loadVoices()) || []; } catch {}
  if (!ST.cur || ST.cur.id !== id) return;
  const pick = (S.ttsVoice && vs.find(v => v.id === S.ttsVoice && v.lang === lang)) || vs.find(v => v.lang === lang) || null;
  b.dataset.vn = pick ? `${langLabel(lang)} · ${pick.name}` : '';
  b.classList.toggle('hidden', !pick);
  if (!pick) readHalt();
  readIcon();
}

// Textarten: gespeichert wird der Schluessel, angezeigt die Sprache der App.
const CATS = [['news', 'Nachrichten', 'News'], ['story', 'Erzählung', 'Story'], ['nonfiction', 'Sachtext', 'Non-fiction'], ['dialog', 'Dialog', 'Dialogue'], ['poem', 'Gedicht', 'Poem'], ['other', 'Sonstiges', 'Other']];
const catLabel = k => { const c = CATS.find(x => x[0] === k); return c ? (LANG === 'de' ? c[1] : c[2]) : (k || ''); };

// ---------- Ablage ----------
let persistTimer = 0;
function persist(which) {
  clearTimeout(persistTimer);
  persistTimer = setTimeout(() => {
    window.api.studySave('texts', ST.texts).catch(() => {});
    window.api.studySave('vocab', ST.vocab).catch(() => {});
  }, 400);
}

// ---------- Modell und Uebersetzung ----------
// Wortuebersetzungen laufen ueber denselben Weg wie die grossen Uebersetzungen.
// Ist gerade ein reines Uebersetzungsmodell (NLLB, TranslateGemma) gewaehlt,
// wird das Hilfsmodell aus den Einstellungen genommen.
function studyModel() {
  // Eigenes Modell fuer Wortuebersetzungen, sonst das des Uebersetzers
  // (`S.model`, die Leiste im Uebersetzer-Arbeitsplatz).
  if (S.studyModel) return S.studyModel;
  if (!S.model) return '';
  if (window.trIsStudyless && window.trIsStudyless()) return window.trHelperModel() || '';
  return S.model;
}

// Welches Modell gerade die angetippten Woerter uebersetzt, steht als
// Prozessor-Symbol in den Kopfzeilen von Lesen, Vokabeln und Leseoberflaeche
// ([data-study-model]); der Modellname steht im Tooltip.
function renderStudyModel() {
  const eff = studyModel();
  document.querySelectorAll('[data-study-model]').forEach(b => {
    b.innerHTML = ic('cpu');
    b.title = `${t('studyModelT')}: ${eff ? shortModel(eff) : '—'}` +
      (S.storyOn ? `\n${t('storyModelT')}: ${storyModel() ? shortModel(storyModel()) : '—'}` : '');
  });
}

// Modellwahl des Lese-Arbeitsplatzes. Oben, womit Woerter uebersetzt werden;
// darunter - nur wenn der Kurzgeschichten-Generator an ist - das Schreibmodell
// fuer die Geschichten. Dieselbe Aufteilung wie im Texteditor.
async function studyModelPop(btn) {
  closePops();
  const p = $('#pop');
  p.innerHTML = '';
  // Das Menue geht sofort auf; die Aussprache-Gruppe kommt nach. Wird es bis
  // dahin geschlossen, landet sie nicht mehr darin (renderer.js: POP_SEQ).
  const seq = POP_SEQ;
  const head = k => { const h = document.createElement('div'); h.className = 'row hd'; h.textContent = t(k); p.append(h); };
  const row = (label, value, cur, icons, apply) => {
    const r = document.createElement('div');
    r.className = 'row' + (cur ? ' sel' : '');
    r.innerHTML = `<span>${esc(label)}</span><span>${icons || ''}</span>`;
    r.onclick = () => { apply(); save(); closePops(); };
    p.append(r);
  };
  const setWord = v => { S.studyModel = v; wordCache.clear(); renderStudyModel(); };
  head('studyModelT');
  row(t('studyModelAuto'), '', !S.studyModel, '', () => setWord(''));
  for (const o of MODEL_OPTS) row(o.label, o.value, o.value === S.studyModel, modelIcons(o), () => setWord(o.value));
  if (S.storyOn) {
    p.append(document.createElement('hr'));
    head('writeModel');
    const gens = storyModels();
    row(t('storyModelAuto'), '', !S.storyModel || !gens.includes(S.storyModel), '', () => { S.storyModel = ''; });
    // Angezeigt wird der kurze Name (renderer.js: shortModel), gewaehlt wird die
    // volle Kennung - sonst stuende hier "hf.co/<nutzer>/..." wie frueher.
    gens.forEach(n => row(shortModel(n), n, !!S.storyModel && n === S.storyModel, '', () => { S.storyModel = n; }));
  }
  const r = btn.getBoundingClientRect();
  Object.assign(p.style, { left: Math.max(8, Math.min(r.left, innerWidth - 320)) + 'px', top: r.bottom + 6 + 'px' });
  p.classList.remove('hidden');
  // Die Vorlesestimme und das Tempo stehen im selben Menue, aber unter einem
  // Trennstrich: sie uebersetzen nichts und gehoeren nicht zur Wortuebersetzung.
  // Markiert wird die Stimme, die diesen Text jetzt wirklich lesen wuerde.
  const g = await ttsMenuGroup(speechLang((ST.cur && ST.cur.src) || S.studySrc));
  if (seq !== POP_SEQ) return;
  p.append(g);
}
const langName = c => (!c || c === 'auto') ? 'the source language' : (window.trEngName ? window.trEngName(c) : c);

async function llmJson(prompt, purpose = 'study-word') {
  const mdl = studyModel();
  if (!mdl) throw new Error('nomodel');
  const out = await window.trChat([
    { role: 'system', content: 'You are a bilingual dictionary. Reply with JSON only, no explanations.' },
    { role: 'user', content: prompt }
  ], { model: mdl, format: 'json', purpose });
  try { return JSON.parse(out); } catch { return {}; }
}

// Sprachenpaar: Jedes Paar fuehrt seine eigenen Texte und Vokabeln. Die Auswahl
// steht oben in beiden Vokabel-Arbeitsplaetzen und in der Ablage; sie ist eine
// gemeinsame Einstellung, damit ein Wechsel ueberall gleichzeitig gilt.
const curPair = () => ({ src: S.studySrc, tgt: S.studyTgt });
const pairOf = (o, d) => ({ src: (o && o.src) || (d || curPair()).src, tgt: (o && o.tgt) || (d || curPair()).tgt });
const samePair = (a, b) => a.src === b.src && a.tgt === b.tgt;
const inPair = o => samePair(pairOf(o), curPair());

const wordCache = new Map();

// Das Modell haengt gern dieselbe Bedeutung mehrfach an ("Minuten / Minuten /
// Minuten"). Doppelte Teile fallen weg, damit sie nicht im Eintrag landen; ein
// echter Unterschied wie "der See / die See" bleibt stehen.
function cleanTranslation(tr) {
  const parts = String(tr || '').split(/\s*\/\s*/).map(s => s.trim()).filter(Boolean);
  const out = [], seen = new Set();
  for (const p of parts) {
    const k = nrm(p);
    if (seen.has(k)) continue;
    seen.add(k);
    out.push(p);
  }
  return out.join(' / ');
}

// Alle KI-Funktionen der App haengen am softwareweiten Schalter `aiAll`
// (Abschnitte 131 und 138): Wortuebersetzung und Kurzgeschichten sind keine
// Sonderfaelle mehr. Ist er aus, wird kein Modell gefragt; `aioff` traegt die
// Meldung nach aussen. Der Schalter selbst heisst neutral `aiAllOn()` und steht
// zentral in renderer.js - er gilt fuer die ganze App, nicht fuer einen
// einzelnen Arbeitsplatz.

async function translateTerm(term, ctx, pair, single) {
  if (!aiAllOn()) throw new Error('aioff');
  const P = pair || curPair();
  const key = nrm(term) + '|' + nrm(ctx) + '|' + P.src + '>' + P.tgt + (single ? '|one' : '');
  if (wordCache.has(key)) return wordCache.get(key);
  const prompt =
    `Word or phrase: "${term}"\n` +
    (ctx ? `Sentence it appears in: ${ctx}\n` : '') +
    `Translate it from ${langName(P.src)} into ${langName(P.tgt)}, exactly as used in the sentence.\n` +
    'Return JSON with these keys:\n' +
    (single
      ? '  "translation": the one translation that fits this sentence, no alternatives\n'
      : '  "translation": the translation (up to three equivalent meanings separated by " / ", each different - never the same word twice)\n') +
    '  "lemma": the dictionary form\n' +
    '  "pos": part of speech in the target language\n' +
    `  "gender": "m", "f", "n", or "" - grammatical gender of the translation, if ${langName(P.tgt)} marks gender\n` +
    '  "article": the definite article of the translation if the target language has one, otherwise ""';
  const j = await llmJson(prompt);
  const r = {
    translation: cleanTranslation(String(j.translation || '').trim()),
    lemma: String(j.lemma || term).trim(),
    pos: String(j.pos || '').trim(),
    gender: String(j.gender || '').trim(),
    article: String(j.article || '').trim()
  };
  if (!r.translation) throw new Error('empty');
  wordCache.set(key, r);
  return r;
}

// ---------- Vokabeln ----------
function findVocab(term, pair) {
  const n = nrm(term), P = pair || curPair();
  return ST.vocab.find(v => nrm(v.term) === n && samePair(pairOf(v, P), P));
}

function addVocab(term, res, ctx, tx, pair) {
  const t0 = String(term || '').trim();
  if (!t0 || !res) return null;
  const P = pair || (tx ? pairOf(tx) : curPair());
  let e = findVocab(t0, P);
  if (!e) {
    e = {
      id: uid('v'), term: t0, translation: res.translation || '', lemma: res.lemma || t0,
      pos: res.pos || '', gender: res.gender || '', article: res.article || '',
      alternatives: [], context: '', textId: tx ? tx.id : '', textTitle: tx ? tx.title : '',
      src: P.src, tgt: P.tgt,
      created: Date.now(), level: 0, interval: 0, due: todayISO()
    };
    ST.vocab.unshift(e);
  }
  if (!e.translation && res.translation) e.translation = res.translation;
  if (!e.gender && res.gender) { e.gender = res.gender; e.article = res.article || ''; }
  if (!e.pos && res.pos) e.pos = res.pos;
  if (!e.src) e.src = P.src;
  if (!e.tgt) e.tgt = P.tgt;
  if (ctx && !e.context) e.context = ctx;
  persist('vocab');
  renderVocab();
  if (tx) {
    if (ST.cur && ST.cur.id === tx.id && S.view === 'read') readerMeta(ST.cur);
    renderTexts();
  }
  return e;
}

function dropVocab(id) {
  const i = ST.vocab.findIndex(v => v.id === id);
  if (i < 0) return;
  ST.vocab.splice(i, 1);
  persist('vocab');
  renderVocab();
}

const genderMark = e => e.gender === 'm' ? 'm' : e.gender === 'f' ? 'f' : e.gender === 'n' ? 'n' : '';
// Loesung mit Artikel, aber nie doppelt: steht der Artikel schon in der
// Uebersetzung (aeltere Eintraege, oder wer ihn frueher mitgetippt hat), wird
// er nicht noch einmal davorgesetzt.
function solutionOf(e) {
  const tr = String(e.translation || '').trim(), a = String(e.article || '').trim();
  if (!tr) return '';
  if (a && nrm(tr).startsWith(nrm(a) + ' ')) return tr;
  return [a, tr].filter(Boolean).join(' ');
}

function vocRow(e, opts) {
  const g = genderMark(e);
  const picked = ST.selMode && ST.sel.has(e.id);
  const ctx = e.context ? String(e.context) : '';
  const masked = ctx && e.term ? esc(ctx).replace(new RegExp(`(${esc(e.term).replace(/[.*+?^${}()|[\]\\]/g, '\\$&')})`, 'giu'), '<mark>$1</mark>') : '';
  // In der Auswahl traegt die Zeile links ein Kaestchen; Bearbeiten und
  // Loeschen je Zeile treten zurueck, damit ein Klick die Zeile anwaehlt.
  const box = ST.selMode ? `<span class="pickbox">${picked ? ic('check', 13) : ''}</span>` : '';
  const acts = ST.selMode ? '' : `<button class="rm spk" data-say="${e.id}" title="${esc(t('speak'))}">${ic('volume', 15)}</button>
    <button class="rm ed" data-edit="${e.id}" title="${esc(t('edit'))}">${ic('pencil', 15)}</button>
    <button class="rm" data-del="${e.id}" title="${esc(t('remove'))}">${ic('x', 15)}</button>`;
  return `<div class="vrow${ST.selMode ? ' selmode' : ''}${picked ? ' picked' : ''}" data-id="${e.id}"${ST.selMode ? ` data-sel="${e.id}"` : ''}>
    ${box}
    <div class="vmain">
      <div class="vterm">${g ? `<span class="vg">${g}</span>` : ''}${esc(e.term)}${isMastered(e) ? `<span class="vok" title="${esc(t('srsMastered'))}">${ic('know', 13)}</span>` : ''}<span class="vtr">${esc(e.translation)}</span></div>
      ${masked ? `<div class="vctx">${masked}</div>` : ''}
      <div class="vmeta">${e.textTitle ? esc(e.textTitle) : ''}${e.interval ? ` · ${e.interval} ${LANG === 'de' ? 'Tage' : 'days'}` : ''}${opts && opts.due ? ` · ${esc(e.due || '')}` : ''}</div>
    </div>
    ${acts}
  </div>`;
}

// Was die Liste zeigt: Paar, Filter, Suche und Reihenfolge.
function vocVisible() {
  let list = ST.vocab.filter(inPair);
  const f = ST.vocFilter;
  if (f === 'due') list = list.filter(e => (e.due || todayISO()) <= todayISO());
  else if (f === 'mastered') list = list.filter(isMastered);
  else if (f === 'open') list = list.filter(e => !isMastered(e));
  else if (f === 'ctx') list = list.filter(e => String(e.context || '').trim());
  const q = nrm(ST.vocFind);
  if (q) list = list.filter(e => [e.term, e.translation, e.lemma, e.context, e.textTitle].some(x => nrm(x).includes(q)));
  const s = ST.vocSort;
  if (s === 'az') list.sort((a, b) => a.term.localeCompare(b.term, LANG));
  else if (s === 'za') list.sort((a, b) => b.term.localeCompare(a.term, LANG));
  else if (s === 'new') list.sort((a, b) => (Number(b.created) || 0) - (Number(a.created) || 0));
  else list.sort((a, b) => (a.due || '').localeCompare(b.due || '') || a.term.localeCompare(b.term, LANG));
  return list;
}

function renderVocab() {
  const box = V(); if (!box) return;
  const mine = ST.vocab.filter(inPair);
  const list = vocVisible();
  box.innerHTML = list.map(e => vocRow(e, { due: true })).join('');
  const other = ST.vocab.length - mine.length;
  $('#vocCount').textContent = (mine.length ? t('vocCount', mine.length) : '') + (other > 0 ? ` · ${t('vocOther', other)}` : '');
  $('#vocEmpty').classList.toggle('hidden', mine.length > 0);
  box.classList.toggle('hidden', mine.length === 0);
  // Filter und Suche duerfen ins Leere laufen; das soll nicht wie "keine
  // Vokabeln" aussehen.
  const hit = $('#vocNoHit');
  if (hit) hit.classList.toggle('hidden', !(mine.length > 0 && list.length === 0));
  vocSelSync();
  // Lupe und Trichter zeigen wie die anderen Knoepfe, ob sie gerade wirken.
  const fb = $('#vocFilterBtn');
  if (fb) fb.classList.toggle('on', ST.vocFilter !== 'all');
  const fw = $('#vocFindWrap'), sb = $('#vocFindBtn');
  const finding = !!fw && !fw.classList.contains('hidden');
  if (sb) sb.classList.toggle('on', finding);
  // Immer nur ein Feld an derselben Stelle: die Suche ersetzt die Eingabe, die
  // Auswahl blendet sie ebenfalls aus. Die Werkzeuge rechts bleiben stehen.
  const aw = $('#vocAddWrap');
  if (aw) aw.classList.toggle('hidden', finding || !!ST.selMode);
}
window.studyVocRender = renderVocab;

// ---------- Sortierung, Filter und Mehrfachauswahl ----------
function vocFilterPop(btn) {
  closePops();
  const p = $('#pop');
  p.innerHTML = '';
  const head = txt => {
    const h = document.createElement('div');
    h.className = 'ph'; h.textContent = txt;
    p.append(h);
  };
  const row = (label, cur, set) => {
    const r = document.createElement('div');
    r.className = 'row' + (cur ? ' sel' : '');
    r.innerHTML = `<span>${esc(label)}</span><span>${cur ? ic('check', 15) : ''}</span>`;
    r.onclick = () => { set(); closePops(); renderVocab(); };
    p.append(r);
  };
  head(t('vocSortH'));
  row(t('vocSortAz'), ST.vocSort === 'az', () => { ST.vocSort = 'az'; });
  row(t('vocSortZa'), ST.vocSort === 'za', () => { ST.vocSort = 'za'; });
  row(t('vocSortNew'), ST.vocSort === 'new', () => { ST.vocSort = 'new'; });
  row(t('vocSortDue'), ST.vocSort === 'due', () => { ST.vocSort = 'due'; });
  head(t('vocFilterH'));
  row(t('vocFAll'), ST.vocFilter === 'all', () => { ST.vocFilter = 'all'; });
  row(t('vocFDue'), ST.vocFilter === 'due', () => { ST.vocFilter = 'due'; });
  row(t('vocFOpen'), ST.vocFilter === 'open', () => { ST.vocFilter = 'open'; });
  row(t('vocFMastered'), ST.vocFilter === 'mastered', () => { ST.vocFilter = 'mastered'; });
  row(t('vocFCtx'), ST.vocFilter === 'ctx', () => { ST.vocFilter = 'ctx'; });
  const r = btn.getBoundingClientRect();
  Object.assign(p.style, { left: Math.max(8, Math.min(r.left, innerWidth - 300)) + 'px', top: r.bottom + 6 + 'px' });
  p.classList.remove('hidden');
}

function vocSelSync() {
  const btn = $('#vocSelBtn');
  if (btn) {
    btn.classList.toggle('on', !!ST.selMode);
    btn.title = t(ST.selMode ? 'vocSelEnd' : 'vocSelectT');
  }
  const bar = $('#vocSelBar');
  if (bar) bar.classList.toggle('hidden', !ST.selMode);
  const n = ST.sel.size;
  // Auch bei null eine Zahl zeigen: eine Leiste, die nur aus vier Symbolen
  // besteht, sieht aus wie eine zufaellige Knopfreihe.
  const c = $('#vocSelCount'); if (c) c.textContent = t('vocSelN', n);
  const del = $('#vocSelDelBtn'); if (del) del.disabled = !n;
  const rp = $('#vocSelRepeatBtn'); if (rp) rp.disabled = !n;
  const all = $('#vocSelAllBtn');
  if (all) {
    // Der Knopf schaltet um: alles auswaehlen oder alles abwaehlen. Der Tooltip
    // sagt, was der naechste Klick tut; vorher stand dort immer "Alle auswaehlen".
    const ids = vocVisible().map(e => e.id);
    all.disabled = ids.length === 0;
    all.title = t(ids.length && ids.every(id => ST.sel.has(id)) ? 'vocSelNone' : 'vocSelAll');
  }
}

function vocSelMode(on) {
  ST.selMode = typeof on === 'boolean' ? on : !ST.selMode;
  if (!ST.selMode) ST.sel.clear();
  renderVocab();
}

// Nur die eine Zeile umschalten statt die ganze Liste neu zu bauen: das haelt
// die Klicks schnell und die uebrigen Zeilen bleiben stehen.
function vocSelToggle(id) {
  if (!id) return;
  const on = !ST.sel.has(id);
  if (on) ST.sel.add(id); else ST.sel.delete(id);
  const row = document.querySelector(`#vocList .vrow[data-id="${id}"]`);
  if (row) {
    row.classList.toggle('picked', on);
    const box = row.querySelector('.pickbox');
    if (box) box.innerHTML = on ? ic('check', 13) : '';
  }
  vocSelSync();
}

function vocSelAll() {
  const ids = vocVisible().map(e => e.id);
  if (ids.length && ids.every(id => ST.sel.has(id))) ST.sel.clear();
  else ids.forEach(id => ST.sel.add(id));
  renderVocab();
}

// Mehrere auf einmal loeschen: fragt einmal nach und raeumt dann alle weg.
async function vocSelDelete() {
  const n = ST.sel.size;
  if (!n) return;
  const ok = await ask(t('vocSelDelQ', n), t('yes'), t('cancel'));
  if (!ok) return;
  ST.vocab = ST.vocab.filter(v => !ST.sel.has(v.id));
  ST.sel.clear();
  persist('vocab');
  renderVocab(); renderTexts();
  if (ST.cur) readerMeta(ST.cur);
}

// ---------- Auswahlmodus im Uebersetzer ----------
// Fuer editor.js: Ist der Lernen-Arbeitsplatz abgeschaltet, wird der
// Auswahlmodus beendet; der Knopf verschwindet mit dem Arbeitsplatz.
window.studyPickOff = () => { if (!ST.pick) return; ST.pick = false; pickSync(); };

function pickSync() {
  const layer = $('#pickLayer');
  if (!layer) return;
  const live = ST.pick && S.view === 'tr';
  layer.classList.toggle('hidden', !live);
  // Der Knopf steht fest im Fuss des Eingabefelds; seit dem Wegfall der
  // Vokabelspalte wird er nicht mehr versteckt, nur noch hervorgehoben.
  const btn = $('#pickBtn');
  if (btn) btn.classList.toggle('on', ST.pick);
  if (live) drawPickLayer();
}

// Der Auswahlmodus legt eine mitlaufende Ebene ueber das Eingabefeld. Sie zeigt
// denselben Text in derselben Schrift, aber als einzelne Woerter; das
// Textfeld bleibt darunter fuer Tastatur und Cursor zustaendig.
function drawPickLayer() {
  const layer = $('#pickLayer'), ta = $('#input');
  layer.innerHTML = '';
  const txt = ta.value;
  // Ohne Text zeigt die Ebene sonst eine leere weisse Flaeche: kein Hinweis,
  // was hier zu tun ist, und der Knopf unten war nicht mehr zu sehen.
  if (!txt.trim()) {
    const h = document.createElement('span');
    h.className = 'pickhint';
    h.textContent = t('pickHint');
    layer.append(h);
    return;
  }
  const RE = /([\p{L}\p{M}]+(?:['’\-][\p{L}\p{M}]+)*)/gu;
  let last = 0, m;
  while ((m = RE.exec(txt))) {
    if (m.index > last) layer.append(txt.slice(last, m.index));
    const s = document.createElement('span');
    s.className = 'w'; s.dataset.w = m[0]; s.dataset.o = String(m.index);
    s.textContent = m[0];
    layer.append(s);
    last = m.index + m[0].length;
  }
  layer.append(txt.slice(last));
  layer.scrollTop = ta.scrollTop;
}

// ---------- Sprachenpaar ----------
// Die Auswahl steht oben in beiden Vokabel-Arbeitsplaetzen und in der Ablage.
// Sie liegt in S.studySrc/S.studyTgt und gilt daher ueberall gleichzeitig: Wer
// die Sprache in „Ueben“ umstellt, sieht in „Vokabeln“ dasselbe Paar.
// Die linke Pille ist die gelernte Sprache, die rechte die schon bekannte.
// Das sagt der Doktorhut bzw. der Haken im Kreis; der Tooltip nennt es ganz.
function pairHtml() {
  return `<button class="pill" data-pair="src" title="${esc(t('learnLang'))}">${ic('learn', 15)}<span>${esc(langLabel(S.studySrc))}</span></button>` +
    `<button class="ib" data-pair="swap" title="${esc(t('swap'))}">${ic('swap', 15)}</button>` +
    `<button class="pill" data-pair="tgt" title="${esc(t('nativeLang'))}">${ic('know', 15)}<span>${esc(langLabel(S.studyTgt))}</span></button>`;
}
function renderPairs() {
  for (const id of ['#vocPair', '#srsPair', '#libPair']) {
    const el = $(id);
    if (el) el.innerHTML = pairHtml();
  }
  document.querySelectorAll('.pairbar [data-pair]').forEach(b => { b.onclick = () => pairClick(b); });
  renderStudyModel();
}
function pairClick(b) {
  const k = b.dataset.pair;
  if (k === 'swap') {
    const a = S.studySrc;
    S.studySrc = S.studyTgt; S.studyTgt = a;
    save(); pairChanged();
    return;
  }
  langPop(b, k === 'src' ? 'studySrc' : 'studyTgt', {
    langs: OLLAMA_LANGS, noAuto: true, pins: () => S.pins.ollama,
    isSrc: k === 'src', done: pairChanged
  });
}
function pairChanged() {
  // Beide Seiten gleich ergibt keine Uebersetzung; die Muttersprache weicht aus.
  if (S.studySrc === S.studyTgt) S.studyTgt = S.studySrc === 'en' ? 'de' : 'en';
  save();
  renderPairs(); renderVocab(); renderTexts();
  if (S.view === 'srs') { srsShow = false; typeQueue = []; srsQueue = ST.vocab.filter(e => inPair(e) && (e.due || todayISO()) <= todayISO()); renderSrs(); }
}

// ---------- Texte ----------
function renderTexts() {
  const grid = $('#txtGrid'); if (!grid) return;
  grid.querySelectorAll('.txtcard').forEach(c => c.remove());
  const frag = document.createDocumentFragment();
  const mine = ST.texts.filter(inPair);
  const other = ST.texts.length - mine.length;
  for (const tx of mine) {
    // Eine Kurzgeschichte entsteht erst im Hintergrund: Solange sie geschrieben
    // wird (pending 1), steht die Vorgabe in der Vorschau und der Zustand an der
    // Stelle der Textart. Nach einem Fehler (pending -1) schreibt ein Klick auf
    // die Karte neu.
    const state = Number(tx.pending) || 0;
    const card = document.createElement('div');
    card.className = 'libcard txtcard' + (state ? ' pending' : '') + (state < 0 ? ' failed' : ''); card.dataset.tx = tx.id; card.tabIndex = 0;
    const excerpt = String(state ? (tx.brief || '') : (tx.text || '')).slice(0, 420);
    // Die Textart steht im Kopf der Karte bei Titel und Sprachpaar – dort sucht
    // man sie. Unten links in der Vorschau war sie kaum zu lesen und wurde
    // ohnehin uebersehen.
    const badge = state > 0 ? `<span class="txbadge work"><span class="spin"></span>${esc(t('stWorking'))}</span>`
      : state < 0 ? `<span class="txbadge bad">${esc(t('stFailed'))}</span>`
      : `<span class="txbadge">${esc(catLabel(tx.cat))}</span>`;
    const sub = state < 0 ? t('stRetry') : t('vocCount', ST.vocab.filter(v => v.textId === tx.id).length);
    card.innerHTML = `<div class="libprev"><div class="doc">${esc(excerpt)}</div></div>
      <div class="libmeta">${badge}<div class="libname">${esc(tx.title)}</div><div class="libdate">${esc(langLabel(tx.src))} → ${esc(langLabel(tx.tgt))} · ${esc(sub)}</div></div>
      ${state ? '' : `<button class="rm ed" title="${esc(t('edit'))}">${ic('pencil', 13)}</button>`}
      <button class="rm" title="${esc(t('remove'))}">${ic('x', 14)}</button>`;
    card.onclick = ev => {
      if (ev.target.closest('.rm') || state > 0) return;
      if (state < 0) return storyGenerate(tx.id);
      openText(tx.id);
    };
    card.onkeydown = ev => { if (ev.key === 'Enter' || ev.key === ' ') { ev.preventDefault(); card.click(); } };
    const edBtn = card.querySelector('.rm.ed');
    if (edBtn) edBtn.onclick = ev => { ev.stopPropagation(); txEdit(tx.id); };
    card.querySelector('.rm:not(.ed)').onclick = ev => { ev.stopPropagation(); dropText(tx.id); };
    frag.append(card);
  }
  grid.prepend(frag);
  const hint = $('#txOther');
  if (hint) hint.textContent = other > 0 ? t('vocOther', other) : '';
  const empty = $('#txtEmpty');
  if (empty) empty.classList.toggle('hidden', mine.length > 0);
  renderStoryBtn();
}
window.studyTextsRender = renderTexts;

function fillCats() {
  const sel = $('#txCat'); if (!sel) return;
  sel.innerHTML = CATS.map(c => `<option value="${c[0]}">${esc(LANG === 'de' ? c[1] : c[2])}</option>`).join('');
}

// ---------- Text hinzufuegen ----------
// Ein Dialog sammelt alles, was ein neuer Lesetext braucht: den Text selbst
// (eingefuegt, aus der Zwischenablage oder aus einer Datei), den Titel und die
// Textart. Der Titel wird beim Waehlen einer Datei vorbelegt, solange der
// Nutzer ihn nicht selbst angefasst hat.
let txAutoTitle = '', txEditId = null;

// Derselbe Dialog legt an und bearbeitet. Im Bearbeiten-Modus heissen
// Ueberschrift und Knopf anders, damit der Klick sagt, was er tut.
function txMode(edit) {
  const d = $('#txDlg'); if (!d) return;
  const h = d.querySelector('.dh h2');
  if (h) { h.dataset.t = edit ? 'txEditH' : 'txAdd'; h.textContent = t(edit ? 'txEditH' : 'txAdd'); }
  const ok = $('#txOk');
  if (ok) { ok.dataset.t = edit ? 'txSave' : 'txOkL'; ok.textContent = t(edit ? 'txSave' : 'txOkL'); }
}

function txOpen() {
  const d = $('#txDlg'); if (!d) return;
  txEditId = null;
  $('#txTitle').value = ''; $('#txBody').value = '';
  txAutoTitle = '';
  const cat = $('#txCat'); if (cat) cat.value = 'other';
  txMode(false);
  d.showModal();
  $('#txBody').focus();
}

// Bearbeiten eines Lesetexts: Titel, Textart und der ganze Text stehen im
// selben Dialog wie beim Anlegen, nur vorbefuellt.
function txEdit(id) {
  const tx = ST.texts.find(x => x.id === id); if (!tx) return;
  const d = $('#txDlg'); if (!d) return;
  txEditId = id;
  txAutoTitle = '';
  $('#txTitle').value = tx.title || '';
  $('#txBody').value = tx.text || '';
  const cat = $('#txCat'); if (cat) cat.value = tx.cat || 'other';
  txMode(true);
  d.showModal();
  $('#txTitle').focus();
  $('#txTitle').select();
}

function txSuggestTitle(title) {
  const inp = $('#txTitle'); if (!inp) return;
  const t0 = String(title || '').trim();
  if (inp.value.trim() && inp.value !== txAutoTitle) return;
  inp.value = t0; txAutoTitle = t0;
}

function txAppend(text) {
  const body = $('#txBody'); if (!body) return;
  const add = String(text || '').trim();
  if (!add) return;
  body.value = body.value.trim() ? body.value.trim() + '\n\n' + add : add;
  body.focus();
}

function txAdd() {
  const body = $('#txBody'), d = $('#txDlg');
  const text = body ? String(body.value).trim() : '';
  if (!text) return window.trStatus(t('txNeedText'));
  const title = ($('#txTitle').value || '').trim() || t('txUntitled');
  const cat = ($('#txCat') && $('#txCat').value) || 'other';
  // Bearbeiten: derselbe Text bleibt es, nur Titel, Art und Inhalt aendern
  // sich; die Vokabeln haengen an der ID und bleiben erhalten.
  if (txEditId) {
    const id = txEditId; txEditId = null;
    const tx = ST.texts.find(x => x.id === id);
    if (tx) {
      tx.title = title; tx.cat = cat; tx.text = text;
      tx.pos = 0; tx.anchor = null; tx.updated = Date.now();
      persist('texts');
      renderTexts();
      if (ST.cur && ST.cur.id === tx.id) { readHalt(); drawReader(); }
    }
    if (d) d.close();
    return window.trStatus(t('txSaved'));
  }
  ST.texts.unshift({ id: uid('t'), title, cat, text, pos: 0, src: S.studySrc, tgt: S.studyTgt, created: Date.now(), updated: Date.now() });
  persist('texts');
  renderTexts();
  if (d) d.close();
  window.trStatus(t('txtAdded'));
}

function dropText(id) {
  const tx = ST.texts.find(x => x.id === id); if (!tx) return;
  ask(t('txDelQ'), t('yes'), t('cancel')).then(ok => {
    if (!ok) return;
    ST.texts = ST.texts.filter(x => x.id !== id);
    ST.vocab = ST.vocab.filter(v => v.textId !== id);
    persist('texts'); persist('vocab');
    renderTexts(); renderVocab();
  });
}

// ---------- Leseoberflaeche ----------
function openText(id) {
  ST.cur = ST.texts.find(x => x.id === id) || null;
  // Eine Geschichte, die noch geschrieben wird, hat keinen Text zum Lesen.
  if (!ST.cur || Number(ST.cur.pending)) return;
  readHalt();
  // `showView` liegt in der IIFE von editor.js; nach aussen reicht sie nur als
  // window.edShowView - der direkte Name ist hier nicht sichtbar.
  window.edShowView('read');
  drawReader();
}

const SENT = /[^.!?…]+[.!?…]+["'»”)]?\s*|[^.!?…]+$/g;
function sentenceAt(txt, o) {
  SENT.lastIndex = 0;
  let m;
  while ((m = SENT.exec(txt))) {
    if (m.index <= o && o <= m.index + m[0].length) return m[0].trim();
    if (m.index > o) break;
  }
  return String(txt).trim();
}

function drawReader() {
  const tx = ST.cur; if (!tx) return;
  $('#rTitle').textContent = tx.title;
  readerMeta(tx);
  updRSpeak(tx);
  const art = $('#reader');
  art.innerHTML = '';
  const paras = String(tx.text || '').split(/\r?\n\s*\r?\n|\r?\n/).filter(p => p.trim());
  const RE = /([\p{L}\p{M}]+(?:['’\-][\p{L}\p{M}]+)*)/gu;
  paras.forEach((p, pi) => {
    const el = document.createElement(pi === 0 ? 'p' : 'p');
    el.dataset.txt = p;
    let last = 0, m;
    RE.lastIndex = 0;
    while ((m = RE.exec(p))) {
      if (m.index > last) el.append(p.slice(last, m.index));
      const s = document.createElement('span');
      s.className = 'rw'; s.dataset.w = m[0]; s.dataset.o = String(m.index); s.dataset.p = String(pi);
      s.textContent = m[0];
      el.append(s);
      last = m.index + m[0].length;
    }
    el.append(p.slice(last));
    art.append(el);
  });
  const end = document.createElement('div');
  end.className = 'rend';
  end.innerHTML = `<div class="muted">${esc(t('readEnd'))}</div>
    <div class="rendbtns"><button class="primary" id="rendVoc">${esc(t('readToVoc'))}</button>
    <button class="ghost" id="rendLib">${esc(t('readToLib'))}</button></div>`;
  art.append(end);
  end.querySelector('#rendVoc').onclick = () => { savePos(); window.edShowView('voc'); };
  end.querySelector('#rendLib').onclick = () => { savePos(); window.edShowView('txt'); };
  // Zurueck an die Stelle, an der zuletzt gelesen wurde. Der Bruchteil allein
  // reicht nicht mehr, seit Absaetze ausserhalb des Bildes nur geschaetzt hoch
  // sind (content-visibility): die Lage wird ueber den Absatz gesucht und in
  // einigen Bildern nachgezogen, bis die geschaetzten Hoehen durch echte
  // ersetzt sind.
  const wrap = $('#readerWrap');
  if (wrap && tx.anchor && Number.isInteger(tx.anchor.i) && tx.anchor.i >= 0) {
    const a = tx.anchor, to = Number(a.d) || 0;
    const put = () => {
      const p = art.querySelectorAll('p')[a.i]; if (!p) return;
      const d = p.getBoundingClientRect().top - wrap.getBoundingClientRect().top - to;
      if (Math.abs(d) >= 1) wrap.scrollTop += d;
    };
    // Sofort setzen (sonst blitzt der Text oben auf) und dann in einigen Bildern
    // nachziehen, bis die geschaetzten Hoehen durch echte ersetzt sind.
    put();
    let n = 0;
    const settle = () => { put(); if (++n < 6) requestAnimationFrame(settle); };
    requestAnimationFrame(settle);
  } else if (wrap) {
    wrap.scrollTop = (Number(tx.pos) || 0) * Math.max(1, wrap.scrollHeight - wrap.clientHeight);
  }
}

// Zahl der Vokabeln dieses Textes; wird nach jedem neuen Wort aufgefrischt,
// damit der Kopf nicht bei 0 stehen bleibt.
function readerMeta(tx) {
  const el = $('#rMeta'); if (!el || !tx) return;
  el.textContent = [`${langLabel(tx.src || S.studySrc)} → ${langLabel(tx.tgt || S.studyTgt)}`, catLabel(tx.cat), t('vocCount', ST.vocab.filter(v => v.textId === tx.id).length)].join(' · ');
}

function savePos() {
  if (!ST.cur) return;
  const wrap = $('#readerWrap'); if (!wrap) return;
  const max = Math.max(1, wrap.scrollHeight - wrap.clientHeight);
  ST.cur.pos = Math.min(1, Math.max(0, wrap.scrollTop / max));
  // Dazu der Absatz, der oben steht, und sein Abstand zum oberen Rand: nur so
  // landet ein langer Text nach dem Wiederoeffnen an derselben Stelle.
  const art = $('#reader'), wt = wrap.getBoundingClientRect().top;
  const ps = art ? [...art.querySelectorAll('p')] : [];
  const i = ps.findIndex(p => p.getBoundingClientRect().bottom > wt);
  ST.cur.anchor = i < 0 ? null : { i, d: Math.round(ps[i].getBoundingClientRect().top - wt) };
  persist('texts');
}
window.studyRestore = () => { readHalt(); if (ST.cur) drawReader(); };

// ---------- Antippen im Text ----------
// Eine Vokabel bekommt eine Blase ueber dem Wort. Ein Klick darauf streicht das
// Wort durch, nimmt die Vokabel zurueck und laesst die Blase fuenf Sekunden
// stehen.
function killChip() {
  if (ST.chip) { ST.chip.remove(); ST.chip = null; }
  clearTimeout(ST.chipTimer);
}

// Fehlermeldung als Blase: bleibt vier Sekunden stehen und verschwindet dann.
function chipFail(anchor, msg) {
  showChip(anchor, msg, null, { err: true });
  clearTimeout(ST.chipTimer);
  ST.chipTimer = setTimeout(killChip, 4000);
}

// opts.busy zeigt einen Spinner statt der Uebersetzung, opts.err die Meldung.
// Das ist im Lesen die einzige Rueckmeldung: die Statuszeile #status gehoert
// zum Uebersetzer und ist dort nicht zu sehen.
function showChip(anchor, text, onClick, opts) {
  killChip();
  const c = document.createElement('div');
  c.className = 'wchip' + (opts && opts.busy ? ' busy' : '') + (opts && opts.err ? ' err' : '');
  c.innerHTML = (opts && opts.busy ? '<span class="spin"></span>' : '') + `<span>${esc(text)}</span>`;
  document.body.append(c);
  const r = anchor.getBoundingClientRect();
  const w = c.offsetWidth, h = c.offsetHeight;
  c.style.left = Math.max(10, Math.min(window.innerWidth - w - 10, r.left + r.width / 2 - w / 2)) + 'px';
  c.style.top = Math.max(10, r.top - h - 6) + 'px';
  c.onclick = onClick;
  ST.chip = c;
}

// Klick auf die Blase: das Wort wird durchgestrichen, die Blase bleibt fuenf
// Sekunden stehen und verschwindet dann; erst damit ist die Vokabel ausgetragen.
function removeWithChip(spans, entry) {
  spans.forEach(s => s.classList.add('gone'));
  const c = ST.chip; ST.chip = null;
  // Die Blase blendet aus und ist danach weg; vorher blieb sie fuenf Sekunden
  // halbdurchsichtig ueber dem Text stehen.
  if (c) { c.classList.add('going'); setTimeout(() => c.remove(), 600); }
  clearTimeout(ST.chipTimer);
  ST.chipTimer = setTimeout(() => {
    spans.forEach(s => s.classList.remove('gone'));
    if (entry) dropVocab(entry.id);
  }, 5000);
}

// Die Blase gehoert zum sichtbaren Fenster und zum Text, ueber dem sie steht:
// beim Wechsel in ein anderes Fenster, beim Minimieren und beim Scrollen
// verschwindet sie sofort, statt ueber fremdem Text haengen zu bleiben.
window.addEventListener('blur', () => killChip());
document.addEventListener('visibilitychange', () => { if (document.hidden) killChip(); });
window.addEventListener('scroll', () => killChip(), true);

// Aus welchem Paar eine angetippte Stelle uebersetzt wird: im Lesen aus dem
// geoeffneten Text, im Uebersetzer aus dessen Sprachen. Steht der Uebersetzer
// auf einer anderen Sprache als die Vokabel auswahl, wird diese mitgezogen -
// sonst waere der neue Eintrag in der Liste nicht zu sehen.
function pairForPick() {
  if (S.view === 'read' && ST.cur) return pairOf(ST.cur);
  if (S.view === 'tr') {
    const src = (S.src && S.src !== 'auto') ? S.src : S.studySrc;
    const tgt = S.tgt || S.studyTgt;
    // Steht der Uebersetzer auf derselben Sprache auf beiden Seiten (etwa
    // Quellenwahl „Auto“ mit Zielsprache Englisch bei Lernpaar Englisch →
    // Deutsch), ergibt das Paar keine Uebersetzung und der Eintrag waere in
    // keiner Liste zu sehen. Dann gilt das eingestellte Lernpaar.
    if (src && tgt && src !== tgt) return { src, tgt };
  }
  return curPair();
}
function usePair(P) {
  if (!P || !P.src || !P.tgt || P.src === P.tgt) return;
  if (samePair(P, curPair())) return;
  S.studySrc = P.src; S.studyTgt = P.tgt; save();
  renderPairs(); renderVocab(); renderTexts();
}

async function pickWord(el) {
  if (S.studyWord === false) return;
  const term = el.dataset.w || el.textContent;
  const pair = pairForPick();
  let ctx = '';
  if (S.view === 'read' && el.parentElement && el.parentElement.dataset.txt) {
    ctx = sentenceAt(el.parentElement.dataset.txt, Number(el.dataset.o) || 0);
  } else {
    ctx = sentenceAt($('#input').value || '', Number(el.dataset.o) || 0);
  }
  window.trStatus(t('wordTr'));
  showChip(el, t('wordTr'), null, { busy: true });
  let res;
  try { res = await translateTerm(term, ctx, pair); }
  catch (e) {
    const msg = e.message === 'aioff' ? t('aiOffMsg') : e.message === 'nomodel' ? t('studyNoModel') : t('wordErr');
    window.trStatus(msg);
    chipFail(el, msg);
    return;
  }
  window.trStatus('');
  if (S.view === 'tr') usePair(pair);
  const entry = addVocab(term, res, ctx, S.view === 'read' ? ST.cur : null, pair);
  const label = [res.article, res.translation].filter(Boolean).join(' ');
  showChip(el, label, () => removeWithChip([el], entry));
}

// Mehrere Woerter markieren: ganze Wendung uebersetzen (Redewendungen).
async function pickPhrase(sel) {
  if (S.studyWord === false) return;
  const phrase = sel.toString().trim().replace(/\s+/g, ' ');
  if (!phrase || phrase.split(/\s+/).length < 2) return;
  const host = sel.anchorNode && sel.anchorNode.parentElement;
  const para = host && host.closest ? host.closest('[data-txt]') : null;
  const ctx = para ? para.dataset.txt : (S.view === 'tr' ? $('#input').value : '');
  const pair = pairForPick();
  const spans = [];
  if (para) para.querySelectorAll('.rw').forEach(s => { if (sel.containsNode(s, true)) spans.push(s); });
  window.trStatus(t('wordTr'));
  const anchor0 = spans[0] || (para || document.body);
  showChip(anchor0, t('wordTr'), null, { busy: true });
  let res;
  try { res = await translateTerm(phrase, ctx, pair); }
  catch (e) {
    const msg = e.message === 'aioff' ? t('aiOffMsg') : e.message === 'nomodel' ? t('studyNoModel') : t('wordErr');
    window.trStatus(msg);
    chipFail(anchor0, msg);
    return;
  }
  window.trStatus('');
  if (S.view === 'tr') usePair(pair);
  const entry = addVocab(phrase, res, ctx, S.view === 'read' ? ST.cur : null, pair);
  const anchor = spans[0] || (para || document.body);
  showChip(anchor, res.translation, () => removeWithChip(spans, entry));
}

function bindPicking(container, selector) {
  container.addEventListener('click', e => {
    const w = e.target.closest(selector);
    if (!w) return;
    if (S.view === 'tr' && !ST.pick) return;
    const sel = window.getSelection();
    if (sel && !sel.isCollapsed && sel.toString().trim()) return;
    pickWord(w);
  });
  container.addEventListener('mouseup', e => {
    if (S.view === 'tr' && !ST.pick) return;
    const sel = window.getSelection();
    if (!sel || sel.isCollapsed) return;
    const txt = sel.toString().trim();
    if (txt.split(/\s+/).length < 2) return;
    const host = sel.anchorNode && sel.anchorNode.parentElement
      ? sel.anchorNode.parentElement.closest('#reader, #pickLayer') : null;
    if (host) pickPhrase(sel);
  });
}

// ---------- Vokabel-Arbeitsplatz ----------
// Meldungen zu Eintrag und Uebersetzung gehoeren in diesen Arbeitsplatz: Die
// Statuszeile des Uebersetzers (#status) steht in einer anderen Ansicht und
// waere hier unsichtbar.
function vocStatus(text, err) {
  const el = $('#vocMsg'); if (!el) return;
  el.textContent = text || '';
  el.classList.toggle('hidden', !text);
  el.classList.toggle('err', !!text && !!err);
}
// Lautsprecher in der Liste: Ohne Sprachausgabe sagt die Zeile ueber der Liste
// (#vocMsg), was fehlt — sie steht fest ueber dem scrollenden #vocList und ist
// damit auch bei langen Listen zu sehen. Ein Klick ohne jede Rueckmeldung liest
// sich sonst wie ein Fehler.
async function sayVocab(e) {
  if (!await speakVocab(e)) return vocStatus(t('speakNone'), true);
  if ($('#vocMsg').textContent === t('speakNone')) vocStatus('');
}

async function addFromSearch() {
  const inp = $('#vocTerm');
  const term = inp.value.trim();
  if (!term) { vocStatus(t('vocNeedTerm')); inp.focus(); return; }
  vocStatus(t('wordTr'));
  let res;
  try { res = await translateTerm(term, ''); }
  catch (e) { vocStatus(e.message === 'aioff' ? t('aiOffMsg') : e.message === 'nomodel' ? t('studyNoModel') : t('wordErr'), true); return; }
  vocStatus('');
  addVocab(term, res, '', null);
  inp.value = '';
  renderVocab();
}

// Wörter ohne Übersetzung — etwa aus einer Readlang-Datei, die nur das Wort
// kannte — trägt das Modell nach. Vorher wird gefragt, und jedes Wort geht mit
// seinem Kontextsatz mit, damit die Übersetzung zur Stelle passt. Nach jedem
// Eintrag wird gespeichert, damit ein langer Lauf nicht verloren geht.
async function fillMissingTranslations() {
  const todo = ST.vocab.filter(e => inPair(e) && !String(e.translation || '').trim());
  if (!todo.length) { vocStatus(t('vocFillNone')); return; }
  if (!aiAllOn()) { vocStatus(t('aiOffMsg'), true); return; }
  if (!studyModel()) { vocStatus(t('studyNoModel'), true); return; }
  if (!await ask(t('vocFillAsk', todo.length), t('vocFillDo'), t('cancel'))) return;
  let done = 0, fails = 0;
  for (let i = 0; i < todo.length; i++) {
    const e = todo[i];
    vocStatus(t('vocFillRun', i, todo.length));
    try {
      // Eine einzige, zum Satz passende Uebersetzung: die Liste braucht keine
      // Bedeutungsvarianten, sondern den einen Treffer.
      const r = await translateTerm(e.term, e.context || '', pairOf(e), true);
      if (!e.translation && r.translation) e.translation = r.translation;
      if (!e.gender && r.gender) { e.gender = r.gender; e.article = r.article || ''; }
      if (!e.pos && r.pos) e.pos = r.pos;
      done++;
    } catch { fails++; }
    persist('vocab');
    if (i % 5 === 4) renderVocab();
  }
  renderVocab();
  vocStatus(t('vocFillDone', done, fails));
}

// ---------- Vokabel bearbeiten ----------
// Der Dialog aendert nur die drei Texte. Paar, Intervall und Lerndaten bleiben,
// damit eine Korrektur nicht den Lernstand zuruecksetzt.
let vocEditId = '';

function vocDlgStatus(text) {
  const el = $('#vocDlgMsg'); if (!el) return;
  el.textContent = text || '';
  el.classList.toggle('hidden', !text);
  el.classList.toggle('err', !!text);
}

function vocOpenEdit(id) {
  const e = ST.vocab.find(v => v.id === id); if (!e) return;
  vocEditId = id;
  vocDlgStatus('');
  $('#vocTerm2').value = e.term || '';
  $('#vocTr2').value = e.translation || '';
  $('#vocCtx2').value = e.context || '';
  $('#vocDlg').showModal();
  $('#vocTerm2').focus(); $('#vocTerm2').select();
}

// Bestimmte Artikel der Zielsprachen mit dem Genus, das sie anzeigen. Die Karte
// setzt `e.article` selbst vor die Uebersetzung; tippt jemand den Artikel mit
// ins Feld („das Adresszeichen“), stand er danach doppelt da („die das
// Adresszeichen“), und das alte Genus blieb stehen. Deshalb loest das
// Speichern einen fuehrenden Artikel ab und uebernimmt ihn.
const ARTICLES = {
  de: [['der', 'm'], ['die', 'f'], ['das', 'n']],
  es: [['el', 'm'], ['la', 'f'], ['los', 'm'], ['las', 'f']],
  fr: [['le', 'm'], ['la', 'f'], ['les', ''], ["l'", '']],
  it: [['il', 'm'], ['lo', 'm'], ['la', 'f'], ['i', 'm'], ['gli', 'm'], ['le', 'f']],
  pt: [['o', 'm'], ['a', 'f'], ['os', 'm'], ['as', 'f']],
  nl: [['de', ''], ['het', 'n']],
  sv: [['en', ''], ['ett', 'n']],
  da: [['en', ''], ['et', 'n']], nb: [['en', ''], ['et', 'n']], nn: [['en', ''], ['et', 'n']]
};
// Zerlegt „das Adresszeichen“ in Artikel und Rest. Ohne Treffer bleibt alles,
// wie es getippt wurde; ein Artikel allein (ohne Rest) wird nie abgetrennt.
function splitArticle(tr, tgt, cur) {
  const text = String(tr || '').trim();
  const i = text.search(/\s/);
  if (i < 0) return { translation: text, article: '', gender: '' };
  const head = text.slice(0, i), rest = text.slice(i + 1).trim();
  if (!rest) return { translation: text, article: '', gender: '' };
  const low = head.toLocaleLowerCase(), c = nrm(cur);
  const known = (ARTICLES[tgt] || []).find(a => a[0] === low);
  if (!known && !(c && c === low)) return { translation: text, article: '', gender: '' };
  return { translation: rest, article: head, gender: known ? known[1] : '' };
}

function vocSaveEdit() {
  const e = ST.vocab.find(v => v.id === vocEditId);
  if (!e) { $('#vocDlg').close(); return; }
  const term = $('#vocTerm2').value.trim();
  if (!term) return vocDlgStatus(t('vocNeedTerm'));
  // Dasselbe Wort im selben Paar gibt es schon: lieber hier melden als eine
  // zweite Zeile anlegen, die die Abfrage doppelt fragt.
  const dup = ST.vocab.find(v => v.id !== e.id && nrm(v.term) === nrm(term) && samePair(pairOf(v), pairOf(e)));
  if (dup) return vocDlgStatus(t('wordExists'));
  // Das Lemma war beim Anlegen oft nur eine Kopie des Wortes; dann zieht es
  // mit. Steht dort eine eigene Grundform, bleibt sie stehen.
  const oldTerm = e.term;
  e.term = term;
  if (!e.lemma || nrm(e.lemma) === nrm(oldTerm)) e.lemma = term;
  // Getippter Artikel wandert in `article` (und setzt das Genus mit), damit die
  // Karte ihn nicht ein zweites Mal davorsetzt.
  const tgt = (pairOf(e) || {}).tgt || '';
  const sp = splitArticle($('#vocTr2').value, tgt, e.article);
  e.translation = sp.translation;
  if (sp.article) { e.article = sp.article; if (sp.gender) e.gender = sp.gender; }
  e.context = $('#vocCtx2').value.trim();
  e.updated = Date.now();
  persist('vocab');
  renderVocab();
  // Steht die Karte gerade aufgedeckt im Üben, soll die korrigierte Übersetzung
  // sofort dort stehen (srsShow bleibt gesetzt, die Karte bleibt also offen).
  if (S.view === 'srs') renderSrs();
  $('#vocDlg').close();
  vocStatus('');
}

// ---------- Spaced Repetition ----------
const LADDER = [1, 2, 4, 8, 16, 32, 64, 128, 256];
const addDays = (n) => { const d = new Date(); d.setDate(d.getDate() + n); return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`; };
function rate(e, how) {
  let lv = Number(e.level) || 0;
  if (how === 'again') { lv = 0; e.due = addDays(0); }
  else {
    if (how === 'good') lv = Math.min(LADDER.length - 1, lv + 1);
    else if (how === 'easy') lv = Math.min(LADDER.length - 1, lv + 2);
    e.due = addDays(LADDER[lv]);
  }
  e.level = lv;
  e.interval = how === 'again' ? 1 : LADDER[lv];
  e.updated = Date.now();
  persist('vocab');
}

let srsQueue = [], srsShow = false;
function maskTerm(text, term) {
  if (!text) return '';
  const re = new RegExp(term.replace(/[.*+?^${}()|[\]\\]/g, '\\$&'), 'giu');
  // Die Luecke ist ein Strich, keine Unterstrich-Zeichen: Breite nach der Laenge
  // der Loesung (ohne Leerzeichen), gedeckelt, damit lange Woerter die Zeile
  // nicht sprengen. Fuer die Sprachausgabe ist der Strich stumm (aria-hidden).
  const bar = m => {
    const w = Math.min(5, Math.max(2.2, m.replace(/\s+/g, '').length * 0.5)).toFixed(1);
    return `<span class="srsblank" style="min-width:${w}em" aria-hidden="true"></span>`;
  };
  return esc(text).replace(re, bar);
}

// ---------- Mastery-Modus: tippen ----------
// Wie bei Readlang wird die Uebersetzung getippt statt nur aufgedeckt. Drei
// richtige Antworten meistern ein Wort; gemeisterte Woerter fragt dieser Modus
// nicht mehr ab, waehrend die normalen Karten unveraendert weiterlaufen.
const MASTER = 3;
const isMastered = e => (Number(e.mastery) || 0) >= MASTER;

// Zeichen, die auf einer deutschen oder englischen Tastatur fehlen, je
// Zielsprache. Die Tipp-Abfrage bietet sie als Knoepfe an, damit niemand eine
// Tastaturbelegung wechseln muss. Leere Liste = Sprache ohne Zusatzzeichen.
const EXTRA_CHARS = {
  de: 'äöüß', fr: 'àâæçéèêëîïôœùûüÿ', es: '¿¡áéíóúüñ', it: 'àèéìíòóùú',
  pt: 'áâãàçéêíóôõú', nl: 'áéíóúäëïöü', pl: 'ąćęłńóśźż', cs: 'áčďéěíňóřšťúůýž',
  sv: 'åäö', da: 'æøå', no: 'æøå', fi: 'äö', tr: 'çğıİöşü', ro: 'ăâîșț', hu: 'áéíóöőúüű'
};
const extraChars = () => [...new Set(EXTRA_CHARS[S.studyTgt] || '')];

// Buchstaben, die NFD nicht zerlegt, obwohl sie auf einer Tastatur ohne diese
// Tasten nur als Umschreibung tippbar sind (ß, æ, ø ...).
const foldLetters = s => s
  .replace(/ß/g, 'ss').replace(/æ/g, 'ae').replace(/œ/g, 'oe').replace(/ø/g, 'o')
  .replace(/ł/g, 'l').replace(/đ|ð/g, 'd').replace(/þ/g, 'th').replace(/ı/g, 'i');

// Vergleich: Gross-/Kleinschreibung, Akzente, Satzzeichen und ein fuehrender
// Artikel duerfen abweichen; " / " trennt mehrere gueltige Loesungen, wie sie
// das Modell und der CSV-Import liefern. Frage- und Ausrufezeichen zaehlen
// nicht, also gilt ein normales "?" auch fuer das umgedrehte "¿".
const ansNorm = s => foldLetters(String(s || '').toLowerCase()
  .normalize('NFD').replace(/[\u0300-\u036f]/g, ''))
  .replace(/[.,;:!?¡¿"'„“”«»()]/g, ' ')
  .replace(/\s+/g, ' ').trim();
function answerOk(e, typed) {
  const arts = ansNorm(e.article).split(' ').filter(Boolean);
  const dropArt = x => { const w = x.split(' '); return arts.includes(w[0]) ? w.slice(1).join(' ') : x; };
  const want = [e.translation, ...(e.alternatives || [])]
    .flatMap(x => String(x || '').split('/')).map(ansNorm).filter(Boolean);
  const got = ansNorm(typed);
  if (!got) return false;
  return want.some(w => w === got || dropArt(w) === dropArt(got));
}

let typeQueue = [], srsNote = null;

// ---------- Endlos-Runde („Repeat“) ----------
// Eine Auswahl wird so lange abgefragt, bis der Nutzer aufhoert. Gefragt ist
// die Uebersetzung, getippt wird das Wort in der gelernten Sprache. Die Karten
// laufen in einer Schlange: richtig -> ein paar Karten spaeter wieder, falsch
// -> die Karte bleibt stehen, die Loesung ist zu sehen und muss erneut getippt
// werden. Je oefter eine Karte danebenging, desto kuerzer kommt sie zurueck.
// Faelligkeit und Meisterschaft bleiben unberuehrt: das ist eine Uebung, kein
// Lernstand.
let rep = null;

function repShuffle(list) {
  const a = list.slice();
  for (let i = a.length - 1; i > 0; i--) {
    const j = Math.floor(Math.random() * (i + 1));
    [a[i], a[j]] = [a[j], a[i]];
  }
  return a;
}

// Tastaturhilfen beim Tippen: Akzente duerfen fehlen (? statt ¿, ! statt ¡,
// n statt ñ), Satzzeichen stoeren nicht, Gross-/Kleinschreibung und doppelte
// Leerzeichen auch nicht - geprueft wird die Schreibweise.
//
// Zusaetzlich gelten die Umschreibungen, die in einer Sprache ueblich sind,
// wenn die Taste fehlt: im Deutschen ae/oe/ue fuer ä/ö/ü und ss fuer ß, im
// Schwedischen und Daenischen aa fuer å, im Franzoesischen oe fuer œ, im
// Tuerkischen i fuer ı (das punktlose i). Beide Formen werden verglichen,
// "für", "fuer" und "fur" sind also alle richtig, "fuehr" nicht.
const REP_ALIAS = {
  de: { 'ä':'ae', 'ö':'oe', 'ü':'ue', 'ß':'ss' },
  sv: { 'å':'aa', 'ä':'ae', 'ö':'oe' },
  da: { 'å':'aa', 'æ':'ae', 'ø':'oe' },
  no: { 'å':'aa', 'æ':'ae', 'ø':'oe' },
  fi: { 'ä':'ae', 'ö':'oe' },
  is: { 'þ':'th', 'ð':'d', 'æ':'ae', 'ö':'oe' },
  fr: { 'œ':'oe', 'æ':'ae' },
  tr: { 'ı':'i' }
};
// Satzzeichen und doppelte Leerzeichen weg - sie sind keine Schreibweise.
const repFlat = s => String(s || '')
  .replace(/[¿?¡!.,;:'"…()\[\]{}]/g, '')
  .replace(/\s+/g, ' ')
  .trim();
// Akzente und Sonderbuchstaben auf ihre Grundform (ä -> a, ß -> ss, ñ -> n).
const repFold = s => foldLetters(repFlat(nrm(s)).normalize('NFD').replace(/[\u0300-\u036f]/g, ''));
// Dieselbe Sache, aber mit den Umschreibungen der gelernten Sprache davor.
function repSpell(s) {
  const m = REP_ALIAS[S.studySrc] || {};
  const base = [...nrm(repFlat(s))].map(c => m[c] || c).join('');
  return foldLetters(base.normalize('NFD').replace(/[\u0300-\u036f]/g, ''));
}
// Zwei gleichwertige Lesarten einer Antwort; getippt und erwartet werden beide
// verglichen, damit weder die Tastaturhilfe noch die strenge Form noetig ist.
const repForms = s => [...new Set([repFold(s), repSpell(s)].filter(Boolean))];
// Die Zeichen, die auf einer deutschen Tastatur fuer die gelernte Sprache
// fehlen (Tabelle EXTRA_CHARS), als Knoepfe ueber dem Eingabefeld.
const repChars = () => [...new Set(EXTRA_CHARS[S.studySrc] || '')];
function termOk(e, typed) {
  const got = repForms(typed);
  if (!got.length) return false;
  return [e.term, e.lemma].some(w => repForms(w).some(x => got.includes(x)));
}

function startRepeat(ids) {
  const cards = (ids || [])
    .map(id => ST.vocab.find(v => v.id === id))
    .filter(e => e && solutionOf(e));
  if (!cards.length) return window.trStatus(t('repNone'));
  const order = repShuffle(cards.map(e => e.id));
  clearTimeout(repTimer);
  rep = { ids: cards.map(e => e.id), queue: order, wrongs: new Map(), seen: new Set(), streak: 0, note: null, lock: false };
  vocSelMode(false);
  window.edShowView('srs');
  renderSrs();
}

function stopRepeat() {
  clearTimeout(repTimer);
  rep = null;
  const tg = $('#srsType'); if (tg) tg.classList.remove('hidden');
  renderSrs();
}

// Bei einer Serie von mehr als fuenf richtigen Antworten kommt ein Wort nach
// vorn, das in dieser Runde noch nicht dran war.
function repPickNext() {
  if (!rep || rep.queue.length < 2 || rep.streak <= 5) return;
  const i = rep.queue.findIndex(id => !rep.seen.has(id));
  if (i > 0) { const [id] = rep.queue.splice(i, 1); rep.queue.unshift(id); }
}

// Wieder einreihen: ohne Fehler fuenf Karten spaeter, mit jedem Fehlversuch
// eine Karte frueher, spaetestens direkt hinter die naechste Karte.
function repReinsert(e) {
  const wrongs = rep.wrongs.get(e.id) || 0;
  rep.queue.splice(Math.min(Math.max(1, 4 - wrongs), rep.queue.length), 0, e.id);
}

// Die Rueckmeldung gehoert auf die Karte, die gerade beantwortet wurde. Bei
// einer richtigen Antwort bleibt die Karte deshalb kurz stehen (rund anderthalb
// Sekunden, `REP_PAUSE`), zeigt "Richtig" und geht dann von allein weiter;
// vorher sprang sie sofort um und die Meldung stand auf der naechsten Frage.
// Ein Fehlversuch bleibt wie bisher stehen, bis derselbe Eintrag getippt ist.
const REP_PAUSE = 1500;
let repTimer = null;
function repCheck() {
  if (!rep || rep.lock || !rep.queue.length) return;
  const e = ST.vocab.find(v => v.id === rep.queue[0]);
  const inp = $('#srsIn');
  if (!e || !inp || !inp.value.trim()) return;
  if (termOk(e, inp.value)) {
    rep.streak += 1;
    rep.note = { ok: true, text: t('srsRight') };
    rep.lock = true;
    renderSrs();
    clearTimeout(repTimer);
    repTimer = setTimeout(repAdvance, REP_PAUSE);
  } else {
    rep.streak = 0;
    rep.wrongs.set(e.id, (rep.wrongs.get(e.id) || 0) + 1);
    rep.note = { ok: false, text: t('srsWrong') + ' ' + e.term, retry: true };
    renderSrs();
  }
}

// Karte aus der Schlange nehmen und passend zum Fehlerstand wieder einreihen.
function repAdvance() {
  clearTimeout(repTimer); repTimer = null;
  if (!rep) return;
  const e = ST.vocab.find(v => v.id === rep.queue[0]);
  if (e) { rep.queue.shift(); repReinsert(e); }
  rep.note = null;
  rep.lock = false;
  renderSrs();
}

function renderRepeat(box) {
  // Entfernte Karten fallen aus der Runde.
  rep.queue = rep.queue.filter(id => ST.vocab.some(v => v.id === id));
  if (!rep.queue.length) { rep = null; renderSrs(); return; }
  // Waehrend der kurzen Pause nach einer richtigen Antwort bleibt die Karte
  // stehen; auch das Nachziehen ungesehener Woerter wartet so lange.
  if (!rep.lock) repPickNext();
  const e = ST.vocab.find(v => v.id === rep.queue[0]);
  rep.seen.add(e.id);
  const P = pairOf(e);
  const chars = repChars().map(ch => `<button class="ch" data-ch="${esc(ch)}">${esc(ch)}</button>`).join('');
  const note = rep.note
    ? `<div class="srsnote ${rep.note.ok ? 'ok' : 'bad'}">${esc(rep.note.text)}${rep.note.retry ? `<div class="repretry muted">${esc(t('repAgain'))}</div>` : ''}</div>`
    : '';
  box.innerHTML = `<div class="srscard typing">
    <div class="replabel muted">${esc(langLabel(P.tgt))} → ${esc(langLabel(P.src))}</div>
    <div class="srsword">${esc(solutionOf(e))}</div>
    ${chars ? `<div class="charbar above${rep.lock ? ' off' : ''}" title="${esc(t('srsChars'))}">${chars}</div>` : ''}
    <div class="srsinput${rep.lock ? ' locked' : ''}"><input id="srsIn" autocomplete="off" spellcheck="false"${rep.lock ? ' readonly' : ''} placeholder="${esc(t('repTypeIn'))}"><button class="ib go" id="srsCheckBtn"${rep.lock ? ' disabled' : ''} title="${esc(t('srsCheck'))}">${ic('check', 18)}</button></div>
    ${note}
    <div class="srsendless muted">${esc(t('repEndless'))}</div>
    <div class="srsbtns"><button class="ghost" id="repStop">${esc(t('repStop'))}</button></div>
  </div>`;
  const inp = box.querySelector('#srsIn');
  if (inp) {
    inp.addEventListener('keydown', ev => {
      if (ev.key === 'Enter') { ev.preventDefault(); repCheck(); }
      else if (ev.key === 'Escape') { ev.preventDefault(); stopRepeat(); }
    });
    inp.focus();
  }
  const btn = box.querySelector('#srsCheckBtn');
  if (btn) btn.onclick = repCheck;
  box.querySelectorAll('.ch').forEach(b => b.onclick = () => srsInsertChar(b.dataset.ch));
  const st = box.querySelector('#repStop');
  if (st) st.onclick = stopRepeat;
}

function typeFill() {
  const due = ST.vocab.filter(e => inPair(e) && !isMastered(e) && (e.due || todayISO()) <= todayISO());
  const ids = new Set(due.map(e => e.id));
  typeQueue = typeQueue.filter(id => ids.has(id));
  for (const e of due) if (!typeQueue.includes(e.id)) typeQueue.push(e.id);
}

function typeCheck() {
  const e = ST.vocab.find(v => v.id === typeQueue[0]); if (!e) return;
  const inp = $('#srsIn');
  if (!inp || !inp.value.trim()) return;
  if (answerOk(e, inp.value)) {
    e.mastery = Math.min(MASTER, (Number(e.mastery) || 0) + 1);
    rate(e, 'good');
    srsNote = { ok: true, text: t('srsRight') + (isMastered(e) ? ' · ' + t('srsMastered') : '') };
    typeQueue.shift();
  } else {
    // Falsch: Wort nach hinten, Meisterschaft zuruecksetzen, Loesung zeigen.
    e.mastery = 0;
    rate(e, 'again');
    srsNote = { ok: false, text: t('srsWrong') + ' ' + solutionOf(e) };
    typeQueue.push(typeQueue.shift());
  }
  persist('vocab');
  renderVocab();
  renderSrs();
  const next = $('#srsIn'); if (next) next.focus();
}

function renderType(box, due) {
  typeFill();
  const e = ST.vocab.find(v => v.id === typeQueue[0]);
  const note = srsNote ? `<div class="srsnote ${srsNote.ok ? 'ok' : 'bad'}">${esc(srsNote.text)}</div>` : '';
  if (!e) {
    const anyLeft = due.some(x => !isMastered(x));
    box.innerHTML = `<div class="srsempty"><p>${esc(anyLeft ? t('srsNone') : t('srsAllMastered'))}</p><p class="muted">${esc(t('srsMasteredHint'))}</p></div>${note}`;
    return;
  }
  const ctx = e.context ? maskTerm(e.context, e.term) : '';
  const chars = extraChars().map(ch => `<button class="ch" data-ch="${esc(ch)}">${esc(ch)}</button>`).join('');
  box.innerHTML = `<div class="srscard typing">
    ${ctx ? `<div class="srsctx">${ctx}</div>` : ''}
    <div class="srsword">${esc(e.term)}</div>
    <div class="srsinput"><input id="srsIn" autocomplete="off" spellcheck="false" placeholder="${esc(t('srsTypeIn'))}"><button class="ib go" id="srsCheckBtn" title="${esc(t('srsCheck'))}">${ic('check', 18)}</button></div>
    ${chars ? `<div class="charbar" title="${esc(t('srsChars'))}">${chars}</div>` : ''}
    <div class="srsmastery muted">${esc(t('srsMastered'))} ${Math.min(MASTER, Number(e.mastery) || 0)} / ${MASTER}</div>
    ${note}
  </div>`;
  const inp = box.querySelector('#srsIn');
  if (inp) {
    inp.addEventListener('keydown', ev => { if (ev.key === 'Enter') { ev.preventDefault(); typeCheck(); } });
    inp.focus();
  }
  const btn = box.querySelector('#srsCheckBtn');
  if (btn) btn.onclick = typeCheck;
  box.querySelectorAll('.ch').forEach(b => b.onclick = () => srsInsertChar(b.dataset.ch));
}

// Zeichen an der Cursorposition einfuegen und den Fokus im Feld lassen.
function srsInsertChar(ch) {
  const inp = $('#srsIn'); if (!inp || !ch) return;
  const a = inp.selectionStart == null ? inp.value.length : inp.selectionStart;
  const b = inp.selectionEnd == null ? a : inp.selectionEnd;
  inp.value = inp.value.slice(0, a) + ch + inp.value.slice(b);
  const pos = a + ch.length;
  inp.focus();
  try { inp.setSelectionRange(pos, pos); } catch { }
}

function renderSrs() {
  const box = $('#srsBox'); if (!box) return;
  // Die Endlos-Runde hat ihre eigene Anzeige: kein Faelligkeitszaehler, kein
  // Umschalter fuer den Tipp-Modus.
  if (rep) {
    const tg0 = $('#srsType');
    if (tg0) { tg0.classList.add('hidden'); tg0.classList.remove('on'); }
    $('#srsCount').textContent = t('repStreak', rep.streak);
    return renderRepeat(box);
  }
  const due = ST.vocab.filter(e => inPair(e) && (e.due || todayISO()) <= todayISO());
  $('#srsCount').textContent = due.length ? t('vocabDue', due.length) : '';
  const tg = $('#srsType');
  if (tg) { tg.classList.remove('hidden'); tg.classList.toggle('on', !!S.srsType); }
  if (S.srsType) return renderType(box, due);
  srsQueue = srsQueue.filter(inPair);
  if (!due.length) {
    box.innerHTML = `<div class="srsempty"><span class="badge">${ic('check', 22)}</span><p>${esc(t('srsNone'))}</p><p class="muted">${esc(t('srsDone'))}</p></div>`;
    return;
  }
  const e = srsQueue[0] || due[0];
  const ctx = e.context ? maskTerm(e.context, e.term) : '';
  // Ohne Kontextsatz ist das Wort selbst die Frage; es steht dann schon gross
  // ueber der Loesung, und die kleine Zeile darunter zeigt nur Lemma und
  // Wortart, statt das Wort ein zweites Mal zu nennen.
  // Manche Wörter kamen ohne Übersetzung herein (etwa aus einer Readlang-Datei,
  // die nur das Wort kannte); dann sagt die Karte das, statt leer zu bleiben.
  const sol = solutionOf(e);
  const sub = [e.lemma && e.lemma !== e.term ? esc(e.lemma) : '', e.pos ? esc(e.pos) : ''].filter(Boolean).join(' · ');
  const subLine = !sub ? (ctx ? `<div class="srsctx2 muted">${esc(e.term)}</div>` : '')
    : `<div class="srsctx2 muted">${ctx ? esc(e.term) + ' · ' : ''}${sub}</div>`;
  // Die aufgedeckte Karte traegt oben rechts Sprachausgabe und Stift; der
  // zusaetzliche Innenabstand haelt den Satz davon frei (has-acts).
  box.innerHTML = `<div class="srscard${srsShow ? ' has-acts' : ''}">
    ${ctx ? `<div class="srsctx">${ctx}</div>` : ''}
    <div class="srsword">${srsShow || !ctx ? esc(e.term) : ''}</div>
    ${srsShow ? `<div class="srsres">${e.gender ? `<span class="vg">${genderMark(e)}</span>` : ''}${sol ? esc(sol) : `<span class="muted">${esc(t('srsNoTr'))}</span>`}</div>
      ${subLine}
      <div class="srseditrow"><button class="ib srsedit" id="srsSay" title="${esc(t('speak'))}" aria-label="${esc(t('speak'))}">${ic('volume', 16)}</button><button class="ib srsedit" id="srsEdit" title="${esc(t('edit'))}" aria-label="${esc(t('edit'))}">${ic('pencil', 16)}</button></div>
      <div class="srssay hidden"></div>
      <div class="srsbtns rate">
        <button class="ghost" data-r="again">${esc(t('srsAgain'))}</button>
        <button class="ghost" data-r="hard">${esc(t('srsHard'))}</button>
        <button class="primary" data-r="good">${esc(t('srsGood'))}</button>
        <button class="ghost" data-r="easy">${esc(t('srsEasy'))}</button>
      </div>`
      : `<div class="srsbtns one"><button class="primary" id="srsReveal">${esc(t('srsReveal'))}</button></div>`}
  </div>`;
  const rv = box.querySelector('#srsReveal');
  if (rv) rv.onclick = () => { srsShow = true; renderSrs(); };
  // Die aufgedeckte Karte lässt sich korrigieren, bevor bewertet wird: falsche
  // oder fehlende Übersetzung tippt man hier nach, ohne die Runde zu verlassen.
  // Der Stift ist derselbe wie in der Vokabelliste (#vocDlg, vocOpenEdit).
  const ed = box.querySelector('#srsEdit');
  if (ed) ed.onclick = () => vocOpenEdit(e.id);
  const say = box.querySelector('#srsSay');
  // Ohne Sprachausgabe sagt die Zeile in der Karte, was fehlt: die Liste ist
  // hier nicht zu sehen und die Statuszeile des Uebersetzers liegt in einer
  // anderen Ansicht.
  if (say) say.onclick = async () => {
    const ok = await speakVocab(e);
    const m = box.querySelector('.srssay'); if (!m) return;
    m.textContent = ok ? '' : t('speakNone');
    m.classList.toggle('hidden', ok);
  };
  box.querySelectorAll('[data-r]').forEach(b => b.onclick = () => {
    rate(e, b.dataset.r);
    srsShow = false;
    srsQueue = ST.vocab.filter(x => (x.due || todayISO()) <= todayISO());
    renderSrs(); renderVocab();
  });
}
window.studySrsRender = () => {
  srsShow = false; srsNote = null; typeQueue = [];
  srsQueue = ST.vocab.filter(e => inPair(e) && (e.due || todayISO()) <= todayISO());
  renderSrs();
};

// ---------- CSV im Readlang-Format ----------
// Spalten: Wort/Wendung, Uebersetzung, Kontext, Intervall in Tagen, naechstes
// Datum. Synonyme und Alternativuebersetzungen werden mit " / " getrennt.
const csvCell = v => { const s = String(v == null ? '' : v); return /[",\r\n]/.test(s) ? '"' + s.replace(/"/g, '""') + '"' : s; };
function vocabCsv() {
  return ST.vocab.filter(inPair).map(e => {
    const cols = [e.term + (e.alternatives?.length ? ' / ' + e.alternatives.join(' / ') : ''), e.translation, e.context || ''];
    if (S.csvSrs) cols.push(String(e.interval || 0), e.due || '');
    return cols.map(csvCell).join(',');
  }).join('\r\n');
}

function parseCsv(text) {
  const rows = []; let row = [], cell = '', q = false;
  const s = String(text).replace(/^\uFEFF/, '');
  for (let i = 0; i < s.length; i++) {
    const c = s[i];
    if (q) {
      if (c === '"') { if (s[i + 1] === '"') { cell += '"'; i++; } else q = false; }
      else cell += c;
    } else if (c === '"') q = true;
    else if (c === ',') { row.push(cell); cell = ''; }
    else if (c === '\n') { row.push(cell); rows.push(row); row = []; cell = ''; }
    else if (c === '\r') { }
    else cell += c;
  }
  if (cell || row.length) { row.push(cell); rows.push(row); }
  return rows.filter(r => r.some(x => String(x).trim()));
}

// Gemeinsames Eintragen: vorhandene Woerter werden ergaenzt, nicht doppelt
// angelegt. Kontextsaetze sammeln sich an, die Uebersetzung bleibt die erste.
function mergeVocab(items) {
  const P = curPair();
  let n = 0;
  for (const it of items) {
    const term = String(it.term || '').trim();
    if (!term) continue;
    let e = findVocab(term, P);
    if (!e) {
      e = { id: uid('v'), term, translation: it.translation || '', lemma: term, pos: '', gender: '', article: '', alternatives: [], context: '', textId: '', textTitle: '', src: P.src, tgt: P.tgt, created: Date.now(), level: 0, interval: 0, due: todayISO() };
      ST.vocab.unshift(e);
    }
    e.alternatives = [...new Set([...(e.alternatives || []), ...(it.alternatives || [])].filter(Boolean))];
    if (!e.translation) e.translation = it.translation || '';
    if (it.context && !e.context) e.context = it.context;
    else if (it.context && e.context && !e.context.includes(it.context)) e.context = e.context + ' ' + it.context;
    if (S.csvSrs) {
      if (!Number.isNaN(it.interval) && it.interval > 0) { e.interval = it.interval; e.level = Math.max(0, LADDER.indexOf(it.interval)); }
      if (it.due) e.due = it.due;
    }
    n++;
  }
  persist('vocab');
  renderVocab();
  window.trStatus(t('csvDone', n));
  return n;
}

function importCsv(text) {
  const items = [];
  for (const r of parseCsv(text)) {
    const first = String(r[0] || '').trim();
    if (!first) continue;
    const terms = first.split(' / ').map(s => s.trim()).filter(Boolean);
    const trs = String(r[1] || '').split(' / ').map(s => s.trim()).filter(Boolean);
    const iv = parseInt(String(r[3] || '').trim(), 10);
    const due = /^\d{4}-\d{2}-\d{2}$/.test(String(r[4] || '').trim()) ? String(r[4]).trim() : '';
    items.push({
      term: terms[0], alternatives: [...terms.slice(1), ...trs.slice(1)],
      translation: trs[0] || '', context: String(r[2] || '').trim(),
      interval: Number.isNaN(iv) ? 0 : iv, due
    });
  }
  return mergeVocab(items);
}

// ---------- Vokabelliste aus Readlang ----------
// Readlang schreibt die Vokabeln als Textdatei, deren Spalten man im Export
// selbst zusammenstellt (z. B. Kontext mit Uebersetzung, Original, Frequenz)
// und deren Trenner Tab, Semikolon oder ein eigenes Zeichen ist. Ein Feld wird
// deshalb nach seinem Aussehen einsortiert, nicht nach seiner Position:
// [[...]] bzw. {{c1::...}} traegt den Kontext, reine Zahlen die Wortfrequenz,
// das kuerzeste uebrige Feld ist das Wort.
const RL_MARK = /\[\[(.+?)\]\]|\{\{c?\d*::(.+?)\}\}/;

// Trenner bestimmen. Anfuehrungszeichen deuten auf unsere eigene CSV-Datei.
function readlangDelim(text) {
  const s = String(text).replace(/^\uFEFF/, '').replace(/\r\n?/g, '\n');
  const lines = s.split('\n').filter(l => l.trim());
  if (!lines.length) return '';
  const share = ch => lines.filter(l => l.includes(ch)).length / lines.length;
  if (share('\t') > 0.8) return '\t';
  if (share('|') > 0.8) return '|';
  if (!s.includes('"') && share(';') > 0.8) return ';';
  return '';
}

// Nennt ein Feld das Wort (gebeugt, mit Artikel davor)? Dann ist es der Satz,
// aus dem das Wort stammt, und keine Uebersetzung. Ohne diese Pruefung landet
// bei einer Zeile ohne Markierung der ganze Satz als Entsprechung im Eintrag.
function sentenceOf(field, term) {
  if (String(field).split(/\s+/).length < 2) return false;
  const core = (String(term).toLowerCase().match(/[\p{L}]+/gu) || []).pop() || '';
  if (core.length < 3) return false;
  const esc = core.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
  return new RegExp('(^|[^\\p{L}])' + esc + '([^\\p{L}]|$)', 'iu').test(field);
}

// Listet die Vokabeln einer Readlang-Textdatei; null, wenn es keine ist.
function readlangWords(text) {
  const delim = readlangDelim(text);
  if (!delim) return null;
  const lines = String(text).replace(/^\uFEFF/, '').replace(/\r\n?/g, '\n').split('\n').filter(l => l.trim());
  const isFreq = s => /^\d+(?:[.,]\d+)?$/.test(s);
  // Readlang schreibt in die Frequenzspalte auch „undefined“, wenn es keine
  // Frequenz gibt. Das Feld ist kurz und wurde deshalb zum „Wort“: Die echte
  // Vokabel wurde zur Uebersetzung, und alle betroffenen Zeilen fielen auf
  // einen einzigen Eintrag „undefined“ zusammen.
  const isMeta = s => /^(?:undefined|null|nan)$/i.test(s);
  const out = [], seen = new Map();
  for (const line of lines) {
    const fields = line.split(delim).map(x => x.trim()).filter(Boolean);
    if (!fields.length) continue;
    let ctx = '', bracket = '', words = [];
    for (const f of fields) {
      if (isFreq(f) || isMeta(f)) continue;
      const m = f.match(RL_MARK);
      if (m && !ctx) { ctx = f; bracket = (m[1] || m[2] || '').trim(); continue; }
      if (f === 'No context') continue;
      words.push(f);
    }
    if (!words.length) continue;
    const term = words.slice().sort((a, b) => a.length - b.length)[0];
    let rest = words.filter(w => w !== term);
    // Ohne Markierung steht in der Kontextspalte der Satz selbst (etwa
    // „Tenemos un perro pequeño.“); er nennt das Wort und ist kein Equivalent.
    if (!ctx) {
      const i = rest.findIndex(w => sentenceOf(w, term));
      if (i >= 0) { ctx = rest[i]; rest = rest.slice(0, i).concat(rest.slice(i + 1)); }
    }
    let translation = bracket;
    // „Kontext mit Originalwort“ stellt genau das Wort in die Klammern; eine
    // reine Gross-/Kleinschreibung („[[Miguel]]“ zu „miguel“) bleibt erhalten.
    if (translation && translation === term) translation = '';
    if (!translation && rest.length) translation = rest[0] || '';
    let context = '';
    if (ctx) {
      // Die Klammern durch das Wort ersetzen ergibt den Satz, wie er im Text stand.
      context = ctx.replace(RL_MARK, term).replace(/^["'„“]\s*/, '').replace(/\s*["'“”]$/, '').trim();
      if (nrm(context) === nrm(term)) context = '';
    }
    const key = nrm(term);
    const prev = seen.get(key);
    if (prev) {
      if (!prev.translation && translation) prev.translation = translation;
      if (!prev.context && context) prev.context = context;
      continue;
    }
    const e = { term, translation, context };
    seen.set(key, e);
    out.push(e);
  }
  return out;
}

// Der Import nimmt beide Formen: die Textdatei aus Readlang und unsere CSV.
function importVocab(text) {
  const words = readlangWords(text);
  if (words) return words.length ? mergeVocab(words) : 0;
  return importCsv(text);
}

// ---------- Texte als Datei ----------
function textsJson() {
  return JSON.stringify({
    app: 'uebersetzer', kind: 'texts', version: 1,
    items: ST.texts.map(t => ({ title: t.title, cat: t.cat, text: t.text, pos: t.pos || 0, created: t.created }))
  }, null, 2);
}
function importTexts(text) {
  let data;
  try { data = JSON.parse(text); } catch { return window.trStatus(t('txtErr')); }
  const items = Array.isArray(data) ? data : (data.items || []);
  let n = 0;
  for (const it of items) {
    const body = String(it.text || '').trim();
    if (!body) continue;
    if (ST.texts.some(x => x.title === it.title && x.text === body)) continue;
    ST.texts.unshift({ id: uid('t'), title: String(it.title || 'Text').slice(0, 200), cat: it.cat || 'other', text: body, pos: Number(it.pos) || 0, created: it.created || Date.now(), updated: Date.now() });
    n++;
  }
  persist('texts');
  renderTexts();
  window.trStatus(t('txtDone', n));
}

// ---------- Aus der Dokumentenablage in die Lesetexte ----------
// Ein Dokument der Ablage laesst sich in die Lese-Bibliothek einer Sprache
// kopieren; das Original bleibt in der Ablage unberuehrt. Die Kopie traegt das
// Sprachpaar, damit sie im Lese-Arbeitsplatz des passenden Paares erscheint.
function htmlToPlain(html) {
  // Blockenden werden zu Zeilenumbruechen, bevor der Text aus dem DOM kommt.
  const s = String(html || '')
    .replace(/<\/(p|h[1-6]|li|blockquote|div|tr|figure|section)>/gi, '\n')
    .replace(/<br\s*\/?>/gi, '\n');
  const d = document.createElement('div');
  d.innerHTML = s;
  return String(d.textContent || '')
    .replace(/\r/g, '')
    .replace(/[ \t]+\n/g, '\n')
    .replace(/\n{3,}/g, '\n\n')
    .trim();
}

function libToReading(doc, lang) {
  const text = htmlToPlain(doc && doc.html);
  if (!text) return window.trStatus(t('txNeedText'));
  const src = lang || S.studySrc;
  // Bekannte Sprache bleibt; ist sie dieselbe wie die gelernte, muss eine
  // andere her, sonst filtert `inPair` den Text aus jeder Liste heraus.
  let tgt = S.studyTgt;
  if (tgt === src) tgt = LANG !== src ? LANG : (src === 'en' ? 'de' : 'en');
  ST.texts.unshift({
    id: uid('t'), title: String((doc && doc.name) || t('txUntitled')).slice(0, 200),
    cat: 'other', text, pos: 0, src, tgt, created: Date.now(), updated: Date.now()
  });
  persist('texts');
  // Auf das Paar der Kopie umstellen, damit der Text gleich in der Leseliste steht.
  if (src !== S.studySrc || tgt !== S.studyTgt) { S.studySrc = src; S.studyTgt = tgt; save(); pairChanged(); }
  renderTexts();
  // In die Leseliste wechseln: dort steht die Kopie sofort sichtbar, waehrend
  // die Statuszeile des Uebersetzers in der Ablage unsichtbar waere.
  window.edShowView('txt');
  window.trStatus(t('libToReadDone', `${langLabel(src)} → ${langLabel(tgt)}`));
}

// Wird vom Kartenknopf in der Ablage gerufen (editor.js): erst die Sprache
// waehlen, dann kopieren. Rechnet das Lesen aus, nicht das Paar, damit ein
// unbekannter Text nicht als gelernte Sprache gilt.
window.studyLibToReading = async (id, btn) => {
  let doc = null;
  try { doc = await window.api.libRead(id); } catch { }
  if (!doc) return window.trStatus(t('readErr'));
  const pick = { src: S.studySrc };
  langPop(btn, 'src', {
    obj: pick, langs: OLLAMA_LANGS, noAuto: true,
    pins: () => S.pins.ollama, isSrc: true,
    done: () => libToReading(doc, pick.src)
  });
};

// ---------- Kurzgeschichten ----------
// Der Generator baut aus einem Lueckentext und einigen Woertern eine Vorgabe und
// laesst ein Schreibmodell daraus eine kurze Geschichte in der gelernten Sprache
// schreiben. Damit die Wartezeit nicht im Dialog haengt, legt der Start sofort
// einen Lesetext mit `pending` an: Die Bibliothek zeigt ihn als „wird
// geschrieben“, und sobald das Modell antwortet, steht dort eine normale Karte
// mit Titel und Text.

// Die Luecken der Vorgabe. Die Woerter sind hier ausgesucht, nicht vom Modell:
// Jede Luecke hat ihre eigene kleine Auswahl mit passendem Zeichen (Cowboy ->
// Hut, Wueste -> Kaktus, Pirat -> Totenkopf). Angezeigt wird die Spalte der
// bekannten Sprache (rechte Pille des Paares, sonst Englisch); die Vorgabe fuer
// das Modell entsteht daraus auf Englisch.
//   [Schluessel, Beschriftung, [[deutsch, englisch, Icon], ...]]
const ST_SLOTS = [
  ['fig', 'stSlotFig', [
    ['einen Piraten', 'a pirate', 'sgPirate'],
    ['einen Cowboy', 'a cowboy', 'sgCowboy'],
    ['eine Ritterin', 'a knight', 'sgKnight'],
    ['einen Astronauten', 'an astronaut', 'sgAstro'],
    ['eine Hexe', 'a witch', 'sgWitch'],
    ['einen Detektiv', 'a detective', 'sgDetective'],
    ['einen Drachen', 'a dragon', 'sgDragon'],
    ['einen Roboter', 'a robot', 'sgRobot'],
    ['eine Prinzessin', 'a princess', 'sgCrown'],
    ['eine Seefahrerin', 'a sailor', 'sgAnchor']
  ]],
  ['place', 'stSlotPlace', [
    ['in der Wüste', 'in the desert', 'sgDesert'],
    ['auf dem Mond', 'on the moon', 'sgMoon'],
    ['im tiefen Wald', 'in a deep forest', 'sgForest'],
    ['auf einer Insel', 'on an island', 'sgIsland'],
    ['in einem alten Schloss', 'in an old castle', 'sgCastle'],
    ['in der Großstadt', 'in a big city', 'sgCity'],
    ['unter Wasser', 'under water', 'sgSea'],
    ['in den Bergen', 'in the mountains', 'sgMountains'],
    ['im Dschungel', 'in the jungle', 'sgLeaf'],
    ['auf einem Bauernhof', 'on a farm', 'sgBarn']
  ]],
  ['mate', 'stSlotMate', [
    ['ein Papagei', 'a parrot', 'sgParrot'],
    ['ein Pferd', 'a horse', 'sgHorse'],
    ['eine Katze', 'a cat', 'sgCat'],
    ['ein Hund', 'a dog', 'sgDog'],
    ['ein Riese', 'a giant', 'sgGiant'],
    ['ein Gespenst', 'a ghost', 'sgGhost'],
    ['ein Bär', 'a bear', 'sgBear'],
    ['eine Maus', 'a mouse', 'sgMouse'],
    ['ein Frosch', 'a frog', 'sgFrog'],
    ['eine Eule', 'an owl', 'sgOwl']
  ]],
  // Die Handlung bleibt bewusst allgemein: Jede Wendung muss nach „Doch dann“
  // mit jeder Figur, jedem Schauplatz und jedem Begleiter funktionieren, ohne
  // Ort, Jahreszeit oder Zauberei vorauszusetzen.
  ['turn', 'stSlotTurn', [
    ['geht etwas schief', 'something goes wrong', 'sgWarn'],
    ['kommt alles anders als gedacht', 'everything turns out differently', 'sgTwist'],
    ['ist der Weg versperrt', 'the way is blocked', 'sgDoor'],
    ['bleibt nur noch wenig Zeit', 'there is little time left', 'sgClock'],
    ['wird etwas Wichtiges gefunden', 'something important is found', 'sgTreasure'],
    ['kommt jemand Fremdes dazu', 'a stranger joins in', 'sgFootprints'],
    ['muss ein Rätsel gelöst werden', 'a riddle must be solved', 'sgRiddle'],
    ['gibt es ein Missverständnis', 'there is a misunderstanding', 'sgChat'],
    ['zieht ein Sturm auf', 'a storm comes up', 'sgStorm'],
    ['geschieht etwas Wunderbares', 'something wonderful happens', 'sgSparkle']
  ]],
  // Der Schluss bleibt offen: den schreibt das Modell, damit die Geschichte
  // beim Lesen ueberrascht. Statt seiner waehlt man einen Gegenstand.
  ['item', 'stSlotItem', [
    ['ein Schlüssel', 'a key', 'sgKey'],
    ['ein Brief', 'a letter', 'sgLetter'],
    ['eine Landkarte', 'a map', 'sgMap'],
    ['ein Spiegel', 'a mirror', 'sgMirror'],
    ['eine Laterne', 'a lantern', 'sgLantern'],
    ['ein Buch', 'a book', 'sgBook'],
    ['ein Amulett', 'an amulet', 'sgAmulet'],
    ['eine Flöte', 'a flute', 'sgFlute'],
    ['ein Kompass', 'a compass', 'sgCompass'],
    ['ein Schwert', 'a sword', 'sgSword']
  ]]
];
// Der sichtbare Lueckentext: Textstuecke und Luecken im Wechsel. Ausformuliert
// sind Deutsch und Englisch; andere Anzeigesprachen nehmen den englischen Satz,
// die Woerter selbst bleiben in der bekannten Sprache.
// Luecken stehen als Objekte im Rahmen (`{ s: 'fig' }`), damit sie nicht mit
// Textstuecken verwechselt werden.
const ST_FRAME = {
  de: ['Eine Kurzgeschichte über ', { s: 'fig' }, ', die ', { s: 'place' }, ' spielt. Mit dabei: ', { s: 'mate' }, '. Doch dann ', { s: 'turn' }, '. Am Ende taucht ', { s: 'item' }, ' auf.'],
  en: ['A short story about ', { s: 'fig' }, ', set ', { s: 'place' }, '. Along for the ride: ', { s: 'mate' }, '. But then ', { s: 'turn' }, '. In the end, ', { s: 'item' }, ' appears.']
};
// Bezeichnungen der Luecken in der englischen Vorgabe fuer das Modell.
const ST_RECIPE = { fig: 'Main character', place: 'Setting', mate: 'Companion', turn: 'What happens', item: 'Object' };
// Laenge der Geschichte: Wortzahl und Absaetze, die das Modell anstreben soll.
// Kurz ist die fruehere Fassung; mittel und lang geben dem Text mehr Raum.
const ST_LEN = { short: [120, 180, 2, 3], mid: [250, 350, 3, 5], long: [450, 650, 6, 9] };
const storyLen = () => ST_LEN[S.storyLen] ? S.storyLen : 'mid';
const storyFrame = () => ST_FRAME[LANG] || ST_FRAME.en;
const slotDef = k => ST_SLOTS.find(s => s[0] === k);
const wordCol = () => S.studyTgt === 'de' ? 0 : 1;
let storySel = {}, storySlot = 'fig';

// Schreibmodell: eigene Wahl, sonst das des Texteditors, sonst das Hilfsmodell -
// dieselbe Reihenfolge wie im Editor (`writeModel`).
function storyModels() { return Object.keys(caps).filter(n => caps[n].includes('completion') && !/translategemma/i.test(n)); }
function storyModel() {
  const gens = storyModels();
  if (S.storyModel && gens.includes(S.storyModel)) return S.storyModel;
  if (S.edWrite && gens.includes(S.edWrite)) return S.edWrite;
  return window.trHelperModel() || gens[0] || '';
}
// Der Lueckentext im Dialog: Textstuecke und Luecken im Wechsel. Eine Luecke ist
// entweder leer (gestrichelt, mit ihrer Bezeichnung) oder gefuellt (Zeichen und
// Wort). Ein Klick macht die Luecke aktiv; rechts stehen dann ihre Woerter.
function drawStoryCloze() {
  const box = $('#stCloze'); if (!box) return;
  box.innerHTML = '';
  // Die Beschriftung steht in der Karte, damit ihre Oberkante mit der
  // Wortliste rechts auf einer Hoehe liegt.
  const cap = document.createElement('div');
  cap.className = 'stframehead';
  cap.textContent = t('stFrameL');
  box.append(cap);
  for (const part of storyFrame()) {
    if (typeof part === 'string') { box.append(part); continue; }
    const d = slotDef(part.s); if (!d) continue;
    const sel = storySel[part.s];
    const b = document.createElement('button');
    b.type = 'button';
    b.className = 'stslot' + (sel ? ' filled' : '') + (storySlot === part.s ? ' on' : '');
    b.dataset.slot = part.s;
    b.title = t(d[1]);
    b.innerHTML = sel
      ? `${ic(sel[2], 15)}<span>${esc(sel[wordCol()])}</span>`
      : `<span class="ph">${esc(t(d[1]))}</span>`;
    b.onclick = () => { storySlot = part.s; drawStoryCloze(); drawStoryPicks(); };
    box.append(b);
  }
  storySync();
}

// Jedes Wort nur einmal: Steht es schon in einer anderen Luecke, ist es hier
// gesperrt - sonst entsteht „Doch dann kommt ein Brief an. Am Ende taucht ein
// Brief auf.“
function storyTaken(k, o) {
  return ST_SLOTS.some(([key]) => key !== k && storySel[key] && storySel[key][1] === o[1]);
}

// Die Wortoptionen der aktiven Luecke: Pillen mit dem Zeichen des Wortes, in der
// Reihenfolge der Liste - nichts wird umsortiert.
function drawStoryPicks() {
  const head = $('#stPickHead'), box = $('#stPicks');
  const d = slotDef(storySlot); if (!head || !box || !d) return;
  head.textContent = t(d[1]);
  // Kein `data-i` fuer den Index: das ist in dieser App die Icon-Angabe, ein
  // spaeterer Icon-Durchlauf wuerde den Pilleninhalt damit ueberschreiben.
  box.innerHTML = d[2].map((o, i) => {
    const taken = storyTaken(d[0], o);
    return `<button type="button" class="wpill${storySel[d[0]] === o ? ' on' : ''}${taken ? ' taken' : ''}" data-opt="${i}"${taken ? ' disabled' : ''}>${ic(o[2], 16)}<span>${esc(o[wordCol()])}</span></button>`;
  }).join('');
  box.querySelectorAll('.wpill').forEach((b, i) => { b.onclick = () => storyPickWord(d[0], i); });
}

// Nach dem Setzen springt die naechste offene Luecke nach vorn, damit man der
// Reihe nach durchkommt.
function storyPickWord(k, i) {
  const d = slotDef(k); if (!d || !d[2][i]) return;
  if (storyTaken(k, d[2][i])) return;
  storySel[k] = d[2][i];
  const order = ST_SLOTS.map(s => s[0]);
  const rest = order.slice(order.indexOf(k) + 1).concat(order.slice(0, order.indexOf(k)));
  storySlot = rest.find(x => !storySel[x]) || k;
  drawStoryCloze(); drawStoryPicks();
}

// Die gewaehlte Laenge hervorheben; sie gilt fuer die naechste Geschichte.
function drawStoryLen() {
  const cur = storyLen();
  document.querySelectorAll('#stLen .stseg').forEach(b => {
    const on = b.dataset.len === cur;
    b.classList.toggle('on', on);
    b.setAttribute('aria-pressed', String(on));
  });
}

// Zaehler und Schreibknopf: erst wenn jede Luecke belegt ist, geht es los.
function storySync() {
  const c = $('#stCount'), go = $('#stGo');
  const n = ST_SLOTS.filter(s => storySel[s[0]]).length;
  if (c) c.textContent = t('stFill', n, ST_SLOTS.length);
  if (go) go.disabled = n < ST_SLOTS.length;
}

// Der Lueckentext als ein Satz - steht so auf der Karte, solange geschrieben wird.
function storySentence() {
  return storyFrame().map(p => typeof p === 'string' ? p : (storySel[p.s] ? storySel[p.s][wordCol()] : t(slotDef(p.s)[1]))).join('');
}

function storyOpen() {
  if (!S.storyOn) return;
  const d = $('#stDlg'); if (!d) return;
  storySel = {};
  storySlot = ST_SLOTS[0][0];
  const msg = $('#stMsg'); if (msg) { msg.textContent = ''; msg.classList.add('hidden', 'err'); }
  drawStoryCloze();
  drawStoryPicks();
  drawStoryLen();
  d.showModal();
  // Der Fokus liegt auf der ersten Lücke: Enter oeffnet gleich ihre Wortliste,
  // und der Modellknopf traegt keinen Fokusrahmen beim Aufgehen.
  const first = $('#stCloze .stslot'); if (first) first.focus();
}

// Die Karte in der Bibliothek, solange geschrieben wird. Fußt auf denselben
// Feldern wie jeder Lesetext; `pending` steuert nur die Darstellung.
function storyStart() {
  if (ST_SLOTS.some(s => !storySel[s[0]])) return;
  const msg = $('#stMsg');
  if (!aiAllOn()) {
    if (msg) { msg.textContent = t('aiOffMsg'); msg.classList.remove('hidden'); msg.classList.add('err'); }
    return;
  }
  if (!storyModel()) {
    if (msg) { msg.textContent = t('stNoModel'); msg.classList.remove('hidden'); msg.classList.add('err'); }
    return;
  }
  // Die Vorgabe fuer das Modell entsteht auf Englisch: Stichworte und Anweisung
  // stehen dann in einer Sprache, und geschrieben wird trotzdem in der gelernten.
  const recipe = ST_SLOTS.map(([k]) => `- ${ST_RECIPE[k]}: ${storySel[k][1]}`).join('\n');
  const tx = {
    id: uid('t'), title: t('stUntitled'), cat: 'story', text: '', pos: 0,
    src: S.studySrc, tgt: S.studyTgt, created: Date.now(), updated: Date.now(),
    pending: 1, brief: storySentence(), recipe, len: storyLen()
  };
  ST.texts.unshift(tx);
  persist('texts');
  storySel = {};
  renderTexts();
  $('#stDlg').close();
  storyGenerate(tx.id);
}

async function storyGenerate(id) {
  const tx = ST.texts.find(x => x.id === id); if (!tx) return;
  tx.pending = 1;
  persist('texts'); renderTexts();
  const lang = tx.src;
  try {
    if (!aiAllOn()) throw new Error('aioff');
    const mdl = storyModel();
    if (!mdl) throw new Error('nomodel');
    const [lo, hi, p1, p2] = ST_LEN[tx.len] || ST_LEN.mid;
    const out = await window.trChat([
      { role: 'system', content: 'You write short stories for language learners. Reply with JSON only, without markdown or explanations.' },
      { role: 'user', content:         `Write a short story in ${langName(lang)}. Language code: ${lang}.\n` +
        `- Write the whole story in ${langName(lang)}; the recipe below is in English only as a guide.\n` +
        `- Length: about ${lo} to ${hi} words in ${p1} to ${p2} short paragraphs; keep sentences short and the vocabulary everyday.\n` +
        `- Follow this recipe and keep the main character, the setting and the object as given.\n${tx.recipe || '- (your choice)'}\n` +
        `- Surprise the reader with the ending; do not give it away early.\n` +
        `- Give the story a fitting title in ${langName(lang)}.\n` +
        `Reply as JSON: {"title": "...", "story": "..."}` }
    ], { model: mdl, format: 'json', purpose: 'story' });
    let title = '', story = '';
    try {
      const j = JSON.parse(out);
      title = String(j.title || '').trim();
      story = String(j.story || j.text || '').trim();
    } catch { story = String(out || '').trim(); }
    if (!story) throw new Error('empty');
    if (!title) title = story.split(/[.!?\n]/)[0];
    tx.title = (title || t('stUntitled')).replace(/\s+/g, ' ').slice(0, 120).trim();
    tx.text = story.replace(/\r/g, '').replace(/\n{3,}/g, '\n\n').trim();
    tx.pending = 0;
  } catch {
    tx.pending = -1;
  }
  tx.updated = Date.now();
  persist('texts'); renderTexts();
  // Keine Meldung im Arbeitsplatz: Die Karte in der Ablage zeigt den Fortschritt
  // („wird geschrieben“, „nicht fertig geschrieben“) und danach Titel und Text.
  if (ST.cur && ST.cur.id === tx.id && !Number(tx.pending)) drawReader();
}

// Der Knopf in der Lesebibliothek gehoert zum Generator: ausgeschaltet ist er weg.
// Der Modellknopf im Kopf nennt das Schreibmodell nur im Tooltip, solange der
// Generator laeuft - deshalb hier gleich mit auffrischen.
function renderStoryBtn() {
  const b = $('#txStory'); if (!b) return;
  b.classList.toggle('hidden', !S.storyOn || !aiAllOn());
  renderStudyModel();
}
window.studyStorySync = renderStoryBtn;

// Texte des Generators in allen sechs Sprachen. Sie werden hier nachgetragen,
// weil Renderer und Editor geladen sind, wenn dieses Skript laeuft.
const STORY_STR = {
  storyT: ['Kurzgeschichte schreiben', 'Write a short story', 'Kort verhaal schrijven', 'Escribir un relato corto', 'Kısa hikâye yaz', 'كتابة قصة قصيرة'],
  storyH: ['Kurzgeschichte', 'Short story', 'Kort verhaal', 'Relato corto', 'Kısa hikâye', 'قصة قصيرة'],
  storyModelT: ['Schreibmodell für Kurzgeschichten', 'Writing model for short stories', 'Schrijfmodel voor korte verhalen', 'Modelo de escritura para relatos', 'Kısa hikâye yazma modeli', 'نموذج كتابة القصص'],
  storyModelAuto: ['Standard (Schreibmodell des Editors)', 'Default (editor writing model)', 'Standaard (schrijfmodel van de editor)', 'Predeterminado (modelo del editor)', 'Varsayılan (düzenleyicinin yazma modeli)', 'افتراضي (نموذج المحرر)'],
  stFrameL: ['Vorgabe', 'Recipe', 'Opzet', 'Guion', 'Tarif', 'الوصفة'],
  stSlotFig: ['Figur', 'Main character', 'Hoofdpersoon', 'Protagonista', 'Ana karakter', 'الشخصية الرئيسية'],
  stSlotPlace: ['Schauplatz', 'Setting', 'Plaats', 'Lugar', 'Mekân', 'المكان'],
  stSlotMate: ['Begleiter', 'Companion', 'Metgezel', 'Acompañante', 'Yol arkadaşı', 'الرفيق'],
  stSlotTurn: ['Handlung', 'What happens', 'Wat gebeurt er', 'Qué pasa', 'Ne oluyor', 'ما الذي يحدث'],
  stSlotItem: ['Gegenstand', 'Object', 'Voorwerp', 'Objeto', 'Nesne', 'الشيء'],
  stHint: ['Wähle zu jeder Lücke ein Wort.', 'Pick a word for every gap.', 'Kies bij elk gat een woord.', 'Elige una palabra para cada hueco.', 'Her boşluk için bir kelime seç.', 'اختر كلمة لكل فراغ.'],
  stLenL: ['Länge', 'Length', 'Lengte', 'Longitud', 'Uzunluk', 'الطول'],
  stLenShort: ['Kurz', 'Short', 'Kort', 'Corto', 'Kısa', 'قصيرة'],
  stLenMid: ['Mittel', 'Medium', 'Middel', 'Medio', 'Orta', 'متوسطة'],
  stLenLong: ['Lang', 'Long', 'Lang', 'Largo', 'Uzun', 'طويلة'],
  stFill: [(n, all) => `${n} von ${all} Lücken gefüllt`, (n, all) => `${n} of ${all} gaps filled`, (n, all) => `${n} van ${all} gaten gevuld`,
    (n, all) => `${n} de ${all} huecos rellenos`, (n, all) => `${all} boşluktan ${n} tanesi dolu`, (n, all) => `${n} من ${all} فراغات مملوءة`],
  stClear: ['Auswahl leeren', 'Clear choices', 'Keuzes wissen', 'Borrar elección', 'Seçimi temizle', 'إفراغ الاختيار'],
  stGo: ['Schreiben', 'Write', 'Schrijven', 'Escribir', 'Yaz', 'اكتب'],
  stWorking: ['Wird geschrieben …', 'Writing …', 'Wordt geschreven …', 'Escribiendo …', 'Yazılıyor …', 'يجري الكتابة …'],
  stFailed: ['Nicht fertig geschrieben', 'Not finished', 'Niet afgemaakt', 'Sin terminar', 'Tamamlanmadı', 'لم تكتمل'],
  stRetry: ['Klicken: noch einmal versuchen', 'Click to try again', 'Klik om het opnieuw te proberen', 'Haz clic para reintentar', 'Yeniden denemek için tıkla', 'انقر للمحاولة مجدداً'],
  stNoModel: ['Kein Schreibmodell gefunden.', 'No writing model found.', 'Geen schrijfmodel gevonden.', 'No se encontró modelo de escritura.', 'Yazma modeli bulunamadı.', 'لم يتم العثور على نموذج كتابة.'],
  stUntitled: ['Kurzgeschichte', 'Short story', 'Kort verhaal', 'Relato corto', 'Kısa hikâye', 'قصة قصيرة'],
  aiOffMsg: ['Die KI-Funktionen sind aus (Einstellungen → KI-Funktionen).', 'AI functions are off (Settings → AI functions).', 'De AI-functies staan uit (Instellingen → AI-functies).', 'Las funciones de IA están desactivadas (Ajustes → Funciones de IA).', 'YZ işlevleri kapalı (Ayarlar → YZ işlevleri).', 'وظائف الذكاء الاصطناعي مغلقة (الإعدادات ← وظائف الذكاء الاصطناعي).'],
  s_story: ['Kurzgeschichten-Generator (Beta)', 'Short story generator (Beta)', 'Korteverhalengenerator (Beta)', 'Generador de relatos (Beta)', 'Kısa hikâye üreticisi (Beta)', 'مولّد القصص القصيرة (Beta)']
};
for (const [k, v] of Object.entries(STORY_STR)) {
  window.K[k] = [v[0], v[1]];
  window.NL[k] = v[2]; window.ES[k] = v[3]; window.TR[k] = v[4]; window.AR[k] = v[5];
}

// ---------- Verdrahtung ----------
function wire() {
  $('#txStory').onclick = storyOpen;
  $('#stClose').onclick = () => $('#stDlg').close();
  $('#stCancel').onclick = () => $('#stDlg').close();
  $('#stGo').onclick = storyStart;
  $('#stClear').onclick = () => { storySel = {}; storySlot = ST_SLOTS[0][0]; drawStoryCloze(); drawStoryPicks(); };
  document.querySelectorAll('#stLen .stseg').forEach(b => {
    b.onclick = () => { S.storyLen = b.dataset.len; save(); drawStoryLen(); };
  });
  $('#rBack').onclick = () => { readHalt(); savePos(); window.edShowView('txt'); };
  $('#rSpeak').onclick = () => {
    if (!ST.cur) return;
    if (READ.on) return readToggle();
    readStart(ST.cur);
  };
  const wrap = $('#readerWrap');
  if (wrap) wrap.addEventListener('scroll', () => { clearTimeout(ST.posTimer); ST.posTimer = setTimeout(savePos, 600); });
  bindPicking($('#reader'), '.rw');
  bindPicking($('#pickLayer'), '.w');

  $('#pickBtn').onclick = () => {
    ST.pick = !ST.pick;
    pickSync();
    // Der Modus legt eine Ebene ueber das Textfeld; ohne Fokus im Feld laesst
    // sich danach nichts mehr tippen.
    if (ST.pick && S.view === 'tr') $('#input').focus();
  };
  // Klick neben die Woerter setzt wieder ins Textfeld, damit Tippen im
  // Auswahlmodus weitergeht (die Ebene liegt ueber dem Feld).
  $('#pickLayer').addEventListener('click', e => { if (!e.target.closest('.w')) $('#input').focus(); });
  document.querySelectorAll('[data-study-model]').forEach(b => { b.onclick = () => studyModelPop(b); });
  // Nach dem Laden der Modelle (auch beim Nachladen) die Pille auffrischen.
  const edModelsBase = window.edModels;
  window.edModels = () => { if (edModelsBase) edModelsBase(); renderStudyModel(); };
  $('#input').addEventListener('input', () => { if (ST.pick && S.view === 'tr') drawPickLayer(); });
  $('#input').addEventListener('scroll', () => { const l = $('#pickLayer'); if (l && !l.classList.contains('hidden')) l.scrollTop = $('#input').scrollTop; });

  $('#vocAddBtn').onclick = addFromSearch;
  $('#vocTerm').addEventListener('input', () => vocStatus(''));
  $('#vocTerm').addEventListener('keydown', e => { if (e.key === 'Enter') { e.preventDefault(); addFromSearch(); } });
  $('#vocList').addEventListener('click', e => {
    const say = e.target.closest('[data-say]');
    if (say) return sayVocab(ST.vocab.find(v => v.id === say.dataset.say));
    const ed = e.target.closest('[data-edit]');
    if (ed) return vocOpenEdit(ed.dataset.edit);
    const b = e.target.closest('[data-del]');
    if (b) return dropVocab(b.dataset.del);
    const row = e.target.closest('[data-sel]');
    if (row && ST.selMode) vocSelToggle(row.dataset.sel);
  });
  // Suche: die Lupe klappt das Feld auf, Tippen filtert sofort.
  $('#vocFindBtn').onclick = () => {
    const wrap = $('#vocFindWrap');
    const open = wrap.classList.contains('hidden');
    wrap.classList.toggle('hidden', !open);
    $('#vocFindBtn').classList.toggle('on', open);
    // Das Eingabefeld weicht der Suche; die Werkzeuge bleiben an ihrem Platz.
    const aw = $('#vocAddWrap');
    if (aw) aw.classList.toggle('hidden', open);
    if (open) { $('#vocFind').value = ST.vocFind; vocFindClearSync(); $('#vocFind').focus(); }
    else { $('#vocFind').value = ''; ST.vocFind = ''; renderVocab(); }
  };
  // Das Kreuz im Suchfeld raeumt den Text weg - solange keiner drinsteht, ist es
// nur ein Knopf ohne Aufgabe und bleibt deshalb unsichtbar.
function vocFindClearSync() {
  const cl = $('#vocFindClear');
  if (cl) cl.classList.toggle('hidden', !($('#vocFind') && $('#vocFind').value));
}

// Die ganze Liste neu zu bauen kostet bei vielen Vokabeln je Anschlag Zeit;
  // gesucht wird deshalb erst, wenn das Tippen kurz ruht.
  $('#vocFind').addEventListener('input', () => {
    vocFindClearSync();
    ST.vocFind = $('#vocFind').value;
    clearTimeout(ST.findTimer);
    ST.findTimer = setTimeout(renderVocab, 120);
  });
  $('#vocFindClear').onclick = () => { $('#vocFind').value = ''; vocFindClearSync(); ST.vocFind = ''; renderVocab(); $('#vocFind').focus(); };
  // Escape schliesst die Suche und gibt das Eingabefeld zurueck.
  $('#vocFind').addEventListener('keydown', e => {
    if (e.key !== 'Escape') return;
    e.preventDefault();
    $('#vocFindBtn').click();
    $('#vocTerm').focus();
  });
  $('#vocFilterBtn').onclick = () => vocFilterPop($('#vocFilterBtn'));
  $('#vocSelBtn').onclick = () => vocSelMode();
  $('#vocSelAllBtn').onclick = vocSelAll;
  $('#vocSelRepeatBtn').onclick = () => startRepeat([...ST.sel]);
  $('#vocSelDelBtn').onclick = vocSelDelete;
  $('#vocSelEndBtn').onclick = () => vocSelMode(false);
  $('#vocClose').onclick = () => $('#vocDlg').close();
  $('#vocCancel').onclick = () => $('#vocDlg').close();
  $('#vocSaveBtn').onclick = vocSaveEdit;
  $('#vocDlg').addEventListener('keydown', ev => {
    if (ev.key === 'Enter' && ev.target.tagName !== 'TEXTAREA') { ev.preventDefault(); vocSaveEdit(); }
  });
  $('#srsType').onclick = () => {
    S.srsType = !S.srsType;
    save();
    typeQueue = []; srsNote = null;
    renderSrs();
  };

  $('#txAdd').onclick = txOpen;
  $('#txClose').onclick = () => $('#txDlg').close();
  $('#txCancel').onclick = () => $('#txDlg').close();
  $('#txOk').onclick = txAdd;
  $('#txFile').onclick = async () => {
    const r = await window.api.studyImportText();
    if (!r) return;
    if (r.error) return window.trStatus(t('txNoRead'));
    txAppend(r.text);
    txSuggestTitle(r.title || r.name || '');
  };
  $('#txPaste').onclick = async () => {
    let s = '';
    try { s = await window.api.clipText(); } catch { }
    if (!String(s || '').trim()) return window.trStatus(t('txClipEmpty'));
    txAppend(s);
  };
  $('#txExp').onclick = () => window.api.studyExportFile('texte', textsJson(), 'json');
  $('#txImp').onclick = async () => {
    const r = await window.api.studyImportFile('json');
    if (r) importTexts(r.text);
  };
  $('#vocFillBtn').onclick = fillMissingTranslations;
  $('#vocCsvExp').onclick = () => window.api.studyExportFile('vokabeln', vocabCsv(), 'csv');
  $('#vocCsvImp').onclick = async () => {
    const r = await window.api.studyImportFile('csv,txt,tsv');
    if (!r) return;
    try { importVocab(r.text); } catch { window.trStatus(t('csvErr')); }
  };
}

// ---------- Start ----------
(async () => {
  try {
    const tx = await window.api.studyLoad('texts');
    if (Array.isArray(tx)) ST.texts = tx;
  } catch { }
  try {
    const vo = await window.api.studyLoad('vocab');
    if (Array.isArray(vo)) ST.vocab = vo;
  } catch { }
  // Sprachenpaar: Standard aus dem Uebersetzer und der Anzeigesprache ableiten.
  // Texte und Vokabeln aus der Zeit vor der Sprachtrennung bekommen das
  // aktuelle Paar, damit nichts aus den Listen verschwindet.
  if (!S.studyTgt) S.studyTgt = (LANG === 'de') ? 'de' : (S.tgt && S.tgt !== 'auto' ? S.tgt : 'en');
  if (!S.studySrc) S.studySrc = (S.src && S.src !== 'auto') ? S.src : (S.studyTgt === 'de' ? 'en' : 'de');
  if (S.studySrc === S.studyTgt) S.studyTgt = S.studySrc === 'en' ? 'de' : 'en';
  for (const tx of ST.texts) if (!tx.src) { tx.src = S.studySrc; tx.tgt = S.studyTgt; }
  for (const v of ST.vocab) if (!v.src) { v.src = S.studySrc; v.tgt = S.studyTgt; }
  // Fruehere Laeufe haben die Wiederholung schon eingetragen; einmal aufraeumen.
  for (const v of ST.vocab) if (v.translation) {
    const t = cleanTranslation(v.translation);
    if (t !== v.translation) v.translation = t;
  }
  // Eine Geschichte, die beim Beenden noch geschrieben wurde, kann nicht
  // weiterlaufen: sie steht als Fehler in der Liste und laesst sich dort mit
  // einem Klick neu schreiben.
  for (const x of ST.texts) if (Number(x.pending) === 1) x.pending = -1;
  save();
  persist('texts'); persist('vocab');
  fillCats();
  wire();
  renderPairs();
  renderVocab();
  renderTexts();
  pickSync();
  applyI18n();
  if (S.view === 'voc') renderVocab();
})();
