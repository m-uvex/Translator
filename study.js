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
  if (!S.model) return '';
  if (window.trIsStudyless && window.trIsStudyless()) return window.trHelperModel() || '';
  return S.model;
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

const wordCache = new Map();
async function translateTerm(term, ctx) {
  const key = nrm(term) + '|' + nrm(ctx) + '|' + S.tgt;
  if (wordCache.has(key)) return wordCache.get(key);
  const prompt =
    `Word or phrase: "${term}"\n` +
    (ctx ? `Sentence it appears in: ${ctx}\n` : '') +
    `Translate it from ${langName(S.src)} into ${langName(S.tgt)}, exactly as used in the sentence.\n` +
    'Return JSON with these keys:\n' +
    '  "translation": the translation (up to three equivalent meanings separated by " / ")\n' +
    '  "lemma": the dictionary form\n' +
    '  "pos": part of speech in the target language\n' +
    `  "gender": "m", "f", "n", or "" - grammatical gender of the translation, if ${langName(S.tgt)} marks gender\n` +
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
function findVocab(term) { const n = nrm(term); return ST.vocab.find(v => nrm(v.term) === n); }

function addVocab(term, res, ctx, tx) {
  const t0 = String(term || '').trim();
  if (!t0 || !res) return null;
  let e = findVocab(t0);
  if (!e) {
    e = {
      id: uid('v'), term: t0, translation: res.translation || '', lemma: res.lemma || t0,
      pos: res.pos || '', gender: res.gender || '', article: res.article || '',
      alternatives: [], context: '', textId: tx ? tx.id : '', textTitle: tx ? tx.title : '',
      created: Date.now(), level: 0, interval: 0, due: todayISO()
    };
    ST.vocab.unshift(e);
  }
  if (!e.translation && res.translation) e.translation = res.translation;
  if (!e.gender && res.gender) { e.gender = res.gender; e.article = res.article || ''; }
  if (!e.pos && res.pos) e.pos = res.pos;
  if (ctx && !e.context) e.context = ctx;
  persist('vocab');
  renderVocab();
  renderPanel();
  return e;
}

function dropVocab(id) {
  const i = ST.vocab.findIndex(v => v.id === id);
  if (i < 0) return;
  ST.vocab.splice(i, 1);
  persist('vocab');
  renderVocab();
  renderPanel();
}

const genderMark = e => e.gender === 'm' ? 'm' : e.gender === 'f' ? 'f' : e.gender === 'n' ? 'n' : '';

function vocRow(e, opts) {
  const g = genderMark(e);
  const ctx = e.context ? String(e.context) : '';
  const masked = ctx && e.term ? esc(ctx).replace(new RegExp(`(${esc(e.term).replace(/[.*+?^${}()|[\]\\]/g, '\\$&')})`, 'giu'), '<mark>$1</mark>') : '';
  return `<div class="vrow" data-id="${e.id}">
    <div class="vmain">
      <div class="vterm">${g ? `<span class="vg">${g}</span>` : ''}${esc(e.term)}<span class="vtr">${esc(e.translation)}</span></div>
      ${masked ? `<div class="vctx">${masked}</div>` : ''}
      <div class="vmeta">${e.textTitle ? esc(e.textTitle) : ''}${e.interval ? ` · ${e.interval} ${LANG === 'de' ? 'Tage' : 'days'}` : ''}${opts && opts.due ? ` · ${esc(e.due || '')}` : ''}</div>
    </div>
    <button class="rm" data-del="${e.id}" title="${esc(t('remove'))}">${ic('x', 13)}</button>
  </div>`;
}

function renderVocab() {
  const box = V(); if (!box) return;
  const q = nrm($('#vocTerm') ? $('#vocTerm').value : '');
  const list = q ? ST.vocab.filter(e => nrm(e.term).includes(q) || nrm(e.translation).includes(q)) : ST.vocab.slice();
  list.sort((a, b) => (a.due || '').localeCompare(b.due || '') || a.term.localeCompare(b.term));
  box.innerHTML = list.map(e => vocRow(e, { due: true })).join('');
  $('#vocCount').textContent = ST.vocab.length ? t('vocCount', ST.vocab.length) : '';
  $('#vocEmpty').classList.toggle('hidden', ST.vocab.length > 0);
  box.classList.toggle('hidden', ST.vocab.length === 0);
}
window.studyVocRender = renderVocab;

// ---------- Seitenspalte neben dem Uebersetzer ----------
function renderPanel() {
  const p = $('#vPanel'); if (!p || p.classList.contains('hidden')) return;
  const due = ST.vocab.filter(e => (e.due || todayISO()) <= todayISO()).length;
  p.innerHTML = `<div class="vph"><b>${esc(t('vocTitle'))}</b><span class="muted">${esc(t('vocCount', ST.vocab.length))}</span></div>
    <button class="ghost" id="vPanelTrain">${esc(t('srsTitle'))} · ${esc(t('vocabDue', due))}</button>
    <div class="vplist">${ST.vocab.slice(0, 40).map(e => vocRow(e)).join('')}</div>`;
  p.querySelector('#vPanelTrain').onclick = () => showView('srs');
  p.querySelectorAll('[data-del]').forEach(b => b.onclick = () => dropVocab(b.dataset.del));
}

function pickSync() {
  const panelOn = !!S.studyPanel;
  const btn = $('#pickBtn');
  if (btn) btn.classList.toggle('hidden', !panelOn);
  const live = panelOn && ST.pick && S.view === 'tr';
  const layer = $('#pickLayer');
  if (!layer) return;
  layer.classList.toggle('hidden', !live);
  if (btn) btn.classList.toggle('on', ST.pick && panelOn);
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

window.studyPanelSync = () => {
  const want = !!S.studyPanel && S.view === 'tr';
  const p = $('#vPanel');
  if (p) p.classList.toggle('hidden', !want);
  $('#vT').classList.toggle('with-vpanel', want);
  if (want) renderPanel();
  pickSync();
};

// ---------- Texte ----------
function renderTexts() {
  const grid = $('#libGrid'); if (!grid) return;
  grid.querySelectorAll('.txtcard').forEach(c => c.remove());
  const frag = document.createDocumentFragment();
  for (const tx of ST.texts) {
    const card = document.createElement('div');
    card.className = 'libcard txtcard'; card.dataset.tx = tx.id; card.tabIndex = 0;
    const excerpt = String(tx.text || '').slice(0, 420);
    card.innerHTML = `<div class="libprev"><div class="doc">${esc(excerpt)}</div><span class="txbadge">${esc(catLabel(tx.cat))}</span></div>
      <div class="libmeta"><div class="libname">${esc(tx.title)}</div><div class="libdate">${esc(t('vocCount', ST.vocab.filter(v => v.textId === tx.id).length))}</div></div>
      <button class="rm" title="${esc(t('remove'))}">${ic('x', 14)}</button>`;
    card.onclick = ev => { if (!ev.target.closest('.rm')) openText(tx.id); };
    card.onkeydown = ev => { if (ev.key === 'Enter' || ev.key === ' ') { ev.preventDefault(); openText(tx.id); } };
    card.querySelector('.rm').onclick = ev => { ev.stopPropagation(); dropText(tx.id); };
    frag.append(card);
  }
  grid.prepend(frag);
  const empty = $('#libEmpty');
  if (empty && ST.texts.length) empty.classList.add('hidden');
}
window.studyDrawTexts = renderTexts;

function fillCats() {
  const sel = $('#txCat'); if (!sel) return;
  sel.innerHTML = CATS.map(c => `<option value="${c[0]}">${esc(LANG === 'de' ? c[1] : c[2])}</option>`).join('');
}

async function addTextFromFile() {
  const r = await window.api.studyImportText();
  if (!r) return;
  if (r.error) return window.trStatus(t('txNoRead'));
  const title = await askLine(t('txAdd'), r.name || '');
  if (title === null) return;
  const cat = $('#txCat').value || 'other';
  const tx = { id: uid('t'), title: title.trim() || r.name, cat, text: r.text, created: Date.now(), updated: Date.now(), pos: 0 };
  ST.texts.unshift(tx);
  persist('texts');
  await drawLib();
}

function dropText(id) {
  const tx = ST.texts.find(x => x.id === id); if (!tx) return;
  ask(t('txDelQ'), t('yes'), t('cancel')).then(ok => {
    if (!ok) return;
    ST.texts = ST.texts.filter(x => x.id !== id);
    ST.vocab = ST.vocab.filter(v => v.textId !== id);
    persist('texts'); persist('vocab');
    drawLib(); renderVocab();
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
  $('#rMeta').textContent = [catLabel(tx.cat), t('vocCount', ST.vocab.filter(v => v.textId === tx.id).length)].join(' · ');
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
  end.querySelector('#rendLib').onclick = () => { savePos(); showView('lib'); };
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

async function pickWord(el) {
  const term = el.dataset.w || el.textContent;
  let ctx = '';
  if (S.view === 'read' && el.parentElement && el.parentElement.dataset.txt) {
    ctx = sentenceAt(el.parentElement.dataset.txt, Number(el.dataset.o) || 0);
  } else {
    ctx = sentenceAt($('#input').value || '', Number(el.dataset.o) || 0);
  }
  window.trStatus(t('wordTr'));
  let res;
  try { res = await translateTerm(term, ctx); }
  catch (e) {
    window.trStatus(e.message === 'nomodel' ? t('studyNoModel') : t('wordErr'));
    return;
  }
  window.trStatus('');
  const entry = addVocab(term, res, ctx, S.view === 'read' ? ST.cur : null);
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
  const spans = [];
  if (para) para.querySelectorAll('.rw').forEach(s => { if (sel.containsNode(s, true)) spans.push(s); });
  window.trStatus(t('wordTr'));
  let res;
  try { res = await translateTerm(phrase, ctx); }
  catch (e) { window.trStatus(e.message === 'nomodel' ? t('studyNoModel') : t('wordErr')); return; }
  window.trStatus('');
  const entry = addVocab(phrase, res, ctx, S.view === 'read' ? ST.cur : null);
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
async function addFromSearch() {
  const inp = $('#vocTerm');
  const term = inp.value.trim();
  if (!term) return;
  window.trStatus(t('wordTr'));
  let res;
  try { res = await translateTerm(term, ''); }
  catch (e) { window.trStatus(e.message === 'nomodel' ? t('studyNoModel') : t('wordErr')); return; }
  window.trStatus('');
  addVocab(term, res, '', null);
  inp.value = '';
  renderVocab();
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

function renderSrs() {
  const box = $('#srsBox'); if (!box) return;
  const due = ST.vocab.filter(e => (e.due || todayISO()) <= todayISO());
  $('#srsCount').textContent = due.length ? t('vocabDue', due.length) : '';
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
window.studySrsRender = () => { srsShow = false; srsQueue = ST.vocab.filter(e => (e.due || todayISO()) <= todayISO()); renderSrs(); };

// ---------- CSV im Readlang-Format ----------
// Spalten: Wort/Wendung, Uebersetzung, Kontext, Intervall in Tagen, naechstes
// Datum. Synonyme und Alternativuebersetzungen werden mit " / " getrennt.
const csvCell = v => { const s = String(v == null ? '' : v); return /[",\r\n]/.test(s) ? '"' + s.replace(/"/g, '""') + '"' : s; };
function vocabCsv() {
  return ST.vocab.map(e => {
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

function importCsv(text) {
  const rows = parseCsv(text);
  let n = 0;
  for (const r of rows) {
    const first = String(r[0] || '').trim();
    if (!first) continue;
    const terms = first.split(' / ').map(s => s.trim()).filter(Boolean);
    const term = terms[0];
    const trs = String(r[1] || '').split(' / ').map(s => s.trim()).filter(Boolean);
    const ctx = String(r[2] || '').trim();
    const iv = parseInt(String(r[3] || '').trim(), 10);
    const due = /^\d{4}-\d{2}-\d{2}$/.test(String(r[4] || '').trim()) ? String(r[4]).trim() : '';
    let e = findVocab(term);
    if (!e) {
      e = { id: uid('v'), term, translation: trs[0] || '', lemma: term, pos: '', gender: '', article: '', alternatives: [], context: '', textId: '', textTitle: '', created: Date.now(), level: 0, interval: 0, due: todayISO() };
      ST.vocab.unshift(e);
    }
    e.alternatives = [...new Set([...(e.alternatives || []), ...terms.slice(1), ...trs.slice(1)].filter(Boolean))];
    if (!e.translation) e.translation = trs[0] || '';
    if (ctx && !e.context) e.context = ctx;
    else if (ctx && e.context && !e.context.includes(ctx)) e.context = e.context + ' ' + ctx;
    if (S.csvSrs) {
      if (!Number.isNaN(iv) && iv > 0) { e.interval = iv; e.level = Math.max(0, LADDER.indexOf(iv)); }
      if (due) e.due = due;
    }
    n++;
  }
  persist('vocab');
  renderVocab();
  window.trStatus(t('csvDone', n));
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
  drawLib();
  window.trStatus(t('txtDone', n));
}

// ---------- Kleine Rueckfrage mit Eingabefeld ----------
// Nutzt denselben Dialog wie ask(), nur mit sichtbarem Feld.
function askLine(label, value = '') {
  const d = $('#ask'), inp = $('#askIn');
  $('#askText').textContent = label;
  inp.classList.remove('hidden'); inp.value = value;
  $('#askYes').textContent = t('yes'); $('#askNo').textContent = t('cancel');
  d.showModal();
  inp.focus(); inp.select();
  return new Promise(res => {
    let done = false;
    const fin = v => { if (done) return; done = true; d.close(); inp.classList.add('hidden'); res(v); };
    $('#askYes').onclick = () => fin(inp.value);
    $('#askNo').onclick = () => fin(null);
    inp.onkeydown = ev => { if (ev.key === 'Enter') { ev.preventDefault(); fin(inp.value); } };
    d.oncancel = ev => { ev.preventDefault(); fin(null); };
  });
}

// ---------- Verdrahtung ----------
function wire() {
  $('#rBack').onclick = () => { savePos(); showView('lib'); };
  const wrap = $('#readerWrap');
  if (wrap) wrap.addEventListener('scroll', () => { clearTimeout(ST.posTimer); ST.posTimer = setTimeout(savePos, 600); });
  bindPicking($('#reader'), '.rw');
  bindPicking($('#pickLayer'), '.w');

  $('#pickBtn').onclick = () => { ST.pick = !ST.pick; pickSync(); };
  $('#input').addEventListener('input', () => { if (ST.pick && S.view === 'tr') drawPickLayer(); });
  $('#input').addEventListener('scroll', () => { const l = $('#pickLayer'); if (l && !l.classList.contains('hidden')) l.scrollTop = $('#input').scrollTop; });

  $('#vocAddBtn').onclick = addFromSearch;
  $('#vocTerm').addEventListener('keydown', e => { if (e.key === 'Enter') { e.preventDefault(); addFromSearch(); } });
  $('#vocList').addEventListener('click', e => {
    const b = e.target.closest('[data-del]');
    if (b) dropVocab(b.dataset.del);
  });
  $('#vPanel')?.addEventListener('click', e => {
    const b = e.target.closest('[data-del]');
    if (b) dropVocab(b.dataset.del);
  });

  $('#txAdd').onclick = addTextFromFile;
  $('#txExp').onclick = () => window.api.studyExportFile('texte', textsJson(), 'json');
  $('#txImp').onclick = async () => {
    const r = await window.api.studyImportFile('json');
    if (r) importTexts(r.text);
  };
  $('#vocCsvExp').onclick = () => window.api.studyExportFile('vokabeln', vocabCsv(), 'csv');
  $('#vocCsvImp').onclick = async () => {
    const r = await window.api.studyImportFile('csv');
    if (!r) return;
    try { importCsv(r.text); } catch { window.trStatus(t('csvErr')); }
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
  fillCats();
  wire();
  renderVocab();
  studyPanelSync();
  renderPanel();
  pickSync();
  applyI18n();
  if (S.view === 'voc') renderVocab();
})();
