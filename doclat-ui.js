'use strict';
// Dokument uebersetzen - offline mit dem Modell des Uebersetzers.
//
// Die Kachel sitzt in der rechten Spalte (index.html #docBox), neben dem Text,
// dessen Sprachenpaar sie benutzt. Uebersetzt wird mit dem eingestellten
// Modell (TranslateGemma, NLLB oder ein anderes lokales Modell): kein
// Schluessel, keine Cloud, kein Wasserzeichen.
//
// Der Fortschritt kommt aus dem Hauptprozess (doclat:progress) und wird als
// Balken und Prozentzahl gezeigt. Die Datei landet neben dem Original.
(function () {
  const box = document.getElementById('docBox');
  if (!box) return;

  const el = id => document.getElementById(id);
  const kurz = p => String(p || '').split(/[\\/]/).pop();
  let datei = '';

  const meldung = (text, schlecht) => {
    const m = el('docLatMsg');
    if (!m) return;
    m.textContent = text || '';
    m.classList.toggle('hidden', !text);
    m.classList.toggle('bad', !!schlecht);
  };

  const tgtSel = el('docLatTgt');

  // Zielsprachen kommen aus derselben Liste wie der Uebersetzer; damit passt
  // die Auswahl zum eingestellten Modell (Ollama-Codes oder NLLB-Codes).
  function buildTgt() {
    if (!tgtSel) return;
    const list = (typeof tgtLangs === 'function') ? tgtLangs() : [];
    if (!list.length) return;
    const keep = tgtSel.value || (typeof S !== 'undefined' ? S.tgt : '');
    tgtSel.textContent = '';
    for (const [code] of list) {
      const o = document.createElement('option');
      o.value = code;
      o.textContent = (typeof langLabel === 'function') ? langLabel(code) : code;
      tgtSel.appendChild(o);
    }
    if (list.some(([c]) => c === keep)) tgtSel.value = keep;
  }

  function showModel() {
    const m = el('docLatModel');
    if (!m) return;
    const name = (typeof S !== 'undefined' && S.model) ? S.model : '';
    m.textContent = name ? t('docLatModel', name) : '';
    m.classList.toggle('hidden', !name);
  }

  function setProgress(p) {
    const bar = el('docLatProg');
    const fill = bar && bar.querySelector('i');
    if (!bar || !fill) return;
    const on = p && p.total > 0 && p.percent < 100;
    bar.classList.toggle('hidden', !on && !(p && p.percent === 100));
    fill.style.width = (p && Number.isFinite(p.percent) ? p.percent : 0) + '%';
    if (p && p.total) meldung(t('docLatProg', Math.round(p.percent || 0)));
  }

  const pick = el('docLatPick');
  if (pick) pick.addEventListener('click', async () => {
    try {
      const f = await window.api.doclatPick();
      if (!f) return;
      datei = f;
      el('docLatName').textContent = kurz(f);
      el('docLatShow').classList.add('hidden');
      setProgress(null);
      el('docLatProg').classList.add('hidden');
      meldung('');
    } catch (e) { meldung(String((e && e.message) || e), true); }
  });

  const go = el('docLatGo');
  if (go) go.addEventListener('click', async () => {
    if (!datei) { meldung(t('docLatNeedFile'), true); return; }
    const tgt = tgtSel ? tgtSel.value : '';
    if (!tgt) { meldung(t('docLatNoTgt'), true); return; }
    go.disabled = true;
    el('docLatShow').classList.add('hidden');
    meldung(t('docLatRun'));
    const engine = (typeof isMt === 'function' && isMt(S.model)) ? 'nllb' : 'ollama';
    try {
      const r = await window.api.doclatRun(datei, S.src, tgt, engine, S.model, S.url, S.nllb);
      if (!r || !r.ok) {
        meldung(t('error', String((r && r.error) || '?')), true);
      } else {
        meldung(t('docLatDone', kurz(r.out)));
        const show = el('docLatShow');
        show.dataset.f = r.out;
        show.classList.remove('hidden');
      }
    } catch (e) {
      meldung(t('error', String((e && e.message) || e)), true);
    } finally {
      go.disabled = false;
      const bar = el('docLatProg');
      if (bar) setTimeout(() => { bar.classList.add('hidden'); const i = bar.querySelector('i'); if (i) i.style.width = '0'; }, 800);
    }
  });

  const show = el('docLatShow');
  if (show) show.addEventListener('click', () => {
    if (show.dataset.f) window.api.doclatShow(show.dataset.f);
  });

  if (window.api && typeof window.api.onDoclatProgress === 'function') {
    window.api.onDoclatProgress(p => setProgress(p));
  }

  // Beim Aufklappen die Sprachliste nachziehen: Modelle und Sprachen koennen
  // erst nach dem Start geladen sein.
  box.addEventListener('toggle', () => { if (box.open) { buildTgt(); showModel(); } });

  buildTgt();
  showModel();
})();
