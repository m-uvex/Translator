# Wörterbücher

Hier liegen die Hunspell-Wörterbücher, die beide Fassungen des Übersetzers
verwenden: die Electron-Fassung über `speller()` in `main.js` und die
Rust-Fassung über `rust/src/spell.rs`. Jeder Ordner enthält `index.aff`,
`index.dic` und die zugehörige Lizenz als `LICENSE.txt`.

Die fünf npm-Pakete wurden am 12. November 2026 entpackt, die
`package.json` bereits als Abhängigkeiten der Electron-Fassung nennt. Es sind
also genau die Fassungen, die `nspell` dort geladen hat.

| Ordner | npm-Paket | Herkunft | Lizenz (Volltext in `LICENSE.txt`) |
| --- | --- | --- | --- |
| `de` | dictionary-de 3.0.0 | igerman98, Björn Jacke | GPL 2.0 / LGPL 2.1 / MPL 1.1 |
| `de-ch` | — | LibreOffice `de_CH_frami` (Schweizer Schreibweise, von igerman98 abgeleitet) | GPL 2.0 / LGPL 2.1 / MPL 1.1 |
| `en` | dictionary-en 4.0.0 | en_US Hunspell Dictionary 2020.12.07, SCOWL (wordlist.sourceforge.net) | SCOWL-Lizenz (freizügig, Copyright-Hinweise bleiben) |
| `es` | dictionary-es 4.0.0 | Diccionario RLA für LibreOffice/Apache OpenOffice | Dreifachlizenz GPL / LGPL / MPL |
| `nl` | dictionary-nl 2.0.0 | OpenTaal (opentaal.org) | Revised BSD und/oder CC BY 3.0 |
| `tr` | dictionary-tr 2.0.0 | Harun Reşit Zafer | MIT |
| `ar` | — | lag bereits im Projekt, Herkunft nicht dokumentiert | keine Lizenzdatei vorhanden |

Zwei Ordner wurden nachbearbeitet. `dict/de-ch` lag als ISO-8859-1 vor (`SET ISO8859-1`) und wurde nach UTF-8 umgewandelt, weil `hunspell-wasm` die Dateien als UTF-8 einliest; `SET` steht dort jetzt auf `UTF-8`, der Wortbestand ist unverändert. Im türkischen `.aff` benutzt das Paket das Flag `0`, das `spellbook` ablehnt (`MalformedFlag(ZeroFlag)`); Definition und Verwendung heißen deshalb `30000`, auch dort ist der Wortbestand unverändert. Beide Fassungen lesen die Dateien über `hunspell-wasm` (Electron) bzw. `spellbook` (Rust).

## Pflege

Ein Wörterbuch austauschen heißt: `index.aff` und `index.dic` im jeweiligen
Ordner ersetzen und die Lizenzdatei mitziehen. Eine neue Sprache braucht einen
Ordner `dict/<code>/` und einen Eintrag in `DICTS` in `rust/src/app.rs`
(beziehungsweise in `editor.js` für die Electron-Fassung). Der Test
`gebuendelte_woerterbuecher_laden` in `rust/src/spell.rs` prüft, dass jedes
Wörterbuch sich parsen lässt und ein bekanntes Wort kennt.
