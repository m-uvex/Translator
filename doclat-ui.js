'use strict';
// Dokument uebersetzen ueber DeepL.
//
// Die Kachel sitzt in der rechten Spalte des Uebersetzers (index.html #docBox):
// dort steht sie neben dem Text, ohne einen eigenen Arbeitsplatz zu belegen.
// Sie benutzt das eingestellte Sprachenpaar (S.src/S.tgt) und schreibt die
// uebersetzte Datei neben das Original.
//
// Der DeepL-Schluessel liegt im Hauptprozess in deepl.json; hier wird er nur
// eingetippt und nie wieder angezeigt. Die Oberflaeche erfaehrt nur, ob einer
// hinterlegt ist (letzte vier Zeichen als Kennzeichen).
(function () {
  const box = typeof $ === 'function' ? $('#docBox') : null;
  if (!box) return;

  let datei = '';
  const el = id => document.getElementById(id);
  const meldung = (text, schlecht) => {
    const m = el('docLatMsg');
    if (!m) return;
    m.textContent = text || '';
    m.classList.toggle('hidden', !text);
    m.classList.toggle('bad', !!schlecht);
  };
  const kurz = p => String(p || '').split(/[\\/]/).pop();

  async function schluesselStand() {
    const s = el('docLatKeyState');
    if (!s) return;
    try {
      const r = await window.api.doclatKey();
      s.textContent = r && r.set ? t('docLatKeySet') + (r.hint ? ' · …' + r.hint : '') : '';
    } catch { s.textContent = ''; }
  }

  const pick = el('docLatPick');
  if (pick) pick.addEventListener('click', async () => {
    try {
      const f = await window.api.doclatPick();
      if (!f) return;
      datei = f;
      el('docLatName').textContent = kurz(f);
      el('docLatShow').classList.add('hidden');
      meldung('');
    } catch (e) { meldung(String((e && e.message) || e), true); }
  });

  const keyBtn = el('docLatKeyBtn');
  if (keyBtn) keyBtn.addEventListener('click', async () => {
    const feld = el('docLatKey');
    const v = String(feld.value || '').trim();
    if (!v) return;
    try {
      await window.api.doclatKey(v);
      feld.value = '';
      await schluesselStand();
      meldung('');
    } catch (e) { meldung(String((e && e.message) || e), true); }
  });

  const go = el('docLatGo');
  if (go) go.addEventListener('click', async () => {
    if (!datei) { meldung(t('docLatNeedFile'), true); return; }
    if (!S.tgt) { meldung(t('docLatNoTgt'), true); return; }
    go.disabled = true;
    el('docLatShow').classList.add('hidden');
    meldung(t('docLatRun'));
    try {
      const r = await window.api.doclatRun(datei, S.src, S.tgt);
      if (!r || !r.ok) {
        meldung(r && r.error === 'noKey' ? t('docLatNoKey') : t('error', String((r && r.error) || '?')), true);
      } else {
        meldung(t('docLatDone', kurz(r.out)));
        const show = el('docLatShow');
        show.dataset.f = r.out;
        show.classList.remove('hidden');
      }
    } catch (e) {
      meldung(t('error', String((e && e.message) || e)), true);
    } finally { go.disabled = false; }
  });

  const show = el('docLatShow');
  if (show) show.addEventListener('click', () => {
    if (show.dataset.f) window.api.doclatShow(show.dataset.f);
  });

  // Beim Aufklappen den hinterlegten Schluessel anzeigen (kein Geheimnis, nur
  // die letzten vier Zeichen) und die Sprachrichtung nennen.
  box.addEventListener('toggle', () => { if (box.open) schluesselStand(); });
  schluesselStand();
})();
