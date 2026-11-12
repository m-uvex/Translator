# Problemliste — Electron-Fassung (Vorlage) gegen Rust-Fassung (Nachbau)

Stand: 12. November 2026. An diesem Projekt arbeiten zwei Agents parallel.
Jeder trägt hier ein, woran er gerade sitzt, damit der andere nicht dieselbe
Datei anfasst. Nach jedem erledigten Punkt: Eintrag nach „Erledigt“ verschieben,
mit Commit-Kurz-Hash.

## Gerade in Arbeit

- **Buffy:** P1, P2, P3, P7, P11, P12, P14, P16, P17, P18, P19, P20, P21, P22,
  P23, P25, P26, P27, P28, P29, P30, P31, P33, P36, P39, P40 und P41 sind
  erledigt, P9 nachgesehen (Rest: Dokumentenablage, Fensterleiste).
  Nachgetragen: P32 bis P41 (fehlende Funktionen) und P42 bis P55 (Rückmeldung
  des Nutzers zum Editor). P42 bis P51 sind erledigt. Offen bleiben P24, P32,
  P34, P35, P37, P38 und P52 bis P55.
- **Zweiter Agent (`rust-oberflaeche`):** Uebersetzer, Seitenleiste,
  Fensterleiste, Navigationsleiste und Dokumentenablage sind angeglichen
  (`1863576`). P15 und P4 sind damit ganz erledigt, P5/P6/P8/P10/P13 fuer diese
  Ansichten. Als Naechstes: Rest von P5/P6/P10/P13 in Editor und Dialogen,
  danach P9/P11/P14. `fn editor` und `fn models_menu` bleiben unberuehrt.

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

## Gemeldete Punkte prüfen

Meldungen eines zweiten Agents (oder des Nutzers) werden **vor** dem Reparieren
an beiden Fassungen nachgeprüft — Codevergleich und, wo möglich, ein Lauf.
Bestaetigt sich eine Meldung, wandert sie als eigener Punkt in die Tabelle.
Erweist sie sich als unzutreffend, bleibt sie als Eintrag stehen und bekommt
einen Kommentar mit dem Beleg (Datei, Zeile, Lauf), damit der andere Agent
nachvollziehen kann, wer daneben lag. Erst wenn beide dieselbe Stelle gleich
lesen, wird der Punkt geschlossen.

## Offene Punkte

