# Referenzmaterial: Übersetzer 1.5.0 (Electron)

Dieser Ordner ist die Bild- und Verhaltensreferenz für die Rust-Neufassung. Die
Aufnahmen stammen aus der unveränderten Electron-Oberfläche (`index.html`,
`style.css`, `renderer.js`, `editor.js`), die für die Bildschirmfotos in einem
Browser mit Attrappen der Electron-Brücke geladen wurde (`harness.html`). Der
Harness ersetzt ausschließlich `window.api` und die beiden lokalen Dienste; am
Markup, an den Skripten und am CSS wurde nichts geändert.

## Aufnahmen

| Datei | Inhalt |
| --- | --- |
| `screenshots/01-uebersetzer.png` | Übersetzer, Hauptansicht mit Eingabe, Ausgabe, Stil-Seitenleiste |
| `screenshots/02-sprachauswahl.png` | Sprach-Popup mit Suchfeld, Pins und „Sprache erkennen“ |
| `screenshots/03-modellauswahl.png` | Modell-Popup mit Bild- und Denk-Kennzeichnung |
| `screenshots/04-einstellungen-allgemein.png` | Einstellungen, Allgemein und Weitere Einstellungen |
| `screenshots/05-einstellungen-modelle.png` | Einstellungen, Verbindungen, Übersetzung und NLLB-Modelle |
| `screenshots/06-editor.png` | Texteditor mit Werkzeugleiste, Seite und Statusleiste |
| `screenshots/07-kontextleiste.png` | Schwebende Kontextleiste über einer Textauswahl |
| `screenshots/08-dokumentenablage.png` | Dokumentenablage mit Vorschaukarten |
| `screenshots/09-woerterbuch.png` | Persönliches Wörterbuch mit Export/Import |
| `screenshots/10-mini-uebersetzer.png` | Mini-Übersetzer mit Stilgruppen |
| `screenshots/11-arabisch-rtl.png` | Arabische Oberfläche, rechts-nach-links |
| `screenshots/12-helles-theme.png` | Helles Theme |
| `screenshots/13-rechtschreibung.png` | Rechtschreibprüfung der Rust-Fassung, deutsches Wort mit Vorschlägen |
| `screenshots/14-kontextleiste.png` | Kontextleiste der Rust-Fassung über einer Textauswahl |
| `screenshots/15-wellenlinien.png` | Wellenlinien unter falsch geschriebenen Wörtern |
| `screenshots/16-kontextleiste-stil.png` | Stil-Auswahl der Kontextleiste |

Hinweis: Die Bilder 01 bis 12 zeigen die Electron-Fassung, aufgenommen über
den Harness. Bild 13 zeigt die Rust-Fassung selbst (echtes Fenster, 1600 x 950,
UI-Sprache Englisch, Wörterbuchsprache Deutsch).

Hinweis: Das Vorschaufenster der Aufnahmeumgebung blendet links oben ein
eigenes Badge ein. Es wurde in den Bildern mit der Hintergrundfarbe überdeckt,
weil es nicht zur Anwendung gehört.

## Aufbau der Anwendung

Drei Arbeitsplätze teilen sich eine 56 px breite Navigationsleiste: Übersetzer
(`#vT`), Textbearbeitung (`#vE`) und Dokumentenablage (`#vL`). Darüber liegt die
eigene 32 px hohe Fensterleiste mit Minimieren, Maximieren und Schließen
(`frame: false` in `main.js`). Unten im Rail sitzt das Zahnrad für die
Einstellungen.

### Übersetzer

Der Kopf enthält links Quellsprache, Tauschknopf und Zielsprache, rechts Modell
und Aktualisieren. Die Mitte bilden zwei gleich große Karten (Eingabe, Ausgabe).
Rechts steht eine 300 px breite Seitenleiste mit vier Stilgruppen (Art, Ton,
Empfänger*in, Absender*in), Zusammenhang und Glossar.

- **Sprach-Popup** (`langPop`): Suchfeld, Sterne zum Anpinnen, Trenner zwischen
  angepinnten und übrigen Sprachen, „Sprache erkennen“ nur als Quellsprache und
  nur bei allgemeinen LLMs. Pins sind je Engine getrennt (`S.pins.ollama`,
  `S.pins.nllb`).
- **Modell-Popup** (`modelPop`): Einträge aus `/api/tags` plus installierte
  NLLB-Modelle als `nllb:<id>`. Ein Auge markiert Bildfähigkeit, eine Glühbirne
  Denkfähigkeit (`/api/show` → `capabilities`).
