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
    b.title = `${t('studyModelT')}: ${eff || '—'}`;
  });
}

// Auswahl wie beim Editor: dieselbe Liste, nur ohne Schreibmodell - hier wird
// ausschliesslich uebersetzt. Die erste Zeile stellt auf das Uebersetzer-Modell
// zurueck.
function studyModelPop(btn) {
  closePops();
  const p = $('#pop');
  p.innerHTML = '';
  const row = (label, value, cur, icons) => {
    const r = document.createElement('div');
    r.className = 'row' + (cur ? ' sel' : '');
    r.innerHTML = `<span>${esc(label)}</span><span>${icons || ''}</span>`;
    r.onclick = () => {
      S.studyModel = value;
      save();
      wordCache.clear();
      renderStudyModel();
      closePops();
    };
    p.append(r);
  };
  row(t('studyModelAuto'), '', !S.studyModel, '');
  for (const o of MODEL_OPTS) row(o.label, o.value, o.value === S.studyModel, modelIcons(o));
  const r = btn.getBoundingClientRect();
  Object.assign(p.style, { left: Math.max(8, Math.min(r.left, innerWidth - 320)) + 'px', top: r.bottom + 6 + 'px' });
  p.classList.remove('hidden');
}
const langName = c => (!c || c === 'auto') ? 'the source language' : (window.trEngName ? window.trEngName(c) : c);

async function llmJson(prompt) {
  const mdl = studyModel();
  if (!mdl) throw new Error('nomodel');
  const out = await window.trChat([
    { role: 'system', content: 'You are a bilingual dictionary. Reply with JSON only, no explanations.' },
    { role: 'user', content: prompt }
  ], { model: mdl, format: 'json' });
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
async function translateTerm(term, ctx, pair, single) {
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
      : '  "translation": the translation (up to three equivalent meanings separated by " / ")\n') +
    '  "lemma": the dictionary form\n' +
    '  "pos": part of speech in the target language\n' +
    `  "gender": "m", "f", "n", or "" - grammatical gender of the translation, if ${langName(P.tgt)} marks gender\n` +
    '  "article": the definite article of the translation if the target language has one, otherwise ""';
  const j = await llmJson(prompt);
  const r = {
    translation: String(j.translation || '').trim(),
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
  const acts = ST.selMode ? '' : `<button class="rm ed" data-edit="${e.id}" title="${esc(t('edit'))}">${ic('pencil', 13)}</button>
    <button class="rm" data-del="${e.id}" title="${esc(t('remove'))}">${ic('x', 13)}</button>`;
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
  const fr = $('#vocFindRow'), sb = $('#vocFindBtn');
  if (sb) sb.classList.toggle('on', !!fr && !fr.classList.contains('hidden'));
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
  const c = $('#vocSelCount'); if (c) c.textContent = n ? t('vocSelN', n) : '';
  const del = $('#vocSelDelBtn'); if (del) del.disabled = !n;
  const all = $('#vocSelAllBtn'); if (all) all.disabled = vocVisible().length === 0;
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
    const card = document.createElement('div');
    card.className = 'libcard txtcard'; card.dataset.tx = tx.id; card.tabIndex = 0;
    const excerpt = String(tx.text || '').slice(0, 420);
    card.innerHTML = `<div class="libprev"><div class="doc">${esc(excerpt)}</div><span class="txbadge">${esc(catLabel(tx.cat))}</span></div>
      <div class="libmeta"><div class="libname">${esc(tx.title)}</div><div class="libdate">${esc(langLabel(tx.src))} → ${esc(langLabel(tx.tgt))} · ${esc(t('vocCount', ST.vocab.filter(v => v.textId === tx.id).length))}</div></div>
      <button class="rm" title="${esc(t('remove'))}">${ic('x', 14)}</button>`;
    card.onclick = ev => { if (!ev.target.closest('.rm')) openText(tx.id); };
    card.onkeydown = ev => { if (ev.key === 'Enter' || ev.key === ' ') { ev.preventDefault(); openText(tx.id); } };
    card.querySelector('.rm').onclick = ev => { ev.stopPropagation(); dropText(tx.id); };
    frag.append(card);
  }
  grid.prepend(frag);
  const hint = $('#txOther');
  if (hint) hint.textContent = other > 0 ? t('vocOther', other) : '';
  const empty = $('#txtEmpty');
  if (empty) empty.classList.toggle('hidden', mine.length > 0);
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
let txAutoTitle = '';

function txOpen() {
  const d = $('#txDlg'); if (!d) return;
  $('#txTitle').value = ''; $('#txBody').value = '';
  txAutoTitle = '';
  const cat = $('#txCat'); if (cat) cat.value = 'other';
  d.showModal();
  $('#txBody').focus();
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
  if (!ST.cur) return;
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
  requestAnimationFrame(() => {
    const wrap = $('#readerWrap');
    if (wrap) wrap.scrollTop = (Number(tx.pos) || 0) * Math.max(1, wrap.scrollHeight - wrap.clientHeight);
  });
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
  persist('texts');
}
window.studyRestore = () => { if (ST.cur) drawReader(); };

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
  if (c) c.classList.add('going');
  clearTimeout(ST.chipTimer);
  ST.chipTimer = setTimeout(() => {
    if (c) c.remove();
    spans.forEach(s => s.classList.remove('gone'));
    if (entry) dropVocab(entry.id);
  }, 5000);
}

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
    const msg = e.message === 'nomodel' ? t('studyNoModel') : t('wordErr');
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
    const msg = e.message === 'nomodel' ? t('studyNoModel') : t('wordErr');
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

