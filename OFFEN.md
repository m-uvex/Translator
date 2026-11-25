# Offene Punkte — Electron-Fassung

Diese Liste sammelt, was bewusst noch nicht entschieden oder noch nicht gebaut
ist. Sie ersetzt keine Doku: Der Grund steht hier in einem Satz, die
Einzelheiten im genannten Doku-Abschnitt. Die Rust-Fassung hat ihre eigene Liste
in `PROBLEME.md`; Fehler der Electron-Fassung gehören hierher, nicht dorthin.

Stand: 25. November 2026, Version 1.15.195.

## Wortwahl (deutsch)

Entschieden und umgesetzt in §213. Hier steht nur, was dort offen blieb.

| Stelle | jetzt | im Gespräch | warum noch offen |
| --- | --- | --- | --- |
| `s_story` | „Kurzgeschichten-Generator (Beta)“ | „Kurzgeschichtenwerkstatt“, „Erzählwerkstatt“, „Kurzgeschichten-Schreiber“ | Nutzer noch unentschieden; ein Wechsel betrifft genau einen Eintrag |

Frühere offene Punkte aus §7, weiterhin gültig: `s_start`/`s_bootModel`
„Hochfahren“ (Alternative „Beim Start …“), `serverDown` „mit eingerichtet“
(Alternative „wird mitinstalliert“), `st_pro` „Fachlich“ (Alternative
„Professionell“), `o_natural` „Natürlich“ und `o_formal` „Formell“ (bewusst
gelassen, damit „Gehoben“ frei bleibt).

Der Regelprüfer (`npm run regeln`) kennt diese Ausnahmen in `AUSNAHMEN`; wer eine
davon anders entscheidet, streicht sie dort und ändert den Text.

## Noch nicht entschieden

- **Gemeinsame Prüfdateien für Electron und Rust** (aus der Ideenrunde, Punkt 6):
  ein Satz Eingabedateien mit erwarteter Ausgabe für Seitenumbruch, Lernleiter,
  Worthäufigkeit und Glossarersetzung, den beide Fassungen abarbeiten. Offen ist,
  ob die Rust-Fassung das jetzt schon aufnehmen kann.
- **„Was ist neu“ aus der Doku** (Punkt 4): auf Wunsch zurückgestellt. Die Quelle
  wäre die Abschnittsüberschriften mit Versionsnummer; beim ersten Start nach
  einer neuen Version eine kurze Liste.

## Bekannte Grenzen (kein Fehler, aber gut zu wissen)

- Die Sprachfassungen NL, ES, TR und AR haben einzelne Schlüssel nicht; dort
  greift Englisch. Die Zahlen stehen im Rauchtest.
- `localStorage` ist auf wenige Megabyte begrenzt; der Bestand liegt inzwischen
  zum größten Teil in Dateien unter `%APPDATA%`, die Einstellungen aber weiter
  im `localStorage`.