| Nr. | Problem | Stand |
| --- | --- | --- |
| P1 | **Modellauswahl im Editor:** CPU-Knopf mit verankertem Untermenü, getrennt nach Schreib- und Übersetzungsmodell. | erledigt (`e3339ea`) |
| P2 | **Modellauswahl im Übersetzer:** freischwebendes Fenster ohne Symbole. | erledigt (`e3339ea`) |
| P3 | **Blauer Aktivitäts-Punkt** am Neuladen-Knopf, solange Modelle geladen werden; die Fähigkeiten des Modells stehen als Symbole im Modellknopf. | erledigt (`2e700b7`) |
| P4 | **Document tray** („Aktuell geöffnete Seiten“) sieht komplett falsch aus: Form der Seiten, Darstellung der Seiten selbst und Positionierung der Elemente weichen ab. | erledigt (`1863576`) |
| P5 | **Dunkelblaue Schrift ohne Grund** (Übersetzer, Document tray, weitere Stellen) — Kontrast und Farbe gegen `style.css` prüfen. | Übersetzer und Seitenleiste erledigt (`1863576`); übrige Ansichten offen |
| P6 | **Icon-Knopf-Grundsätze** sind bei fast allen Knöpfen verletzt (graue Kreise ohne Grund, Squircles, falsche Größen). Im Editor größtenteils behoben, anderswo nicht. | Fensterleiste, Navigationsleiste, Übersetzer, Seitenleiste und Ablage erledigt (`1863576`); Dialoge offen |
| P7 | **Editor: größere Probleme** jenseits der Leiste (Seitenmaß zu breit und zu kurz, Statusleiste am Boden, Darstellung der Fläche). | erledigt (`aecc093`), am laufenden Fenster geprüft |
| P8 | **Hover-Animationen** der Knöpfe entfernen; stattdessen den Highlighter der Vorlage (siehe Regel 3). | in den bearbeiteten Ansichten erledigt (`1863576`); Rest prüfen |
| P9 | **Unlesbarer Text, fehlende Symbole, fehlende Hintergrundkarten** in mehreren Ansichten; auch in der Fensterleiste (Minimieren/Maximieren/Schließen). | nachgesehen: Übersetzer, Stil-Leiste, Einstellungen, Wörterbuch und Rechtschreibfenster lesbar und mit Karten; alle 93 Symbolnamen vorhanden. Offen bleiben Dokumentenablage und Fensterleiste (nicht nachgesehen) |
| P10 | **Ränder und Abstände:** Elemente zu nah an Rändern, sinnlose Trennstriche, zu große Abstände zwischen Teilen einer Arbeitsfläche, untere Karte/Leiste zu groß. | Übersetzer, Seitenleiste und Ablage erledigt (`1863576`); Editor offen |
| P11 | **Dialoge:** Stilfenster und Mini-Übersetzer hängen an der Kontextleiste; Mini-Übersetzer und Synonyme sind nach der Vorlage gebaut, Schließen-Kreuz bleibt sichtbar. | erledigt, echter Aufruf aus der Leiste nicht durchgespielt |
| P12 | **Installation:** Wizard (`rust/Installieren.bat`), Deinstallation (`rust/Uninstall.ps1`) und NLLB-200 samt Modellverwaltung. | erledigt (`2e700b7`); Autostart siehe P17 |
| P13 | **Squircles statt Kreisen** an einzelnen Stellen; Rundungen insgesamt gegen `style.css` abgleichen. | bearbeitete Ansichten erledigt (`1863576`); Dialoge offen |
| P14 | **Alle Menüs prüfen:** freischwebende Fenster durch verankerte Menüs bzw. mittige, unverschiebbare Dialoge ersetzen (`pop_at`, `dialog`). | erledigt, Stilfenster und Mini-Übersetzer hängen noch nicht an der Kontextleiste |
| P15 | **Übersetzer-Textfelder** sollen keinen Trennstrich zu ihren Knöpfen haben und sich fast bis zum Boden strecken. | erledigt (`1863576`) |
| P16 | **Überschriften-Dropdown** im Editor: Verhalten und Aussehen gegen die Vorlage prüfen (gesetzt sind H1–H3 über `editor::set_heading`). | erledigt (`1dac5ba`): volle Namen „Absatz/Überschrift 1–3“, Pille wie `.tsel`, Stelle hinter Rückgängig/Wiederholen |
| P17 | **NLLB-Server startet nicht mit der App:** Schalter in den Einstellungen fehlte, der Server musste von Hand gestartet werden. | erledigt (`b0ebe30`) |
| P18 | **Modellbezogene Symbole zu nah am Modellnamen** im Übersetzer-Arbeitsplatz: Auge (Sehen) und Denk-Symbol kleben am Namen; Abstand wie in der Vorlage (`.mpill` in `style.css`) herstellen. | erledigt (`8273e8d`), 6 px Abstand wie `.mi` |
| P19 | **Karte der Translation-Style-Optionen rutscht aus dem Fenster:** Die rechte Kante der Stil-Seitenleiste ist nicht mehr zu sehen; Breite und Abstände so setzen, dass die Karte vollständig im Fenster liegt. | erledigt (`8273e8d`), Leiste vollständig im Fenster |
| P20 | **Wörterzähler in der Statusleiste steht nicht mittig:** Zahl und Meldung kleben am oberen Rand der Karte, während die Knöpfe rechts mittig sitzen. | erledigt (`bce4125`), mittig am laufenden Fenster geprüft |
| P21 | **Schreibseite wird zu früh abgeschnitten:** In der Vorlage endet die Seite erst an der Kante der Fläche, auf der sie liegt; sie muss die Fläche immer ausfüllen. | erledigt (`bce4125`), Seite endet an der Kante der Fläche |
| P22 | **Fensterecken sind eckig:** Die Vorlage läuft im normalen Windows-Fenster mit abgerundeten Ecken, die Rust-Fassung ohne Titelleiste hat eckige. | erledigt (`bce4125`), Ecken am Fenster gerundet |
| P23 | **Modell-Auswahl-Menü lässt sich nicht öffnen:** Es blitzt kurz auf und schließt sich sofort wieder, im Übersetzer wie im Text-Arbeitsplatz. | erledigt (`8273e8d`), nicht angeklickt geprüft |
| P24 | **Editor-Leiste: Schrift- und Größenauswahl fehlen** ganz, die Vorlage hat drei Auswahlfelder `.tsel` nebeneinander (Schrift, Größe, Überschrift). | offen |
| P25 | **Schreibseite hängt über die Kante ihrer Fläche und klebt links** statt mittig: Sie lief genau auf die gerundete Kante zu und `allocate_exact_size` setzte sie an den linken Rand. | erledigt (`f3bb7f6`), mittig und 8 px über der Kante; Sichtprüfung stand noch aus, weil das Fenster beim Aufnehmen verdeckt war |
| P26 | **Einstellungen wirken fremd gegenüber der Vorlage:** Texte und Auswahlfelder zu nah an den Innenkanten, Knöpfe im dunklen Modus zu hell unterlegt, hässliche Häkchen-Kästchen, schlecht greifbare Zahlenfelder („Slider“ unter dem System-Prompt), insgesamt egui-Standardoptik. | erledigt (`48bf9bd`): Zeilen mit fettem Titel und Pille rechts, Schalter als „An/Aus“-Liste, Zahlen und Textfelder auf weichem Grund, 14 px Innenabstand, Auswahl im Menü nicht mehr grell |
| P27 | **Sprachauswahlmenü falsch dargestellt:** Namen und Sterne standen in eigenen Rahmen, die Scrollleiste überdeckte die Stern-Knöpfe, und für Koreanisch, Japanisch und Chinesisch fehlten die Schriften (leere Kästchen). | erledigt (`66919e5`): Zeilen wie `.row` mit Hover statt Rahmen, Stern als reines Symbol mit Platz vor der Leiste, Malgun/Yu Gothic/YaHei nachgeladen |
| P28 | **„MT“ neben TranslateGemma ist unverständlich** und als Buchstaben-Kennzeichnung unerwünscht; Symbole mit Buchstaben sind ausdrücklich nicht erlaubt. | erledigt (`aabba79`): Pfeil-Symbol `swap` mit erklärendem Hinweis `mt_tip` |
| P29 | **„Alternativmodell“ bleibt leer:** Die Einstellung für Ersatzwörter war ein freies Feld ohne Auswahlmöglichkeiten. | erledigt (`5135758`): Auswahlliste „Automatisch“ plus alle allgemeinen Ollama-Modelle |
| P30 | **Oberflächensprache lässt sich nicht live ändern:** Nur Englisch wirkte sofort, alle anderen brauchten einen Neustart. | erledigt (`59c46c5`): Sprache wird in jedem Bild aus den Einstellungen gelesen |
| P31 | **Wörterbuchmenü falsch aufgebaut:** Statt eines Popups über dem Knopf erschien ein mittiger Dialog mit Häkchen, Trennstrichen und umrandeten Sprachknöpfen. | erledigt (`fba0ec7`): Popup oberhalb des Globus-Knopfs, Zeilen wie `.row`, Haken bei „Rechtschreibung prüfen“ |
| P32 | **Bilder im Übersetzer fehlen ganz:** Anhängen, Hineinziehen, Einfügen aus der Zwischenablage, Vorschautapel und der Vision-Rückfall sind nicht umgesetzt; im Rust-Code steht überall `has_images: false`, Datei-Ablage (`dropped_files`) gibt es nicht. | offen |
| P33 | **Wortalternativen fehlen:** In der Vorlage ist jedes Wort der Ausgabe anklickbar (`#alt`, `reword`) und lässt sich im Satz ersetzen; bei uns ist die Ausgabe ein reines Textfeld. | erledigt (`54376b0`): Ausgabe als Layout, Klick auf ein Wort öffnet die Alternativen, Auswahl formuliert den Satz neu |
| P34 | **Modell lässt sich nicht nachladen:** Die Vorlage ruft `/api/pull` mit Fortschritt auf (`pullModel`); in Rust fehlt jeder solche Aufruf. | offen |
| P35 | **Ausrichtungen fehlen in der Editor-Leiste:** links, zentriert, rechts und Blocksatz (`alignL/alignC/alignR/alignJ` in der Vorlage) gibt es in Rust nicht. | offen |
| P36 | **Speichern ohne Menü:** Die Vorlage öffnet ein Menü mit „In Ablage speichern“ und Markdown, Text, PDF (`savePop`, `doSave`); bei uns schrieb der Knopf sofort eine `.md`-Datei. | erledigt (`54376b0`): verankertes Menü mit Ablage, Markdown, Text und PDF (neues Modul `pdf.rs`) |
| P37 | **Import nur txt/md/html:** Die Vorlage liest auch PDF und DOCX (Editor-Ablage und Übersetzer über `window.api.extract`). | offen |
| P38 | **„Dynamisches Markdown“ ohne Wirkung:** Die Einstellung `ed_markdown` steht in den Einstellungen, wird aber nirgends ausgewertet. | offen |
| P39 | **Wörterbuch ohne Import und Export:** Die zwei Knöpfe im Kopf des Wörterbuchfensters (`dictExport`, `dictImport`) fehlten samt Übersetzungstexten. | erledigt (`54376b0`): zwei Kreise im Kopf, eine Wortliste je Zeile, Zusammenführen ohne Doppelte |
| P40 | **Rechtschreibwellen animieren fehlt:** Einstellung `edSpellAnim` und die Bewegung der Wellenlinie sind nicht umgesetzt. | erledigt (`54376b0`): Schalter `ed_spell_anim`, Wellen wandern mit der Zeit |
| P41 | **Sitzungskarten lassen sich nicht umsortieren:** Die Vorlage verschiebt Karten beim Ziehen (`dockDragStart/Move/End`); bei uns standen sie fest. | erledigt (`54376b0`): Ziehen setzt die Karte an die Stelle unter dem Zeiger, die aktive Karte rückt mit |
| P42 | **Rechtschreibmenü viel zu groß:** Es spannte über fast die ganze Fensterbreite (Trennstrich dehnt die Fläche), Knöpfe statt Zeilen wie `.row`. | erledigt (`11b6866`): Karte von 240 px, Wort rot, Zeilen mit Hover, Strich in Kartenbreite |
| P43 | **Statusleiste des Editors:** Das `abc`-Symbol gehört dort nicht hin, die Knöpfe standen in falscher Reihenfolge. | erledigt (`11b6866`): `abc` entfernt, Reihenfolge Dokumente, Buch, Globus; roter Zähler entfallen |
| P44 | **Seite erreicht die Kante der Karte nicht:** Unter der Schreibseite blieb ein Streifen der Karte sichtbar. | erledigt (`11b6866`): Höhe aus der Fläche darunter, Seite endet an der Kante |
| P45 | **Falscher Statustext:** Beim Verlängern, Kürzen, Zusammenfassen und Probelesen stand „Translating …“ in der Leiste. | erledigt (`11b6866`): Status bleibt leer, Rückmeldung ist die Animation und der Ladekreis |
| P46 | **Prompt landet im Dokument:** Die Antwort wurde samt Anweisung übernommen, außerdem streamte die Überarbeitung live. | erledigt (`11b6866`): Anfrage ohne Stream, `clean_editor_output` schneidet Anweisungen, `<text>`-Reste, Zäune und Anführungszeichen weg |
| P47 | **Partikel-Animation über dem Text fehlt:** Die Vorlage legt `.ai-processing` über den bearbeiteten Text. | erledigt (`11b6866`): Text wird verdeckt, drei Lagen Punkte wandern nach oben |
| P48 | **Werkzeugleiste falsch bestückt:** Das Code-Symbol gehörte nicht hinein, die vier Ausrichtungen fehlten. | erledigt (`11b6866`): Code-Symbol entfernt, vier Ausrichtungen ergänzt; Schrift und Größe bleiben P24 |
| P49 | **Ladeanzeige am falschen Ort:** Die Vorlage macht aus der Kontextleiste eine kleine Pille mit Ladekreis. | erledigt (`11b6866`): `ladekreis` in der Pille, auch in Synonym- und Kontext-Übersetzer-Menü; Übersetzen-Knopf bleibt sichtbar |
| P50 | **„Alles verkürzen" wirkt nicht:** Der ganze Text ging in einem Stück ans Modell (Kontextgrenze), die Vorlage zerlegt lange Texte in Abschnitte (`chunkText`, `summarizeLong`). | erledigt (`cc4f4b3`): `chunk_text`, stückweise Anfragen mit Fortschritt „Teil 3 von 7“, Zusammenfassen in zwei Durchgängen; Abschnitte bekommen einen Anschluss (Stelle im Dokument, Schluss des vorigen Ergebnisses) — auch in der Electron-Fassung (`4b4264b`, `chunkInstr` in `editor.js`) |
| P51 | **Probelesen falsch:** Ablauf und Fenster wichen ab; die Vorlage ersetzt den Text durch Markierungen und entscheidet je Änderung. | erledigt (`e5dfffe`): Liste mit alt→neu je Änderung, Haken/Kreuz, entschiedene Zeilen treten zurück, alles entschieden setzt den Text, Schließen lässt Offenes alt; Markierungs-Probelesen vergleicht den neuen Dokumenttext. Inline-Markierungen bleiben offen, weil der Editor auf Markdown-Text arbeitet |
| P52 | **Stil-Option der Kontextleiste falsch:** Sie nutzt Textknöpfe; die Vorlage zeigt Symbolreihen (`.icg`) und eine Auswahl der Stilrichtungen mit Symbolen. | offen |
| P53 | **Kontext-Übersetzer:** zu breit, der Übersetzen-Knopf unten zu klein (Vorlage: `#trpop` mit 280 px Mindestbreite und `.go`-Knopf rechts). | offen |
| P54 | **Pin-Symbol unkenntlich und uneinheitlich:** Es sieht nicht wie eine Nadel aus und ist in Übersetzer und Editor verschieden. | offen |
| P55 | **Aktives-Seiten-Menü falsch:** Aufbau und Verhalten der Sitzungsliste weichen ab (Karten, Vorschau, Kopf, Öffnen/Schließen). | offen |

