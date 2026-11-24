import json, os

p = os.path.join(os.environ['APPDATA'], 'uebersetzer', 'study')
old = {e['id']: e for e in json.load(open(os.path.join(p, 'vocab.json.bak-20261124-111546'), encoding='utf-8'))}
new = json.load(open(os.path.join(p, 'vocab.json'), encoding='utf-8'))

FIELDS = [('term', 'Wort'), ('translation', 'Übersetzung'), ('article', 'Artikel'),
          ('gender', 'Geschlecht'), ('context', 'Beispielsatz'), ('pos', 'Wortart'),
          ('lemma', 'Grundform')]

changed = []
counts = {k: 0 for k, _ in FIELDS}
for e in new:
    if e.get('src') != 'es':
        continue
    o = old.get(e['id'])
    if o is None:
        continue
    diffs = [(k, lab, o.get(k, ''), e.get(k, '')) for k, lab in FIELDS if str(o.get(k, '')) != str(e.get(k, ''))]
    if not diffs:
        continue
    for k, _, _, _ in diffs:
        counts[k] += 1
    changed.append((e.get('term', ''), diffs))

changed.sort(key=lambda x: x[0].lower())

lines = []
lines.append('# Prüfliste: reparierte spanische Vokabeln')
lines.append('')
lines.append(f'{len(changed)} Einträge geändert, verteilt auf: ' +
             ', '.join(f'{lab} {counts[k]}' for k, lab in FIELDS if counts[k]) + '.')
lines.append('')
lines.append('Verglichen wird der Stand vor der Reparatur '
             '(`vocab.json.bak-20261124-111546`) mit dem jetzigen Bestand, nur Einträge mit '
             'Lernsprache Spanisch. Je Eintrag steht links der alte, rechts der neue Wert.')
lines.append('')

by_kind = {}
for term, diffs in changed:
    for k, lab, a, b in diffs:
        by_kind.setdefault(k, []).append((term, a, b))

for k, lab in FIELDS:
    if not by_kind.get(k):
        continue
    lines.append(f'## {lab} ({len(by_kind[k])})')
    lines.append('')
    for term, a, b in by_kind[k]:
        a = str(a).replace('|', '\\|')
        b = str(b).replace('|', '\\|')
        lines.append(f'- **{term}**: „{a}“ → „{b}“')
    lines.append('')

out = os.path.join('pruefliste-es.md')
open(out, 'w', encoding='utf-8').write('\n'.join(lines))
print('entries', len(changed), 'counts', counts, 'chars', len('\n'.join(lines)))