async function addFromSearch() {
  const inp = $('#vocTerm');
  const term = inp.value.trim();
  if (!term) { vocStatus(t('vocNeedTerm')); inp.focus(); return; }
  vocStatus(t('wordTr'));
  let res;
  try { res = await translateTerm(term, ''); }
  catch (e) { vocStatus(e.message === 'nomodel' ? t('studyNoModel') : t('wordErr'), true); return; }
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
  return esc(text).replace(re, '<span class="srsblank">_____</span>');
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
  const due = ST.vocab.filter(e => inPair(e) && (e.due || todayISO()) <= todayISO());
  $('#srsCount').textContent = due.length ? t('vocabDue', due.length) : '';
  const tg = $('#srsType'); if (tg) tg.classList.toggle('on', !!S.srsType);
  if (S.srsType) return renderType(box, due);
  srsQueue = srsQueue.filter(inPair);
  if (!due.length) {
    box.innerHTML = `<div class="srsempty"><p>${esc(t('srsNone'))}</p><p class="muted">${esc(t('srsDone'))}</p></div>`;
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
  box.innerHTML = `<div class="srscard">
    ${ctx ? `<div class="srsctx">${ctx}</div>` : ''}
    <div class="srsword">${srsShow || !ctx ? esc(e.term) : ''}</div>
    ${srsShow ? `<div class="srsres">${e.gender ? `<span class="vg">${genderMark(e)}</span>` : ''}${sol ? esc(sol) : `<span class="muted">${esc(t('srsNoTr'))}</span>`}</div>
      ${subLine}
      <div class="srseditrow"><button class="ib srsedit" id="srsEdit" title="${esc(t('edit'))}" aria-label="${esc(t('edit'))}">${ic('pencil', 16)}</button></div>
      <div class="srsbtns">
        <button class="ghost" data-r="again">${esc(t('srsAgain'))}</button>
        <button class="ghost" data-r="hard">${esc(t('srsHard'))}</button>
        <button class="primary" data-r="good">${esc(t('srsGood'))}</button>
        <button class="ghost" data-r="easy">${esc(t('srsEasy'))}</button>
      </div>`
      : `<div class="srsbtns"><button class="primary" id="srsReveal">${esc(t('srsReveal'))}</button></div>`}
  </div>`;
  const rv = box.querySelector('#srsReveal');
  if (rv) rv.onclick = () => { srsShow = true; renderSrs(); };
  // Die aufgedeckte Karte lässt sich korrigieren, bevor bewertet wird: falsche
  // oder fehlende Übersetzung tippt man hier nach, ohne die Runde zu verlassen.
  // Der Stift ist derselbe wie in der Vokabelliste (#vocDlg, vocOpenEdit).
  const ed = box.querySelector('#srsEdit');
  if (ed) ed.onclick = () => vocOpenEdit(e.id);
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

// ---------- Verdrahtung ----------
function wire() {
  $('#rBack').onclick = () => { savePos(); window.edShowView('txt'); };
  const wrap = $('#readerWrap');
  if (wrap) wrap.addEventListener('scroll', () => { clearTimeout(ST.posTimer); ST.posTimer = setTimeout(savePos, 600); });
  bindPicking($('#reader'), '.rw');
  bindPicking($('#pickLayer'), '.w');

  $('#pickBtn').onclick = () => { ST.pick = !ST.pick; pickSync(); };
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
    const ed = e.target.closest('[data-edit]');
    if (ed) return vocOpenEdit(ed.dataset.edit);
    const b = e.target.closest('[data-del]');
    if (b) return dropVocab(b.dataset.del);
    const row = e.target.closest('[data-sel]');
    if (row && ST.selMode) vocSelToggle(row.dataset.sel);
  });
  // Suche: die Lupe klappt das Feld auf, Tippen filtert sofort.
  $('#vocFindBtn').onclick = () => {
    const row = $('#vocFindRow');
    const open = row.classList.contains('hidden');
    row.classList.toggle('hidden', !open);
    $('#vocFindBtn').classList.toggle('on', open);
    if (open) { $('#vocFind').value = ST.vocFind; $('#vocFind').focus(); }
    else { $('#vocFind').value = ''; ST.vocFind = ''; renderVocab(); }
  };
  $('#vocFind').addEventListener('input', () => { ST.vocFind = $('#vocFind').value; renderVocab(); });
  $('#vocFindClear').onclick = () => { $('#vocFind').value = ''; ST.vocFind = ''; renderVocab(); $('#vocFind').focus(); };
  $('#vocFilterBtn').onclick = () => vocFilterPop($('#vocFilterBtn'));
  $('#vocSelBtn').onclick = () => vocSelMode();
  $('#vocSelAllBtn').onclick = vocSelAll;
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