- **Ausgabe**: Wörter sind einzeln anklickbar und öffnen Wortalternativen
  (`#alt`). Nur bei LLM-Modellen; NLLB und TranslateGemma zeigen stattdessen
  einen Hinweis.
- **Fußzeilen**: links Anhängen, Zeichenzahl, „Übersetzen“; rechts Status,
  Stopp-Achteck nur während eines Laufs (nach 150 ms), Kopieren.
- **Bilderstapel**: bis zu drei Vorschauen als gefächerter Stapel, Klick klappt
  ein Raster auf (4 Spalten, bei Bedarf 5, max. 40 % der Textfläche), erneuter
  Klick in das Textfeld klappt per FLIP-Animation zusammen.
- **Dateiimport**: Bilder als base64, PDF/DOCX über IPC (`mammoth`,
  `pdf-parse`), Text/Markdown/CSV/SRT/JSON/HTML direkt.

### Texteditor

Werkzeugleiste mit Dateioperationen (Neu, Öffnen, Speichern), Rückgängig,
Schrift, Größe, Absatzart, Fett/Kursiv/Unterstrichen/Durchgestrichen, Listen,
Zitat, vier Ausrichtungen, Link, Formatierung entfernen und drei
Ganztext-KI-Aktionen. Oben rechts schwebt der Modellknopf (CPU-Chip). Darunter
liegt die A4-Seite (`max-width: 794px`, `min-height: 1040px`, Padding 64/72 px)
auf einer Leinwand. Unten die Statusleiste mit Wortzahl, Statustext,
Sitzungsliste, persönlichem Wörterbuch und Wörterbuchsprache.

- **Kontextleiste** (`#ctxBar`): erscheint bei Auswahl und bietet Verkürzen,
  Verlängern, Zusammenfassen, Probelesen, Synonyme, Stil und Mini-Übersetzer.
- **KI-Operationen**: laufen über `withBusy`, markieren den bearbeiteten Text
  mit `.ai-processing` (Partikel-Spoiler) und schreiben das Ergebnis über
  `applyContent`, damit ein einzelnes Undo zurückführt.
- **Probelesen**: wortbasierter LCS-Diff, grüne `.diff-ins`, rote
  `.diff-del`-Durchstreichungen, Panel rechts; Schließen verwirft offene
  Änderungen.
- **Markdown-Modus** (Standard aus): `#`, `##`, `###`, `>`, `-`, `*`, `1.` am
  Zeilenanfang, `**fett**`, `*kursiv*`, `~~durchgestrichen~~`, `` `Code` ``,
  `---` und Strg+Enter als Seitenumbruch.
- **Rechtschreibung**: Wörterbücher aus `node_modules/dictionary-<lang>` oder
  `dict/<lang>`, Wellen als Canvas-Sinuslinie (`#spellOverlay`), Komma-
  Heuristik für Deutsch, persönliches Wörterbuch in
  `%APPDATA%\Uebersetzer\personal-dictionary.json`. Seit dem 12. November
  2026 liegen alle sechs Wörterbücher im Projekt unter `dict/<lang>` (Herkunft
  und Lizenzen in `dict/README.md`), sodass beide Fassungen ohne
  `node_modules` prüfen können.
- **Sitzungsliste** (`#docDock`): Karten der seit dem Start geöffneten
  Dokumente, ziehbar, mit Schließen-Kreuz; nur im Speicher.
- **Export**: Markdown, Text, PDF (über `printToPDF`).

### Dokumentenablage

Kartenraster mit Seitenvorschau, Name und Änderungsdatum, Öffnen per Klick,
Entfernen per Kreuz mit Rückfrage und FLIP-Animation. Ablage als
`%APPDATA%\Uebersetzer\library\<id>.html` plus `<id>.json` (`id`, `name`,
`created`, `updated`).

## Datenfluss und Zustand

Alle Einstellungen liegen in `localStorage` unter `s` (`D` sind die Defaults,
`S` der aktive Zustand). Felder: `url`, `nllb`, `model`, `pref`, `theme`,
`uiLang`, `start`, `workspace`, `helper`, `src`, `tgt`, `pins`, `style`,
`uiMode`, `gloss`, `ctxt`, `temp`, `numctx`, `bootModel`, `profanityFilter`,
`profanityStyle`, `edMarkdown`, `edSpellAnim`, `autoCopy`, `autoCopyTyped`,
`sys`. Editor-Zustand (`ed`, `edStyles`, `edWrite`, `edTrans`, `dictLang`,
`edSpell`, `edDoc`, `edDocId`) liegt im selben Objekt, wird aber nicht von
„Alles zurücksetzen“ erfasst.

