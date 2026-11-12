# Problemliste — Electron-Fassung (Vorlage) gegen Rust-Fassung (Nachbau)

Stand: 12. November 2026. An diesem Projekt arbeiten zwei Agents parallel.
Jeder trägt hier ein, woran er gerade sitzt, damit der andere nicht dieselbe
Datei anfasst. Nach jedem erledigten Punkt: Eintrag nach „Erledigt“ verschieben,
mit Commit-Kurz-Hash.

## Gerade in Arbeit

- **Buffy:** Rechtschreibung und Editor-Leiste sind abgeschlossen (Commits
  `e921a63`, `ab212fc`, `6bd2990`). Als Nächstes: **P1 Modellauswahl im Editor**
  (CPU-Symbol mit verankertem Untermenü, getrennt nach Übersetzungs- und
  Schreibmodell). Wer parallel arbeitet, sollte `rust/src/app.rs` in
  `fn editor` / `fn models_window` vorerst meiden.
- **Zweiter Agent:** *hier eintragen.*

## Regeln (aus den Hinweisen des Nutzers)

1. **Die Electron-Fassung ist maßgeblich.** Beide Fassungen direkt
   nebeneinander laufen lassen und vergleichen, nicht nur Referenz-Screenshots
   aus `docs/referenz/screenshots/` ansehen. Kleinste Designunterschiede zählen.
2. **Keine freischwebenden, verschiebbaren Fenster.** Die Electron-Fassung
   benutzt ausschließlich an ihrem Auslöser verankerte Menüs (Dropdown direkt
   unter dem Knopf). Alle `egui::Window`-Dialoge sind zu prüfen und durch
   verankerte Menüs zu ersetzen.
3. **Keine Hover-Animationen** an Knöpfen. Gleicher Highlighter wie in
   `style.css` (`.ib:hover`, `.btn:hover`), keine Skalierung, kein Ein-/Ausblenden.
4. **Icon-Knöpfe:** runde Kreise (Rundung 999), 32 px, Symbol als SVG-Pfad mit
   `stroke-width 1.8`; kein grauer Kreis ohne Grund — Hintergrund nur bei Hover
   oder aktivem Zustand. Gilt für fast alle Knöpfe, nicht für alle.
5. **Akzentfarbe Babyblau**, keine dunkelblaue Schrift ohne Grund.
6. **Lesbarkeit vor Zierde:** Text muss auf seinem Hintergrund lesbar sein,
   Karten brauchen einen sichtbaren Hintergrund.

## Offene Punkte

| Nr. | Problem | Stand |
| --- | --- | --- |
| P1 | **Modellauswahl im Editor:** In der Vorlage ein Knopf mit CPU-Symbol, der direkt darunter ein Untermenü öffnet — getrennt nach Modell für Übersetzung im Editor und Modell fürs Schreiben im Editor. In der Rust-Fassung ist es eine einzelne Pille, die ein freischwebendes Fenster öffnet. | offen, als Nächstes |
| P2 | **Modellauswahl im Übersetzer** ist ein freischwebendes Fenster statt eines an der Pille verankerten Dropdowns. Darin fehlen außerdem die Symbole (Fähigkeiten wie Bild/Denken, MT-Kennzeichnung). | offen |
| P3 | **Kein blauer Aktivitäts-Punkt** neben dem Neuladen-Knopf im Übersetzer; die Vorlage zeigt dort an, dass Modelle geladen bzw. aktiv sind. | offen |
| P4 | **Document tray** („Aktuell geöffnete Seiten“) sieht komplett falsch aus: Form der Seiten, Darstellung der Seiten selbst und Positionierung der Elemente weichen ab. | offen |
| P5 | **Dunkelblaue Schrift ohne Grund** (Übersetzer, Document tray, weitere Stellen) — Kontrast und Farbe gegen `style.css` prüfen. | offen |
| P6 | **Icon-Knopf-Grundsätze** sind bei fast allen Knöpfen verletzt (graue Kreise ohne Grund, Squircles, falsche Größen). Im Editor größtenteils behoben, anderswo nicht. | offen |
| P7 | **Editor: größere Probleme** jenseits der Leiste (Seitenmaß zu breit und zu kurz, Statusleiste am Boden, Darstellung der Fläche). | offen |
| P8 | **Hover-Animationen** der Knöpfe entfernen; stattdessen den Highlighter der Vorlage (siehe Regel 3). | offen |
| P9 | **Unlesbarer Text, fehlende Symbole, fehlende Hintergrundkarten** in mehreren Ansichten; auch in der Fensterleiste (Minimieren/Maximieren/Schließen). | offen |
| P10 | **Ränder und Abstände:** Elemente zu nah an Rändern, sinnlose Trennstriche, zu große Abstände zwischen Teilen einer Arbeitsfläche, untere Karte/Leiste zu groß. | offen |
| P11 | **Dialoge:** Schließen-Kreuz der Einstellungen verschwindet beim Überfahren; Mini-Übersetzer sieht falsch aus; Stil-Optionen im Kontext öffnen ein Übersetzer-Panel statt eines eigenen Popups; Synonyme-Fenster. | offen |
| P12 | **Installation:** Installations-Wizard, Installations- und Deinstallationsfunktion, NLLB-200-Unterstützung (Server und Modellverwaltung). | offen |
| P13 | **Squircles statt Kreisen** an einzelnen Stellen; Rundungen insgesamt gegen `style.css` abgleichen. | offen |
| P14 | **Alle bestehenden Menüs prüfen:** freischwebende Fenster durch verankerte Menüs ersetzen (Regel 2). | offen |
| P15 | **Übersetzer-Textfelder** sollen keinen Trennstrich zu ihren Knöpfen haben und sich fast bis zum Boden strecken. | offen |
| P16 | **Überschriften-Dropdown** im Editor: Verhalten und Aussehen gegen die Vorlage prüfen (gesetzt sind H1–H3 über `editor::set_heading`). | zu prüfen |

## Erledigt

- **Ollama-Verbindung:** Voreinstellung `127.0.0.1`, Rückfall auf die andere
  Schreibweise, sichtbarer Fehlertext im Modellbereich — `6bd2990`.
- **Editor-Werkzeugleiste:** Überschriften im Dropdown, drei KI-Knöpfe fürs
  ganze Dokument, Modellpille rechts, Neuladen rechts vom Modellknopf, runde
  Bögen in Symbolen — `ab212fc`.
- **Rechtschreibung:** geschlängelte Wellenlinie (Sinus), Rechtsklick-Menü mit
  Wörterbuch-Vorschlägen, Ignorieren, Ins-Wörterbuch; Prüffenster am ersten
  Fund — `e921a63`.
- **Kontextleiste:** sieben Aktionen in der Reihenfolge der Vorlage, Stil- und
  Mini-Übersetzer-Knopf ergänzt — `c69c0aa`, `87ffeea`.