## Erledigt

- **Ollama-Verbindung:** Voreinstellung `127.0.0.1`, Rückfall auf die andere
  Schreibweise, sichtbarer Fehlertext im Modellbereich — `6bd2990`.
- **Editor-Werkzeugleiste:** Überschriften im Dropdown, drei KI-Knöpfe fürs
  ganze Dokument, Modellpille rechts, Neuladen rechts vom Modellknopf, runde
  Bögen in Symbolen — `ab212fc`.
- **NLLB-Server:** Schalter in den Einstellungen, Start neben der Exe und
  Beenden beim Schließen (`nllb.rs`, `nllb_autostart`) — `b0ebe30`.
- **Mini-Übersetzer und Synonyme:** Sprachpillen, Stilreihen mit Symbolen,
  Ersetzen der Auswahl; Synonyme als anklickbare Liste unter der Leiste.
- **Kontextleiste als Anker:** Stilfenster und Mini-Übersetzer hängen unter
  der Leiste (`ctx_bar_rect`); Schließen-Kreuz der Einstellungen bleibt sichtbar.
- **Dialoge:** `pop_at` für verankerte Menüs, `dialog` für mittige,
  unverschiebbare Dialoge; Sprachmenü hängt an der Sprachpille.
- **Einrichtung:** `rust/Installieren.bat`, `rust/install.ps1`,
  `rust/Uninstall.ps1`, NLLB-200 mit eigener Python-Umgebung und Modellwahl.