## Modelltypen

`engineOf(m)` unterscheidet `nllb:` (Python-Server), `omt:` (vorbereitet, nicht
verfügbar), `translategemma` (eigener Prompt mit zwei Leerzeilen) und alle
übrigen Ollama-Modelle. MT-Modelle und TranslateGemma brauchen eine feste
Ausgangssprache. Denk-Modelle bekommen `think: false`, GPT-OSS `think: 'low'`.

## Gestaltung

Eine Akzentfarbe (Babyblau), neutrale warme Grautöne, ein Standardradius von
18 px, Pillen für Buttons, 22 px für Dialoge, runde Icon-Knöpfe in
Werkzeugleiste und Kontextleiste, Squircle (12 px) außerhalb. Icons sind
SVG-Pfade mit `viewBox="0 0 24 24"`, `stroke-width="1.8"`,
`stroke-linecap="round"`, `stroke-linejoin="round"`.

| Rolle | Hell | Dunkel |
| --- | --- | --- |
| `--bg` | `#f7f7f8` | `#171717` |
| `--card` | `#ffffff` | `#212121` |
| `--soft` | `#f1f1f3` | `#2b2b2d` |
| `--text` | `#1d1d1f` | `#ececec` |
| `--muted` | `#6e6e73` | `#9a9a9f` |
| `--line` | `#e6e6ea` | `#333336` |
| `--accent` | `#9ccff5` | `#5aa9e0` |
| `--accent-hover` | `#cfe7fb` | `#82bdea` |
| `--accent-ink` | `#2a74b5` | `#8cc8f5` |
| `--on-accent` | `#0e2a44` | `#08213a` |

## Sechs Sprachen

Deutsch, Englisch, Niederländisch, Spanisch, Türkisch, Arabisch. Arabisch
setzt `dir="rtl"`: Rail rechts, Werkzeugleiste gespiegelt, absolute Positionen
entsprechend angepasst. Texte sind geschlechtsneutral mit Sternchen
(„Absender*in“).

## Portierungsstand der Rust-Fassung

Die Rust-Fassung liegt in `rust/` und ersetzt die Electron-Hülle durch ein
einzelnes natives Programm (eframe/egui). Sie nutzt dieselben lokalen Dienste,
dieselben Prompt-Regeln und dieselben Ablagedateien.

| Bereich | Stand |
| --- | --- |
| Fenster, eigenes Rahmenlayout, drei Arbeitsplätze | umgesetzt |
| Sechssprachige Oberfläche, Arabisch von rechts nach links | umgesetzt (Arabisch ohne Kursiv-/Buchstabenverbindung, siehe Doku) |
| Einstellungen inkl. Theme, Startansicht, Arbeitsplätze, Verbindungen, NLLB-Verwaltung | umgesetzt |
| Modellliste, Fähigkeiten, Vorwärmen, Modellwahl | umgesetzt |
| Übersetzen über Ollama (Streaming), TranslateGemma, NLLB zeilenweise | umgesetzt |
| Stoppen, Kopieren, automatisches Kopieren, Zeichenzahl | umgesetzt |
| Stilgruppen, Zusammenhang, Glossar | umgesetzt |
| Schimpfwortfilter, Sternchen-Ersetzung, geschützte Zeichenketten | umgesetzt |
| Bilder, PDF-/DOCX-Import, Wortalternativen, Ersatzwort-Anpassung | offen |
| Editor: Markdown-Formatierung mit Umschaltern, Undo/Redo, Sitzungsliste, Diff-Probelesen, KI-Operationen, Ablage, Export | umgesetzt (ohne Rich-Text-Seite) |
| Rechtschreibprüfung mit Hunspell (spellbook, Wellenlinien im Text, Prüffenster mit Vorschlägen, persönliches Wörterbuch) | umgesetzt |
| Schwebende Kontextleiste (Verkürzen, Verlängern, Zusammenfassen, Probelesen, Synonyme, Stil umschreiben, Mini-Übersetzer) | umgesetzt |
| Visuelle Angleichung an die Electron-Fassung | teilweise: Symbole als Vektoren, Segoe UI als Schrift; Farben, Maße und Reihenfolge offen (Doku.txt Abschnitt 15) |
| Seitenumbruch, PDF-Export | offen |
