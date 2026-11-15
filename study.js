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
async function translateTerm(term, ctx, pair) {
  const P = pair || curPair();
  const key = nrm(term) + '|' + nrm(ctx) + '|' + P.src + '>' + P.tgt;
  if (wordCache.has(key)) return wordCache.get(key);
  const prompt =
    `Word or phrase: "${term}"\n` +
    (ctx ? `Sentence it appears in: ${ctx}\n` : '') +
    `Translate it from ${langName(P.src)} into ${langName(P.tgt)}, exactly as used in the sentence.\n` +
    'Return JSON with these keys:\n' +
    '  "translation": the translation (up to three equivalent meanings separated by " / ")\n' +
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

function vocRow(e, opts) {
  const g = genderMark(e);
  const ctx = e.context ? String(e.context) : '';
  const masked = ctx && e.term ? esc(ctx).replace(new RegExp(`(${esc(e.term).replace(/[.*+?^${}()|[\]\\]/g, '\\$&')})`, 'giu'), '<mark>$1</mark>') : '';
  return `<div class="vrow" data-id="${e.id}">
    <div class="vmain">
      <div class="vterm">${g ? `<span class="vg">${g}</span>` : ''}${esc(e.term)}${isMastered(e) ? `<span class="vok" title="${esc(t('srsMastered'))}">${ic('know', 13)}</span>` : ''}<span class="vtr">${esc(e.translation)}</span></div>
      ${masked ? `<div class="vctx">${masked}</div>` : ''}
      <div class="vmeta">${e.textTitle ? esc(e.textTitle) : ''}${e.interval ? ` · ${e.interval} ${LANG === 'de' ? 'Tage' : 'days'}` : ''}${opts && opts.due ? ` · ${esc(e.due || '')}` : ''}</div>
    </div>
    <button class="rm ed" data-edit="${e.id}" title="${esc(t('edit'))}">${ic('pencil', 13)}</button>
    <button class="rm" data-del="${e.id}" title="${esc(t('remove'))}">${ic('x', 13)}</button>
  </div>`;
}

function renderVocab() {
  const box = V(); if (!box) return;
  const mine = ST.vocab.filter(inPair);
  const q = nrm($('#vocTerm') ? $('#vocTerm').value : '');
  const list = q ? mine.filter(e => nrm(e.term).includes(q) || nrm(e.translation).includes(q)) : mine.slice();
  list.sort((a, b) => (a.due || '').localeCompare(b.due || '') || a.term.localeCompare(b.term));
  box.innerHTML = list.map(e => vocRow(e, { due: true })).join('');
  const other = ST.vocab.length - mine.length;
  $('#vocCount').textContent = (mine.length ? t('vocCount', mine.length) : '') + (other > 0 ? ` · ${t('vocOther', other)}` : '');
  $('#vocEmpty').classList.toggle('hidden', mine.length > 0);
  box.classList.toggle('hidden', mine.length === 0);
}
window.studyVocRender = renderVocab;

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
  showView('read');
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
  $('#rMeta').textContent = [`${langLabel(tx.src || S.studySrc)} → ${langLabel(tx.tgt || S.studyTgt)}`, catLabel(tx.cat), t('vocCount', ST.vocab.filter(v => v.textId === tx.id).length)].join(' · ');
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
  end.querySelector('#rendVoc').onclick = () => { savePos(); showView('voc'); };
  end.querySelector('#rendLib').onclick = () => { savePos(); showView('txt'); };
  requestAnimationFrame(() => {
    const wrap = $('#readerWrap');
    if (wrap) wrap.scrollTop = (Number(tx.pos) || 0) * Math.max(1, wrap.scrollHeight - wrap.clientHeight);
  });
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

function showChip(anchor, text, onClick) {
  killChip();
  const c = document.createElement('div');
  c.className = 'wchip';
  c.innerHTML = `<span>${esc(text)}</span>`;
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
  let res;
  try { res = await translateTerm(term, ctx, pair); }
  catch (e) {
    window.trStatus(e.message === 'nomodel' ? t('studyNoModel') : t('wordErr'));
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
  let res;
  try { res = await translateTerm(phrase, ctx, pair); }
  catch (e) { window.trStatus(e.message === 'nomodel' ? t('studyNoModel') : t('wordErr')); return; }
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
  e.translation = $('#vocTr2').value.trim();
  e.context = $('#vocCtx2').value.trim();
  e.updated = Date.now();
  persist('vocab');
  renderVocab();
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
    srsNote = { ok: false, text: t('srsWrong') + ' ' + [e.article, e.translation].filter(Boolean).join(' ') };
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
  box.innerHTML = `<div class="srscard">
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
  box.innerHTML = `<div class="srscard">
    ${ctx ? `<div class="srsctx">${ctx}</div>` : ''}
    <div class="srsword">${esc(srsShow ? e.term : '')}</div>
    ${srsShow ? `<div class="srsres">${e.gender ? `<span class="vg">${genderMark(e)}</span>` : ''}${esc([e.article, e.translation].filter(Boolean).join(' '))}</div>
      <div class="srsctx2 muted">${esc(e.term)}${e.lemma && e.lemma !== e.term ? ` · ${esc(e.lemma)}` : ''}${e.pos ? ` · ${esc(e.pos)}` : ''}</div>
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

// Listet die Vokabeln einer Readlang-Textdatei; null, wenn es keine ist.
function readlangWords(text) {
  const delim = readlangDelim(text);
  if (!delim) return null;
  const lines = String(text).replace(/^\uFEFF/, '').replace(/\r\n?/g, '\n').split('\n').filter(l => l.trim());
  const isFreq = s => /^\d+(?:[.,]\d+)?$/.test(s);
  const out = [], seen = new Map();
  for (const line of lines) {
    const fields = line.split(delim).map(x => x.trim()).filter(Boolean);
    if (!fields.length) continue;
    let ctx = '', bracket = '', words = [];
    for (const f of fields) {
      if (isFreq(f)) continue;
      const m = f.match(RL_MARK);
      if (m && !ctx) { ctx = f; bracket = (m[1] || m[2] || '').trim(); continue; }
      if (f === 'No context') continue;
      words.push(f);
    }
    if (!words.length) continue;
    const term = words.slice().sort((a, b) => a.length - b.length)[0];
    let translation = bracket;
    // „Kontext mit Originalwort“ stellt das Wort selbst in die Klammern.
    if (translation && nrm(translation) === nrm(term)) translation = '';
    if (!translation && words.length > 1) translation = words.find(w => w !== term) || '';
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

// ---------- Verdrahtung ----------
function wire() {
  $('#rBack').onclick = () => { savePos(); showView('txt'); };
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
    if (b) dropVocab(b.dataset.del);
  });
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