- **Modelle:** Fähigkeits-Symbole im Modellknopf, Punkt am Neuladen-Knopf
  während des Ladens.
- **Modellauswahl:** verankertes Menü im Editor (Schreib-/Übersetzungsmodell)
  und im Übersetzer, mit Symbolen für Bild, Denken und MT — `e3339ea`.
- **Rechtschreibung:** geschlängelte Wellenlinie (Sinus), Rechtsklick-Menü mit
  Wörterbuch-Vorschlägen, Ignorieren, Ins-Wörterbuch; Prüffenster am ersten
  Fund — `e921a63`.
- **Kontextleiste:** sieben Aktionen in der Reihenfolge der Vorlage, Stil- und
  Mini-Übersetzer-Knopf ergänzt — `c69c0aa`, `87ffeea`.
- **Übersetzer:** Textfelder ohne Trennstrich bis fast zum Boden, Kopfzeile
  mit 36er Knöpfen, Modellpille ohne `▤`, Stil-Seitenleiste mit Karten,
  Symbolgruppen sowie aufklappbarem Kontext und Glossar — `1863576`.
- **Fensterleiste und Navigationsleiste:** 36er-Kreise, Symbole nur mit
  Hover-/Aktiv-Hintergrund, kein grauer Grund — `1863576`.
- **Dokumentenablage:** Raster mit 210-px-Spalten, 168-px-Vorschau,
  Datum/Uhrzeit, Hover-Rahmen und Löschkreuz — `1863576`.
