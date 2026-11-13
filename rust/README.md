# Rust-Fassung — WIP (On Hold)

**Status: Work in Progress — On Hold (Arbeit vorerst eingestellt).**

Dieser Ordner enthält die Neufassung der Anwendung in Rust. Sie ist ein zweiter Zweig
und **nicht** die Fassung, die benutzt wird: Sie ist unfertig, in einzelnen Ansichten
nur angenähert und nicht vollständig durchgetestet. Die Arbeit daran ruht.

**Die aktuelle Fassung ist die Electron-App im Projektordner darüber** (`main.js`,
`renderer.js`, `index.html`, `style.css`, Start über `npm start` oder `Starten.vbs`).
Sie ist vollständig funktionsfähig; offen sind dort nur kleinere Fehler.

Was hier liegt, ist trotzdem baubar: `cargo build` im Ordner `rust/` erzeugt
`target/debug/uebersetzer.exe`. Die Einzelheiten — Datenpfade, Aufbau der Module,
Abweichungen von der Electron-Fassung — stehen in `Doku.txt`, Abschnitt 13.
