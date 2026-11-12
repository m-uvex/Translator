//! Rechtschreibpruefung auf Basis von Hunspell-Woerterbuechern.
//!
//! Die Electron-Fassung nutzte `nspell` mit den Paketen `dictionary-<lang>`
//! beziehungsweise dem lokalen Ordner `dict/<lang>`. Die Rust-Fassung liest
//! dieselben Dateien ueber `spellbook`, eine Hunspell-Implementierung in Rust.
//! Dadurch funktionieren die vorhandenen Woerterbuecher unveraendert weiter,
//! insbesondere das arabische unter `dict/ar`.
//!
//! Gesucht wird in dieser Reihenfolge: im Programmordner und dessen
//! Elternordnern (also auch im Projektwurzelverzeichnis), im Arbeitsverzeichnis,
//! unter `node_modules/dictionary-<lang>` (die Pakete der Electron-Fassung),
//! unter `dict/<lang>` und unter `%APPDATA%\Uebersetzer\dict\<lang>`.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

/// Ein falsch geschriebenes Wort mit Haeufigkeit und Vorschlaegen.
#[derive(Clone, Debug)]
pub struct Misspelled {
    pub word: String,
    pub count: usize,
    pub suggestions: Vec<String>,
}

pub struct Spell {
    dicts: HashMap<String, Option<spellbook::Dictionary>>,
    ignored: HashSet<String>,
    dirs: Vec<PathBuf>,
}

impl Spell {
    pub fn new() -> Self {
        Self {
            dicts: HashMap::new(),
            ignored: HashSet::new(),
            dirs: search_dirs(),
        }
    }

    /// Laedt ein Woerterbuch einmalig. Ein fehlendes Woerterbuch wird als
    /// `None` gemerkt, damit nicht bei jedem Scan erneut das Dateisystem
    /// durchsucht wird.
    fn ensure(&mut self, lang: &str) {
        if self.dicts.contains_key(lang) {
            return;
        }
        let dict = find_pair(lang, &self.dirs).and_then(|(aff, dic)| {
            let aff_text = std::fs::read_to_string(aff).ok()?;
            let dic_text = std::fs::read_to_string(dic).ok()?;
            spellbook::Dictionary::new(&aff_text, &dic_text).ok()
        });
        self.dicts.insert(lang.to_string(), dict);
    }

    pub fn has_dict(&mut self, lang: &str) -> bool {
        self.ensure(lang);
        self.dicts.get(lang).map(|d| d.is_some()).unwrap_or(false)
    }

    /// Prueft einen Text und liefert die falsch geschriebenen Woerter.
    /// Ein `Err` bedeutet: fuer diese Sprache liegt kein Woerterbuch vor.
    pub fn scan(
        &mut self,
        lang: &str,
        text: &str,
        personal: &[String],
    ) -> Result<Vec<Misspelled>, String> {
        self.ensure(lang);
        let Some(dict) = self.dicts.get(lang).and_then(|d| d.as_ref()) else {
            return Err(lang.to_string());
        };
        let personal: HashSet<String> = personal.iter().map(|w| w.to_lowercase()).collect();
        let mut counts: HashMap<String, usize> = HashMap::new();
        let mut cur = String::new();
        for c in text.chars().chain(std::iter::once(' ')) {
            if c.is_alphabetic() || c == '\'' {
                cur.push(c);
                continue;
            }
            if cur.chars().count() >= 2 && !cur.chars().all(|c| c.is_uppercase()) {
                let low = cur.to_lowercase();
                if !personal.contains(&low)
                    && !self.ignored.contains(&low)
                    && !dict.check(&cur)
                    && !dict.check(&low)
                {
                    *counts.entry(cur.clone()).or_insert(0) += 1;
                }
            }
            cur.clear();
        }

        // Keine Vorschlaege in diesem Durchlauf: sie kosten rund anderthalb
        // Sekunden je Wort (gemessen an einem 200-KB-Text mit 1649 falschen
        // Woertern: 30 s fuer 20 Woerter). Die Vorlage holt sie ebenfalls erst
        // fuer das eine angeklickte Wort (`spellSuggest` in `editor.js`),
        // deshalb liefert `suggest` sie einzeln.
        let mut hits: Vec<Misspelled> = counts
            .into_iter()
            .map(|(word, count)| Misspelled {
                word,
                count,
                suggestions: Vec::new(),
            })
            .collect();
        hits.sort_by(|a, b| b.count.cmp(&a.count).then(a.word.cmp(&b.word)));
        Ok(hits)
    }

    /// Vorschlaege fuer ein einzelnes Wort. `scan` laesst sie bewusst aus;
    /// hier kostet ein Wort Bruchteile einer Sekunde.
    pub fn suggest(&mut self, lang: &str, word: &str) -> Vec<String> {
        self.ensure(lang);
        let Some(dict) = self.dicts.get(lang).and_then(|d| d.as_ref()) else {
            return Vec::new();
        };
        let mut out: Vec<String> = Vec::new();
        dict.checker()
            .into_suggester()
            .with_ngram_suggestions(false)
            .suggest(word, &mut out);
        out.truncate(5);
        out
    }

    pub fn ignore(&mut self, word: &str) {
        self.ignored.insert(word.to_lowercase());
    }

    pub fn clear_cache(&mut self) {
        self.ignored.clear();
    }
}

fn search_dirs() -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            push_ancestors(&mut dirs, parent, 5);
        }
    }
    if let Ok(cwd) = std::env::current_dir() {
        push_ancestors(&mut dirs, &cwd, 4);
    }
    dirs.push(crate::settings::data_dir().join("dict"));
    dirs
}

/// Haengt einen Ordner und seine Elternordner an, hoechstens `depth` Stueck.
/// Damit finden sowohl das Testbinary unter `target/debug/deps` als auch ein
/// Aufruf per `cargo run` aus `rust/` heraus das Woerterbuch im
/// Projektwurzelverzeichnis.
fn push_ancestors(dirs: &mut Vec<PathBuf>, start: &Path, depth: usize) {
    let mut next: Option<PathBuf> = Some(start.to_path_buf());
    for _ in 0..depth {
        match next {
            Some(dir) => {
                dirs.push(dir.clone());
                next = dir.parent().map(|x| x.to_path_buf());
            }
            None => break,
        }
    }
}

fn find_pair(lang: &str, dirs: &[PathBuf]) -> Option<(PathBuf, PathBuf)> {
    for base in dirs {
        let candidates = [
            base.join("dict").join(lang),
            base.join("node_modules").join(format!("dictionary-{}", lang)),
            base.clone(),
        ];
        for dir in candidates {
            let aff = dir.join("index.aff");
            let dic = dir.join("index.dic");
            if aff.is_file() && dic.is_file() {
                return Some((aff, dic));
            }
            let aff2 = dir.join(format!("{}.aff", lang));
            let dic2 = dir.join(format!("{}.dic", lang));
            if aff2.is_file() && dic2.is_file() {
                return Some((aff2, dic2));
            }
        }
    }
    None
}


// ---- Hintergrund-Thread -------------------------------------------------

/// Auftrag an den Rechtschreib-Thread.
enum Auftrag {
    Scan {
        gen: u64,
        lang: String,
        doc: String,
        personal: Vec<String>,
    },
    Suggest {
        lang: String,
        word: String,
    },
}

/// Ergebnis aus dem Rechtschreib-Thread.
pub enum Ergebnis {
    Scan {
        gen: u64,
        doc: String,
        hits: Vec<Misspelled>,
        error: Option<String>,
    },
    Suggest {
        word: String,
        list: Vec<String>,
    },
}

/// Prueft im Hintergrund. Das Woerterbuch wird im Thread einmalig geladen und
/// bleibt dort — im UI-Thread wuerde schon das Laden die Oberflaeche anhalten
/// (gemessen: knapp eine Sekunde fuer Deutsch, sechs Sekunden fuer Tuerkisch).
pub struct SpellWorker {
    tx: std::sync::mpsc::Sender<Auftrag>,
    rx: std::sync::mpsc::Receiver<Ergebnis>,
}

impl SpellWorker {
    pub fn new() -> Self {
        let (tx, job_rx) = std::sync::mpsc::channel::<Auftrag>();
        let (res_tx, rx) = std::sync::mpsc::channel::<Ergebnis>();
        std::thread::spawn(move || {
            let mut spell = Spell::new();
            while let Ok(auftrag) = job_rx.recv() {
                let ergebnis = match auftrag {
                    Auftrag::Scan {
                        gen,
                        lang,
                        doc,
                        personal,
                    } => {
                        let (hits, error) = match spell.scan(&lang, &doc, &personal) {
                            Ok(hits) => (hits, None),
                            Err(code) => (Vec::new(), Some(code)),
                        };
                        Ergebnis::Scan {
                            gen,
                            doc,
                            hits,
                            error,
                        }
                    }
                    Auftrag::Suggest { lang, word } => {
                        let list = spell.suggest(&lang, &word);
                        Ergebnis::Suggest { word, list }
                    }
                };
                if res_tx.send(ergebnis).is_err() {
                    break;
                }
            }
        });
        Self { tx, rx }
    }

    pub fn scan(&self, gen: u64, lang: &str, doc: &str, personal: &[String]) {
        let _ = self.tx.send(Auftrag::Scan {
            gen,
            lang: lang.to_string(),
            doc: doc.to_string(),
            personal: personal.to_vec(),
        });
    }

    pub fn suggest(&self, lang: &str, word: &str) {
        let _ = self.tx.send(Auftrag::Suggest {
            lang: lang.to_string(),
            word: word.to_string(),
        });
    }

    pub fn try_recv(&self) -> Option<Ergebnis> {
        self.rx.try_recv().ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn arabisch() -> Option<(String, String)> {
        for base in [
            PathBuf::from(".."),
            PathBuf::from("."),
            crate::settings::data_dir(),
        ] {
            let dir = base.join("dict").join("ar");
            let aff = dir.join("index.aff");
            let dic = dir.join("index.dic");
            if aff.is_file() && dic.is_file() {
                return Some((
                    std::fs::read_to_string(aff).ok()?,
                    std::fs::read_to_string(dic).ok()?,
                ));
            }
        }
        None
    }

    #[test]
    fn vorschlaege_einzeln() {
        let mut s = Spell::new();
        if !s.has_dict("de") {
            return;
        }
        let hits = s.scan("de", "Das ist ein Tst.", &[]).unwrap_or_default();
        assert!(hits.iter().any(|h| h.word == "Tst"));
        // Der Durchlauf liefert bewusst keine Vorschlaege mit.
        assert!(hits.iter().all(|h| h.suggestions.is_empty()));
        // Einzeln abgefragt kommen sie.
        assert!(!s.suggest("de", "Tst").is_empty());
    }

    #[test]
    fn erkennt_falsches_wort() {
        let Some((aff, dic)) = arabisch() else { return };
        let dict = spellbook::Dictionary::new(&aff, &dic).unwrap();
        assert!(!dict.check("qxzzzz"));
    }

    #[test]
    fn wortzerlegung_ignoriert_kurzes_und_grossschreibung() {
        let mut spell = Spell::new();
        let hits = spell.scan("ar", "A und b", &[]);
        if let Ok(list) = hits {
            assert!(list.iter().all(|h| h.word.chars().count() >= 2));
        }
    }

    #[test]
    fn gebuendelte_woerterbuecher_laden() {
        let faelle: [(&str, &[&str]); 6] = [
            ("de", &["Haus", "haus"]),
            ("en", &["house"]),
            ("nl", &["huis"]),
            ("es", &["casa"]),
            ("tr", &["ev", "kitap"]),
            ("ar", &["بيت", "كتاب"]),
        ];
        let mut geprueft = 0;
        for (code, probe) in faelle {
            let dir = PathBuf::from("../dict").join(code);
            let (aff, dic) = (dir.join("index.aff"), dir.join("index.dic"));
            if !aff.is_file() || !dic.is_file() {
                continue;
            }
            let aff = std::fs::read_to_string(aff).unwrap();
            let dic = std::fs::read_to_string(dic).unwrap();
            let dict = spellbook::Dictionary::new(&aff, &dic)
                .unwrap_or_else(|e| panic!("{}: {:?}", code, e));
            assert!(
                probe.iter().any(|w| dict.check(w)),
                "{}: kein bekanntes Wort gefunden",
                code
            );
            geprueft += 1;
        }
        assert!(geprueft >= 6, "nur {} Woerterbuecher gefunden", geprueft);
    }

    #[test]
    fn suchpfade_finden_projektwoerterbuch() {
        // Prueft dieselbe Pfadsuche, die das Programm nutzt: vom Testbinary
        // unter `target/debug/deps` aus muss der Projektordner mit `dict/`
        // in Reichweite liegen.
        let mut spell = Spell::new();
        assert!(spell.has_dict("de"), "dict/de nicht ueber die Programmpfade gefunden");
    }

    #[test]
    fn ignorierte_woerter_tauchen_nicht_auf() {
        let mut spell = Spell::new();
        spell.ignore("Qxzzzz");
        if let Ok(list) = spell.scan("ar", "Qxzzzz", &[]) {
            assert!(list.is_empty());
        }
    }
}

