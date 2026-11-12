//! Fenster, Ansichten und Dialoge der Rust-Neufassung.

use crate::engine::{
    eng_name, filter_profanity, is_mt, is_tg, lang_label, strip_stars, tg_prompt, user_prompt_for,
    PromptCtx, OLLAMA_LANGS,
};
use crate::i18n::{t, Lang};
use crate::editor::{self, DiffState, Part, Snapshot};
use crate::library::{self, Meta};
use crate::net::{self, Cfg, Model, Msg};
use crate::settings::{Settings, EDITOR_SYS};
use eframe::egui::{self, Color32, RichText, Rounding, Stroke};
use serde_json::json;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;

#[derive(Copy, Clone, PartialEq)]
enum View {
    Translator,
    Editor,
    Library,
}

#[derive(Copy, Clone)]
struct Palette {
    bg: Color32,
    card: Color32,
    soft: Color32,
    text: Color32,
    muted: Color32,
    line: Color32,
    accent: Color32,
    accent_hover: Color32,
    accent_ink: Color32,
    on_accent: Color32,
    dark: bool,
}

impl Palette {
    fn of(dark: bool) -> Self {
        if dark {
            Self {
                bg: Color32::from_rgb(0x17, 0x17, 0x17),
                card: Color32::from_rgb(0x21, 0x21, 0x21),
                soft: Color32::from_rgb(0x2b, 0x2b, 0x2d),
                text: Color32::from_rgb(0xec, 0xec, 0xec),
                muted: Color32::from_rgb(0x9a, 0x9a, 0x9f),
                line: Color32::from_rgb(0x33, 0x33, 0x36),
                accent: Color32::from_rgb(0x5a, 0xa9, 0xe0),
                accent_hover: Color32::from_rgb(0x82, 0xbd, 0xea),
                accent_ink: Color32::from_rgb(0x8c, 0xc8, 0xf5),
                on_accent: Color32::from_rgb(0x08, 0x21, 0x3a),
                dark,
            }
        } else {
            Self {
                bg: Color32::from_rgb(0xf7, 0xf7, 0xf8),
                card: Color32::from_rgb(0xff, 0xff, 0xff),
                soft: Color32::from_rgb(0xf1, 0xf1, 0xf3),
                text: Color32::from_rgb(0x1d, 0x1d, 0x1f),
                muted: Color32::from_rgb(0x6e, 0x6e, 0x73),
                line: Color32::from_rgb(0xe6, 0xe6, 0xea),
                accent: Color32::from_rgb(0x9c, 0xcf, 0xf5),
                accent_hover: Color32::from_rgb(0xcf, 0xe7, 0xfb),
                accent_ink: Color32::from_rgb(0x2a, 0x74, 0xb5),
                on_accent: Color32::from_rgb(0x0e, 0x2a, 0x44),
                dark,
            }
        }
    }
}

#[derive(Clone)]
enum Confirm {
    LibDelete(String),
    ModelDelete(String),
    SessionClose(usize),
}

/// Ein Dokument der Sitzungsliste (nur im Speicher, wie in der
/// Electron-Fassung). `saved_text` ist der zuletzt gespeicherte Stand und
/// dient der Erkennung ungespeicherter Aenderungen.
/// Ein Auftrag ueber das ganze Dokument, in Abschnitte zerlegt (`chunkText`
/// in `editor.js`): jeder Teil geht einzeln ans Modell, danach wird gesetzt.
struct ChunkJob {
    parts: Vec<String>,
    idx: usize,
    out: Vec<String>,
    instr: String,
    /// `summarize` laesst am Ende noch einen Durchgang ueber die Teile laufen.
    zusammenfassen: bool,
    zweiter_durchgang: bool,
}

/// Menue mit Ersatzwoertern unter einem Wort der Ausgabe (`#alt` in der Vorlage).
#[derive(Clone)]
struct AltMenu {
    wort: String,
    nr: usize,
    pos: egui::Pos2,
    list: Option<Vec<String>>,
    fehler: bool,
}

/// Offenes Rechtsklick-Menue zu einem falsch geschriebenen Wort.
#[derive(Clone)]
struct SpellMenu {
    pub word: String,
    pub pos: egui::Pos2,
}

pub struct SessionDoc {
    pub key: u64,
    pub name: String,
    pub text: String,
    pub saved_text: String,
}

pub struct App {
    lang: Lang,
    set: Settings,
    view: View,
    pal: Palette,
    sys_dark: bool,

    tx: Sender<Msg>,
    rx: Receiver<Msg>,
    stop: Arc<AtomicBool>,
    req: String,
    busy: bool,
    editor_job: bool,
    status: String,

    models: Vec<Model>,
    nllb_langs: Vec<(String, String)>,
    mt_models: Vec<serde_json::Value>,
    warmed: bool,
    models_loading: bool,
    /// Verbindungsfehler beim Abruf der Modellliste (Text der Fehlermeldung).
    models_error: Option<String>,
    /// Ausloeser des verankerten Modellmenues.
    models_anchor: egui::Rect,
    /// Zeigt das Menue die Editor-Auswahl (Schreib-/Uebersetzungsmodell)?
    models_for_editor: bool,

    input: String,
    output: String,
    last_src: String,
    last_tr: String,
    sel: Option<(usize, usize)>,
    pending_sel: Option<(usize, usize)>,
    pending_diff: bool,
    sessions: Vec<SessionDoc>,
    active_session: usize,
    next_key: u64,
    show_sessions: bool,
    diff: Option<DiffState>,
    undo: Vec<Snapshot>,
    redo: Vec<Snapshot>,
    last_undo: f64,

    lib: Vec<Meta>,
    doc: String,
    doc_id: Option<String>,
    saved: bool,

    dict_words: Vec<String>,
    dict_new: String,
    rename: Option<usize>,
    /// Rechtschreibung laeuft in einem eigenen Thread (`SpellWorker`); hier
    /// steht nur der Zugang dorthin. Im UI-Thread dauerte ein Durchlauf bei
    /// einem langen Dokument Minuten, weil fuer jedes Wort Vorschlaege
    /// berechnet wurden.
    spell_worker: Option<crate::spell::SpellWorker>,
    /// Laufende Nummer der Pruefungen; aeltere Ergebnisse werden verworfen.
    spell_gen: u64,
    spell_done_gen: u64,
    /// Wartet gerade eine Pruefung? Dann keine neue anstossen.
    spell_waiting: bool,
    /// Vom Nutzer ignorierte Woerter; sie gehen als persoenliches Woerterbuch
    /// mit an den Thread.
    spell_ignored: Vec<String>,
    /// Vorschlaege fuer das offene Rechtsklick-Menue.
    spell_menu_sug: Vec<String>,
    /// Warten die Vorschlaege noch? Bis dahin zeigt das Menue einen Hinweis.
    spell_menu_wait: bool,
    spell_hits: Vec<crate::spell::Misspelled>,
    spell_error: Option<String>,
    spell_scanned_doc: String,
    spell_scan_at: f64,
    show_spell: bool,
    show_spell_lang: bool,
    dict_anchor: Option<egui::Rect>,
    show_save_menu: bool,
    alt_menu: Option<AltMenu>,
    chunk_job: Option<ChunkJob>,
    reword_job: bool,
    save_anchor: Option<egui::Rect>,
    dock_drag: Option<usize>,
    /// Bildschirmrechteck der aktuellen Textauswahl; steuert die schwebende
    /// Kontextleiste (`context_bar`).
    bar_rect: Option<egui::Rect>,
    /// Zeigt der Zeiger auf die Kontextleiste? Sie bleibt dann stehen, auch
    /// wenn der Editor beim Klick den Fokus verliert.
    bar_hover: bool,
    /// Bildschirmposition des ersten markierten Rechtschreibfehlers; dient als
    /// Anker fuer das Rechtschreibfenster.
    spell_anchor: Option<egui::Pos2>,
    /// Einmalige Startposition fuer das Rechtschreibfenster.
    spell_pos: Option<egui::Pos2>,
    /// Rechtecke der unterstrichenen Woerter aus dem letzten Frame; Grundlage
    /// fuer das Rechtsklick-Menue.
    spell_mark_rects: Vec<(String, egui::Rect)>,
    /// Offenes Rechtsklick-Menue auf einem falsch geschriebenen Wort.
    spell_menu: Option<SpellMenu>,
    /// Stil-Auswahl der Kontextleiste (`styleB` der Vorlage).
    show_style_bar: bool,
    /// Mini-Uebersetzer der Kontextleiste (`translateSel` der Vorlage).
    show_mini: bool,
    mini_in: String,
    mini_out: String,
    mini_sel: Option<(usize, usize)>,
    mini_job: bool,

    syn_word: String,
    syn_out: Vec<String>,
    syn_job: bool,
    syn_pending: String,
    syn_sel: Option<(usize, usize)>,

    show_settings: bool,
    show_models: bool,
    show_dict: bool,
    show_style: bool,
    show_syn: bool,
    popup_lang: Option<&'static str>,
    lang_search: String,
    lang_anchor: Option<egui::Rect>,
    ctx_bar_rect: Option<egui::Rect>,
    nllb_child: Option<std::process::Child>,
    confirm: Option<(String, Confirm)>,
}

impl App {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let set = Settings::load();
        let lang = Lang::resolve(&set.ui_lang);
        let sys_dark = crate::settings::system_is_dark();
        let dark = match set.theme.as_str() {
            "dark" => true,
            "light" => false,
            _ => sys_dark,
        };
        let view = match set.start.as_str() {
            "ed" => View::Editor,
            "last" => match set.last_view.as_str() {
                "ed" => View::Editor,
                "lib" => View::Library,
                _ => View::Translator,
            },
            _ => View::Translator,
        };
        let dict_words: Vec<String> = std::fs::read_to_string(crate::settings::data_dir().join("personal-dictionary.json"))
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        let (tx, rx) = channel();
        let mut app = Self {
            lang,
            view,
            pal: Palette::of(dark),
            sys_dark,
            tx: tx.clone(),
            rx,
            stop: Arc::new(AtomicBool::new(false)),
            req: String::new(),
            busy: false,
            editor_job: false,
            status: String::new(),
            models: Vec::new(),
            nllb_langs: Vec::new(),
            mt_models: Vec::new(),
            warmed: false,
            models_loading: false,
            models_error: None,
            models_anchor: egui::Rect::NOTHING,
            models_for_editor: false,
            input: String::new(),
            output: String::new(),
            last_src: String::new(),
            last_tr: String::new(),
            sel: None,
            pending_sel: None,
            pending_diff: false,
            sessions: vec![SessionDoc {
                key: 0,
                name: String::new(),
                text: set.doc.clone(),
                saved_text: set.doc.clone(),
            }],
            active_session: 0,
            next_key: 1,
            show_sessions: false,
            diff: None,
            undo: Vec::new(),
            redo: Vec::new(),
            last_undo: 0.0,
            lib: library::list(),
            doc: set.doc.clone(),
            doc_id: set.doc_id.clone(),
            saved: true,
            dict_words,
            dict_new: String::new(),
            rename: None,
            spell_worker: None,
            spell_gen: 0,
            spell_done_gen: 0,
            spell_waiting: false,
            spell_ignored: Vec::new(),
            spell_menu_sug: Vec::new(),
            spell_menu_wait: false,
            spell_hits: Vec::new(),
            spell_error: None,
            spell_scanned_doc: String::new(),
            spell_scan_at: 0.0,
            show_spell: false,
            show_spell_lang: false,
            dict_anchor: None,
            show_save_menu: false,
            alt_menu: None,
            chunk_job: None,
            reword_job: false,
            save_anchor: None,
            dock_drag: None,
            bar_rect: None,
            bar_hover: false,
            spell_anchor: None,
            spell_pos: None,
            spell_mark_rects: Vec::new(),
            spell_menu: None,
            show_style_bar: false,
            show_mini: false,
            mini_in: String::new(),
            mini_out: String::new(),
            mini_sel: None,
            mini_job: false,
            syn_word: String::new(),
            syn_out: Vec::new(),
            syn_job: false,
            syn_pending: String::new(),
            syn_sel: None,
            show_settings: false,
            show_models: false,
            show_dict: false,
            show_style: false,
            show_syn: false,
            popup_lang: None,
            lang_search: String::new(),
            lang_anchor: None,
            ctx_bar_rect: None,
            nllb_child: None,
            confirm: None,
            set: set.clone(),
        };
        app.set = set;
        if app.set.nllb_autostart {
            app.nllb_child = crate::nllb::start();
        }
        let cfg = app.cfg();
        net::spawn_models(cfg.clone(), tx.clone(), Vec::new());
        net::spawn_nllb_meta(cfg.clone(), tx.clone());
        net::spawn_nllb_models(cfg, tx);
        app
    }

    fn cfg(&self) -> Cfg {
        Cfg {
            url: self.set.url.clone(),
            nllb: self.set.nllb.clone(),
            temp: self.set.temp,
            numctx: self.set.numctx,
        }
    }

    fn save_set(&self) {
        self.set.save();
    }

    fn rtl(&self) -> bool {
        self.lang.rtl()
    }

    fn active_langs(&self) -> Vec<(String, String)> {
        if is_mt(&self.set.model) {
            self.nllb_langs.clone()
        } else {
            OLLAMA_LANGS
                .iter()
                .map(|(c, native, _)| (c.to_string(), native.to_string()))
                .collect()
        }
    }

    fn fix_langs(&mut self) {
        let has = |list: &[(String, String)], code: &str| list.iter().any(|(c, _)| c == code);
        if is_mt(&self.set.model) {
            if self.nllb_langs.is_empty() {
                return;
            }
            if !has(&self.nllb_langs, &self.set.src) {
                self.set.src = if has(&self.nllb_langs, "deu_Latn") { "deu_Latn" } else { &self.nllb_langs[0].0 }.to_string();
            }
            if !has(&self.nllb_langs, &self.set.tgt) {
                self.set.tgt = if has(&self.nllb_langs, "eng_Latn") { "eng_Latn" } else { &self.nllb_langs[0].0 }.to_string();
            }
        } else {
            let has_en = |c: &str| OLLAMA_LANGS.iter().any(|(x, _, _)| *x == c);
            if self.set.src != "auto" && !has_en(&self.set.src) {
                self.set.src = "auto".into();
            }
            if !has_en(&self.set.tgt) {
                self.set.tgt = "en".into();
            }
            if self.set.src == "auto" && is_tg(&self.set.model) {
                self.set.src = if self.set.tgt == "de" { "en".into() } else { "de".into() };
            }
        }
        self.save_set();
    }

    fn upd_models(&mut self, list: Vec<Model>) {
        self.models = list;
        let pref = self.set.pref.clone();
        let names: Vec<String> = self.models.iter().map(|m| m.name.clone()).collect();
        if !names.iter().any(|n| *n == self.set.model) {
            self.set.model = if names.iter().any(|n| *n == pref) {
                pref
            } else {
                names.first().cloned().unwrap_or_default()
            };
        }
        if self.set.pref.is_empty() {
            self.set.pref = self.set.model.clone();
        }
        self.fix_langs();
        self.save_set();
        if self.set.boot_model && !self.warmed && !self.set.model.is_empty() {
            self.warmed = true;
            net::spawn_warmup(self.cfg(), self.set.model.clone());
        }
        if self.models.is_empty() {
            self.status = if self.models_error.is_some() {
                t(self.lang, "ollama_down")
            } else {
                t(self.lang, "no_models")
            };
        }
    }

    fn write_model(&self) -> Option<String> {
        if !self.set.ed_write.is_empty()
            && self
                .models
                .iter()
                .any(|m| !m.mt && m.name == self.set.ed_write)
        {
            return Some(self.set.ed_write.clone());
        }
        if !self.set.helper.is_empty() && self.models.iter().any(|m| m.name == self.set.helper) {
            return Some(self.set.helper.clone());
        }
        if !is_mt(&self.set.model) && !is_tg(&self.set.model) && !self.set.model.is_empty() {
            return Some(self.set.model.clone());
        }
        self.models
            .iter()
            .find(|m| !m.mt && !is_tg(&m.name))
            .map(|m| m.name.clone())
    }

    fn poll(&mut self, ctx: &egui::Context) {
        while let Ok(msg) = self.rx.try_recv() {
            match msg {
                Msg::Models(list) => {
                    self.models_loading = false;
                    self.upd_models(list);
                }
                Msg::Url(u) => {
                    if self.set.url != u {
                        self.set.url = u.clone();
                        self.save_set();
                        self.status = t(self.lang, "ollama_via").replace("<url>", &u);
                    }
                }
                Msg::ModelsError(e) => {
                    self.models_error = Some(e);
                    if self.models.is_empty() {
                        self.status = t(self.lang, "ollama_down");
                    }
                }
                Msg::Meta { langs, models } => {
                    if !langs.is_empty() {
                        self.nllb_langs = langs;
                        self.fix_langs();
                    }
                    for m in models {
                        let name = m.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
                        if !name.is_empty() && !self.models.iter().any(|x| x.name == name) {
                            self.models.push(Model {
                                name,
                                vision: false,
                                thinking: false,
                                mt: true,
                            });
                        }
                    }
                }
                Msg::ModelsMeta(arr) => self.mt_models = arr,
                Msg::Status(s) => {
                    if !s.is_empty() {
                        self.status = s;
                    }
                }
                Msg::Token(text) => {
                    if self.editor_job {
                        self.doc = text;
                    } else if self.syn_job {
                        self.syn_pending = text;
                    } else if self.mini_job {
                        self.mini_out = text;
                    } else {
                        self.output = text;
                    }
                }
                Msg::Alts(list, fehler) => {
                    if let Some(menu) = self.alt_menu.as_mut() {
                        let wort = menu.wort.to_lowercase();
                        menu.fehler = !fehler.is_empty();
                        menu.list = Some(
                            list.into_iter()
                                .filter(|a| !a.is_empty() && a.to_lowercase() != wort)
                                .take(6)
                                .collect(),
                        );
                    }
                    self.status = String::new();
                }
                Msg::Done(text) => {
                    self.busy = false;
                    if self.chunk_job.is_some() {
                        self.chunk_fertig(clean_editor_output(&text));
                        ctx.request_repaint();
                    } else if self.reword_job {
                        self.reword_job = false;
                        let clean = filter_profanity(
                            &strip_stars(&text, &self.last_src),
                            &self.set.tgt,
                            &self.set.src,
                            &self.set.profanity_style,
                        );
                        if !clean.trim().is_empty() {
                            self.output = clean.clone();
                            self.last_tr = clean;
                            if self.set.auto_copy {
                                ctx.output_mut(|o| o.copied_text = self.last_tr.clone());
                            }
                        }
                        self.status = String::new();
                    } else if self.syn_job {
                        self.syn_job = false;
                        self.syn_out = json_strings(&text)
                            .into_iter()
                            .map(|w| w.trim().to_string())
                            .filter(|w| !w.is_empty())
                            .take(6)
                            .collect();
                    } else if self.mini_job {
                        self.mini_job = false;
                        self.mini_out = text.trim_end().to_string();
                        self.status = String::new();
                        if let Some((a, b)) = self.mini_sel.take() {
                            let doc = self.doc.clone();
                            let ba = byte_idx(&doc, a);
                            let bb = byte_idx(&doc, b);
                            if b > a {
                                self.push_undo();
                                self.doc = format!("{}{}{}", &doc[..ba], self.mini_out, &doc[bb..]);
                                self.saved = false;
                            }
                        }
                        self.show_mini = false;
                    } else if self.editor_job {
                        self.editor_job = false;
                        let clean = clean_editor_output(&text);
                        if self.pending_diff {
                            self.pending_diff = false;
                            // Bei einer Markierung erst den neuen Dokumenttext
                            // bauen, dann alt gegen neu vergleichen: so zeigt das
                            // Fenster nur die Aenderungen an dieser Stelle.
                            let neu = match self.pending_sel {
                                Some((a, b)) if b > a => {
                                    let doc = self.doc.clone();
                                    let ba = editor::char_to_byte(&doc, a);
                                    let bb = editor::char_to_byte(&doc, b);
                                    format!("{}{}{}", &doc[..ba], clean, &doc[bb..])
                                }
                                _ => clean.clone(),
                            };
                            self.diff = Some(DiffState::new(&self.doc, &neu));
                        } else if let Some((a, b)) = self.pending_sel {
                            if b > a {
                                let doc = self.doc.clone();
                                let ba = editor::char_to_byte(&doc, a);
                                let bb = editor::char_to_byte(&doc, b);
                                let replaced = format!("{}{}{}", &doc[..ba], clean, &doc[bb..]);
                                self.push_undo();
                                self.doc = replaced;
                            }
                        } else {
                            self.push_undo();
                            self.doc = clean;
                        }
                        self.pending_sel = None;
                        self.saved = false;
                        self.status = String::new();
                        self.set.doc = self.doc.clone();
                        self.save_set();
                        self.sync_active_session();
                    } else {
                        let clean = filter_profanity(
                            &strip_stars(&text, &self.last_src),
                            &self.set.tgt,
                            &self.set.src,
                            &self.set.profanity_style,
                        );
                        self.output = clean.clone();
                        self.last_tr = clean;
                        self.status = String::new();
                        if self.set.auto_copy && !self.last_tr.is_empty() {
                            ctx.output_mut(|o| o.copied_text = self.last_tr.clone());
                        }
                    }
                }
                Msg::Stopped => {
                    self.busy = false;
                    self.editor_job = false;
                    self.mini_job = false;
                    self.reword_job = false;
                    self.chunk_job = None;
                    self.last_tr = self.output.clone();
                    self.status = t(self.lang, "stopped");
                }
                Msg::Error(e) => {
                    self.busy = false;
                    self.editor_job = false;
                    self.mini_job = false;
                    self.reword_job = false;
                    self.chunk_job = None;
                    self.status = format!("{}: {}", t(self.lang, "error"), e);
                }
            }
        }
        if self.busy {
            ctx.request_repaint_after(std::time::Duration::from_millis(120));
        }
    }

    fn start_translate(&mut self) {
        let text = self.input.trim().to_string();
        if self.busy || text.is_empty() {
            return;
        }
        if self.set.model.is_empty() {
            self.status = t(self.lang, "no_model");
            return;
        }
        let model = self.set.model.clone();
        if (is_mt(&model) || is_tg(&model)) && self.set.src == "auto" {
            self.status = t(self.lang, "need_src");
            return;
        }
        self.busy = true;
        self.editor_job = false;
        self.stop.store(false, Ordering::SeqCst);
        self.output.clear();
        self.status = t(self.lang, "translating");
        self.last_src = text.clone();
        let cfg = self.cfg();
        let tx = self.tx.clone();
        let stop = self.stop.clone();
        let req = format!("tr-{}", library::now());
        self.req = req.clone();
        let langs = self.nllb_langs.clone();
        if is_mt(&model) {
            net::spawn_nllb_translate(
                cfg,
                model,
                text,
                self.set.src.clone(),
                self.set.tgt.clone(),
                req,
                tx,
                stop,
            );
        } else if is_tg(&model) {
            let prompt = tg_prompt(&text, &self.set.src, &self.set.tgt, &langs);
            net::spawn_chat(
                cfg,
                model,
                vec![json!({"role":"user","content":prompt})],
                None,
                None,
                true,
                tx,
                stop,
            );
        } else {
            let gloss = self.set.gloss.iter().map(|g| (g.s.clone(), g.t.clone())).collect();
            let pctx = PromptCtx {
                src: &self.set.src,
                tgt: &self.set.tgt,
                style: &self.set.style,
                ctxt: &self.set.ctxt,
                gloss,
                has_images: false,
                nllb: &langs,
            };
            let prompt = user_prompt_for(&text, &pctx);
            let sys = self.set.sys.clone();
            net::spawn_chat(
                cfg,
                model,
                vec![json!({"role":"system","content":sys}), json!({"role":"user","content":prompt})],
                None,
                None,
                true,
                tx,
                stop,
            );
        }
    }

    fn stop_translation(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if is_mt(&self.set.model) && !self.req.is_empty() {
            net::cancel_nllb(&self.cfg(), &self.req);
        }
    }

    fn start_editor_op(&mut self, kind: &str) {
        self.start_editor_instr(editor_instr(kind), kind == "proofread");
    }

    /// Wie `start_editor_op`, aber fuer das ganze Dokument — entspricht `allOp`
    /// in der Vorlage: bis 3500 Zeichen ein Aufruf, darueber Abschnitt fuer
    /// Abschnitt, damit lange Texte nicht an der Kontextgrenze scheitern.
    fn start_editor_all(&mut self, kind: &str) {
        if self.busy {
            return;
        }
        if kind == "proofread" {
            let saved = self.sel;
            self.sel = None;
            self.start_editor_op(kind);
            self.sel = saved;
            return;
        }
        if self.write_model().is_none() {
            self.status = t(self.lang, "no_model");
            return;
        }
        let text = self.doc.clone();
        if text.trim().is_empty() {
            return;
        }
        let parts = chunk_text(&text, 3500);
        if parts.len() <= 1 {
            let saved = self.sel;
            self.sel = None;
            self.start_editor_op(kind);
            self.sel = saved;
            return;
        }
        self.chunk_job = Some(ChunkJob {
            parts,
            idx: 0,
            out: Vec::new(),
            instr: editor_instr(kind).to_string(),
            zusammenfassen: kind == "summarize",
            zweiter_durchgang: false,
        });
        self.busy = true;
        self.editor_job = true;
        self.pending_diff = false;
        self.pending_sel = None;
        self.status = String::new();
        self.spawn_chunk(0);
    }

    /// Schickt den naechsten Abschnitt des laufenden Auftrags ans Modell.
    fn spawn_chunk(&mut self, idx: usize) {
        let Some(model) = self.write_model() else {
            self.status = t(self.lang, "no_model");
            self.chunk_job = None;
            self.busy = false;
            self.editor_job = false;
            return;
        };
        let Some(job) = self.chunk_job.as_ref() else {
            return;
        };
        let Some(teil) = job.parts.get(idx).cloned() else {
            return;
        };
        // Ein Abschnitt allein weiss nichts von den anderen: der Zusatz nennt
        // die Stelle im Dokument und den Schluss des vorigen Ergebnisses.
        let (stelle, gesamt, vorher) = {
            let job = self.chunk_job.as_ref().unwrap();
            (
                idx + 1,
                job.parts.len(),
                job.out.last().cloned().unwrap_or_default(),
            )
        };
        let mut instr = job.instr.clone();
        if gesamt > 1 {
            if job.zusammenfassen {
                instr.push_str(&format!(
                    "\n\nThis is part {} of {} of one longer document; summarize only this part. The single summaries are merged afterwards, so do not add an introduction, a heading or a closing sentence.",
                    stelle, gesamt
                ));
            } else {
                instr.push_str(&format!(
                    "\n\nThis is part {} of {} of one longer document. Keep terminology, tone and tense consistent with the other parts, do not repeat whole sentences from the previous part, and do not add an introduction, a heading or a closing sentence of your own.",
                    stelle, gesamt
                ));
                if !vorher.trim().is_empty() {
                    let schwanz: String = {
                        let zeichen: Vec<char> = vorher.chars().collect();
                        let ab = zeichen.len().saturating_sub(400);
                        zeichen[ab..].iter().collect()
                    };
                    instr.push_str(&format!(
                        "\n\nThe previous part ended with this text — continue seamlessly in the same voice and keep the same terms, but do not repeat it:\n…{}",
                        schwanz
                    ));
                }
            }
        }
        let sys = EDITOR_SYS.to_string();
        self.stop.store(false, Ordering::SeqCst);
        net::spawn_chat(
            self.cfg(),
            model,
            vec![
                json!({"role":"system","content":sys}),
                json!({"role":"user","content":format!("{}\n\n<text>\n{}\n</text>", instr, teil)}),
            ],
            None,
            None,
            false,
            self.tx.clone(),
            self.stop.clone(),
        );
    }

    /// Ergebnis eines Abschnitts: naechster Teil, zweiter Durchgang beim
    /// Zusammenfassen oder fertig.
    fn chunk_fertig(&mut self, text: String) {
        let Some(job) = self.chunk_job.as_mut() else {
            return;
        };
        job.out.push(text);
        job.idx += 1;
        let gesamt = job.parts.len();
        let naechster = job.idx;
        if naechster < gesamt {
            let lang = self.lang;
            let fortschritt = t(lang, "part")
                .replacen("{}", &(naechster + 1).to_string(), 1)
                .replacen("{}", &gesamt.to_string(), 1);
            self.status = fortschritt;
            self.spawn_chunk(naechster);
            return;
        }
        let zusammen = job.out.join("\n\n");
        let nachlauf = job.zusammenfassen && !job.zweiter_durchgang;
        if nachlauf {
            // Wie `summarizeLong`: aus den Teilzusammenfassungen eine machen.
            let instr = "Summarize the following summary concisely in one to three sentences. Output only the summary, in the same language as the input.".to_string();
            self.chunk_job = Some(ChunkJob {
                parts: vec![zusammen],
                idx: 0,
                out: Vec::new(),
                instr,
                zusammenfassen: true,
                zweiter_durchgang: true,
            });
            self.status = String::new();
            self.spawn_chunk(0);
            return;
        }
        self.chunk_job = None;
        self.busy = false;
        self.editor_job = false;
        self.status = String::new();
        if zusammen.trim().is_empty() {
            return;
        }
        self.push_undo();
        self.doc = zusammen;
        self.sel = None;
        self.saved = false;
        self.set.doc = self.doc.clone();
        self.save_set();
        self.sync_active_session();
        self.spell_scanned_doc.clear();
    }

    /// Setzt die Ueberschriftenebene der markierten Zeilen.
    fn set_heading(&mut self, prefix: &str) {
        self.push_undo();
        let (text, sel) = editor::set_heading(&self.doc, self.selection(), prefix);
        self.doc = text;
        self.sel = Some(sel);
        self.saved = false;
        self.set.doc = self.doc.clone();
        self.save_set();
        self.spell_scanned_doc.clear();
    }

    /// Wie `start_editor_op`, aber mit frei formulierter Anweisung; die
    /// Stil-Auswahl der Kontextleiste (`styleB` der Vorlage) nutzt das.
    fn start_editor_instr(&mut self, instr: &str, diff: bool) {
        if self.busy {
            return;
        }
        let Some(model) = self.write_model() else {
            self.status = t(self.lang, "no_model");
            return;
        };
        let text = match self.sel {
            Some((a, b)) if b > a => {
                let src = &self.doc;
                let ba = byte_idx(src, a);
                let bb = byte_idx(src, b);
                src[ba..bb].to_string()
            }
            _ => self.doc.clone(),
        };
        if text.trim().is_empty() {
            return;
        }
        self.busy = true;
        self.editor_job = true;
        self.mini_job = false;
        self.pending_diff = diff;
        self.pending_sel = match self.sel {
            Some((a, b)) if b > a => Some((a, b)),
            _ => None,
        };
        self.stop.store(false, Ordering::SeqCst);
        // Wie `withBusy` in der Vorlage: kein Statustext, die Animation ueber
        // dem Text ist die Rueckmeldung. Und kein Live-Stream: das Modell
        // schreibt erst fertig, dann wird der Text ersetzt.
        self.status = String::new();
        let sys = EDITOR_SYS.to_string();
        net::spawn_chat(
            self.cfg(),
            model,
            vec![
                json!({"role":"system","content":sys}),
                json!({"role":"user","content":format!("{}\n\n<text>\n{}\n</text>", instr, text)}),
            ],
            None,
            None,
            false,
            self.tx.clone(),
            self.stop.clone(),
        );
    }

    fn ask_confirm(&mut self, text: impl Into<String>, action: Confirm) {
        self.confirm = Some((text.into(), action));
    }

    fn apply_theme(&self, ctx: &egui::Context) {
        let mut style = (*ctx.style()).clone();
        let p = self.pal;
        style.visuals = if p.dark { egui::Visuals::dark() } else { egui::Visuals::light() };
        let v = &mut style.visuals;
        v.panel_fill = p.bg;
        v.window_fill = p.card;
        v.extreme_bg_color = p.soft;
        v.faint_bg_color = p.soft;
        v.override_text_color = Some(p.text);
        v.window_rounding = Rounding::same(22.0);
        v.menu_rounding = Rounding::same(16.0);
        v.selection = egui::style::Selection {
            bg_fill: p.accent.gamma_multiply(0.55),
            stroke: Stroke::NONE,
        };
        v.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, p.line);
        v.widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, p.muted);
        for w in [
            &mut v.widgets.inactive,
            &mut v.widgets.hovered,
            &mut v.widgets.active,
            &mut v.widgets.open,
        ] {
            w.rounding = Rounding::same(12.0);
            w.bg_stroke = Stroke::new(1.0_f32, p.line);
        }
        v.widgets.inactive.weak_bg_fill = p.soft;
        v.widgets.inactive.bg_fill = p.soft;
        v.widgets.inactive.fg_stroke = Stroke::new(1.0_f32, p.text);
        v.widgets.hovered.weak_bg_fill = p.accent_hover;
        v.widgets.hovered.bg_fill = p.accent_hover;
        v.widgets.hovered.fg_stroke = Stroke::new(1.0_f32, p.on_accent);
        v.widgets.active.weak_bg_fill = p.accent;
        v.widgets.active.bg_fill = p.accent;
        v.widgets.active.fg_stroke = Stroke::new(1.0_f32, p.on_accent);
        v.widgets.open.weak_bg_fill = p.card;
        style.spacing.item_spacing = egui::vec2(8.0, 8.0);
        style.spacing.button_padding = egui::vec2(12.0, 6.0);
        ctx.set_style(style);
    }
}

/// Verfuegbare Woerterbuchsprachen (wie `DICTS` in `editor.js`).
const DICTS: [(&str, &str); 6] = [
    ("de", "Deutsch"),
    ("en", "English"),
    ("nl", "Nederlands"),
    ("es", "Español"),
    ("tr", "Türkçe"),
    ("ar", "العربية"),
];

fn byte_idx(s: &str, char_idx: usize) -> usize {
    s.char_indices()
        .nth(char_idx)
        .map(|(i, _)| i)
        .unwrap_or(s.len())
}

fn obj(label: &str, text: &str) -> serde_json::Value {
    let _ = label;
    json!({ "role": label, "content": text })
}

/// Pille wie `.pill` in `style.css`: Karte mit Kontur, 36 px hoch, 15 px
/// Schrift, mindestens 150 px breit. Die Kontur wird beim Ueberfahren
/// babyblau, ohne Skalierung oder Einblendung.
fn pill(ui: &mut egui::Ui, p: &Palette, text: &str, min_w: f32) -> egui::Response {
    let galley = ui
        .painter()
        .layout_no_wrap(text.to_string(), egui::FontId::proportional(15.0), p.text);
    let w = (galley.size().x + 36.0).max(min_w);
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(w, 36.0), egui::Sense::click());
    let stroke = if resp.hovered() {
        Stroke::new(1.0_f32, p.accent)
    } else {
        Stroke::new(1.0_f32, p.line)
    };
    let painter = ui.painter();
    painter.rect(rect, Rounding::same(999.0), p.card, stroke);
    painter.galley(
        egui::pos2(
            rect.center().x - galley.size().x / 2.0,
            rect.center().y - galley.size().y / 2.0,
        ),
        galley,
        p.text,
    );
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// Modellpille mit den Faehigkeits-Symbolen in der Pille, wie `updModelBtn`
/// in `renderer.js` (Name, dahinter Auge und Gluehbirne, beide 15 px).
/// Karte wie `.pop` in `style.css`: Rundung 16, Schatten, sechs Pixel
/// Innenabstand, rechtsbuendig unter dem Ausloeser und auf den Bildschirm
/// begrenzt.
fn pop_card(p: &Palette) -> egui::Frame {
    egui::Frame::none()
        .fill(p.card)
        .rounding(Rounding::same(16.0))
        .stroke(Stroke::new(1.0_f32, p.line))
        .inner_margin(egui::Margin::same(6.0))
        .shadow(egui::epaint::Shadow {
            offset: egui::vec2(0.0, 6.0),
            blur: 24.0,
            spread: 0.0,
            color: egui::Color32::from_black_alpha(70),
        })
}

/// Verankertes Menue unter einem Ausloeser; gibt `true` zurueck, wenn es
/// geschlossen werden soll (Klick daneben oder Escape).
fn pop_at(
    ctx: &egui::Context,
    id: &str,
    anchor: egui::Rect,
    width: f32,
    p: &Palette,
    add: impl FnOnce(&mut egui::Ui),
) -> bool {
    let screen = ctx.screen_rect();
    let x = (anchor.right() - width).clamp(8.0, (screen.right() - width - 8.0).max(8.0));
    let y = (anchor.bottom() + 6.0).min((screen.bottom() - 120.0).max(8.0));
    let resp = egui::Area::new(egui::Id::new(id))
        .order(egui::Order::Foreground)
        .fixed_pos(egui::pos2(x, y))
        .show(ctx, |ui| {
            ui.set_width(width);
            pop_card(p).show(ui, |ui| {
                ui.set_width(width - 12.0);
                egui::ScrollArea::vertical()
                    .max_height(screen.height() * 0.6)
                    .auto_shrink([false, true])
                    .show(ui, |ui| {
                        ui.set_width(width - 24.0);
                        add(ui);
                    });
            });
        });
    let rect = resp.response.rect;
    // Der Klick, der das Menue geoeffnet hat, liegt im Ausloeser und darf es
    // nicht sofort wieder schliessen.
    let clicked_out = ctx.input(|i| {
        i.pointer.any_click()
            && i.pointer
                .interact_pos()
                .map(|pos| !rect.contains(pos) && !anchor.contains(pos))
                .unwrap_or(false)
    });
    clicked_out || ctx.input(|i| i.key_pressed(egui::Key::Escape))
}

/// Dialog wie `<dialog>` in `index.html`: mittig, feste Breite, nicht
/// verschiebbar, ohne Titelleiste, mit eigenem Kopf aus Titel und Kreuz.
fn dialog(
    ctx: &egui::Context,
    id: &str,
    title: &str,
    width: f32,
    p: &Palette,
    open: &mut bool,
    add: impl FnOnce(&mut egui::Ui),
) {
    egui::Window::new(id)
        .id(egui::Id::new(id))
        .title_bar(false)
        .collapsible(false)
        .resizable(false)
        .default_width(width)
        .fixed_pos(ctx.screen_rect().center())
        .pivot(egui::Align2::CENTER_CENTER)
        .frame(pop_card(p))
        .show(ctx, |ui| {
            ui.set_width(width - 12.0);
            ui.set_max_width(width - 12.0);
            ui.allocate_ui_with_layout(
                egui::vec2(width - 12.0, 30.0),
                egui::Layout::left_to_right(egui::Align::Center),
                |ui| {
                    if !title.is_empty() {
                        ui.label(RichText::new(title).size(15.0).strong().color(p.text));
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if icon_btn(ui, p, "x", false).clicked() {
                            *open = false;
                        }
                    });
                },
            );
            ui.add_space(6.0);
            // Wie der Innenabstand von `#settings` in `style.css`: der Inhalt
            // steht nicht direkt an der Kante der Karte.
            egui::Frame::none()
                .inner_margin(egui::Margin {
                    left: 14.0,
                    right: 14.0,
                    top: 2.0,
                    bottom: 12.0,
                })
                .show(ui, |ui| add(ui));
        });
}

fn model_pill(
    ui: &mut egui::Ui,
    p: &Palette,
    label: &str,
    vision: bool,
    thinking: bool,
    min_w: f32,
) -> egui::Response {
    let galley = ui.painter().layout_no_wrap(
        label.to_string(),
        egui::FontId::proportional(15.0),
        p.text,
    );
    let icons = (vision as i32 + thinking as i32) as f32;
    // `.mi` in `style.css` hat 6 px Abstand zum Namen.
    let extra = if icons > 0.0 { icons * 19.0 + 6.0 } else { 0.0 };
    let w = (galley.size().x + extra + 36.0).max(min_w);
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(w, 36.0), egui::Sense::click());
    let stroke = if resp.hovered() {
        Stroke::new(1.0_f32, p.accent)
    } else {
        Stroke::new(1.0_f32, p.line)
    };
    let painter = ui.painter();
    painter.rect(rect, Rounding::same(999.0), p.card, stroke);
    let total = galley.size().x + extra;
    let mut x = rect.center().x - total / 2.0;
    painter.galley(
        egui::pos2(x, rect.center().y - galley.size().y / 2.0),
        galley.clone(),
        p.text,
    );
    x += galley.size().x;
    if icons > 0.0 {
        x += 6.0;
    }
    for icon in [vision.then_some("eye"), thinking.then_some("bulb")]
        .into_iter()
        .flatten()
    {
        crate::icons::paint(
            painter,
            egui::Rect::from_center_size(
                egui::pos2(x + 7.5, rect.center().y),
                egui::vec2(15.0, 15.0),
            ),
            icon,
            p.muted,
        );
        x += 19.0;
    }
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// Knopf mit Vektorsymbol aus `icons_data.rs`. Ist kein Symbol vorhanden,
/// wird die Beschriftung als Text gezeichnet (etwa „H1“ oder „abc“).
///
/// Wie `.ib` in `style.css`: ohne Hintergrund, beim Ueberfahren `--soft`,
/// im aktiven Zustand `--accent`. `size` und `rounding` bilden die
/// Ausnahmen der Vorlage ab (28er Fensterknoepfe, 36er Leiste, 32er
/// Symbolgruppen). Keine Hover-Animation, nur der Farbwechsel.
/// Liest eine Liste von Zeichenketten aus einer JSON-Antwort, wie
/// `edRunSynonyms` sie liefert: `["a","b"]` oder `{"synonyms":["a","b"]}`.
fn json_strings(text: &str) -> Vec<String> {
    let Some(start) = text.find('[') else {
        return Vec::new();
    };
    let Some(end) = text[start..].find(']').map(|i| start + i) else {
        return Vec::new();
    };
    let mut out: Vec<String> = Vec::new();
    let mut cur = String::new();
    let mut inside = false;
    let mut esc = false;
    for ch in text[start + 1..end].chars() {
        if inside {
            if esc {
                cur.push(ch);
                esc = false;
            } else if ch == '\u{5c}' {
                esc = true;
            } else if ch == '"' {
                out.push(std::mem::take(&mut cur));
                inside = false;
            } else {
                cur.push(ch);
            }
        } else if ch == '"' {
            inside = true;
        }
    }
    out
}

fn icon_btn_sized(
    ui: &mut egui::Ui,
    p: &Palette,
    name: &str,
    on: bool,
    size: f32,
    rounding: f32,
    accent_hover: bool,
) -> egui::Response {
    if crate::icons::icon(name).is_none() {
        // Kein Symbol vorhanden (etwa „H1“): als Textknopf mit echter Breite.
        let galley = ui
            .painter()
            .layout_no_wrap(name.to_string(), egui::FontId::proportional(13.0), p.text);
        let w = (galley.size().x + 14.0).max(size);
        let (rect, resp) = ui.allocate_exact_size(egui::vec2(w, size), egui::Sense::click());
        let hovered = resp.hovered();
        let fill = if on {
            p.accent
        } else if hovered {
            p.soft
        } else {
            Color32::TRANSPARENT
        };
        let ink = if on {
            p.on_accent
        } else if hovered {
            p.text
        } else {
            p.muted
        };
        if fill != Color32::TRANSPARENT {
            ui.painter().rect_filled(rect, Rounding::same(rounding), fill);
        }
        ui.painter().galley(
            egui::pos2(
                rect.center().x - galley.size().x / 2.0,
                rect.center().y - galley.size().y / 2.0,
            ),
            galley,
            ink,
        );
        return resp.on_hover_cursor(egui::CursorIcon::PointingHand);
    }
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(size, size), egui::Sense::click());
    let hovered = resp.hovered();
    let (fill, ink) = if accent_hover {
        if hovered {
            (p.accent_hover, p.on_accent)
        } else if on {
            (p.accent, p.on_accent)
        } else {
            (Color32::TRANSPARENT, p.muted)
        }
    } else if on {
        (p.accent, p.on_accent)
    } else if hovered {
        (p.soft, p.text)
    } else {
        (Color32::TRANSPARENT, p.muted)
    };
    if fill != Color32::TRANSPARENT {
        ui.painter().rect_filled(rect, Rounding::same(rounding), fill);
    }
    let icon_size = if size <= 28.0 { 16.0 } else { 18.0 };
    crate::icons::paint(
        ui.painter(),
        egui::Rect::from_center_size(rect.center(), egui::vec2(icon_size, icon_size)),
        name,
        ink,
    );
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// Standardknopf der Werkzeugleisten: 32 px, runder Kreis.
fn icon_btn(ui: &mut egui::Ui, p: &Palette, name: &str, on: bool) -> egui::Response {
    icon_btn_sized(ui, p, name, on, 32.0, 999.0, false)
}

/// Symbolknopf der Stilgruppen (`.seg .ib`): 32 px, rund, Hover in der
/// Akzentfarbe statt im Grau der uebrigen Knoepfe.
fn seg_btn(ui: &mut egui::Ui, p: &Palette, name: &str, on: bool) -> egui::Response {
    icon_btn_sized(ui, p, name, on, 32.0, 999.0, true)
}

/// Kopfzeile einer aufklappbaren Karte (`.ctx summary`): Dreieck, 8 px
/// Abstand, danach die Beschriftung. Das Dreieck zeigt geschlossen nach
/// rechts und offen nach unten. Die halbfette Schrift der Vorlage wird durch
/// einen zweiten, leicht versetzten Durchgang angedeutet.
fn bar_header(ui: &mut egui::Ui, p: &Palette, text: &str, open: bool) -> egui::Response {
    let w = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(w, 22.5), egui::Sense::click());
    let cy = rect.center().y;
    let x = rect.left() + 12.0;
    let pts = if open {
        vec![
            egui::pos2(x, cy - 5.0),
            egui::pos2(x + 5.0, cy),
            egui::pos2(x - 5.0, cy),
        ]
    } else {
        vec![
            egui::pos2(x, cy - 5.0),
            egui::pos2(x, cy + 5.0),
            egui::pos2(x + 7.0, cy),
        ]
    };
    let painter = ui.painter();
    painter.add(egui::Shape::convex_polygon(
        pts,
        p.muted,
        Stroke::NONE,
    ));
    let galley = painter.layout_no_wrap(text.to_string(), egui::FontId::proportional(15.0), p.text);
    let pos = egui::pos2(x + 15.0, cy - galley.size().y / 2.0);
    painter.galley(pos + egui::vec2(0.6, 0.0), galley.clone(), p.text);
    painter.galley(pos, galley, p.text);
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// Beschriftung der Seitenleiste mit fester Zeilenhoehe. `egui::Ui::label`
/// in einer waagerechten Zeile reserviert mehr Hoehe als die Schrift braucht;
/// die Karte wuerde dadurch rund 18 px je Gruppe zu hoch. Hier wird die
/// Zeilenhoehe deshalb vorgegeben.
fn side_label(
    ui: &mut egui::Ui,
    text: &str,
    size: f32,
    height: f32,
    indent: f32,
    color: egui::Color32,
    bold: bool,
) {
    let w = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(egui::vec2(w, height), egui::Sense::hover());
    let galley =
        ui.painter()
            .layout_no_wrap(text.to_string(), egui::FontId::proportional(size), color);
    let pos = egui::pos2(rect.left() + indent, rect.top());
    if bold {
        ui.painter()
            .galley(pos + egui::vec2(0.6, 0.0), galley.clone(), color);
    }
    ui.painter().galley(pos, galley, color);
}

/// Die Anweisung zu einem Knopf der Leiste oder Kontextleiste (`OPS` in
/// `editor.js`).
fn editor_instr(kind: &str) -> &'static str {
    match kind {
        "shorten" => "Shorten the following text by about one third. Keep the meaning, tone and key facts. Output only the shortened text, in the same language as the input, without explanations.",
        "lengthen" => "Expand the following text by about 50%: add useful detail, explanation and smoother transitions, but do not invent facts. Output only the expanded text, in the same language as the input.",
        "summarize" => "Summarize the following text concisely in one to three sentences. Output only the summary, in the same language as the input.",
        _ => "Proofread the following text. Correct only spelling, grammar, punctuation and obvious typos. Do not translate, do not change wording or style. Output only the corrected text.",
    }
}

/// Zerlegt Text an Absatzgrenzen in Stuecke von hoechstens `n` Zeichen
/// (`chunkText` in `editor.js`).
fn chunk_text(text: &str, n: usize) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut aktuell = String::new();
    for absatz in text.split('\n') {
        if !aktuell.is_empty() && aktuell.chars().count() + absatz.chars().count() > n {
            out.push(aktuell.clone());
            aktuell.clear();
        }
        aktuell.push_str(absatz);
        aktuell.push('\n');
    }
    if !aktuell.trim().is_empty() {
        out.push(aktuell);
    }
    if out.is_empty() {
        out.push(text.to_string());
    }
    out
}

/// Entfernt, was das Modell ausser dem Text mitliefert: wiederholte
/// Anweisungen, `<text>`-Reste, Code-Zaeune und umschliessende Anfuehrungszeichen.
fn clean_editor_output(text: &str) -> String {
    let mut out = text.trim().to_string();
    if let Some(pos) = out.rfind("</text>") {
        out = out[pos + 7..].trim_start().to_string();
    } else if let Some(pos) = out.rfind("<text>") {
        out = out[pos + 6..].trim_start().to_string();
    }
    let mut zeilen: Vec<String> = out.lines().map(|l| l.to_string()).collect();
    // Eine oder mehrere Anweisungszeilen am Anfang ("Shorten the ... :") fallen weg.
    while zeilen.len() > 1 {
        let erste = zeilen[0].trim().to_string();
        let anweisung = erste.ends_with(':')
            && erste.split_whitespace().count() <= 26
            && !erste.starts_with('"')
            && !erste.starts_with('„');
        if !anweisung {
            break;
        }
        zeilen.remove(0);
        while zeilen.first().map(|l| l.trim().is_empty()).unwrap_or(false) {
            zeilen.remove(0);
        }
    }
    out = zeilen.join("\n").trim().to_string();
    if out.starts_with("```") {
        out = out.trim_start_matches("```").trim_start().to_string();
        if let Some(ende) = out.rfind("```") {
            out.truncate(ende);
        }
        out = out.trim().to_string();
    }
    for paar in [('"', '"'), ('\u{201e}', '\u{201c}'), ('\u{201c}', '\u{201d}'), ('\'', '\'')] {
        if out.starts_with(paar.0) && out.ends_with(paar.1) && out.chars().count() > 2 {
            out = out
                .trim_start_matches(paar.0)
                .trim_end_matches(paar.1)
                .trim()
                .to_string();
        }
    }
    out
}

/// Ladekreis wie `.spin` in der Vorlage: 18 px, grauer Ring, heller Bogen,
/// dreht sich mit der Zeit.
fn ladekreis(ui: &mut egui::Ui, p: &Palette) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(18.0, 18.0), egui::Sense::hover());
    let mitte = rect.center();
    let radius = 8.0;
    ui.painter()
        .circle_stroke(mitte, radius, Stroke::new(2.0_f32, p.line));
    let start = (ui.input(|i| i.time) as f32 * 300.0).rem_euclid(360.0);
    let mut pts = Vec::new();
    for i in 0..=24 {
        let winkel = (start + i as f32 * 3.6).to_radians();
        pts.push(mitte + egui::vec2(winkel.cos(), winkel.sin()) * radius);
    }
    ui.painter()
        .add(egui::Shape::line(pts, Stroke::new(2.0_f32, p.accent_ink)));
    ui.ctx()
        .request_repaint_after(std::time::Duration::from_millis(40));
}

/// Zeile in einem ankergebundenen Menue wie `.row` in `style.css`: 32 px hoch,
/// Hintergrund nur beim Ueberfahren, Name links, optional ein Haken rechts.
/// Rechnet einen Kartenplatz nach dem Umsortieren um: die gezogene Karte landet
/// bei `nach`, alle dazwischen ruecken um eins weiter.
fn verschieben(aktiv: usize, von: usize, nach: usize) -> usize {
    if aktiv == von {
        nach
    } else if von < nach && aktiv > von && aktiv <= nach {
        aktiv - 1
    } else if von > nach && aktiv >= nach && aktiv < von {
        aktiv + 1
    } else {
        aktiv
    }
}

fn row_zeile(ui: &mut egui::Ui, p: &Palette, label: &str, sel: bool, haken: bool) -> bool {
    let breite = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(breite, 32.0), egui::Sense::click());
    if resp.hovered() {
        ui.painter().rect_filled(rect, Rounding::same(10.0), p.soft);
    }
    let text = if sel {
        format!("• {}", label)
    } else {
        label.to_string()
    };
    ui.painter().text(
        egui::pos2(rect.left() + 12.0, rect.center().y),
        egui::Align2::LEFT_CENTER,
        text,
        egui::FontId::proportional(14.0),
        if sel { p.accent_ink } else { p.text },
    );
    if haken {
        crate::icons::paint(
            ui.painter(),
            egui::Rect::from_center_size(
                egui::pos2(rect.right() - 18.0, rect.center().y),
                egui::vec2(15.0, 15.0),
            ),
            "check",
            p.accent_ink,
        );
    }
    resp.clicked()
}

/// Eine Zeile im Modellmenue: Name links, rechts Symbole fuer Bild- und
/// Denkfaehigkeit sowie die MT-Kennzeichnung. Entspricht `.row` in `style.css`.
fn menu_row(
    ui: &mut egui::Ui,
    p: &Palette,
    lang: Lang,
    name: &str,
    sel: bool,
    vision: bool,
    thinking: bool,
    mt: bool,
) -> bool {
    let (rect, resp) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 28.0), egui::Sense::click());
    if resp.hovered() {
        ui.painter().rect_filled(rect, Rounding::same(10.0), p.soft);
    }
    let color = if sel { p.accent_ink } else { p.text };
    let reserved = 14.0
        + if thinking { 18.0 } else { 0.0 }
        + if vision { 18.0 } else { 0.0 }
        + if mt { 24.0 } else { 0.0 };
    let mut x = rect.right() - 12.0;
    if thinking {
        crate::icons::paint(
            ui.painter(),
            egui::Rect::from_center_size(egui::pos2(x, rect.center().y), egui::vec2(15.0, 15.0)),
            "bulb",
            p.muted,
        );
        x -= 18.0;
    }
    if vision {
        crate::icons::paint(
            ui.painter(),
            egui::Rect::from_center_size(egui::pos2(x, rect.center().y), egui::vec2(15.0, 15.0)),
            "eye",
            p.muted,
        );
        x -= 18.0;
    }
    if mt {
        // Reines Uebersetzungsmodell: zwei Pfeile statt der Buchstaben „MT“.
        let sym = egui::Rect::from_center_size(
            egui::pos2(x - 7.5, rect.center().y),
            egui::vec2(15.0, 15.0),
        );
        crate::icons::paint(ui.painter(), sym, "swap", p.muted);
        ui.interact(sym, egui::Id::new(("mt_tip", name)), egui::Sense::hover())
            .on_hover_text(t(lang, "mt_tip"));
    }
    let text_rect = egui::Rect::from_min_max(
        rect.left_top(),
        egui::pos2(rect.right() - reserved, rect.bottom()),
    );
    ui.painter().with_clip_rect(text_rect).text(
        egui::pos2(rect.left() + 10.0, rect.center().y),
        egui::Align2::LEFT_CENTER,
        name,
        egui::FontId::proportional(13.0),
        color,
    );
    resp.clicked()
}

fn card_ui<R>(
    ui: &mut egui::Ui,
    p: &Palette,
    add: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    egui::Frame::none()
        .fill(p.card)
        .stroke(Stroke::new(1.0_f32, p.line))
        .rounding(Rounding::same(18.0))
        .inner_margin(egui::Margin::same(0.0))
        .show(ui, add)
        .inner
}

impl eframe::App for App {
    /// Beendet den mitgestarteten NLLB-Server, wie `will-quit` in `main.js`.
    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        crate::nllb::stop(&mut self.nllb_child);
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let dark = match self.set.theme.as_str() {
            "dark" => true,
            "light" => false,
            _ => self.sys_dark,
        };
        self.pal = Palette::of(dark);
        self.apply_theme(ctx);
        self.poll(ctx);

        // Die Oberflaechensprache folgt der Einstellung sofort, ohne Neustart —
        // das Gegenstueck zu `renderAll()` in `renderer.js`, das nach jeder
        // Aenderung neu beschriftet.
        self.lang = Lang::resolve(&self.set.ui_lang);
        let pal = self.pal;
        let rtl = self.rtl();
        let lang = self.lang;

        // ---- Fensterleiste -------------------------------------------------
        egui::TopBottomPanel::top("titlebar")
            .exact_height(32.0)
            .frame(
                egui::Frame::none().fill(pal.bg).inner_margin(egui::Margin {
                    left: 12.0,
                    right: 6.0,
                    top: 0.0,
                    bottom: 0.0,
                }),
            )
            .show(ctx, |ui| {
                ui.horizontal_centered(|ui| {
                    ui.label(RichText::new(t(lang, "app")).size(12.5).color(pal.muted));
                    let drag = ui.interact(
                        egui::Rect::from_min_size(
                            egui::pos2(ui.min_rect().left() + 60.0, ui.min_rect().top()),
                            egui::vec2((ui.available_width() - 120.0).max(10.0), 32.0),
                        ),
                        egui::Id::new("dragbar"),
                        egui::Sense::drag(),
                    );
                    if drag.drag_started() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
                    }
                    ui.with_layout(
                        egui::Layout::right_to_left(egui::Align::Center),
                        |ui| {
                            ui.spacing_mut().item_spacing.x = 2.0;
                            if icon_btn_sized(ui, &pal, "x", false, 36.0, 999.0, false)
                                .on_hover_text(t(lang, "win_close"))
                                .clicked()
                            {
                                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                            }
                            let maximized =
                                ctx.input(|i| i.viewport().maximized.unwrap_or(false));
                            let max_icon = if maximized { "winRestore" } else { "winMax" };
                            if icon_btn_sized(ui, &pal, max_icon, false, 36.0, 999.0, false)
                                .on_hover_text(t(lang, "win_max"))
                                .clicked()
                            {
                                ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(!maximized));
                            }
                            if icon_btn_sized(ui, &pal, "winMin", false, 36.0, 999.0, false)
                                .on_hover_text(t(lang, "win_min"))
                                .clicked()
                            {
                                ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
                            }
                        },
                    );
                });
            });

        // ---- Navigationsleiste --------------------------------------------
        let rail = if rtl {
            egui::SidePanel::right("rail")
        } else {
            egui::SidePanel::left("rail")
        };
        rail
            .exact_width(36.0)
            .resizable(false)
            .show_separator_line(false)
            .frame(
                egui::Frame::none().fill(pal.bg).inner_margin(egui::Margin {
                    left: 10.0,
                    right: 10.0,
                    top: 12.0,
                    bottom: 12.0,
                }),
            )
            .show(ctx, |ui| {
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 6.0;
                    let both = self.set.workspace == "both";
                    if both || self.set.workspace == "tr" {
                        if icon_btn_sized(
                            ui,
                            &pal,
                            "languages",
                            self.view == View::Translator,
                            36.0,
                            999.0,
                            false,
                        )
                        .on_hover_text(t(lang, "nav_tr"))
                        .clicked()
                        {
                            self.set_view(View::Translator);
                        }
                    }
                    if both || self.set.workspace == "ed" {
                        if icon_btn_sized(
                            ui,
                            &pal,
                            "doc",
                            self.view == View::Editor,
                            36.0,
                            999.0,
                            false,
                        )
                        .on_hover_text(t(lang, "nav_ed"))
                        .clicked()
                        {
                            self.set_view(View::Editor);
                        }
                        if icon_btn_sized(
                            ui,
                            &pal,
                            "library",
                            self.view == View::Library,
                            36.0,
                            999.0,
                            false,
                        )
                        .on_hover_text(t(lang, "nav_lib"))
                        .clicked()
                        {
                            self.set_view(View::Library);
                        }
                    }
                    ui.add_space((ui.available_height() - 36.0).max(0.0));
                    if icon_btn_sized(ui, &pal, "sliders", false, 36.0, 999.0, false)
                        .on_hover_text(t(lang, "settings"))
                        .clicked()
                    {
                        self.show_settings = true;
                    }
                });
            });

        if self.view == View::Translator {
            self.side_panel(ctx);
        }

        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(pal.bg))
            .show(ctx, |ui| match self.view {
                View::Translator => {
                    let margin = if rtl {
                        egui::Margin {
                            left: 8.0,
                            right: 0.0,
                            top: 16.0,
                            bottom: 16.0,
                        }
                    } else {
                        egui::Margin {
                            left: 0.0,
                            right: 8.0,
                            top: 16.0,
                            bottom: 16.0,
                        }
                    };
                    egui::Frame::none()
                        .inner_margin(margin)
                        .show(ui, |ui| self.translator(ui));
                }
                View::Editor => {
                    egui::Frame::none()
                        .inner_margin(egui::Margin::symmetric(10.0, 8.0))
                        .show(ui, |ui| self.editor(ui));
                }
                View::Library => {
                    let margin = if rtl {
                        egui::Margin {
                            left: 12.0,
                            right: 0.0,
                            top: 12.0,
                            bottom: 12.0,
                        }
                    } else {
                        egui::Margin {
                            left: 0.0,
                            right: 12.0,
                            top: 12.0,
                            bottom: 12.0,
                        }
                    };
                    egui::Frame::none()
                        .inner_margin(margin)
                        .show(ui, |ui| self.library(ui));
                }
            });

        self.poll_spell();
        if self.spell_waiting {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }

        self.lang_popup(ctx);
        self.settings_window(ctx);
        self.models_menu(ctx);
        self.dict_window(ctx);
        self.style_window(ctx);
        self.syn_window(ctx);
        self.confirm_window(ctx);
        self.diff_window(ctx);
        self.spell_window(ctx);
        self.dict_menu(ctx);
        self.save_menu(ctx);
        self.alt_menu_window(ctx);
        self.style_bar_window(ctx);
        self.mini_window(ctx);
        self.spell_menu_window(ctx);
    }
}

impl App {
    fn set_view(&mut self, view: View) {
        self.view = view;
        self.set.last_view = match view {
            View::Translator => "tr",
            View::Editor => "ed",
            View::Library => "lib",
        }
        .into();
        self.save_set();
        if view == View::Library {
            self.lib = library::list();
        }
    }

    // ---- Uebersetzer ------------------------------------------------------
    fn translator(&mut self, ui: &mut egui::Ui) {
        let pal = self.pal;
        let lang = self.lang;
        let (vision, thinking) = self
            .models
            .iter()
            .find(|m| m.name == self.set.model)
            .map(|m| (m.vision, m.thinking))
            .unwrap_or((false, false));

        // Feste Abstaende wie in `style.css` (12 px zwischen Kopf und Karten).
        ui.spacing_mut().item_spacing.y = 0.0;

        // Kopfzeile: Sprachen links, Modell rechts, dazwischen die
        // Tausch- und Neuladen-Knoepfe. Entspricht `.langs`/`.model` und
        // `header` in `style.css`.
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            let src = if self.set.src == "auto" {
                t(lang, "detect")
            } else {
                lang_label(&self.set.src, &self.nllb_langs)
            };
            let src_resp = pill(ui, &pal, &src, 150.0);
            if src_resp.clicked() {
                self.lang_search.clear();
                self.popup_lang = Some("src");
                self.lang_anchor = Some(src_resp.rect);
            }
            if icon_btn_sized(ui, &pal, "swap", false, 36.0, 12.0, false)
                .on_hover_text(t(lang, "swap"))
                .clicked()
                && self.set.src != "auto"
            {
                std::mem::swap(&mut self.set.src, &mut self.set.tgt);
                if !self.last_tr.is_empty() {
                    self.input = std::mem::take(&mut self.last_tr);
                    self.output.clear();
                }
                self.save_set();
            }
            let tgt_resp = pill(ui, &pal, &lang_label(&self.set.tgt, &self.nllb_langs), 150.0);
            if tgt_resp.clicked() {
                self.lang_search.clear();
                self.popup_lang = Some("tgt");
                self.lang_anchor = Some(tgt_resp.rect);
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                let rresp = icon_btn_sized(ui, &pal, "refresh", false, 36.0, 12.0, false)
                    .on_hover_text(t(lang, "reload"));
                // Blauer Punkt am Neuladen-Knopf, solange die Modellliste geholt wird.
                if self.models_loading {
                    let r = rresp.rect;
                    ui.painter().circle_filled(
                        egui::pos2(r.right() - 5.0, r.top() + 5.0),
                        3.5,
                        pal.accent_ink,
                    );
                }
                if rresp.clicked() {
                    self.models_loading = true;
                    self.models_error = None;
                    net::spawn_models(self.cfg(), self.tx.clone(), Vec::new());
                }
                let mresp = model_pill(
                    ui,
                    &pal,
                    &short_model(&self.set.model),
                    vision,
                    thinking,
                    150.0,
                );
                if mresp.clicked() {
                    self.models_anchor = mresp.rect;
                    self.models_for_editor = false;
                    self.show_models = !self.show_models;
                }
            });
        });
        ui.add_space(12.0);

        let gap = 12.0;
        let col_w = ((ui.available_width() - gap) / 2.0).max(160.0);
        let full_h = ui.available_height();
        let lim = is_mt(&self.set.model) || is_tg(&self.set.model);
        let mut card_h = full_h;
        if lim {
            ui.label(
                RichText::new(t(
                    lang,
                    if is_mt(&self.set.model) {
                        "note_nllb"
                    } else {
                        "note_tg"
                    },
                ))
                .size(12.0)
                .color(pal.muted),
            );
            card_h = (full_h - 22.0).max(120.0);
        }

        let footer_h = 52.0;
        let body_h = (card_h - footer_h).max(80.0);

        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            ui.allocate_ui(egui::vec2(col_w, card_h), |ui| {
                card_ui(ui, &pal, |ui| {
                    ui.set_min_size(egui::vec2(col_w, card_h));
                    ui.vertical(|ui| {
                        ui.spacing_mut().item_spacing.y = 0.0;
                        egui::ScrollArea::vertical()
                            .id_source("in_scroll")
                            .max_height(body_h)
                            .auto_shrink([false, false])
                            .show(ui, |ui| {
                                ui.add(
                                    egui::TextEdit::multiline(&mut self.input)
                                        .frame(false)
                                        .font(egui::FontId::proportional(17.0))
                                        .desired_width(f32::INFINITY)
                                        .hint_text(RichText::new(t(lang, "input_ph")).color(pal.muted))
                                        .margin(egui::vec2(20.0, 18.0)),
                                );
                            });
                        egui::Frame::none()
                            .inner_margin(egui::Margin {
                                left: 12.0,
                                right: 12.0,
                                top: 8.0,
                                bottom: 8.0,
                            })
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    ui.spacing_mut().item_spacing.x = 8.0;
                                    if icon_btn_sized(ui, &pal, "clip", false, 36.0, 12.0, false)
                                        .on_hover_text(t(lang, "attach"))
                                        .clicked()
                                    {
                                        if let Some(path) = rfd::FileDialog::new()
                                            .add_filter(
                                                "Text",
                                                &["txt", "md", "csv", "srt", "json", "html"],
                                            )
                                            .pick_file()
                                        {
                                            if let Ok(text) = std::fs::read_to_string(&path) {
                                                if !self.input.is_empty() {
                                                    self.input.push_str("\n\n");
                                                }
                                                self.input.push_str(&text);
                                            }
                                        }
                                    }
                                    ui.label(
                                        RichText::new(format!(
                                            "{} {}",
                                            self.input.chars().count(),
                                            t(lang, "chars")
                                        ))
                                        .size(13.0)
                                        .color(pal.muted),
                                    );
                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::Center),
                                        |ui| {
                                            if ui
                                                .add_enabled(
                                                    !self.busy,
                                                    egui::Button::new(
                                                        RichText::new(t(lang, "translate"))
                                                            .color(pal.on_accent)
                                                            .size(15.0)
                                                            .strong(),
                                                    )
                                                    .fill(pal.accent)
                                                    .stroke(Stroke::NONE)
                                                    .rounding(Rounding::same(999.0))
                                                    .min_size(egui::vec2(110.0, 36.0)),
                                                )
                                                .clicked()
                                            {
                                                self.start_translate();
                                            }
                                        },
                                    );
                                });
                            });
                    });
                });
            });
            ui.add_space(gap);
            ui.allocate_ui(egui::vec2(col_w, card_h), |ui| {
                card_ui(ui, &pal, |ui| {
                    ui.set_min_size(egui::vec2(col_w, card_h));
                    ui.vertical(|ui| {
                        ui.spacing_mut().item_spacing.y = 0.0;
                        egui::ScrollArea::vertical()
                            .id_source("out_scroll")
                            .max_height(body_h)
                            .auto_shrink([false, false])
                            .show(ui, |ui| {
                                egui::Frame::none()
                                    .inner_margin(egui::Margin {
                                        left: 20.0,
                                        right: 20.0,
                                        top: 18.0,
                                        bottom: 0.0,
                                    })
                                    .show(ui, |ui| {
                                        ui.set_width(ui.available_width());
                                        let text = if self.output.is_empty() {
                                            " ".to_string()
                                        } else {
                                            filter_profanity(
                                                &self.output,
                                                &self.set.tgt,
                                                &self.set.src,
                                                &self.set.profanity_style,
                                            )
                                        };
                                        // Wie `#output .w` in der Vorlage: jedes Wort ist
                                        // anklickbar und bietet Ersatzwoerter an.
                                        let font = egui::FontId::proportional(17.0);
                                        let galley = ui.fonts(|f| {
                                            f.layout(
                                                text.clone(),
                                                font,
                                                pal.text,
                                                ui.available_width(),
                                            )
                                        });
                                        let (rect, resp) = ui.allocate_exact_size(
                                            galley.size(),
                                            egui::Sense::click(),
                                        );
                                        ui.painter().galley(rect.min, galley.clone(), pal.text);
                                        if resp.clicked() {
                                            if let Some(pos) = resp.interact_pointer_pos() {
                                                let idx = galley
                                                    .cursor_from_pos(pos - rect.min)
                                                    .ccursor
                                                    .index;
                                                self.open_alternatives(&text, idx, pos);
                                            }
                                        }
                                    });
                            });
                        egui::Frame::none()
                            .inner_margin(egui::Margin {
                                left: 12.0,
                                right: 12.0,
                                top: 8.0,
                                bottom: 8.0,
                            })
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    ui.spacing_mut().item_spacing.x = 8.0;
                                    ui.label(
                                        RichText::new(&self.status).size(13.0).color(pal.muted),
                                    );
                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::Center),
                                        |ui| {
                                            if icon_btn_sized(
                                                ui, &pal, "copy", false, 36.0, 12.0, false,
                                            )
                                            .on_hover_text(t(lang, "copy"))
                                            .clicked()
                                            {
                                                ui.output_mut(|o| {
                                                    o.copied_text = if self.last_tr.is_empty() {
                                                        self.output.clone()
                                                    } else {
                                                        self.last_tr.clone()
                                                    }
                                                });
                                                self.status = t(lang, "copied");
                                            }
                                            if self.busy
                                                && icon_btn_sized(
                                                    ui, &pal, "stop", false, 36.0, 12.0, false,
                                                )
                                                .on_hover_text(t(lang, "stop"))
                                                .clicked()
                                            {
                                                self.stop_translation();
                                            }
                                        },
                                    );
                                });
                            });
                    });
                });
            });
        });
    }

    // ---- Rechte Seitenleiste ---------------------------------------------
    fn side_panel(&mut self, ctx: &egui::Context) {
        let pal = self.pal;
        let lang = self.lang;
        egui::SidePanel::right("style_side")
            .exact_width(300.0)
            .resizable(false)
            .show_separator_line(false)
            .frame(egui::Frame::none().fill(pal.bg).inner_margin(egui::Margin {
                left: 14.0,
                right: 14.0,
                // Wie `alignSideTop` in renderer.js: Die Karte beginnt auf
                // derselben Hoehe wie die beiden Textkarten (Kopfzeile 36 px
                // plus 12 px Abstand plus 16 px Rand der Mittelebene).
                top: 64.0,
                bottom: 14.0,
            }))
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    // Feste Innenbreite der Leiste (300 px minus 2 x 14 px Rand),
                    // damit die Karten nicht ueber den Fensterrand hinauswachsen.
                    let side_w = 300.0 - 28.0;
                    ui.set_width(side_w);
                    // Die Vorlage rechnet mit festen Abstaenden (8 px). egui
                    // wuerde zwischen jedem Element zusaetzlich 8 px setzen;
                    // hier wird deshalb nur mit `add_space` gearbeitet.
                    ui.spacing_mut().item_spacing.y = 0.0;
                    // ---- Karte mit den Stilgruppen -------------------------
                    egui::Frame::none()
                        .fill(pal.card)
                        .stroke(Stroke::new(1.0_f32, pal.line))
                        .rounding(Rounding::same(18.0))
                        .inner_margin(egui::Margin {
                            left: 12.0,
                            right: 12.0,
                            top: 12.0,
                            bottom: 16.0,
                        })
                        .show(ui, |ui| {
                            ui.set_width(side_w - 24.0);
                            side_label(ui, &t(lang, "style"), 15.0, 22.5, 4.0, pal.text, true);
                            ui.add_space(16.0);
                            let groups: [(&str, &[(&str, &str)]); 4] = [
                                ("l_type", &[("natural", "natural"), ("literal", "literal")]),
                                (
                                    "l_formal",
                                    &[
                                        ("standard", "dash"),
                                        ("formal", "hat"),
                                        ("informal", "smile"),
                                    ],
                                ),
                                (
                                    "l_gender",
                                    &[
                                        ("unknown", "qmark"),
                                        ("neutral", "nb"),
                                        ("male", "male"),
                                        ("female", "female"),
                                    ],
                                ),
                                (
                                    "l_writer",
                                    &[
                                        ("unknown", "qmark"),
                                        ("neutral", "nb"),
                                        ("male", "male"),
                                        ("female", "female"),
                                    ],
                                ),
                            ];
                            for (i, (label, opts)) in groups.iter().enumerate() {
                                if i > 0 {
                                    ui.add_space(16.0);
                                }
                                side_label(
                                    ui,
                                    &t(lang, label),
                                    12.5,
                                    25.6,
                                    2.0,
                                    pal.muted,
                                    false,
                                );
                                ui.horizontal_wrapped(|ui| {
                                    ui.spacing_mut().item_spacing = egui::vec2(2.0, 2.0);
                                    for (value, icon) in opts.iter() {
                                        let current = match *label {
                                            "l_type" => &self.set.style.kind,
                                            "l_formal" => &self.set.style.formal,
                                            "l_gender" => &self.set.style.gender,
                                            _ => &self.set.style.writer,
                                        };
                                        let on = current == value;
                                        let tip = t(lang, &format!("o_{}", value));
                                        if seg_btn(ui, &pal, icon, on)
                                            .on_hover_text(tip)
                                            .clicked()
                                        {
                                            match *label {
                                                "l_type" => self.set.style.kind = value.to_string(),
                                                "l_formal" => {
                                                    self.set.style.formal = value.to_string()
                                                }
                                                "l_gender" => {
                                                    self.set.style.gender = value.to_string()
                                                }
                                                _ => self.set.style.writer = value.to_string(),
                                            }
                                            self.save_set();
                                        }
                                    }
                                });
                            }
                        });
                    ui.add_space(8.0);

                    // ---- Zusammenhang (aufklappbar) ------------------------
                    let ctx_id = egui::Id::new("side_ctx_open");
                    let mut ctx_open = ui.data(|d| d.get_temp::<bool>(ctx_id).unwrap_or(false));
                    egui::Frame::none()
                        .fill(pal.soft)
                        .rounding(Rounding::same(18.0))
                        .inner_margin(egui::Margin {
                            left: 12.0,
                            right: 12.0,
                            top: 10.0,
                            bottom: 10.0,
                        })
                        .show(ui, |ui| {
                            ui.set_width(side_w - 24.0);
                            if bar_header(ui, &pal, &t(lang, "context"), ctx_open).clicked() {
                                ctx_open = !ctx_open;
                            }
                            if ctx_open {
                                ui.add_sized(
                                    egui::vec2(ui.available_width(), 110.0),
                                    egui::TextEdit::multiline(&mut self.set.ctxt)
                                        .frame(false)
                                        .font(egui::FontId::proportional(14.0))
                                        .margin(egui::vec2(0.0, 10.0))
                                        .hint_text(RichText::new(t(lang, "ctx_ph")).color(pal.muted)),
                                );
                                if ui.input(|i| i.pointer.any_released()) {
                                    self.save_set();
                                }
                            }
                        });
                    ui.data_mut(|d| d.insert_temp(ctx_id, ctx_open));
                    ui.add_space(8.0);

                    // ---- Glossar (aufklappbar) ------------------------------
                    let gl_id = egui::Id::new("side_gloss_open");
                    let mut gl_open = ui.data(|d| d.get_temp::<bool>(gl_id).unwrap_or(false));
                    egui::Frame::none()
                        .fill(pal.soft)
                        .rounding(Rounding::same(18.0))
                        .inner_margin(egui::Margin {
                            left: 12.0,
                            right: 12.0,
                            top: 10.0,
                            bottom: 10.0,
                        })
                        .show(ui, |ui| {
                            ui.set_width(side_w - 24.0);
                            if bar_header(ui, &pal, &t(lang, "glossary"), gl_open).clicked() {
                                gl_open = !gl_open;
                            }
                            if gl_open {
                                let mut remove: Option<usize> = None;
                                for (i, g) in self.set.gloss.iter_mut().enumerate() {
                                    ui.add_space(6.0);
                                    let w = ui.available_width();
                                    let field_w = ((w - 36.0 - 18.0 - 9.0) / 2.0).max(40.0);
                                    ui.horizontal(|ui| {
                                        ui.spacing_mut().item_spacing.x = 6.0;
                                        egui::Frame::none()
                                            .fill(pal.card)
                                            .stroke(Stroke::new(1.0_f32, pal.line))
                                            .rounding(Rounding::same(10.0))
                                            .inner_margin(egui::Margin::symmetric(8.0, 6.0))
                                            .show(ui, |ui| {
                                                ui.add(
                                                    egui::TextEdit::singleline(&mut g.s)
                                                        .frame(false)
                                                        .font(egui::FontId::proportional(13.0))
                                                        .desired_width(field_w - 18.0)
                                                        .hint_text(RichText::new(t(lang, "term")).color(pal.muted)),
                                                );
                                            });
                                        ui.label(RichText::new("→").size(13.0).color(pal.muted));
                                        egui::Frame::none()
                                            .fill(pal.card)
                                            .stroke(Stroke::new(1.0_f32, pal.line))
                                            .rounding(Rounding::same(10.0))
                                            .inner_margin(egui::Margin::symmetric(8.0, 6.0))
                                            .show(ui, |ui| {
                                                ui.add(
                                                    egui::TextEdit::singleline(&mut g.t)
                                                        .frame(false)
                                                        .font(egui::FontId::proportional(13.0))
                                                        .desired_width(field_w - 18.0)
                                                        .hint_text(RichText::new(t(lang, "term_t")).color(pal.muted)),
                                                );
                                            });
                                        if icon_btn_sized(ui, &pal, "x", false, 36.0, 12.0, false)
                                            .on_hover_text(t(lang, "remove"))
                                            .clicked()
                                        {
                                            remove = Some(i);
                                        }
                                    });
                                }
                                if let Some(i) = remove {
                                    self.set.gloss.remove(i);
                                    self.save_set();
                                }
                                ui.add_space(6.0);
                                let add = t(lang, "add_entry");
                                let galley = ui.painter().layout_no_wrap(
                                    add.clone(),
                                    egui::FontId::proportional(13.0),
                                    pal.text,
                                );
                                let (rect, resp) = ui.allocate_exact_size(
                                    egui::vec2(galley.size().x + 28.0, 30.0),
                                    egui::Sense::click(),
                                );
                                let fill = if resp.hovered() { pal.card } else { pal.soft };
                                ui.painter().rect(rect, Rounding::same(999.0), fill, Stroke::NONE);
                                ui.painter().galley(
                                    egui::pos2(
                                        rect.center().x - galley.size().x / 2.0,
                                        rect.center().y - galley.size().y / 2.0,
                                    ),
                                    galley,
                                    pal.text,
                                );
                                if resp
                                    .on_hover_cursor(egui::CursorIcon::PointingHand)
                                    .clicked()
                                {
                                    self.set.gloss.push(crate::settings::GlossEntry {
                                        s: String::new(),
                                        t: String::new(),
                                    });
                                    self.save_set();
                                }
                            }
                        });
                    ui.data_mut(|d| d.insert_temp(gl_id, gl_open));
                });
            });
    }

    // ---- Sprachauswahl ----------------------------------------------------
    fn lang_popup(&mut self, ctx: &egui::Context) {
        let Some(target) = self.popup_lang else { return };
        let pal = self.pal;
        let lang = self.lang;
        
        let langs = self.active_langs();
        let is_src = target == "src";
        let current = if is_src { self.set.src.clone() } else { self.set.tgt.clone() };
        let pins_key = if is_mt(&self.set.model) { "nllb" } else { "ollama" };
        let search = self.lang_search.to_lowercase();
        let title = t(lang, if is_src { "auto" } else { "translate" });
        let anchor = self
            .lang_anchor
            .unwrap_or_else(|| egui::Rect::from_min_size(egui::pos2(100.0, 90.0), egui::vec2(150.0, 32.0)));
        let _ = title;
        let closed = pop_at(ctx, "lang_pop", anchor, 300.0, &pal, |ui| {
                // Suchfeld wie `.psearch`: volle Breite, weicher Grund.
                ui.scope(|ui| {
                    ui.style_mut().visuals.extreme_bg_color = pal.soft;
                                        ui.add_sized(
                        egui::vec2(ui.available_width(), 34.0),
                        egui::TextEdit::singleline(&mut self.lang_search)
                            .font(egui::FontId::proportional(15.0))
                            .hint_text(RichText::new(t(lang, "search_lang")).color(pal.muted))
                            .margin(egui::vec2(10.0, 8.0)),
                    );
                });
                ui.add_space(4.0);
                let mut pick: Option<String> = None;
                let mut toggle_pin: Option<String> = None;
                let pins = if pins_key == "nllb" {
                    self.set.pins.nllb.clone()
                } else {
                    self.set.pins.ollama.clone()
                };
                let mut rows: Vec<(String, String, bool)> = Vec::new();
                if is_src && !is_mt(&self.set.model) && !is_tg(&self.set.model) {
                    rows.push(("auto".into(), t(lang, "detect"), false));
                }
                for (code, native) in &langs {
                    let eng = eng_name(code, &self.nllb_langs);
                    let label = if native.is_empty() { eng.clone() } else { native.clone() };
                    if !search.is_empty()
                        && !label.to_lowercase().contains(&search)
                        && !eng.to_lowercase().contains(&search)
                        && !code.to_lowercase().contains(&search)
                    {
                        continue;
                    }
                    let pinned = pins.contains(code);
                    rows.push((code.clone(), label, pinned));
                }
                // Reihenfolge wie `langPop` in `renderer.js`: erst „Sprache
                // erkennen“, dann die angepinnten Sprachen, ein Trennstrich und
                // danach der Rest der Liste.
                rows.sort_by_key(|(code, _, pinned)| {
                    if code == "auto" {
                        0u8
                    } else if *pinned {
                        1
                    } else {
                        2
                    }
                });
                let mut shown = 0usize;
                let mut vorige_gruppe = 0u8;
                egui::ScrollArea::vertical()
                    .max_height(360.0)
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        // Platz fuer die Scrollleiste, damit sie die Sterne am
                        // rechten Rand nicht ueberdeckt.
                        let breite = (ui.available_width() - 12.0).max(120.0);
                        for (code, label, pinned) in rows {
                            shown += 1;
                            let on = code == current;
                            let gruppe = if code == "auto" {
                                0u8
                            } else if pinned {
                                1
                            } else {
                                2
                            };
                            // Trennstrich zwischen angepinnten und uebrigen
                            // Sprachen wie `hr` in `.pop` (4 px Abstand, 1 px
                            // Linie, seitlich 8 px eingerueckt).
                            if vorige_gruppe == 1 && gruppe == 2 {
                                ui.add_space(4.0);
                                let (linie, _) = ui.allocate_exact_size(
                                    egui::vec2(breite - 8.0, 1.0),
                                    egui::Sense::hover(),
                                );
                                ui.painter().rect_filled(linie, Rounding::ZERO, pal.line);
                                ui.add_space(4.0);
                            }
                            vorige_gruppe = gruppe;
                            // Ganze Zeile wie `.row` in `style.css`: 32 px hoch,
                            // 10er Rundung, Hintergrund nur beim Ueberfahren.
                            let (rect, resp) = ui.allocate_exact_size(
                                egui::vec2(breite, 32.0),
                                egui::Sense::click(),
                            );
                            if resp.hovered() {
                                ui.painter().rect_filled(rect, Rounding::same(10.0), pal.soft);
                            }
                            ui.painter().text(
                                egui::pos2(rect.left() + 12.0, rect.center().y),
                                egui::Align2::LEFT_CENTER,
                                label.clone(),
                                egui::FontId::proportional(14.0),
                                if on { pal.accent_ink } else { pal.text },
                            );
                            let mut stern = false;
                            if code != "auto" {
                                let stern_rect = egui::Rect::from_center_size(
                                    egui::pos2(rect.right() - 18.0, rect.center().y),
                                    egui::vec2(26.0, 26.0),
                                );
                                let sresp = ui.interact(
                                    stern_rect,
                                    egui::Id::new(("lang_pin", code.as_str())),
                                    egui::Sense::click(),
                                );
                                if sresp.hovered() {
                                    ui.painter().rect_filled(
                                        stern_rect,
                                        Rounding::same(999.0),
                                        pal.soft,
                                    );
                                }
                                crate::icons::paint(
                                    ui.painter(),
                                    stern_rect.shrink(5.0),
                                    "star",
                                    if pinned { pal.accent_ink } else { pal.muted },
                                );
                                if sresp.on_hover_text(t(lang, "pin")).clicked() {
                                    stern = true;
                                }
                            }
                            if stern {
                                toggle_pin = Some(code.clone());
                            } else if resp.clicked() {
                                pick = Some(code.clone());
                            }
                        }
                    });
                if shown == 0 {
                    ui.label(RichText::new(t(lang, "no_hits")).color(pal.muted));
                }
                if let Some(code) = pick {
                    if is_src {
                        self.set.src = code;
                    } else {
                        self.set.tgt = code;
                    }
                    self.save_set();
                    self.popup_lang = None;
                }
                if let Some(code) = toggle_pin {
                    let list = if pins_key == "nllb" {
                        &mut self.set.pins.nllb
                    } else {
                        &mut self.set.pins.ollama
                    };
                    if let Some(pos) = list.iter().position(|c| *c == code) {
                        list.remove(pos);
                    } else {
                        list.push(code);
                    }
                    self.save_set();
                }
            });
        if closed {
            self.popup_lang = None;
            self.lang_anchor = None;
        }
    }

    // ---- Modellauswahl ----------------------------------------------------
    /// Modellauswahl als verankertes Menue unter dem Ausloeser; entspricht
    /// `#pop` sowie `modelPop`/`modelMenu` der Electron-Fassung. Im Editor
    /// stehen Schreib- und Uebersetzungsmodell getrennt.
    fn models_menu(&mut self, ctx: &egui::Context) {
        if !self.show_models {
            return;
        }
        let pal = self.pal;
        let lang = self.lang;
        let width = 300.0;
        let screen = ctx.screen_rect();
        let mut pos = egui::pos2(
            self.models_anchor.right() - width,
            self.models_anchor.bottom() + 6.0,
        );
        pos.x = pos.x.clamp(8.0, (screen.right() - width - 8.0).max(8.0));
        pos.y = pos.y.min((screen.bottom() - 180.0).max(8.0));
        let rows: Vec<(String, bool, bool, bool)> = self
            .models
            .iter()
            .map(|m| (m.name.clone(), m.vision, m.thinking, m.mt))
            .collect();
        let mut pick_model: Option<String> = None;
        let mut pick_write: Option<String> = None;
        let mut pick_trans: Option<String> = None;
        let mut retry = false;
        let area = egui::Area::new(egui::Id::new("models_menu"))
            .order(egui::Order::Foreground)
            .fixed_pos(pos)
            .show(ctx, |ui| {
                egui::Frame::none()
                    .fill(pal.card)
                    .stroke(Stroke::new(1.0_f32, pal.line))
                    .rounding(Rounding::same(16.0))
                    .inner_margin(egui::Margin::same(6.0))
                    .shadow(egui::epaint::Shadow {
                        offset: egui::vec2(0.0, 8.0),
                        blur: 30.0,
                        spread: 0.0,
                        color: Color32::from_black_alpha(70),
                    })
                    .show(ui, |ui| {
                        ui.set_width(width - 12.0);
                        if rows.is_empty() {
                            ui.label(
                                RichText::new(t(lang, "ollama_down"))
                                    .strong()
                                    .color(Color32::from_rgb(0xb9, 0x1c, 0x1c)),
                            );
                            if let Some(err) = &self.models_error {
                                ui.label(RichText::new(err).size(11.0).color(pal.muted));
                            }
                            ui.label(
                                RichText::new(t(lang, "ollama_hint").replace("<url>", &self.set.url))
                                    .size(11.0)
                                    .color(pal.muted),
                            );
                            if ui.button(t(lang, "reload")).clicked() {
                                retry = true;
                            }
                            return;
                        }
                        egui::ScrollArea::vertical()
                            .id_source("models_menu_scroll")
                            .max_height(340.0)
                            .auto_shrink([false, false])
                            .show(ui, |ui| {
                                // Lange Modellnamen duerfen das Menue nicht breiter machen.
                                ui.set_width(width - 24.0);
                                if self.models_for_editor {
                                    ui.label(
                                        RichText::new(t(lang, "write_model"))
                                            .size(12.0)
                                            .color(pal.muted),
                                    );
                                    for (name, vision, thinking, mt) in &rows {
                                        if *mt || is_tg(name) {
                                            continue;
                                        }
                                        let sel = if self.set.ed_write.is_empty() {
                                            self.write_model().as_deref() == Some(name.as_str())
                                        } else {
                                            self.set.ed_write == *name
                                        };
                                        if menu_row(ui, &pal, lang, name, sel, *vision, *thinking, false) {
                                            pick_write = Some(name.clone());
                                        }
                                    }
                                    ui.separator();
                                    ui.label(
                                        RichText::new(t(lang, "trans_model"))
                                            .size(12.0)
                                            .color(pal.muted),
                                    );
                                    for (name, vision, thinking, mt) in &rows {
                                        let sel = self.set.ed_trans == *name;
                                        if menu_row(ui, &pal, lang, name, sel, *vision, *thinking, *mt) {
                                            pick_trans = Some(name.clone());
                                        }
                                    }
                                } else {
                                    for (name, vision, thinking, mt) in &rows {
                                        let sel = self.set.model == *name;
                                        if menu_row(ui, &pal, lang, name, sel, *vision, *thinking, *mt) {
                                            pick_model = Some(name.clone());
                                        }
                                    }
                                }
                            });
                    });
            });
        let anchor = self.models_anchor;
        let mut close = ctx.input(|i| i.key_pressed(egui::Key::Escape));
        // Nur ein Klick ausserhalb von Menue und Ausloeser schliesst. Der Klick,
        // der das Menue geoeffnet hat, liegt im Ausloeser und wuerde es sonst
        // noch im selben Bild wieder zumachen.
        let clicked_out = ctx.input(|i| {
            i.pointer.button_clicked(egui::PointerButton::Primary)
                && i.pointer
                    .interact_pos()
                    .map(|pos| !area.response.rect.contains(pos) && !anchor.contains(pos))
                    .unwrap_or(false)
        });
        if clicked_out {
            close = true;
        }
        if retry {
            self.models_error = None;
            net::spawn_models(self.cfg(), self.tx.clone(), Vec::new());
        }
        if let Some(name) = pick_write {
            self.set.ed_write = name;
            self.save_set();
        }
        if let Some(name) = pick_trans {
            self.set.ed_trans = name;
            self.save_set();
        }
        if let Some(name) = pick_model {
            self.set.pref = name.clone();
            self.set.model = name;
            self.fix_langs();
            self.save_set();
            close = true;
        }
        if close {
            self.show_models = false;
        }
    }

    // ---- Einstellungen ----------------------------------------------------
    fn settings_window(&mut self, ctx: &egui::Context) {
        if !self.show_settings {
            return;
        }
        let pal = self.pal;
        let lang = self.lang;
        let mut open = true;
        let hoehe = (ctx.screen_rect().height() * 0.72).max(320.0);
        dialog(ctx, "settings_win", &t(lang, "settings"), 560.0, &pal, &mut open, |ui| {
                // Feste Hoehe wie `max-height:86vh` der Vorlage; ohne sie zog
                // sich das Fenster auf die ersten Zeilen zusammen.
                egui::ScrollArea::vertical()
                    .max_height(hoehe)
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                    // Die Bildlaufleiste liegt am rechten Rand der Flaeche; ohne
                    // diesen Abstand stiessen die Auswahlfelder an sie.
                    let inhalt = (ui.available_width() - 12.0).max(200.0);
                    ui.set_width(inhalt);
                    ui.set_max_width(inhalt);
                    section(ui, &pal, t(lang, "s_group_general").as_str());
                    set_select(ui, &pal, "uilang", t(lang, "s_uilang").as_str(), &mut self.set.ui_lang, &[
                        ("auto", t(lang, "auto_sys")),
                        ("de", "Deutsch".into()),
                        ("en", "English".into()),
                        ("nl", "Nederlands".into()),
                        ("es", "Español".into()),
                        ("tr", "Türkçe".into()),
                        ("ar", "العربية".into()),
                    ]);
                    set_select(ui, &pal, "theme", t(lang, "s_theme").as_str(), &mut self.set.theme, &[
                        ("auto", t(lang, "auto_sys")),
                        ("light", t(lang, "light")),
                        ("dark", t(lang, "dark")),
                    ]);
                    set_select(ui, &pal, "start", t(lang, "s_start").as_str(), &mut self.set.start, &[
                        ("tr", t(lang, "start_tr")),
                        ("ed", t(lang, "start_ed")),
                        ("last", t(lang, "start_last")),
                    ]);
                    set_select(ui, &pal, "workspace", t(lang, "s_workspace").as_str(), &mut self.set.workspace, &[
                        ("both", t(lang, "ws_both")),
                        ("tr", t(lang, "ws_tr")),
                        ("ed", t(lang, "ws_ed")),
                    ]);

                    section(ui, &pal, t(lang, "s_group_extra").as_str());
                    set_bool(ui, &pal, "boot_model", lang, t(lang, "s_boot").as_str(), &mut self.set.boot_model);
                    set_bool(
                        ui,
                        &pal,
                        "profanity_filter",
                        lang,
                        t(lang, "s_prof").as_str(),
                        &mut self.set.profanity_filter,
                    );
                    let nllb_vorher = self.set.nllb_autostart;
                    set_bool(
                        ui,
                        &pal,
                        "nllb_autostart",
                        lang,
                        t(lang, "nllb_auto").as_str(),
                        &mut self.set.nllb_autostart,
                    );
                    if nllb_vorher != self.set.nllb_autostart {
                        self.save_set();
                        if self.set.nllb_autostart {
                            if self.nllb_child.is_none() {
                                self.nllb_child = crate::nllb::start();
                            }
                        } else {
                            crate::nllb::stop(&mut self.nllb_child);
                        }
                    }
                    set_select(ui, &pal, "profstyle", t(lang, "s_profstyle").as_str(), &mut self.set.profanity_style, &[
                        ("stars", t(lang, "o_stars")),
                        ("symbols", t(lang, "o_symbols")),
                    ]);
                    set_bool(ui, &pal, "ed_markdown", lang, t(lang, "s_md").as_str(), &mut self.set.ed_markdown);
                    set_bool(
                        ui,
                        &pal,
                        "ed_spell_anim",
                        lang,
                        t(lang, "s_spellanim").as_str(),
                        &mut self.set.ed_spell_anim,
                    );
                    set_bool(ui, &pal, "auto_copy", lang, t(lang, "s_autocopy").as_str(), &mut self.set.auto_copy);
                    set_bool(
                        ui,
                        &pal,
                        "auto_copy_typed",
                        lang,
                        t(lang, "s_autocopy_t").as_str(),
                        &mut self.set.auto_copy_typed,
                    );

                    section(ui, &pal, t(lang, "s_group_conn").as_str());
                    set_field(ui, &pal, t(lang, "s_url").as_str(), |ui| {
                        ui.add(
                            egui::TextEdit::singleline(&mut self.set.url)
                                .frame(false)
                                .desired_width(f32::INFINITY),
                        );
                    });
                    set_field(ui, &pal, t(lang, "s_nllb").as_str(), |ui| {
                        ui.add(
                            egui::TextEdit::singleline(&mut self.set.nllb)
                                .frame(false)
                                .desired_width(f32::INFINITY),
                        );
                    });

                    section(ui, &pal, t(lang, "s_group_trans").as_str());
                    // Wie `helper` in `renderer.js`: eine Auswahlliste aus den
                    // allgemeinen Modellen, dazu „Automatisch“.
                    let helper_namen: Vec<String> = self
                        .models
                        .iter()
                        .filter(|m| !m.mt && !is_tg(&m.name))
                        .map(|m| m.name.clone())
                        .collect();
                    let mut helper_opts: Vec<(String, String)> =
                        vec![(String::new(), t(lang, "auto_model"))];
                    for name in helper_namen {
                        helper_opts.push((name.clone(), name));
                    }
                    let helper_vorher = self.set.helper.clone();
                    set_select(
                        ui,
                        &pal,
                        "helper",
                        t(lang, "s_helper").as_str(),
                        &mut self.set.helper,
                        &helper_opts,
                    );
                    if helper_vorher != self.set.helper {
                        self.save_set();
                    }
                    set_field(ui, &pal, t(lang, "s_sys").as_str(), |ui| {
                        ui.add(
                            egui::TextEdit::multiline(&mut self.set.sys)
                                .frame(false)
                                .desired_width(f32::INFINITY)
                                .desired_rows(5),
                        );
                    });
                    set_number(ui, &pal, t(lang, "s_temp").as_str(), |ui| {
                        ui.add(
                            egui::DragValue::new(&mut self.set.temp)
                                .speed(0.05)
                                .clamp_range(0.0..=2.0)
                                .fixed_decimals(2),
                        );
                    });
                    set_number(ui, &pal, t(lang, "s_ctx").as_str(), |ui| {
                        ui.add(
                            egui::DragValue::new(&mut self.set.numctx)
                                .speed(64.0)
                                .clamp_range(256..=32768),
                        );
                    });

                    section(ui, &pal, t(lang, "nllb_h").as_str());
                    if self.mt_models.is_empty() {
                        ui.label(RichText::new(t(lang, "server_down")).size(12.0).color(pal.muted));
                    }
                    let mut action: Option<(String, &'static str)> = None;
                    for m in &self.mt_models {
                        let id = m.get("id").and_then(|v| v.as_str()).unwrap_or("");
                        let installed = m.get("installed").and_then(|v| v.as_bool()).unwrap_or(false);
                        let gb = m.get("gb").and_then(|v| v.as_f64()).unwrap_or(0.0);
                        ui.horizontal(|ui| {
                            ui.label(format!("{} · {:.1} GB", id, gb));
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if installed {
                                    if ui.button(t(lang, "delete")).clicked() {
                                        action = Some((format!("nllb:{}", id), "delete"));
                                    }
                                } else if ui.button(t(lang, "download")).clicked() {
                                    action = Some((format!("nllb:{}", id), "install"));
                                }
                            });
                        });
                    }
                    if let Some((id, act)) = action {
                        let mid = id.split(':').nth(1).unwrap_or("").to_string();
                        if act == "install" {
                            net::spawn_model_action(self.cfg(), mid, "install", self.tx.clone());
                        } else {
                            self.confirm = Some((id.clone(), Confirm::ModelDelete(id)));
                        }
                    }

                    ui.add_space(10.0);
                    ui.horizontal(|ui| {
                        if ui.button(t(lang, "reset_all")).clicked() {
                            let doc = self.set.doc.clone();
                            self.set = Settings::default();
                            self.set.doc = doc;
                            self.save_set();
                            self.lang = Lang::resolve(&self.set.ui_lang);
                        }
                        if ui.button(t(lang, "close")).clicked() {
                            self.save_set();
                            self.show_settings = false;
                        }
                    });
                });
            });
        self.save_set();
        if !open {
            self.show_settings = false;
        }
    }

    // ---- Persoenliches Woerterbuch ---------------------------------------
    /// Persoenliches Woerterbuch wie `#dictDlg` in `editor.js`: Kopf mit Export
    /// und Import, darueber die Eingabe mit dem Hauptknopf, dann die Liste in
    /// einem gerahmten Kasten und unten rechts „Schliessen“.
    fn dict_window(&mut self, ctx: &egui::Context) {
        if !self.show_dict {
            return;
        }
        let pal = self.pal;
        let lang = self.lang;
        let mut open = true;
        let mut changed = false;
        let mut export = false;
        let mut import = false;
        let mut entfernen: Option<usize> = None;
        let mut neues: Option<String> = None;
        let mut schliessen = false;
        let mut woerter = std::mem::take(&mut self.dict_words);
        let mut bearbeiten = self.rename;
        dialog(
            ctx,
            "dict_win",
            &t(lang, "personal_dict"),
            480.0,
            &pal,
            &mut open,
            |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(t(lang, "dict_words"))
                            .size(12.0)
                            .color(pal.muted),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if icon_btn(ui, &pal, "upload", false)
                            .on_hover_text(t(lang, "dict_export"))
                            .clicked()
                        {
                            export = true;
                        }
                        if icon_btn(ui, &pal, "download", false)
                            .on_hover_text(t(lang, "dict_import"))
                            .clicked()
                        {
                            import = true;
                        }
                    });
                });
                ui.add_space(6.0);
                // `.dict-add`: Eingabe links, Hauptknopf rechts, ueber der Liste.
                ui.horizontal(|ui| {
                    let breite = (ui.available_width() - 150.0).max(140.0);
                    ui.add_sized(
                        egui::vec2(breite, 32.0),
                        egui::TextEdit::singleline(&mut self.dict_new)
                            .hint_text(RichText::new(t(lang, "word_ph")).color(pal.muted))
                            .margin(egui::vec2(10.0, 7.0)),
                    );
                    let knopf = egui::Button::new(
                        RichText::new(t(lang, "add_word"))
                            .size(13.0)
                            .color(pal.on_accent),
                    )
                    .fill(pal.accent)
                    .stroke(Stroke::NONE)
                    .rounding(Rounding::same(999.0))
                    .min_size(egui::vec2(0.0, 32.0));
                    if ui.add(knopf).clicked() && !self.dict_new.trim().is_empty() {
                        neues = Some(self.dict_new.trim().to_string());
                    }
                });
                ui.add_space(8.0);
                // `.dict-list`: gerahmter Kasten mit den Woertern.
                egui::Frame::none()
                    .stroke(Stroke::new(1.0_f32, pal.line))
                    .rounding(Rounding::same(12.0))
                    .inner_margin(egui::Margin::same(4.0))
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        if woerter.is_empty() {
                            ui.add_space(10.0);
                            ui.vertical_centered(|ui| {
                                ui.label(
                                    RichText::new(t(lang, "no_dict_words"))
                                        .size(12.0)
                                        .color(pal.muted),
                                );
                            });
                            ui.add_space(10.0);
                        } else {
                            egui::ScrollArea::vertical()
                                .id_source("dict_list")
                                .max_height(280.0)
                                .show(ui, |ui| {
                                    for (i, w) in woerter.iter_mut().enumerate() {
                                        ui.horizontal(|ui| {
                                            if bearbeiten == Some(i) {
                                                let breite =
                                                    (ui.available_width() - 34.0).max(80.0);
                                                ui.add_sized(
                                                    egui::vec2(breite, 26.0),
                                                    egui::TextEdit::singleline(w)
                                                        .margin(egui::vec2(8.0, 4.0)),
                                                );
                                                if icon_btn(ui, &pal, "check", false)
                                                    .on_hover_text(t(lang, "save"))
                                                    .clicked()
                                                {
                                                    bearbeiten = None;
                                                    changed = true;
                                                }
                                            } else {
                                                let zeile = ui.add(
                                                    egui::Label::new(
                                                        RichText::new(w.clone()).size(14.0),
                                                    )
                                                    .sense(egui::Sense::click()),
                                                );
                                                if zeile.clicked() {
                                                    bearbeiten = Some(i);
                                                }
                                                ui.with_layout(
                                                    egui::Layout::right_to_left(
                                                        egui::Align::Center,
                                                    ),
                                                    |ui| {
                                                        if icon_btn(ui, &pal, "x", false)
                                                            .on_hover_text(t(lang, "del"))
                                                            .clicked()
                                                        {
                                                            entfernen = Some(i);
                                                        }
                                                    },
                                                );
                                            }
                                        });
                                    }
                                });
                        }
                    });
                ui.add_space(10.0);
                // Fuss wie im Vorbild: der Schliessen-Knopf rechts.
                ui.horizontal(|ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let knopf = egui::Button::new(
                            RichText::new(t(lang, "close")).size(13.0),
                        )
                        .fill(pal.soft)
                        .stroke(Stroke::NONE)
                        .rounding(Rounding::same(999.0))
                        .min_size(egui::vec2(0.0, 30.0));
                        if ui.add(knopf).clicked() {
                            schliessen = true;
                        }
                    });
                });
            },
        );
        self.dict_words = woerter;
        self.rename = bearbeiten;
        if schliessen {
            open = false;
        }
        if let Some(i) = entfernen {
            if i < self.dict_words.len() {
                self.dict_words.remove(i);
                self.spell_scanned_doc.clear();
                changed = true;
            }
        }
        if let Some(word) = neues {
            if self
                .dict_words
                .iter()
                .any(|w| w.to_lowercase() == word.to_lowercase())
            {
                self.status = t(lang, "word_exists");
            } else {
                self.dict_words.push(word);
                self.dict_new.clear();
                self.spell_scanned_doc.clear();
                changed = true;
            }
        }
        if export {
            if let Some(path) = rfd::FileDialog::new()
                .set_file_name("woerterbuch.txt")
                .save_file()
            {
                let _ = std::fs::write(path, self.dict_words.join("\n"));
                self.status = t(lang, "saved");
            }
        }
        if import {
            if let Some(path) = rfd::FileDialog::new().pick_file() {
                if let Ok(text) = std::fs::read_to_string(&path) {
                    for wort in text.lines().map(str::trim).filter(|w| !w.is_empty()) {
                        if !self
                            .dict_words
                            .iter()
                            .any(|w| w.to_lowercase() == wort.to_lowercase())
                        {
                            self.dict_words.push(wort.to_string());
                            changed = true;
                        }
                    }
                    self.spell_scanned_doc.clear();
                    self.status = t(lang, "dict_loaded");
                }
            }
        }
        if changed {
            let path = crate::settings::data_dir().join("personal-dictionary.json");
            if let Ok(text) = serde_json::to_string_pretty(&self.dict_words) {
                let _ = std::fs::write(path, text);
            }
        }
        if !open {
            self.show_dict = false;
        }
    }

    fn style_window(&mut self, ctx: &egui::Context) {
        if !self.show_style {
            return;
        }
        let pal = self.pal;
        let mut open = true;
        let title = t(self.lang, "style");
        dialog(ctx, "style_win", &title, 320.0, &pal, &mut open, |ui| {
            ui.label(RichText::new(title.clone()).color(pal.muted));
        });
        if !open {
            self.show_style = false;
        }
    }

    /// Synonyme zur Auswahl; entspricht dem `#alt`-Menue unter der
    /// Kontextleiste in `editor.js` (Zeilen zum Anklicken, keine Eingabe).
    fn syn_window(&mut self, ctx: &egui::Context) {
        if !self.show_syn {
            return;
        }
        let lang = self.lang;
        let pal = self.pal;
        let anchor = self
            .ctx_bar_rect
            .unwrap_or_else(|| egui::Rect::from_center_size(ctx.screen_rect().center(), egui::vec2(1.0, 1.0)));
        let mut pick: Option<String> = None;
        let closed = pop_at(ctx, "syn_pop", anchor, 280.0, &pal, |ui| {
            if self.syn_job {
                ui.horizontal(|ui| {
                    ladekreis(ui, &pal);
                    ui.label(RichText::new(t(lang, "synonyms")).size(12.5).color(pal.muted));
                });
            } else if self.syn_out.is_empty() {
                ui.label(
                    RichText::new(t(lang, "no_alts"))
                        .size(12.5)
                        .color(pal.muted),
                );
            } else {
                for word in self.syn_out.clone() {
                    let row = egui::Button::new(RichText::new(&word).size(13.5).color(pal.text))
                        .fill(egui::Color32::TRANSPARENT)
                        .stroke(Stroke::NONE)
                        .rounding(Rounding::same(10.0))
                        .min_size(egui::vec2(ui.available_width(), 28.0));
                    if ui.add(row).clicked() {
                        pick = Some(word.clone());
                    }
                }
            }
        });
        if let Some(word) = pick {
            if let Some((a, b)) = self.syn_sel.take() {
                let doc = self.doc.clone();
                if b > a {
                    self.push_undo();
                    let ba = byte_idx(&doc, a);
                    let bb = byte_idx(&doc, b);
                    self.doc = format!("{}{}{}", &doc[..ba], word, &doc[bb..]);
                    self.saved = false;
                    self.set.doc = self.doc.clone();
                    self.save_set();
                }
            }
            self.show_syn = false;
            return;
        }
        if closed {
            self.show_syn = false;
        }
    }

    fn request_synonyms(&mut self) {
        let Some(model) = self.write_model() else {
            self.status = t(self.lang, "no_model");
            return;
        };
        let word = self.syn_word.trim().to_string();
        if word.is_empty() {
            return;
        }
        self.syn_out.clear();
        self.syn_pending.clear();
        self.syn_job = true;
        let prompt = format!(
            "Give up to 5 {} synonyms that could replace the word \"{}\" in a sentence. Same part of speech and grammatical form. No translations, no explanations, no duplicates. Output JSON: {{\"synonyms\":[\"...\"]}}",
            lang_label(&self.set.dict_lang, &self.nllb_langs),
            word.replace('"', "'")
        );
        net::spawn_chat(
            self.cfg(),
            model,
            vec![
                obj("system", "You are a thesaurus. Output only valid JSON."),
                obj("user", &prompt),
            ],
            Some(0.3),
            Some("json"),
            false,
            self.tx.clone(),
            self.stop.clone(),
        );
    }

    fn confirm_window(&mut self, ctx: &egui::Context) {
        let Some((text, action)) = self.confirm.clone() else { return };
        let pal = self.pal;
        let lang = self.lang;
        let mut open = true;
        let mut yes = false;
        dialog(ctx, "confirm_win", "", 360.0, &pal, &mut open, |ui| {
                ui.label(RichText::new(&text).color(pal.text));
                ui.horizontal(|ui| {
                    if ui.button(t(lang, "cancel")).clicked() {
                        self.confirm = None;
                    }
                    if ui
                        .add(
                            egui::Button::new(RichText::new(t(lang, "ok")).color(pal.on_accent))
                                .fill(pal.accent)
                                .rounding(Rounding::same(999.0)),
                        )
                        .clicked()
                    {
                        yes = true;
                    }
                });
            });
        if yes {
            match action {
                Confirm::LibDelete(id) => {
                    library::delete(&id);
                    self.lib = library::list();
                }
                Confirm::ModelDelete(id) => {
                    let mid = id.split(':').nth(1).unwrap_or("").to_string();
                    net::spawn_model_action(self.cfg(), mid, "delete", self.tx.clone());
                }
                Confirm::SessionClose(idx) => {
                    self.close_session(idx);
                }
            }
            self.confirm = None;
        }
        if !open {
            self.confirm = None;
        }
    }

    // ---- Editor -----------------------------------------------------------
    fn editor(&mut self, ui: &mut egui::Ui) {
        let pal = self.pal;
        let lang = self.lang;
        self.handle_shortcuts(ui.ctx());

        ui.horizontal_wrapped(|ui| {
            if icon_btn(ui, &pal, "filenew", false)
                .on_hover_text(t(lang, "new_doc"))
                .clicked()
            {
                self.new_document();
            }
            if icon_btn(ui, &pal, "folder", false)
                .on_hover_text(t(lang, "open"))
                .clicked()
            {
                self.import_document();
            }
            // Wie `savePop` in `editor.js`: ein Menue statt direktem Schreiben.
            let save_resp = icon_btn(ui, &pal, "save", false).on_hover_text(t(lang, "save"));
            if save_resp.clicked() {
                self.save_anchor = Some(save_resp.rect);
                self.show_save_menu = !self.show_save_menu;
            }
            ui.separator();
            if icon_btn(ui, &pal, "undo", false)
                .on_hover_text(t(lang, "undo"))
                .clicked()
            {
                self.undo();
            }
            if icon_btn(ui, &pal, "redo", false)
                .on_hover_text(t(lang, "redo"))
                .clicked()
            {
                self.redo();
            }
            ui.separator();
            // Ueberschrift wie die Auswahl `.tsel` in der Vorlage: eine runde
            // Pille auf weichem Grund, mit den vollen Namen statt „H1“.
            let level = editor::heading_of(&self.doc, self.selection().0);
            let mut new_level: Option<&str> = None;
            let head_label = match level {
                1 => t(lang, "h1"),
                2 => t(lang, "h2"),
                3 => t(lang, "h3"),
                _ => t(lang, "heading"),
            };
            ui.scope(|ui| {
                let st = ui.style_mut();
                st.spacing.button_padding = egui::vec2(10.0, 6.0);
                for w in [
                    &mut st.visuals.widgets.inactive,
                    &mut st.visuals.widgets.hovered,
                    &mut st.visuals.widgets.active,
                    &mut st.visuals.widgets.open,
                ] {
                    w.bg_fill = pal.soft;
                    w.weak_bg_fill = pal.soft;
                    w.bg_stroke = Stroke::NONE;
                    w.rounding = Rounding::same(999.0);
                }
                egui::ComboBox::from_id_source("ed_head")
                    .selected_text(RichText::new(&head_label).size(13.0).color(pal.text))
                    .width(150.0)
                    .show_ui(ui, |ui| {
                        if ui
                            .selectable_label(
                                level == 0,
                                RichText::new(t(lang, "heading")).size(13.0),
                            )
                            .clicked()
                        {
                            new_level = Some("");
                        }
                        for (lvl, key, prefix) in
                            [(1usize, "h1", "# "), (2, "h2", "## "), (3, "h3", "### ")]
                        {
                            if ui
                                .selectable_label(level == lvl, RichText::new(t(lang, key)).size(13.0))
                                .clicked()
                            {
                                new_level = Some(prefix);
                            }
                        }
                    });
            });
            if let Some(prefix) = new_level {
                self.set_heading(prefix);
            }
            ui.separator();
            if icon_btn(ui, &pal, "bold", self.selection_wrapped("**", "**"))
                .on_hover_text(t(lang, "bold"))
                .clicked()
            {
                self.format_wrap("**", "**");
            }
            if icon_btn(ui, &pal, "italic", self.selection_wrapped("*", "*"))
                .on_hover_text(t(lang, "italic"))
                .clicked()
            {
                self.format_wrap("*", "*");
            }
            if icon_btn(ui, &pal, "underline", self.selection_wrapped("<u>", "</u>"))
                .on_hover_text(t(lang, "underline"))
                .clicked()
            {
                self.format_wrap("<u>", "</u>");
            }
            if icon_btn(ui, &pal, "strike", self.selection_wrapped("~~", "~~"))
                .on_hover_text(t(lang, "strike"))
                .clicked()
            {
                self.format_wrap("~~", "~~");
            }
            if icon_btn(ui, &pal, "ul", false)
                .on_hover_text(t(lang, "list"))
                .clicked()
            {
                self.format_line("- ");
            }
            if icon_btn(ui, &pal, "ol", false)
                .on_hover_text(t(lang, "list"))
                .clicked()
            {
                self.format_line("1. ");
            }
            if icon_btn(ui, &pal, "quote", false)
                .on_hover_text(t(lang, "quote"))
                .clicked()
            {
                self.format_line("> ");
            }
            ui.separator();
            // Ausrichtungen wie `alignL` bis `alignJ` in der Vorlage.
            for (symbol, ausrichtung, hinweis) in [
                ("alignL", "left", "align_l"),
                ("alignC", "center", "align_c"),
                ("alignR", "right", "align_r"),
                ("alignJ", "justify", "align_j"),
            ] {
                if icon_btn(ui, &pal, symbol, false)
                    .on_hover_text(t(lang, hinweis))
                    .clicked()
                {
                    self.format_wrap(
                        &format!("<div align=\"{}\">", ausrichtung),
                        "</div>",
                    );
                }
            }
            ui.separator();
            if icon_btn(ui, &pal, "link", false)
                .on_hover_text(t(lang, "link"))
                .clicked()
            {
                self.format_link();
            }
            if icon_btn(ui, &pal, "eraser", false)
                .on_hover_text(t(lang, "clear_fmt"))
                .clicked()
            {
                self.clear_formatting();
            }
            ui.separator();
            // Wie in der Vorlage wirken diese drei auf das ganze Dokument;
            // die Auswahl-Aktionen sitzen in der schwebenden Leiste.
            if icon_btn(ui, &pal, "shorten", false)
                .on_hover_text(t(lang, "shorten"))
                .clicked()
            {
                self.start_editor_all("shorten");
            }
            if icon_btn(ui, &pal, "lengthen", false)
                .on_hover_text(t(lang, "lengthen"))
                .clicked()
            {
                self.start_editor_all("lengthen");
            }
            if icon_btn(ui, &pal, "summarize", false)
                .on_hover_text(t(lang, "summarize"))
                .clicked()
            {
                self.start_editor_all("summarize");
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let resp = icon_btn(
                    ui,
                    &pal,
                    "cpu",
                    self.show_models && self.models_for_editor && !self.models.is_empty(),
                )
                .on_hover_text(t(lang, "model_menu"));
                if resp.clicked() {
                    self.models_anchor = resp.rect;
                    self.models_for_editor = true;
                    self.show_models = !self.show_models;
                    if self.models.is_empty() {
                        self.models_error = None;
                        net::spawn_models(self.cfg(), self.tx.clone(), Vec::new());
                    }
                }
            });
        });
        ui.add_space(6.0);

        // Schreibflaeche wie `#pageWrap` und `#page` in `style.css`: weiche
        // Flaeche mit 20er Rundung, darin eine mittige Seite von 794 px Breite,
        // mindestens 1040 px hoch, mit 64/72 px Innenstand.
        let reserved = if self.show_sessions { 150.0 } else { 74.0 };
        let h = (ui.available_height() - reserved).max(160.0);
        // `#pageWrap` in `style.css` hat rundum 28 px Polster (seitlich 16):
        // unter der Seite bleibt derselbe Abstand bis zur Kante der Flaeche,
        // wie oben ueber ihr.
        egui::Frame::none()
            .fill(pal.soft)
            .rounding(Rounding::same(20.0))
            .inner_margin(egui::Margin {
                left: 16.0,
                right: 16.0,
                top: 28.0,
                bottom: 28.0,
            })
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.set_height((h - 36.0).max(80.0));
                egui::ScrollArea::vertical()
                    .id_source("page_scroll")
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        let flaeche = ui.available_rect_before_wrap();
                        let page_w = 794.0_f32.min(flaeche.width());
                        // Die Seite ist immer mindestens so hoch wie die Flaeche
                        // selbst, damit sie erst an deren Kante abgeschnitten wird
                        // und nicht schon vorher aufhoert.
                        // Die Seite reicht bis an die Kante der Karte unter ihr.
                        let page_h = flaeche.height().max(1040.0);
                        let (zeile, _) = ui.allocate_exact_size(
                            egui::vec2(flaeche.width(), page_h),
                            egui::Sense::hover(),
                        );
                        // Mittig auf der Flaeche, nicht am linken Rand.
                        let page_rect = egui::Rect::from_min_size(
                            egui::pos2(
                                zeile.left() + (zeile.width() - page_w) / 2.0,
                                zeile.top(),
                            ),
                            egui::vec2(page_w, page_h),
                        );
                        let painter = ui.painter();
                        painter.rect_filled(
                            page_rect.translate(egui::vec2(0.0, 2.0)),
                            22.0,
                            egui::Color32::from_black_alpha(24),
                        );
                        painter.rect_filled(page_rect, 22.0, pal.card);
                        let inner = page_rect.shrink2(egui::vec2(72.0, 64.0));
                        ui.allocate_ui_at_rect(inner, |ui| {
                        let before = self.doc.clone();
                        let out = egui::TextEdit::multiline(&mut self.doc)
                            .frame(false)
                            .desired_width(f32::INFINITY)
                            .desired_rows(24)
                            .hint_text(RichText::new("…").color(pal.muted))
                            .show(ui);
                        if let Some(range) = out.cursor_range {
                            let a = range.primary.ccursor.index;
                            let b = range.secondary.ccursor.index;
                            self.sel = Some((a.min(b), a.max(b)));
                            if a != b {
                                let r1 = out.galley.pos_from_cursor(&range.primary);
                                let r2 = out.galley.pos_from_cursor(&range.secondary);
                                self.bar_rect =
                                    Some(r1.union(r2).translate(out.galley_pos.to_vec2()));
                            } else if !self.bar_hover {
                                self.bar_rect = None;
                            }
                        } else if !self.bar_hover {
                            self.bar_rect = None;
                        }
                        if self.set.ed_spell && !self.spell_hits.is_empty() {
                            self.paint_spell_marks(ui, &out);
                            if ui
                                .input(|i| i.pointer.button_clicked(egui::PointerButton::Secondary))
                            {
                                if let Some(pos) = ui.input(|i| i.pointer.interact_pos()) {
                                    let hit = self
                                        .spell_mark_rects
                                        .iter()
                                        .find(|(_, r)| r.contains(pos))
                                        .map(|(w, _)| w.clone());
                                    if let Some(word) = hit {
                                        // Vorschlaege im Hintergrund holen;
                                        // fuer alle Woerter eines Textes
                                        // waeren sie viel zu teuer.
                                        self.spell_menu_sug.clear();
                                        self.spell_menu_wait = true;
                                        let lang_code = self.set.dict_lang.clone();
                                        let worker = self.spell_worker.get_or_insert_with(
                                            crate::spell::SpellWorker::new,
                                        );
                                        worker.suggest(&lang_code, &word);
                                        self.spell_menu = Some(SpellMenu { word, pos });
                                    }
                                }
                            }
                        } else {
                            self.spell_anchor = None;
                            self.spell_mark_rects.clear();
                        }
                        if self.editor_job {
                            self.paint_processing(ui, &out);
                        }
                        if out.response.changed() {
                            let now = ui.input(|i| i.time);
                            if now - self.last_undo > 1.0 {
                                self.undo.push(Snapshot {
                                    text: before,
                                    sel: self.sel,
                                    at: now,
                                });
                                if self.undo.len() > 200 {
                                    self.undo.remove(0);
                                }
                                self.redo.clear();
                                self.last_undo = now;
                            }
                            self.saved = false;
                            self.set.doc = self.doc.clone();
                            self.save_set();
                            self.sync_active_session();
                        }
                        });
                    });
            });
        self.context_bar(ui.ctx());

        ui.add_space(6.0);

        egui::Frame::none()
            .fill(pal.card)
            .stroke(Stroke::new(1.0_f32, pal.line))
            .rounding(Rounding::same(20.0))
            .inner_margin(egui::Margin {
                left: 14.0,
                right: 14.0,
                top: 4.0,
                bottom: 4.0,
            })
            .show(ui, |ui| {
        ui.set_width(ui.available_width());
        // Feste Zeilenhoehe, damit Wortzahl und Meldung mittig stehen und nicht
        // am oberen Rand kleben (egui richtet das erste Element sonst oben aus).
        ui.allocate_ui_with_layout(
            egui::vec2(ui.available_width(), 32.0),
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| {
            let words = self.doc.split_whitespace().count();
            ui.label(
                RichText::new(format!("{} {}", words, t(lang, "words")))
                    .size(12.0)
                    .color(pal.muted),
            );
            ui.label(RichText::new(&self.status).size(12.0).color(pal.muted));
            if !self.saved {
                ui.label(RichText::new("•").size(12.0).color(pal.accent_ink));
            }
            // Reihenfolge wie in der Vorlage (`#sbDocs`, `#sbDictWords`, `#sbDict`):
            // rechts haengt die Sprachpille, davor Buch und Dokumente.
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let dict_resp = ui
                    .add(
                        egui::Button::new(
                            RichText::new(format!(
                                "{} ▾",
                                lang_label(&self.set.dict_lang, &self.nllb_langs)
                            ))
                            .size(12.0)
                            .color(pal.muted),
                        )
                        .fill(Color32::TRANSPARENT),
                    )
                    .on_hover_text(t(lang, "dict"));
                if dict_resp.clicked() {
                    self.dict_anchor = Some(dict_resp.rect);
                    self.show_spell_lang = !self.show_spell_lang;
                }
                if icon_btn(ui, &pal, "book", false)
                    .on_hover_text(t(lang, "personal_dict"))
                    .clicked()
                {
                    self.show_dict = true;
                }
                if icon_btn(ui, &pal, "docs", self.show_sessions)
                    .on_hover_text(t(lang, "session_docs"))
                    .clicked()
                {
                    self.show_sessions = !self.show_sessions;
                }
            });
        });
        });
        ui.add_space(12.0);

        self.schedule_spell(ui.ctx());

        if self.show_sessions {
            self.session_dock(ui);
        }
    }

    // ---- Sitzungsliste ----------------------------------------------------
    fn session_dock(&mut self, ui: &mut egui::Ui) {
        let pal = self.pal;
        let lang = self.lang;
        self.sync_active_session();
        let mut switch: Option<usize> = None;
        let mut close: Option<usize> = None;

        // Wie `#docDock` in `style.css`: eine breite Flaeche unten (150 px),
        // davor ein Streifen mit den Karten, die oben darueber hinausragen.
        let breite = ui.available_width();
        let (outer, _) = ui.allocate_exact_size(egui::vec2(breite, 150.0), egui::Sense::hover());
        let strip_w = (breite - 64.0).min(1500.0).max(320.0);
        let panel = egui::Rect::from_center_size(
            egui::pos2(outer.center().x, outer.bottom()),
            egui::vec2(strip_w, 150.0),
        );
        let maler = ui.painter();
        maler.rect_filled(
            panel.translate(egui::vec2(0.0, 6.0)).expand(2.0),
            24.0,
            egui::Color32::from_black_alpha(14),
        );
        maler.rect(panel, 24.0, pal.soft, Stroke::new(1.0_f32, pal.line));
        let strip = egui::Rect::from_min_max(
            egui::pos2(panel.left() + 60.0, panel.bottom() - 18.0 - 262.0),
            egui::pos2(panel.right() - 60.0, panel.bottom() - 18.0),
        );

        let ctx = ui.ctx().clone();
        let mut rects: Vec<egui::Rect> = Vec::new();
        let mut hover_close: Option<usize> = None;
        egui::Area::new(egui::Id::new("session_dock"))
            .order(egui::Order::Foreground)
            .fixed_pos(strip.min)
            .show(&ctx, |ui| {
                ui.set_width(strip.width());
                egui::ScrollArea::horizontal()
                    .id_source("session_dock_scroll")
                    .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
                    .max_height(strip.height())
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 14.0;
                            for i in 0..self.sessions.len() {
                                let aktiv = i == self.active_session;
                                let name = self.sessions[i].name.clone();
                                let text = editor::plain_text(&self.sessions[i].text);
                                // Karte: 118 x 136, aktiv 154 x 186.
                                let (karte_w, karte_h) =
                                    if aktiv { (154.0, 186.0) } else { (118.0, 136.0) };
                                let (zelle, _) = ui.allocate_exact_size(
                                    egui::vec2(karte_w, strip.height()),
                                    egui::Sense::hover(),
                                );
                                let karte = egui::Rect::from_min_max(
                                    egui::pos2(zelle.left(), zelle.bottom() - karte_h),
                                    egui::pos2(zelle.left() + karte_w, zelle.bottom()),
                                );
                                // Nur die Karte selbst ist anklickbar, nicht der
                                // leere Platz darueber.
                                let resp = ui.interact(
                                    karte,
                                    egui::Id::new(("dock_card", i)),
                                    egui::Sense::click_and_drag(),
                                );
                                rects.push(karte);
                                let mp = ui.painter();
                                for schritt in 0..3 {
                                    mp.rect_filled(
                                        karte.translate(egui::vec2(0.0, 2.0 + schritt as f32 * 3.0))
                                            .expand(schritt as f32 * 2.0),
                                        16.0,
                                        egui::Color32::from_black_alpha(10),
                                    );
                                }
                                let rand = if aktiv || resp.hovered() {
                                    pal.accent
                                } else {
                                    pal.line
                                };
                                mp.rect(karte, 16.0, pal.card, Stroke::new(1.0_f32, rand));
                                // Vorschau wie `.dockprev`: eigenes Feld mit feiner
                                // Linie, darin der Dokumenttext klein.
                                let rand_ab = if aktiv { 10.0 } else { 8.0 };
                                let name_h = if aktiv { 30.0 } else { 27.0 };
                                let vor = egui::Rect::from_min_max(
                                    egui::pos2(karte.left() + rand_ab, karte.top() + rand_ab),
                                    egui::pos2(karte.right() - rand_ab, karte.bottom() - name_h),
                                );
                                mp.rect(vor, 10.0, pal.card, Stroke::new(1.0_f32, pal.line));
                                preview_text_at(
                                    mp,
                                    vor.shrink(if aktiv { 10.0 } else { 8.0 }),
                                    &text,
                                    &pal,
                                    if aktiv { 8.5 } else { 6.5 },
                                );
                                // Name unten, mittig.
                                let name_rect = egui::Rect::from_min_max(
                                    egui::pos2(karte.left() + 8.0, karte.bottom() - name_h),
                                    egui::pos2(karte.right() - 8.0, karte.bottom() - 4.0),
                                );
                                let farbe = if aktiv { pal.text } else { pal.muted };
                                let galley = mp.layout(
                                    name.clone(),
                                    egui::FontId::proportional(if aktiv { 11.0 } else { 10.0 }),
                                    farbe,
                                    name_rect.width(),
                                );
                                mp.galley(
                                    egui::pos2(
                                        name_rect.center().x - galley.size().x / 2.0,
                                        name_rect.center().y - galley.size().y / 2.0,
                                    ),
                                    galley,
                                    farbe,
                                );
                                // Kreuz nur beim Ueberfahren, wie `.dockcard .rm`.
                                if resp.hovered() {
                                    let rm = egui::Rect::from_center_size(
                                        egui::pos2(karte.right() - 16.0, karte.top() + 16.0),
                                        egui::vec2(20.0, 20.0),
                                    );
                                    let rresp = ui.interact(
                                        rm,
                                        egui::Id::new(("dock_close", i)),
                                        egui::Sense::click(),
                                    );
                                    ui.painter().circle_filled(rm.center(), 10.0, pal.text);
                                    crate::icons::paint(ui.painter(), rm.shrink(6.0), "x", pal.bg);
                                    if rresp.clicked() {
                                        hover_close = Some(i);
                                    }
                                }
                                if resp.clicked() && hover_close != Some(i) {
                                    switch = Some(i);
                                }
                            }
                        });
                    });
            });

        // Karten ziehen wie `dockDragStart/Move/End` der Vorlage.
        let zeiger = ui.ctx().input(|i| i.pointer.interact_pos());
        let gedrueckt = ui.ctx().input(|i| i.pointer.primary_pressed());
        let losgelassen = ui.ctx().input(|i| i.pointer.primary_released());
        if gedrueckt {
            self.dock_drag = zeiger.and_then(|pos| rects.iter().position(|r| r.contains(pos)));
        }
        if losgelassen {
            if let (Some(von), Some(pos)) = (self.dock_drag.take(), zeiger) {
                if let Some(nach) = rects.iter().position(|r| r.contains(pos)) {
                    if nach != von && nach < self.sessions.len() && von < self.sessions.len() {
                        let karte = self.sessions.remove(von);
                        self.sessions.insert(nach, karte);
                        self.active_session = verschieben(self.active_session, von, nach);
                    }
                }
            }
        }
        if let (Some(von), Some(pos)) = (self.dock_drag, zeiger) {
            if let Some(nach) = rects.iter().position(|r| r.contains(pos)) {
                if nach != von {
                    ui.painter()
                        .rect_stroke(rects[nach], 16.0, Stroke::new(1.5_f32, pal.accent));
                }
            }
        }

        if let Some(i) = hover_close {
            close = Some(i);
        }
        if let Some(i) = switch {
            self.switch_session(i);
        }
        if let Some(i) = close {
            let dirty = self
                .sessions
                .get(i)
                .map(|s| s.text != s.saved_text)
                .unwrap_or(false);
            if dirty {
                self.ask_confirm(t(lang, "discard_q"), Confirm::SessionClose(i));
            } else {
                self.close_session(i);
            }
        }
    }

    fn handle_shortcuts(&mut self, ctx: &egui::Context) {
        let undo = egui::KeyboardShortcut::new(egui::Modifiers::CTRL, egui::Key::Z);
        let redo = egui::KeyboardShortcut::new(
            egui::Modifiers {
                ctrl: true,
                shift: true,
                ..egui::Modifiers::default()
            },
            egui::Key::Z,
        );
        let redo_alt = egui::KeyboardShortcut::new(egui::Modifiers::CTRL, egui::Key::Y);
        let save = egui::KeyboardShortcut::new(egui::Modifiers::CTRL, egui::Key::S);
        let new = egui::KeyboardShortcut::new(egui::Modifiers::CTRL, egui::Key::N);
        if ctx.input_mut(|i| i.consume_shortcut(&undo)) {
            self.undo();
        }
        if ctx.input_mut(|i| i.consume_shortcut(&redo)) {
            self.redo();
        }
        if ctx.input_mut(|i| i.consume_shortcut(&redo_alt)) {
            self.redo();
        }
        if ctx.input_mut(|i| i.consume_shortcut(&save)) {
            self.save_doc_to_tray(true);
        }
        if ctx.input_mut(|i| i.consume_shortcut(&new)) {
            self.new_document();
        }
    }

    fn push_undo(&mut self) {
        self.undo.push(Snapshot {
            text: self.doc.clone(),
            sel: self.sel,
            at: 0.0,
        });
        if self.undo.len() > 200 {
            self.undo.remove(0);
        }
        self.redo.clear();
    }

    fn undo(&mut self) {
        if let Some(snap) = self.undo.pop() {
            self.redo.push(Snapshot {
                text: self.doc.clone(),
                sel: self.sel,
                at: 0.0,
            });
            self.doc = snap.text;
            self.sel = snap.sel;
            self.saved = false;
            self.set.doc = self.doc.clone();
            self.save_set();
            self.sync_active_session();
        }
    }

    fn redo(&mut self) {
        if let Some(snap) = self.redo.pop() {
            self.undo.push(Snapshot {
                text: self.doc.clone(),
                sel: self.sel,
                at: 0.0,
            });
            self.doc = snap.text;
            self.sel = snap.sel;
            self.saved = false;
            self.set.doc = self.doc.clone();
            self.save_set();
            self.sync_active_session();
        }
    }

    fn selection(&self) -> (usize, usize) {
        let end = self.doc.chars().count();
        self.sel.unwrap_or((end, end))
    }

    fn selection_wrapped(&self, left: &str, right: &str) -> bool {
        let sel = self.selection();
        if sel.1 <= sel.0 {
            return false;
        }
        let inner = editor::selected(&self.doc, sel);
        inner.starts_with(left) && inner.ends_with(right)
    }

    fn format_wrap(&mut self, left: &str, right: &str) {
        self.push_undo();
        let (text, sel) = editor::wrap_selection(&self.doc, self.selection(), left, right);
        self.doc = text;
        self.sel = Some(sel);
        self.saved = false;
        self.set.doc = self.doc.clone();
        self.save_set();
    }

    fn format_line(&mut self, prefix: &str) {
        self.push_undo();
        let (text, sel) = editor::toggle_line_prefix(&self.doc, self.selection(), prefix);
        self.doc = text;
        self.sel = Some(sel);
        self.saved = false;
        self.set.doc = self.doc.clone();
        self.save_set();
    }

    /// Schwebende Leiste über der Textauswahl, wie `#bar` in der
    /// Electron-Fassung: dieselben fünf Aktionen in derselben Reihenfolge.
    /// Text der aktuellen Auswahl, wie er in der Kontextleiste gebraucht wird.
    fn selected_text(&self) -> String {
        let (a, b) = self.selection();
        if b <= a {
            return String::new();
        }
        let src = &self.doc;
        src[byte_idx(src, a)..byte_idx(src, b)].to_string()
    }

    /// Uebersetzt die Auswahl im kleinen Fenster der Kontextleiste, ohne den
    /// Uebersetzer-Arbeitsplatz zu benutzen.
    fn start_mini_translate(&mut self) {
        if self.busy {
            return;
        }
        let Some(model) = self.write_model() else {
            self.status = t(self.lang, "no_model");
            return;
        };
        let text = self.mini_in.trim().to_string();
        if text.is_empty() {
            return;
        }
        self.busy = true;
        self.editor_job = false;
        self.mini_job = true;
        self.mini_out.clear();
        self.stop.store(false, Ordering::SeqCst);
        self.status = t(self.lang, "translating");
        let gloss = self
            .set
            .gloss
            .iter()
            .map(|g| (g.s.clone(), g.t.clone()))
            .collect();
        let pctx = PromptCtx {
            src: &self.set.src,
            tgt: &self.set.tgt,
            style: &self.set.style,
            ctxt: &self.set.ctxt,
            gloss,
            has_images: false,
            nllb: &self.nllb_langs,
        };
        let prompt = user_prompt_for(&text, &pctx);
        let sys = self.set.sys.clone();
        net::spawn_chat(
            self.cfg(),
            model,
            vec![
                json!({"role":"system","content":sys}),
                json!({"role":"user","content":prompt}),
            ],
            None,
            None,
            true,
            self.tx.clone(),
            self.stop.clone(),
        );
    }

    /// Schreibt die Auswahl im eingestellten Stil um (`styleB` der Vorlage).
    fn start_editor_style(&mut self) {
        let mut parts: Vec<String> = Vec::new();
        parts.push(
            if self.set.style.kind == "literal" {
                "Rewrite the text as literally as possible, staying close to its wording and structure."
            } else {
                "Rewrite the text naturally and idiomatically."
            }
            .to_string(),
        );
        match self.set.style.formal.as_str() {
            "formal" => parts.push("Use a formal register.".into()),
            "informal" => parts.push("Use an informal, casual register.".into()),
            _ => {}
        }
        match self.set.style.gender.as_str() {
            "neutral" => parts.push("Use gender-neutral wording for the addressed person.".into()),
            "male" => parts.push("The addressed person is male; use matching grammatical forms.".into()),
            "female" => parts.push("The addressed person is female; use matching grammatical forms.".into()),
            _ => {}
        }
        match self.set.style.writer.as_str() {
            "male" => parts.push("The writer is male; use matching grammatical forms for first-person references.".into()),
            "female" => parts.push("The writer is female; use matching grammatical forms for first-person references.".into()),
            "neutral" => parts.push("The writer is gender-neutral; avoid gendered first-person forms.".into()),
            _ => {}
        }
        let instr = format!(
            "{} Do not translate; keep the language and the meaning. Output only the rewritten text.",
            parts.join(" ")
        );
        self.start_editor_instr(&instr, false);
    }

    fn context_bar(&mut self, ctx: &egui::Context) {
        let Some(sel_rect) = self.bar_rect else { return };
        let pal = self.pal;
        let lang = self.lang;
        if self.busy {
            // Wie `.ctxbar.working` in der Vorlage: die Leiste bleibt stehen,
            // die Knoepfe weichen einem Ladekreis in einer kleinen Pille.
            let breite = 48.0;
            let pos = egui::pos2(sel_rect.center().x - breite / 2.0, sel_rect.top() - 50.0);
            egui::Area::new(egui::Id::new("ctx_bar_work"))
                .order(egui::Order::Foreground)
                .fixed_pos(pos)
                .show(ctx, |ui| {
                    egui::Frame::none()
                        .fill(pal.card)
                        .stroke(Stroke::new(1.0_f32, pal.line))
                        .rounding(Rounding::same(999.0))
                        .inner_margin(egui::Margin::symmetric(10.0, 6.0))
                        .shadow(egui::epaint::Shadow {
                            offset: egui::vec2(0.0, 2.0),
                            blur: 10.0,
                            spread: 0.0,
                            color: egui::Color32::from_black_alpha(60),
                        })
                        .show(ui, |ui| {
                            ladekreis(ui, &pal);
                        });
                });
            return;
        }
        let ops: [(&str, &str, &str); 7] = [
            ("shorten", "shorten", "shorten"),
            ("lengthen", "lengthen", "lengthen"),
            ("summarize", "summarize", "summarize"),
            ("proofread", "proofread", "proofread"),
            ("synonyms", "synonyms", "synonyms"),
            ("styleB", "wand", "rewrite"),
            ("translateSel", "languages", "translate_sel"),
        ];
        let width = ops.len() as f32 * 32.0 + (ops.len() as f32 - 1.0) * 6.0 + 16.0;
        let height = 40.0;
        let screen = ctx.screen_rect();
        let mut pos = egui::pos2(sel_rect.center().x - width / 2.0, sel_rect.top() - height - 10.0);
        if pos.y < 60.0 {
            pos.y = sel_rect.bottom() + 10.0;
        }
        pos.x = pos.x.clamp(8.0, (screen.right() - width - 8.0).max(8.0));

        let mut action: Option<&str> = None;
        let mut over_bar = false;
        let bar_resp = egui::Area::new(egui::Id::new("ctx_bar"))
            .order(egui::Order::Foreground)
            .fixed_pos(pos)
            .show(ctx, |ui| {
                egui::Frame::none()
                    .fill(pal.card)
                    .stroke(Stroke::new(1.0_f32, pal.line))
                    .rounding(Rounding::same(999.0))
                    .inner_margin(egui::Margin::symmetric(6.0, 4.0))
                    .shadow(egui::epaint::Shadow {
                        offset: egui::vec2(0.0, 2.0),
                        blur: 10.0,
                        spread: 0.0,
                        color: egui::Color32::from_black_alpha(60),
                    })
                    .show(ui, |ui| {
                        over_bar = ui.rect_contains_pointer(ui.max_rect());
                        ui.horizontal(|ui| {
                            for (op, icon, tip) in ops {
                                if icon_btn(ui, &pal, icon, false)
                                    .on_hover_text(t(lang, tip))
                                    .clicked()
                                {
                                    action = Some(op);
                                }
                            }
                        });
                    });
            });
        self.bar_hover = over_bar;
        self.ctx_bar_rect = Some(bar_resp.response.rect);

        if let Some(op) = action {
            match op {
                "synonyms" => {
                    self.syn_word = self.selected_text().trim().to_string();
                    self.syn_out.clear();
                    self.syn_sel = Some(self.selection());
                    self.show_syn = true;
                    self.request_synonyms();
                }
                "styleB" => self.show_style_bar = true,
                "translateSel" => {
                    let text = self.selected_text();
                    if !text.trim().is_empty() {
                        self.mini_in = text;
                    }
                    self.mini_out.clear();
                    self.mini_sel = Some(self.selection());
                    self.show_mini = true;
                }
                _ => self.start_editor_op(op),
            }
        }
    }

    /// Zeichnet die Wellenlinien unter den falsch geschriebenen Woertern und
    /// merkt sich die Lage des ersten Fundes fuer das Rechtschreibfenster.
    fn paint_spell_marks(&mut self, ui: &egui::Ui, out: &egui::text_edit::TextEditOutput) {
        let chars: Vec<char> = self.doc.chars().collect();
        let mut marks: Vec<(usize, usize)> = Vec::new();
        for hit in &self.spell_hits {
            let needle: Vec<char> = hit.word.chars().collect();
            if needle.is_empty() {
                continue;
            }
            let mut i = 0usize;
            while i + needle.len() <= chars.len() {
                if chars[i..i + needle.len()] == needle[..] {
                    let after = i + needle.len();
                    let before_ok = i == 0 || !chars[i - 1].is_alphanumeric();
                    let after_ok = after >= chars.len() || !chars[after].is_alphanumeric();
                    if before_ok && after_ok {
                        marks.push((i, after));
                        i = after;
                        continue;
                    }
                }
                i += 1;
            }
        }
        let painter = ui.painter();
        let offset = out.galley_pos.to_vec2();
        let color = Color32::from_rgb(0xd0, 0x3a, 0x3a);
        // Wie `startSpellAnim` in der Vorlage: die Wellen wandern, solange die
        // Einstellung an ist. Ohne sie stehen sie still.
        let phase = if self.set.ed_spell_anim {
            ui.input(|i| i.time) as f32 * 6.0
        } else {
            0.0
        };
        let mut anchor: Option<egui::Pos2> = None;
        self.spell_mark_rects.clear();
        for (a, b) in marks {
            let p1 = out.galley.pos_from_ccursor(egui::text::CCursor::new(a));
            let p2 = out.galley.pos_from_ccursor(egui::text::CCursor::new(b));
            let same_row = (p1.top() - p2.top()).abs() < 1.0;
            let x0 = p1.left() + offset.x;
            let x1 = if same_row {
                p2.left() + offset.x
            } else {
                p1.right() + offset.x
            };
            if x1 <= x0 + 1.0 {
                continue;
            }
            let top = p1.top() + offset.y;
            let bottom = p1.bottom() + offset.y;
            let y = bottom - 1.0;
            let word: String = chars[a..b].iter().collect();
            self.spell_mark_rects.push((
                word,
                egui::Rect::from_min_max(egui::pos2(x0, top), egui::pos2(x1, bottom)),
            ));
            if anchor.is_none() {
                anchor = Some(egui::pos2(x0, top));
            }
            // Geschlaengelte Linie: Sinus mit sechs Pixeln Wellenlaenge,
            // abgetastet alle 1,5 Pixel.
            let steps = (((x1 - x0) / 1.5).ceil() as usize).max(4);
            let mut pts = Vec::with_capacity(steps + 1);
            for i in 0..=steps {
                let t = i as f32 / steps as f32;
                let x = x0 + (x1 - x0) * t;
                let dy = 1.4 * ((x - x0) * std::f32::consts::TAU / 6.0 + phase).sin();
                pts.push(egui::pos2(x, y + dy));
            }
            painter.add(egui::Shape::line(pts, Stroke::new(1.4_f32, color)));
        }
        self.spell_anchor = anchor;
        if self.set.ed_spell_anim {
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(60));
        }
    }

    /// Rechtsklick-Menue auf einem falsch geschriebenen Wort: Vorschlaege aus
    /// dem Woerterbuch, Ignorieren und Ins-Woerterbuch. Entspricht dem
    /// Kontextmenue der Electron-Fassung.
    /// Legt die Partikel-Animation ueber den bearbeiteten Text (`.ai-processing`
    /// in `style.css`): der Text verschwindet, drei Lagen kleiner Punkte wandern
    /// nach oben, bis die Antwort da ist.
    fn paint_processing(&mut self, ui: &egui::Ui, out: &egui::text_edit::TextEditOutput) {
        if !self.editor_job {
            return;
        }
        let chars = self.doc.chars().count();
        let (a, b) = match self.pending_sel {
            Some((a, b)) if b > a => (a.min(chars), b.min(chars)),
            _ => (0, chars),
        };
        if b <= a {
            return;
        }
        let zeit = ui.input(|i| i.time) as f32;
        // Nur innerhalb der Schreibflaeche malen: so wandert die Animation beim
        // Scrollen mit dem Text und laeuft nicht ueber Leiste oder Karten.
        let clip = ui.clip_rect();
        let dunkel = self.pal.dark;
        // Drei Lagen wie die drei Punktmuster in `.ai-processing`. Die mittlere
        // wandert in der Vorlage nach unten (`0 43px` im Keyframe), die beiden
        // anderen nach oben; Tempo jeweils eine Kachel in vier Sekunden.
        let lagen = [
            (17.6f32, -14.75f32, if dunkel { 0.62f32 } else { 0.60f32 }),
            (14.3, 10.75, if dunkel { 0.45 } else { 0.42 }),
            (11.0, -7.75, if dunkel { 0.55 } else { 0.32 }),
        ];
        // Zeilenweise statt als ein Rechteck: die Vorlage legt den Schleier auf
        // den umbrochenen Text (`box-decoration-break:clone`), nicht auf den
        // Kasten der Auswahl.
        let mut idx = 0usize;
        for row in &out.galley.rows {
            let start = idx;
            idx += row.glyphs.len() + usize::from(row.ends_with_newline);
            let end = idx;
            if end <= a || start >= b {
                continue;
            }
            let cs = start.max(a);
            let ce = end.min(b);
            if ce <= cs {
                continue;
            }
            let p1 = out
                .galley
                .pos_from_ccursor(egui::text::CCursor::new(cs))
                .translate(out.galley_pos.to_vec2());
            let p2 = out
                .galley
                .pos_from_ccursor(egui::text::CCursor::new(ce))
                .translate(out.galley_pos.to_vec2());
            let zeile = egui::Rect::from_min_max(
                egui::pos2(p1.left(), row.rect.top() + out.galley_pos.y),
                egui::pos2(p2.left().max(p1.left() + 4.0), row.rect.bottom() + out.galley_pos.y),
            )
            .expand2(egui::vec2(2.0, 0.0));
            if !clip.intersects(zeile) {
                continue;
            }
            let maler = ui.painter().with_clip_rect(clip.intersect(zeile));
            maler.rect_filled(zeile, 0.0, self.pal.card);
            let hoehe = zeile.height();
            if hoehe <= 0.0 {
                continue;
            }
            for (raster, tempo, deckkraft) in lagen {
                let farbe = if dunkel {
                    egui::Color32::from_rgba_unmultiplied(255, 255, 255, (deckkraft * 255.0) as u8)
                } else {
                    egui::Color32::from_rgba_unmultiplied(20, 20, 26, (deckkraft * 240.0) as u8)
                };
                // `tempo` ist Pixel je Sekunde; der Versatz laeuft innerhalb
                // einer Kachel um, damit die Punkte gleichmaessig treiben.
                let versatz = (zeit * tempo).rem_euclid(raster);
                let spalten = (zeile.width() / raster).ceil() as i32 + 1;
                let reihen = (hoehe / raster).ceil() as i32 + 2;
                for cx in 0..spalten {
                    for cy in -1..reihen {
                        let h = (cx * 73856093) ^ (cy * 19349663) ^ (raster as i32 * 83492791);
                        let hx = ((h.unsigned_abs() % 977) as f32) / 977.0;
                        let hy = ((h.unsigned_abs() / 977 % 991) as f32) / 991.0;
                        let x = zeile.left() + (cx as f32 + hx) * raster;
                        let roh = zeile.top() + (cy as f32 + hy) * raster + versatz;
                        let y = zeile.top() + (roh - zeile.top()).rem_euclid(hoehe);
                        maler.rect_filled(
                            egui::Rect::from_center_size(egui::pos2(x, y), egui::vec2(1.2, 1.2)),
                            0.0,
                            farbe,
                        );
                    }
                }
            }
        }
        // 60 Bilder je Sekunde statt 25: die Punkte sollen ruhig laufen.
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_millis(16));
    }

    /// Rechtsklick-Menue auf einem falsch geschriebenen Wort: Vorschlaege,
    /// Ignorieren, Ins-Woerterbuch — als schmale Karte mit Zeilen wie `.row`
    /// (ein Trennstrich in der Breite der Karte, kein `ui.separator()`).
    fn spell_menu_window(&mut self, ctx: &egui::Context) {
        let Some(menu) = self.spell_menu.clone() else {
            return;
        };
        let pal = self.pal;
        let lang = self.lang;
        let sug: Vec<String> = self.spell_menu_sug.clone();
        let wartet = self.spell_menu_wait;
        let red = Color32::from_rgb(0xd0, 0x3a, 0x3a);
        let mut pick: Option<String> = None;
        let mut ignore = false;
        let mut add = false;
        let screen = ctx.screen_rect();
        let breite = 240.0;
        let pos = egui::pos2(
            menu.pos.x.clamp(8.0, (screen.right() - breite - 8.0).max(8.0)),
            menu.pos.y.clamp(8.0, (screen.bottom() - 220.0).max(8.0)),
        );
        let area = egui::Area::new(egui::Id::new("spell_menu"))
            .order(egui::Order::Foreground)
            .fixed_pos(pos)
            .show(ctx, |ui| {
                ui.set_width(breite);
                pop_card(&pal).show(ui, |ui| {
                    ui.set_width(breite - 12.0);
                    ui.spacing_mut().item_spacing.y = 0.0;
                    let (kopf, _) = ui.allocate_exact_size(
                        egui::vec2(breite - 12.0, 26.0),
                        egui::Sense::hover(),
                    );
                    ui.painter().text(
                        egui::pos2(kopf.left() + 12.0, kopf.center().y),
                        egui::Align2::LEFT_CENTER,
                        &menu.word,
                        egui::FontId::proportional(13.0),
                        red,
                    );
                    if sug.is_empty() {
                        let (zeile, _) = ui.allocate_exact_size(
                            egui::vec2(breite - 12.0, 22.0),
                            egui::Sense::hover(),
                        );
                        ui.painter().text(
                            egui::pos2(zeile.left() + 12.0, zeile.center().y),
                            egui::Align2::LEFT_CENTER,
                            if wartet {
                                t(lang, "load_alts")
                            } else {
                                t(lang, "no_alts")
                            },
                            egui::FontId::proportional(12.0),
                            pal.muted,
                        );
                    }
                    for vorschlag in &sug {
                        if row_zeile(ui, &pal, vorschlag, false, false) {
                            pick = Some(vorschlag.clone());
                        }
                    }
                    ui.add_space(4.0);
                    let (linie, _) = ui.allocate_exact_size(
                        egui::vec2(breite - 12.0, 1.0),
                        egui::Sense::hover(),
                    );
                    ui.painter().rect_filled(linie, 0.0, pal.line);
                    ui.add_space(4.0);
                    if row_zeile(ui, &pal, &t(lang, "ignore"), false, false) {
                        ignore = true;
                    }
                    if row_zeile(ui, &pal, &t(lang, "add_to_dict"), false, false) {
                        add = true;
                    }
                });
            });
        let mut close = false;
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            close = true;
        }
        if ctx.input(|i| i.pointer.button_clicked(egui::PointerButton::Primary))
            && !area.response.hovered()
        {
            close = true;
        }
        if let Some(vorschlag) = pick {
            self.replace_word_all(&menu.word, &vorschlag);
            self.rescan_spell();
            close = true;
        }
        if ignore {
            self.spell_ignored.push(menu.word.to_lowercase());
            self.rescan_spell();
            close = true;
        }
        if add {
            let word = menu.word.clone();
            if !self.dict_words.iter().any(|w| w.eq_ignore_ascii_case(&word)) {
                self.dict_words.push(word);
                self.dict_words.sort();
                self.save_personal_dict();
            }
            self.rescan_spell();
            close = true;
        }
        if close {
            self.spell_menu = None;
            self.spell_menu_sug.clear();
            self.spell_menu_wait = false;
        }
    }

    fn format_link(&mut self) {
        self.push_undo();
        let (text, sel) = editor::make_link(&self.doc, self.selection());
        self.doc = text;
        self.sel = Some(sel);
        self.saved = false;
        self.set.doc = self.doc.clone();
        self.save_set();
    }

    fn clear_formatting(&mut self) {
        self.push_undo();
        let (text, sel) = editor::strip_markup(&self.doc, self.selection());
        self.doc = text;
        self.sel = Some(sel);
        self.saved = false;
        self.set.doc = self.doc.clone();
        self.save_set();
    }

    fn sync_active_session(&mut self) {
        let name = library::doc_name(&self.doc, &t(self.lang, "doc_name"));
        if let Some(s) = self.sessions.get_mut(self.active_session) {
            s.text = self.doc.clone();
            s.name = name;
        }
    }

    fn new_document(&mut self) {
        if !self.doc.trim().is_empty() {
            self.save_doc_to_tray(false);
        }
        self.sync_active_session();
        let name = t(self.lang, "doc_name");
        self.sessions.push(SessionDoc {
            key: self.next_key,
            name,
            text: String::new(),
            saved_text: String::new(),
        });
        self.next_key += 1;
        self.active_session = self.sessions.len() - 1;
        self.doc.clear();
        self.doc_id = None;
        self.set.doc.clear();
        self.set.doc_id = None;
        self.sel = None;
        self.saved = true;
        self.save_set();
    }

    fn switch_session(&mut self, idx: usize) {
        if idx >= self.sessions.len() || idx == self.active_session {
            return;
        }
        self.sync_active_session();
        self.active_session = idx;
        self.doc = self.sessions[idx].text.clone();
        self.doc_id = None;
        self.set.doc = self.doc.clone();
        self.set.doc_id = None;
        self.sel = None;
        self.saved = self.sessions[idx].text == self.sessions[idx].saved_text;
        self.save_set();
    }

    fn close_session(&mut self, idx: usize) {
        if idx >= self.sessions.len() || self.sessions.len() == 1 {
            return;
        }
        self.sessions.remove(idx);
        if self.active_session >= self.sessions.len() {
            self.active_session = self.sessions.len() - 1;
        } else if idx < self.active_session {
            self.active_session -= 1;
        }
        self.doc = self.sessions[self.active_session].text.clone();
        self.doc_id = None;
        self.set.doc = self.doc.clone();
        self.set.doc_id = None;
        self.sel = None;
        self.saved = self.sessions[self.active_session].text
            == self.sessions[self.active_session].saved_text;
        self.save_set();
    }

    /// Menue am Speichern-Knopf wie `savePop` in `editor.js`: Ablage,
    /// dann Markdown, Text und PDF.
    /// Wort unter dem Zeiger bestimmen und Ersatzwoerter anfragen.
    fn open_alternatives(&mut self, text: &str, idx: usize, pos: egui::Pos2) {
        if self.busy || self.last_tr.trim().is_empty() {
            return;
        }
        let lang = self.lang;
        if is_mt(&self.set.model) {
            self.status = t(lang, "nllb_no_alt");
            return;
        }
        let Some(helper) = self.write_model() else {
            self.status = t(lang, "need_helper");
            return;
        };
        let chars: Vec<char> = text.chars().collect();
        if chars.is_empty() {
            return;
        }
        let mut a = idx.min(chars.len() - 1);
        if !chars[a].is_alphanumeric() && a > 0 {
            a -= 1;
        }
        if !chars[a].is_alphanumeric() {
            return;
        }
        let mut start = a;
        while start > 0 && chars[start - 1].is_alphanumeric() {
            start -= 1;
        }
        let mut ende = a;
        while ende + 1 < chars.len() && chars[ende + 1].is_alphanumeric() {
            ende += 1;
        }
        let wort: String = chars[start..=ende].iter().collect();
        if wort.trim().is_empty() {
            return;
        }
        // Wie in der Vorlage: die wievielte Stelle dieses Wortes ist gemeint.
        let lower = wort.to_lowercase();
        let mut nr = 1usize;
        let mut i = 0usize;
        while i < start {
            if chars[i].is_alphanumeric() && (i == 0 || !chars[i - 1].is_alphanumeric()) {
                let mut e = i;
                while e + 1 < start && chars[e + 1].is_alphanumeric() {
                    e += 1;
                }
                let davor: String = chars[i..=e].iter().collect();
                if davor.to_lowercase() == lower {
                    nr += 1;
                }
                i = e + 1;
            } else {
                i += 1;
            }
        }
        let src = if self.set.src == "auto" {
            String::new()
        } else {
            format!(" ({})", eng_name(&self.set.src, &self.nllb_langs))
        };
        let tgt = eng_name(&self.set.tgt, &self.nllb_langs);
        let quelle = if self.last_src.is_empty() {
            "(image)".to_string()
        } else {
            self.last_src.clone()
        };
        let stelle = if nr > 1 {
            format!(" (occurrence #{})", nr)
        } else {
            String::new()
        };
        let prompt = format!(
            "Source{}: {}\nTranslation ({}): {}\n\nGive up to 6 alternative {} words or short phrases that could replace the word \"{}\"{} in this translation, with the same meaning in context or close synonyms, most likely first. Respond as JSON: {{\"alternatives\":[\"...\"]}}",
            src, quelle, tgt, self.last_tr, tgt, wort, stelle
        );
        self.alt_menu = Some(AltMenu {
            wort,
            nr,
            pos,
            list: None,
            fehler: false,
        });
        self.status = t(lang, "load_alts");
        net::spawn_alts(self.cfg(), helper, prompt, self.tx.clone());
    }

    /// Menue unter dem angeklickten Wort: Ersatzwoerter, Ladehinweis oder Fehler.
    fn alt_menu_window(&mut self, ctx: &egui::Context) {
        let Some(menu) = self.alt_menu.clone() else {
            return;
        };
        let pal = self.pal;
        let lang = self.lang;
        let anchor = egui::Rect::from_min_size(menu.pos, egui::vec2(1.0, 1.0));
        let mut wahl: Option<String> = None;
        let zu = pop_at(ctx, "alt_pop", anchor, 280.0, &pal, |ui| {
            ui.set_width(268.0);
            ui.spacing_mut().item_spacing.y = 0.0;
            match &menu.list {
                None => {
                    ui.label(
                        RichText::new(t(lang, "load_alts"))
                            .size(12.0)
                            .color(pal.muted),
                    );
                }
                Some(list) if list.is_empty() => {
                    let text = if menu.fehler {
                        t(lang, "alt_err")
                    } else {
                        t(lang, "no_alts")
                    };
                    ui.label(RichText::new(text).size(12.0).color(pal.muted));
                }
                Some(list) => {
                    for a in list {
                        if row_zeile(ui, &pal, a, false, false) {
                            wahl = Some(a.to_string());
                        }
                    }
                }
            }
        });
        let gewaehlt = wahl.is_some();
        if let Some(a) = wahl {
            self.reword(a);
        }
        if zu || gewaehlt {
            self.alt_menu = None;
        }
    }

    /// Ersetzt das Wort im Satz; das Hilfsmodell formuliert die Stelle um.
    fn reword(&mut self, alt: String) {
        let Some(menu) = self.alt_menu.clone() else {
            return;
        };
        let lang = self.lang;
        let Some(helper) = self.write_model() else {
            self.status = t(lang, "need_helper");
            return;
        };
        let tgt = eng_name(&self.set.tgt, &self.nllb_langs);
        let stelle = if menu.nr > 1 {
            format!(" (occurrence #{})", menu.nr)
        } else {
            String::new()
        };
        let prompt = format!(
            "Here is a {} translation:\n<text>\n{}\n</text>\n\nReplace the word \"{}\"{} with \"{}\" and change only what grammar requires (articles, inflection, word order, punctuation) so the text stays fluent. Keep everything else unchanged. Output only the revised text.",
            tgt, self.last_tr, menu.wort, stelle, alt
        );
        self.alt_menu = None;
        self.reword_job = true;
        self.busy = true;
        self.status = t(lang, "adjusting");
        net::spawn_chat(
            self.cfg(),
            helper,
            vec![
                json!({"role": "system", "content": self.set.sys}),
                json!({"role": "user", "content": prompt}),
            ],
            None,
            None,
            false,
            self.tx.clone(),
            self.stop.clone(),
        );
    }

    fn save_menu(&mut self, ctx: &egui::Context) {
        if !self.show_save_menu {
            return;
        }
        let Some(anchor) = self.save_anchor else {
            self.show_save_menu = false;
            return;
        };
        let pal = self.pal;
        let lang = self.lang;
        let breite = 240.0;
        let mut in_ablage = false;
        let mut art: Option<&str> = None;
        let zu = pop_at(ctx, "save_pop", anchor, breite, &pal, |ui| {
            ui.set_width(breite - 24.0);
            ui.spacing_mut().item_spacing.y = 0.0;
            if row_zeile(ui, &pal, &t(lang, "lib_save"), false, false) {
                in_ablage = true;
            }
            ui.add_space(4.0);
            let (linie, _) =
                ui.allocate_exact_size(egui::vec2(breite - 24.0, 1.0), egui::Sense::hover());
            ui.painter().rect_filled(linie, 0.0, pal.line);
            ui.add_space(4.0);
            for (kennung, name) in [
                ("md", "Markdown (.md)".to_string()),
                ("txt", t(lang, "save_txt")),
                ("pdf", t(lang, "save_pdf")),
            ] {
                if row_zeile(ui, &pal, &name, false, false) {
                    art = Some(kennung);
                }
            }
        });
        if in_ablage {
            self.save_doc_to_tray(false);
        }
        if let Some(kennung) = art {
            self.export_document_as(kennung);
        }
        if in_ablage || art.is_some() || zu {
            self.show_save_menu = false;
        }
    }

    /// Export als `md`, `txt` oder `pdf`; ein erfolgreicher Export landet wie in
    /// der Vorlage zusaetzlich in der Ablage.
    fn export_document_as(&mut self, art: &str) {
        let lang = self.lang;
        let name = library::doc_name(&self.doc, &t(lang, "doc_name"));
        let (endung, inhalt): (&str, Vec<u8>) = match art {
            "txt" => ("txt", editor::plain_text(&self.doc).into_bytes()),
            "pdf" => ("pdf", crate::pdf::text_pdf(&editor::plain_text(&self.doc))),
            _ => ("md", self.doc.clone().into_bytes()),
        };
        if let Some(path) = rfd::FileDialog::new()
            .set_file_name(format!("{}.{}", name, endung))
            .save_file()
        {
            let _ = std::fs::write(path, inhalt);
            self.status = t(lang, "saved");
            self.save_doc_to_tray(true);
        }
    }

    fn import_document(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("Dokument", &["txt", "md", "html"])
            .pick_file()
        {
            if let Ok(text) = std::fs::read_to_string(&path) {
                self.push_undo();
                self.doc = text;
                self.doc_id = None;
                self.sel = None;
                self.saved = false;
                self.sync_active_session();
            }
        }
    }

    fn save_doc_to_tray(&mut self, quiet: bool) {
        let lang = self.lang;
        let name = library::doc_name(&self.doc, &t(lang, "doc_name"));
        if let Some(id) = library::save(self.doc_id.as_deref(), &name, &self.doc) {
            self.doc_id = Some(id.clone());
            self.set.doc_id = Some(id);
            self.set.doc = self.doc.clone();
            self.save_set();
            self.lib = library::list();
            self.saved = true;
            self.sync_active_session();
            if let Some(s) = self.sessions.get_mut(self.active_session) {
                s.saved_text = self.doc.clone();
                s.name = name;
            }
            if quiet {
                self.status = t(lang, "saved");
            }
        }
    }

    // ---- Probelesen (Diff) ------------------------------------------------
    /// Fenster zum Probelesen wie `#diffPanel` in der Vorlage: Titel und
    /// „Alle uebernehmen“ oben, darunter je Aenderung eine Zeile mit dem alten
    /// und dem neuen Wortlaut und zwei Knoepfen (uebernehmen, verwerfen).
    /// Entschiedene Zeilen treten zurueck; ist alles entschieden, wird gesetzt.
    /// Das Kreuz schliesst und laesst offene Aenderungen beim alten Text.
    fn diff_window(&mut self, ctx: &egui::Context) {
        let Some(diff) = self.diff.clone() else { return };
        let pal = self.pal;
        let lang = self.lang;
        let mut open = true;
        let mut accept_all = false;
        let mut entscheidung: Option<(usize, bool)> = None;
        let mut schliessen = false;
        let hunks: Vec<(String, String)> = diff
            .parts
            .iter()
            .filter_map(|p| match p {
                Part::Hunk { old, new } => Some((old.clone(), new.clone())),
                _ => None,
            })
            .collect();
        dialog(ctx, "diff_win", &t(lang, "diff_title"), 560.0, &pal, &mut open, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(format!("{} {}", hunks.len(), t(lang, "diff_hunks")))
                        .size(12.0)
                        .color(pal.muted),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if icon_btn(ui, &pal, "checkAll", false)
                        .on_hover_text(t(lang, "accept_all"))
                        .clicked()
                    {
                        accept_all = true;
                    }
                });
            });
            ui.add_space(4.0);
            if hunks.is_empty() {
                ui.label(RichText::new(t(lang, "diff_empty")).color(pal.muted));
            }
            egui::ScrollArea::vertical()
                .id_source("diff_list")
                .max_height(360.0)
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    let green = Color32::from_rgb(0x1f, 0x7a, 0x3a);
                    let red = Color32::from_rgb(0xb9, 0x1c, 0x1c);
                    for (i, (old, new)) in hunks.iter().enumerate() {
                        let erledigt = diff.is_decided(i);
                        // Entschiedene Zeilen treten zurueck (in der Vorlage
                        // `opacity: 0.45`); hier ueber gedaempfte Farben.
                        let farbe_alt = if erledigt { pal.muted } else { red };
                        let farbe_neu = if erledigt { pal.muted } else { green };
                        {
                            egui::Frame::none()
                                .fill(pal.soft)
                                .rounding(Rounding::same(10.0))
                                .inner_margin(egui::Margin {
                                    left: 10.0,
                                    right: 8.0,
                                    top: 8.0,
                                    bottom: 8.0,
                                })
                                .show(ui, |ui| {
                                    ui.horizontal_top(|ui| {
                                        ui.vertical(|ui| {
                                            ui.set_width((ui.available_width() - 76.0).max(120.0));
                                            ui.horizontal_wrapped(|ui| {
                                                if !old.trim().is_empty() {
                                                    ui.label(
                                                        RichText::new(old.trim())
                                                            .size(13.0)
                                                            .color(farbe_alt)
                                                            .strikethrough(),
                                                    );
                                                    if !new.trim().is_empty() {
                                                        ui.label(
                                                            RichText::new("→")
                                                                .size(12.0)
                                                                .color(pal.muted),
                                                        );
                                                    }
                                                }
                                                if !new.trim().is_empty() {
                                                    ui.label(
                                                        RichText::new(new.trim())
                                                            .size(13.0)
                                                            .color(farbe_neu),
                                                    );
                                                }
                                            });
                                        });
                                        if !erledigt {
                                            ui.with_layout(
                                                egui::Layout::right_to_left(egui::Align::Min),
                                                |ui| {
                                                    if icon_btn(ui, &pal, "x", false)
                                                        .on_hover_text(t(lang, "discard"))
                                                        .clicked()
                                                    {
                                                        entscheidung = Some((i, false));
                                                    }
                                                    if icon_btn(ui, &pal, "check", false)
                                                    .on_hover_text(t(lang, "accept"))
                                                    .clicked()
                                                    {
                                                        entscheidung = Some((i, true));
                                                    }
                                                },
                                            );
                                        }
                                    });
                                });
                        }
                        ui.add_space(4.0);
                    }
                });
        });

        if let Some((i, annehmen)) = entscheidung {
            if let Some(d) = self.diff.as_mut() {
                d.decide(i, annehmen);
            }
        }
        if accept_all {
            if let Some(d) = self.diff.as_mut() {
                d.decide_all(true);
            }
        }
        if !open {
            schliessen = true;
        }
        let fertig = self.diff.as_ref().map(|d| d.all_decided()).unwrap_or(false);
        if schliessen {
            if let Some(d) = self.diff.as_mut() {
                d.settle();
            }
        }
        if schliessen || fertig {
            self.apply_diff();
        }
    }

    /// Setzt den Text aus den Entscheidungen des Probelesens und schliesst.
    fn apply_diff(&mut self) {
        let Some(diff) = self.diff.take() else { return };
        let result = diff.result();
        if result == self.doc {
            return;
        }
        self.push_undo();
        self.doc = result;
        self.sel = None;
        self.saved = false;
        self.set.doc = self.doc.clone();
        self.save_set();
        self.sync_active_session();
        self.spell_scanned_doc.clear();
        self.status = String::new();
    }

    // ---- Rechtschreibung ---------------------------------------------------
    fn schedule_spell(&mut self, ctx: &egui::Context) {
        if !self.set.ed_spell {
            self.spell_hits.clear();
            return;
        }
        if self.doc.trim().is_empty() {
            self.spell_hits.clear();
            self.spell_error = None;
            self.spell_scanned_doc.clear();
            return;
        }
        if self.spell_scanned_doc == self.doc {
            return;
        }
        if self.spell_waiting {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
            return;
        }
        let now = ctx.input(|i| i.time);
        if now - self.spell_scan_at < 0.5 {
            ctx.request_repaint_after(std::time::Duration::from_millis(250));
            return;
        }
        self.spell_scan_at = now;
        self.rescan_spell();
    }

    /// Schickt den Text an den Rechtschreib-Thread; das Ergebnis kommt
    /// asynchron ueber `poll_spell` zurueck.
    fn rescan_spell(&mut self) {
        let lang = self.set.dict_lang.clone();
        let mut personal = self.dict_words.clone();
        personal.extend(self.spell_ignored.iter().cloned());
        self.spell_gen += 1;
        let gen = self.spell_gen;
        let doc = self.doc.clone();
        let worker = self
            .spell_worker
            .get_or_insert_with(crate::spell::SpellWorker::new);
        worker.scan(gen, &lang, &doc, &personal);
        self.spell_waiting = true;
    }

    /// Holt fertige Pruefungen und Vorschlaege aus dem Thread ab.
    fn poll_spell(&mut self) {
        let Some(worker) = &self.spell_worker else {
            return;
        };
        while let Some(erg) = worker.try_recv() {
            match erg {
                crate::spell::Ergebnis::Scan {
                    gen,
                    doc,
                    hits,
                    error,
                } => {
                    if gen >= self.spell_done_gen {
                        self.spell_done_gen = gen;
                        self.spell_hits = hits;
                        self.spell_error = error;
                        self.spell_scanned_doc = doc;
                    }
                    self.spell_waiting = false;
                }
                crate::spell::Ergebnis::Suggest { word, list } => {
                    if self
                        .spell_menu
                        .as_ref()
                        .map(|m| m.word == word)
                        .unwrap_or(false)
                    {
                        self.spell_menu_sug = list.clone();
                        self.spell_menu_wait = false;
                    }
                    for hit in self.spell_hits.iter_mut() {
                        if hit.word == word {
                            hit.suggestions = list.clone();
                        }
                    }
                }
            }
        }
    }

    fn save_personal_dict(&self) {
        let path = crate::settings::data_dir().join("personal-dictionary.json");
        if let Ok(text) = serde_json::to_string_pretty(&self.dict_words) {
            let _ = std::fs::write(path, text);
        }
    }

    /// Ersetzt ein Wort im ganzen Dokument, aber nur als ganzes Wort.
    fn replace_word_all(&mut self, from: &str, to: &str) {
        if from.is_empty() || from == to {
            return;
        }
        let chars: Vec<char> = self.doc.chars().collect();
        let needle: Vec<char> = from.chars().collect();
        let mut out = String::with_capacity(self.doc.len());
        let mut i = 0usize;
        let mut replaced = false;
        while i < chars.len() {
            let fits = i + needle.len() <= chars.len() && chars[i..i + needle.len()] == needle[..];
            if fits {
                let before_ok = i == 0 || !chars[i - 1].is_alphanumeric();
                let after = i + needle.len();
                let after_ok = after >= chars.len() || !chars[after].is_alphanumeric();
                if before_ok && after_ok {
                    out.extend(to.chars());
                    i = after;
                    replaced = true;
                    continue;
                }
            }
            out.push(chars[i]);
            i += 1;
        }
        if replaced {
            self.push_undo();
            self.doc = out;
            self.saved = false;
            self.set.doc = self.doc.clone();
            self.save_set();
            self.sync_active_session();
            self.spell_scanned_doc.clear();
        }
    }

    /// Setzt die Startposition des Rechtschreibfensters an den ersten
    /// markierten Fehler, begrenzt auf den Bildschirm.
    fn open_spell(&mut self, screen: egui::Rect) {
        if let Some(a) = self.spell_anchor {
            let x = (a.x - 40.0).clamp(8.0, (screen.right() - 420.0).max(8.0));
            let y = (a.y + 20.0).clamp(40.0, (screen.bottom() - 320.0).max(40.0));
            self.spell_pos = Some(egui::pos2(x, y));
        }
    }

    /// Stil-Auswahl der Kontextleiste; entspricht `stylePop` in der
    /// Electron-Fassung.
    fn style_bar_window(&mut self, ctx: &egui::Context) {
        if !self.show_style_bar {
            return;
        }
        let pal = self.pal;
        let lang = self.lang;
        let mut rewrite = false;
        let anchor = self
            .ctx_bar_rect
            .unwrap_or_else(|| egui::Rect::from_center_size(ctx.screen_rect().center(), egui::vec2(1.0, 1.0)));
        let closed = pop_at(ctx, "style_bar_pop", anchor, 340.0, &pal, |ui| {
                let groups: [(&str, &[(&str, &str)]); 4] = [
                    ("l_type", &[("natural", "o_natural"), ("literal", "o_literal")]),
                    (
                        "l_formal",
                        &[("standard", "o_standard"), ("formal", "o_formal"), ("informal", "o_informal")],
                    ),
                    (
                        "l_gender",
                        &[("unknown", "o_unknown"), ("neutral", "o_neutral"), ("male", "o_male"), ("female", "o_female")],
                    ),
                    (
                        "l_writer",
                        &[("unknown", "o_unknown"), ("neutral", "o_neutral"), ("male", "o_male"), ("female", "o_female")],
                    ),
                ];
                for (label, opts) in groups {
                    ui.label(RichText::new(t(lang, label)).size(12.5).color(pal.muted));
                    ui.horizontal_wrapped(|ui| {
                        for (value, text) in opts {
                            let current = match label {
                                "l_type" => &self.set.style.kind,
                                "l_formal" => &self.set.style.formal,
                                "l_gender" => &self.set.style.gender,
                                _ => &self.set.style.writer,
                            };
                            let on = current == value;
                            let option = t(lang, text);
                            let btn = if on {
                                egui::Button::new(
                                    RichText::new(&option).color(pal.on_accent).size(13.0),
                                )
                                .fill(pal.accent)
                                .stroke(Stroke::NONE)
                                .rounding(Rounding::same(999.0))
                                .min_size(egui::vec2(32.0, 30.0))
                            } else {
                                egui::Button::new(RichText::new(&option).color(pal.text).size(13.0))
                                    .fill(pal.card)
                                    .stroke(Stroke::new(1.0_f32, pal.line))
                                    .rounding(Rounding::same(999.0))
                                    .min_size(egui::vec2(32.0, 30.0))
                            };
                            if ui.add(btn).clicked() {
                                match label {
                                    "l_type" => self.set.style.kind = value.to_string(),
                                    "l_formal" => self.set.style.formal = value.to_string(),
                                    "l_gender" => self.set.style.gender = value.to_string(),
                                    _ => self.set.style.writer = value.to_string(),
                                }
                                self.save_set();
                            }
                        }
                    });
                    ui.add_space(6.0);
                }
                ui.separator();
                let go = egui::Button::new(
                    RichText::new(t(lang, "rewrite"))
                        .color(pal.on_accent)
                        .size(13.5),
                )
                .fill(pal.accent)
                .stroke(Stroke::NONE)
                .rounding(Rounding::same(999.0))
                .min_size(egui::vec2(150.0, 34.0));
                if ui.add(go).clicked() {
                    rewrite = true;
                }
            });
        if rewrite {
            self.show_style_bar = false;
            self.start_editor_style();
        }
        if closed {
            self.show_style_bar = false;
        }
    }

    /// Kleiner Uebersetzer fuer die Auswahl; entspricht `transPop` in
    /// `editor.js`: Sprachpille links und rechts, dazwischen ein Pfeil, darunter
    /// je eine Reihe mit runden Symbolknoepfen fuer Formalitaet, Schreiber und
    /// Adressat, unten rechts der Uebersetzen-Knopf.
    fn mini_window(&mut self, ctx: &egui::Context) {
        if !self.show_mini {
            return;
        }
        let pal = self.pal;
        let lang = self.lang;
        let mut go = false;
        let llm = !is_mt(&self.set.model) && !is_tg(&self.set.model);
        let src = if self.set.src == "auto" {
            t(lang, "detect")
        } else {
            lang_label(&self.set.src, &self.nllb_langs)
        };
        let tgt = lang_label(&self.set.tgt, &self.nllb_langs);
        let anchor = self
            .ctx_bar_rect
            .unwrap_or_else(|| egui::Rect::from_center_size(ctx.screen_rect().center(), egui::vec2(1.0, 1.0)));
        // Breite wie `#trpop` in `style.css` (Mindestbreite 280 px).
        let closed = pop_at(ctx, "mini_tr_pop", anchor, 280.0, &pal, |ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            ui.horizontal(|ui| {
                // `.trpop .pill` setzt `min-width:0`: die Pillen sind so breit
                // wie ihre Beschriftung, nicht wie in der Kopfzeile der Seite.
                let r = pill(ui, &pal, &src, 0.0);
                if r.clicked() {
                    self.lang_search.clear();
                    self.popup_lang = Some("src");
                    self.lang_anchor = Some(r.rect);
                }
                let (icon_rect, _) =
                    ui.allocate_exact_size(egui::vec2(16.0, 16.0), egui::Sense::hover());
                crate::icons::paint(ui.painter(), icon_rect, "arrowR", pal.muted);
                let r = pill(ui, &pal, &tgt, 0.0);
                if r.clicked() {
                    self.lang_search.clear();
                    self.popup_lang = Some("tgt");
                    self.lang_anchor = Some(r.rect);
                }
            });
            if llm {
                for (label, opts, field) in [
                    (
                        "l_formal",
                        [("formal", "hat"), ("standard", "dash"), ("informal", "smile")].as_slice(),
                        1,
                    ),
                    (
                        "l_writer",
                        [("unknown", "qmark"), ("neutral", "nb"), ("male", "male"), ("female", "female")].as_slice(),
                        2,
                    ),
                    (
                        "l_gender",
                        [("unknown", "qmark"), ("neutral", "nb"), ("male", "male"), ("female", "female")].as_slice(),
                        3,
                    ),
                ] {
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        ui.add_sized(
                            egui::vec2(78.0, 24.0),
                            egui::Label::new(RichText::new(t(lang, label)).size(12.0).color(pal.muted)),
                        );
                        for (value, icon) in opts.iter() {
                            let current = match field {
                                1 => &self.set.style.formal,
                                2 => &self.set.style.writer,
                                _ => &self.set.style.gender,
                            };
                            let on = current == value;
                            if icon_btn_sized(ui, &pal, icon, on, 32.0, 999.0, true)
                                .on_hover_text(t(lang, &format!("o_{}", value)))
                                .clicked()
                            {
                                match field {
                                    1 => self.set.style.formal = value.to_string(),
                                    2 => self.set.style.writer = value.to_string(),
                                    _ => self.set.style.gender = value.to_string(),
                                }
                                self.save_set();
                            }
                        }
                    });
                }
            }
            // `.go` in `style.css`: Abstand nach oben 10 px, der Knopf rechts.
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    // `.primary`: Polster 8 px oben/unten und 22 px seitlich,
                    // Schrift 15 px halbfett — kein festes Mindestmass.
                    ui.spacing_mut().button_padding = egui::vec2(22.0, 8.0);
                    let btn = egui::Button::new(
                        RichText::new(t(lang, "translate"))
                            .color(pal.on_accent)
                            .size(15.0)
                            .strong(),
                    )
                    .fill(pal.accent)
                    .stroke(Stroke::NONE)
                    .rounding(Rounding::same(999.0));
                    // Waehrend der Uebersetzung bleibt der Knopf stehen, nur
                    // blass; den Ladekreis zeigt die kleine Pille an der
                    // Kontextleiste wie `.ctxbar.working` in der Vorlage.
                    if self.busy {
                        ui.add_enabled(false, btn);
                    } else if ui.add(btn).clicked() {
                        go = true;
                    }
                });
            });
        });
        if go {
            self.start_mini_translate();
        }
        if closed {
            self.show_mini = false;
        }
    }

    fn spell_window(&mut self, ctx: &egui::Context) {
        if !self.show_spell {
            return;
        }
        let pal = self.pal;
        let lang = self.lang;
        let mut open = true;
        let mut recheck = false;
        let mut add_word: Option<String> = None;
        let mut ignore_word: Option<String> = None;
        let mut replace: Option<(String, String)> = None;
        let hits = self.spell_hits.clone();
        let error = self.spell_error.clone();
        let enabled = self.set.ed_spell;
        let title = t(lang, "spell_title");
        dialog(ctx, "spell_win", &title, 520.0, &pal, &mut open, |ui| {
                ui.horizontal(|ui| {
                    ui.checkbox(&mut self.set.ed_spell, t(lang, "spell_on"));
                    ui.label(
                        RichText::new(lang_label(&self.set.dict_lang, &self.nllb_langs))
                            .size(12.0)
                            .color(pal.muted),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button(t(lang, "spell_recheck")).clicked() {
                            recheck = true;
                        }
                    });
                });
                ui.separator();
                if !enabled {
                    ui.label(RichText::new(t(lang, "spell_off_hint")).color(pal.muted));
                    return;
                }
                if let Some(code) = &error {
                    ui.label(
                        RichText::new(t(lang, "spell_missing"))
                            .strong()
                            .color(Color32::from_rgb(0xb9, 0x1c, 0x1c)),
                    );
                    ui.label(
                        RichText::new(
                            t(lang, "spell_missing_hint").replace("<sprache>", code),
                        )
                        .size(12.0)
                        .color(pal.muted),
                    );
                    return;
                }
                if hits.is_empty() {
                    ui.label(RichText::new(t(lang, "spell_no_errors")).color(pal.muted));
                    return;
                }
                egui::ScrollArea::vertical()
                    .id_source("spell_list")
                    .max_height(420.0)
                    .show(ui, |ui| {
                        for hit in &hits {
                            egui::Frame::none()
                                .fill(pal.soft)
                                .rounding(Rounding::same(10.0))
                                .inner_margin(egui::Margin::same(8.0))
                                .show(ui, |ui| {
                                    ui.horizontal_wrapped(|ui| {
                                        ui.label(
                                            RichText::new(&hit.word)
                                                .size(14.0)
                                                .color(Color32::from_rgb(0xb9, 0x1c, 0x1c))
                                                .underline(),
                                        );
                                        ui.label(
                                            RichText::new(format!("×{}", hit.count))
                                                .size(11.0)
                                                .color(pal.muted),
                                        );
                                        if !hit.suggestions.is_empty() {
                                            ui.label(
                                                RichText::new(t(lang, "suggestions"))
                                                    .size(11.0)
                                                    .color(pal.muted),
                                            );
                                        }
                                        for s in &hit.suggestions {
                                            if ui
                                                .add(
                                                    egui::Button::new(
                                                        RichText::new(s).size(13.0),
                                                    )
                                                    .fill(pal.card)
                                                    .rounding(Rounding::same(999.0)),
                                                )
                                                .on_hover_text(t(lang, "replace_all"))
                                                .clicked()
                                            {
                                                replace = Some((hit.word.clone(), s.clone()));
                                            }
                                        }
                                    });
                                    ui.horizontal(|ui| {
                                        if ui.button(t(lang, "add_to_dict")).clicked() {
                                            add_word = Some(hit.word.clone());
                                        }
                                        if ui.button(t(lang, "ignore")).clicked() {
                                            ignore_word = Some(hit.word.clone());
                                        }
                                    });
                                });
                            ui.add_space(4.0);
                        }
                    });
            });

        if let Some((from, to)) = replace {
            self.replace_word_all(&from, &to);
            self.rescan_spell();
        }
        if let Some(word) = add_word {
            if !self.dict_words.iter().any(|w| w.to_lowercase() == word.to_lowercase()) {
                self.dict_words.push(word);
                self.save_personal_dict();
            }
            self.rescan_spell();
        }
        if let Some(word) = ignore_word {
            self.spell_ignored.push(word.to_lowercase());
            self.rescan_spell();
        }
        if recheck {
            self.spell_scanned_doc.clear();
            self.rescan_spell();
        }
        self.save_set();
        if !open {
            self.show_spell = false;
        }
    }

    /// Wörterbuch-Menü der Statusleiste: wie `#pop` in `editor.js` oberhalb
    /// des Knopfes, mit den Wörterbüchern und dem Schalter für die Prüfung.
    fn dict_menu(&mut self, ctx: &egui::Context) {
        if !self.show_spell_lang {
            return;
        }
        let pal = self.pal;
        let lang = self.lang;
        let anchor = self.dict_anchor.unwrap_or_else(|| {
            egui::Rect::from_min_size(egui::pos2(200.0, 700.0), egui::vec2(120.0, 24.0))
        });
        let breite = 240.0;
        let mut pick: Option<String> = None;
        let mut umschalten = false;
        let antwort = egui::Area::new(egui::Id::new("dict_pop"))
            .order(egui::Order::Foreground)
            .fixed_pos(egui::pos2(anchor.right(), anchor.top() - 8.0))
            .pivot(egui::Align2::RIGHT_BOTTOM)
            .show(ctx, |ui| {
                ui.set_width(breite);
                pop_card(&pal).show(ui, |ui| {
                    ui.set_width(breite - 12.0);
                    ui.spacing_mut().item_spacing.y = 0.0;
                    for (code, name) in DICTS {
                        if row_zeile(ui, &pal, name, self.set.dict_lang == code, false) {
                            pick = Some(code.to_string());
                        }
                    }
                    ui.add_space(4.0);
                    let (linie, _) = ui.allocate_exact_size(
                        egui::vec2(breite - 12.0, 1.0),
                        egui::Sense::hover(),
                    );
                    ui.painter().rect_filled(linie, 0.0, pal.line);
                    ui.add_space(4.0);
                    if row_zeile(ui, &pal, &t(lang, "spell_on"), false, self.set.ed_spell) {
                        umschalten = true;
                    }
                });
            });
        let rect = antwort.response.rect;
        let ausserhalb = ctx.input(|i| {
            i.pointer.button_clicked(egui::PointerButton::Primary)
                && i.pointer
                    .interact_pos()
                    .map(|pos| !rect.contains(pos) && !anchor.contains(pos))
                    .unwrap_or(false)
        });
        if ausserhalb || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.show_spell_lang = false;
        }
        if let Some(code) = pick {
            self.set.dict_lang = code;
            self.spell_scanned_doc.clear();
            self.save_set();
            self.show_spell_lang = false;
        }
        if umschalten {
            self.set.ed_spell = !self.set.ed_spell;
            self.spell_scanned_doc.clear();
            self.save_set();
            self.show_spell_lang = false;
        }
    }

    // ---- Dokumentenablage -------------------------------------------------
    fn library(&mut self, ui: &mut egui::Ui) {
        let pal = self.pal;
        let lang = self.lang;
        ui.spacing_mut().item_spacing.y = 0.0;

        // Kopfzeile `.libhead`: 20 px Titel, rechts die Anzahl. Feste
        // Zeilenhoehe 30 px (20 px Schrift, Zeilenhoehe 1.5).
        ui.add_space(4.0);
        let (head, _) = ui.allocate_exact_size(
            egui::vec2(ui.available_width(), 30.0),
            egui::Sense::hover(),
        );
        let title = t(lang, "nav_lib");
        let tg = ui.painter().layout_no_wrap(
            title,
            egui::FontId::proportional(20.0),
            pal.text,
        );
        let tpos = egui::pos2(
            head.left() + 6.0,
            head.center().y - tg.size().y / 2.0,
        );
        ui.painter()
            .galley(tpos + egui::vec2(0.6, 0.0), tg.clone(), pal.text);
        ui.painter().galley(tpos, tg, pal.text);
        if !self.lib.is_empty() {
            let count = format!("{} {}", self.lib.len(), t(lang, "lib_docs"));
            let cg = ui.painter().layout_no_wrap(
                count,
                egui::FontId::proportional(12.5),
                pal.muted,
            );
            ui.painter().galley(
                egui::pos2(
                    head.right() - 6.0 - cg.size().x,
                    head.center().y - cg.size().y / 2.0,
                ),
                cg,
                pal.muted,
            );
        }
        ui.add_space(4.0);
        ui.add_space(8.0);

        if self.lib.is_empty() {
            ui.vertical_centered(|ui| {
                ui.add_space(80.0);
                ui.label(
                    RichText::new(t(lang, "lib_empty"))
                        .size(15.0)
                        .color(pal.muted),
                );
                ui.add_space(6.0);
                ui.label(
                    RichText::new(t(lang, "lib_empty_hint"))
                        .size(13.0)
                        .color(pal.muted),
                );
            });
            return;
        }

        let items = self.lib.clone();
        let mut open_doc: Option<String> = None;
        let mut del: Option<(String, String)> = None;

        // Raster wie `.libgrid`: Spalten mit mindestens 210 px, 16 px
        // Abstand, 6 px Rand an den Seiten.
        let avail = (ui.available_width() - 12.0).max(210.0);
        let cols = (((avail + 16.0) / 226.0).floor() as usize).max(1);
        let card_w = (avail - (cols.saturating_sub(1)) as f32 * 16.0) / cols as f32;

        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.add_space(4.0);
                let mut idx = 0usize;
                while idx < items.len() {
                    let row_end = (idx + cols).min(items.len());
                    let row = &items[idx..row_end];
                    // Hoehe der Namenszeile je Karte (hoechstens zwei
                    // Zeilen), damit alle Karten einer Reihe gleich hoch
                    // sind wie im Raster der Vorlage.
                    let mut metas = Vec::new();
                    for meta in row {
                        let name = ui.painter().layout(
                            meta.name.clone(),
                            egui::FontId::proportional(15.0),
                            pal.text,
                            card_w - 28.0,
                        );
                        let rows = name.rows.len().min(2);
                        let h = 10.0 + rows as f32 * 19.5 + 2.0 + 18.0 + 12.0;
                        metas.push((name, h));
                    }
                    let meta_h = metas
                        .iter()
                        .map(|(_, h)| *h)
                        .fold(0.0_f32, f32::max);
                    let card_h = 1.0 + 168.0 + meta_h + 1.0;
                    let mut row_open: Option<String> = None;
                    let mut row_del: Option<String> = None;
                    ui.horizontal_top(|ui| {
                        ui.spacing_mut().item_spacing.x = 0.0;
                        ui.add_space(6.0);
                        for (k, meta) in row.iter().enumerate() {
                            if k > 0 {
                                ui.add_space(16.0);
                            }
                            let (rect, resp) = ui.allocate_exact_size(
                                egui::vec2(card_w, card_h),
                                egui::Sense::click(),
                            );
                            let hovered = resp.hovered();
                            let painter = ui.painter().clone();
                            let stroke = if hovered {
                                Stroke::new(1.0_f32, pal.accent)
                            } else {
                                Stroke::new(1.0_f32, pal.line)
                            };
                            painter.rect_filled(rect, Rounding::same(18.0), pal.card);
                            painter.rect_stroke(rect, Rounding::same(18.0), stroke);
                            let prev = egui::Rect::from_min_size(
                                rect.min + egui::vec2(1.0, 1.0),
                                egui::vec2(rect.width() - 2.0, 168.0),
                            );
                            painter.rect_filled(
                                prev,
                                Rounding {
                                    nw: 17.0,
                                    ne: 17.0,
                                    sw: 0.0,
                                    se: 0.0,
                                },
                                pal.soft,
                            );
                            // Weisse Vorschauseite wie `.libprev .doc`.
                            let page = egui::Rect::from_min_size(
                                prev.min + egui::vec2(14.0, 14.0),
                                egui::vec2(prev.width() - 28.0, prev.height() - 28.0),
                            );
                            painter.rect_filled(page, Rounding::same(8.0), pal.card);
                            let body = meta.text.clone().unwrap_or_default();
                            preview_text(
                                &painter,
                                page.shrink2(egui::vec2(16.0, 14.0)),
                                &body,
                                &pal,
                            );
                            // Weicher Abschluss unten (`.libprev::after`).
                            let fade_top = prev.bottom() - 44.0;
                            for band in 0..8 {
                                let a = band as f32 / 7.0;
                                let y = fade_top + band as f32 * 5.5;
                                painter.rect_filled(
                                    egui::Rect::from_min_size(
                                        egui::pos2(prev.left(), y),
                                        egui::vec2(prev.width(), 5.5),
                                    ),
                                    Rounding::ZERO,
                                    pal.soft.gamma_multiply(a * 0.85),
                                );
                            }
                            // Trennlinie und Metadaten.
                            let meta_top = prev.bottom();
                            painter.line_segment(
                                [
                                    egui::pos2(rect.left() + 1.0, meta_top),
                                    egui::pos2(rect.right() - 1.0, meta_top),
                                ],
                                Stroke::new(1.0_f32, pal.line),
                            );
                            let (name, _) = &metas[k];
                            let tx = rect.left() + 14.0;
                            let name_pos =
                                egui::pos2(tx, meta_top + 10.0);
                            let clip = egui::Rect::from_min_max(
                                egui::pos2(tx, meta_top + 10.0),
                                egui::pos2(
                                    rect.right() - 14.0,
                                    meta_top + 10.0 + 39.0,
                                ),
                            );
                            painter
                                .with_clip_rect(clip)
                                .galley(name_pos + egui::vec2(0.4, 0.0), name.clone(), pal.text);
                            painter
                                .with_clip_rect(clip)
                                .galley(name_pos, name.clone(), pal.text);
                            let date_pos = egui::pos2(
                                tx,
                                meta_top + 10.0 + name.rows.len().min(2) as f32 * 19.5 + 2.0,
                            );
                            painter.text(
                                date_pos,
                                egui::Align2::LEFT_TOP,
                                format_datetime(meta.updated),
                                egui::FontId::proportional(12.0),
                                pal.muted,
                            );

                            // Entfernen-Kreuz erscheint beim Ueberfahren.
                            if hovered {
                                let rm = egui::Rect::from_min_size(
                                    egui::pos2(rect.right() - 30.0, rect.top() + 8.0),
                                    egui::vec2(22.0, 22.0),
                                );
                                painter.rect_filled(rm, Rounding::same(11.0), pal.text);
                                crate::icons::paint(
                                    &painter,
                                    egui::Rect::from_center_size(
                                        rm.center(),
                                        egui::vec2(11.0, 11.0),
                                    ),
                                    "x",
                                    pal.bg,
                                );
                                let rm_resp = ui.interact(
                                    rm,
                                    egui::Id::new(("lib_rm", &meta.id)),
                                    egui::Sense::click(),
                                );
                                if rm_resp
                                    .on_hover_cursor(egui::CursorIcon::PointingHand)
                                    .clicked()
                                {
                                    row_del = Some(meta.id.clone());
                                }
                            }
                            if resp
                                .on_hover_cursor(egui::CursorIcon::PointingHand)
                                .clicked()
                                && row_del.is_none()
                            {
                                row_open = Some(meta.id.clone());
                            }
                            idx += 1;
                        }
                    });
                    if let Some(id) = row_del {
                        del = Some((id, String::new()));
                    }
                    if let Some(id) = row_open {
                        open_doc = Some(id);
                    }
                    ui.add_space(16.0);
                }
                ui.add_space(4.0);
            });

        if let Some(id) = open_doc {
            if let Some((_meta, text)) = library::read(&id) {
                self.doc = text;
                self.doc_id = Some(id.clone());
                self.set.doc_id = Some(id);
                self.set_view(View::Editor);
            }
        }
        if let Some((id, name)) = del {
            let name = if name.is_empty() {
                self.lib
                    .iter()
                    .find(|m| m.id == id)
                    .map(|m| m.name.clone())
                    .unwrap_or_default()
            } else {
                name
            };
            let text = format!("{} – {}", t(lang, "lib_delete_q"), name);
            self.ask_confirm(text, Confirm::LibDelete(id));
        }
    }
}

/// Zeichnet den Anfang eines Dokuments in die Vorschauseite der Ablage.
/// Ueberschriften werden wie in `.libprev .doc` groesser gesetzt; alles
/// andere bleibt bei 11 px. Der Bereich wird beschnitten, damit nichts
/// ueber die weisse Seite laeuft.
/// Vorschau des Dokumenttexts in einer Groesse relativ zu `basis` (11 px ist
/// die Groesse in der Dateiablage).
fn preview_text_at(
    painter: &egui::Painter,
    rect: egui::Rect,
    text: &str,
    p: &Palette,
    basis: f32,
) {
    let clip = painter.with_clip_rect(rect);
    let faktor = basis / 11.0;
    let mut y = rect.top();
    for line in text.lines() {
        if y >= rect.bottom() {
            break;
        }
        let trimmed = line.trim_end();
        if trimmed.trim().is_empty() {
            y += 5.0 * faktor;
            continue;
        }
        let (size, body, heading) = if let Some(r) = trimmed.strip_prefix("### ") {
            (12.0 * faktor, r, true)
        } else if let Some(r) = trimmed.strip_prefix("## ") {
            (13.0 * faktor, r, true)
        } else if let Some(r) = trimmed.strip_prefix("# ") {
            (15.0 * faktor, r, true)
        } else {
            (basis, trimmed, false)
        };
        let galley = clip.layout(
            body.to_string(),
            egui::FontId::proportional(size),
            p.text,
            rect.width(),
        );
        clip.galley(egui::pos2(rect.left(), y), galley.clone(), p.text);
        y += galley.size().y + if heading { 4.0 * faktor } else { 2.0 * faktor };
    }
}

fn preview_text(painter: &egui::Painter, rect: egui::Rect, text: &str, p: &Palette) {
    let clip = painter.with_clip_rect(rect);
    let mut y = rect.top();
    for line in text.lines() {
        if y >= rect.bottom() {
            break;
        }
        let trimmed = line.trim_end();
        if trimmed.trim().is_empty() {
            y += 5.0;
            continue;
        }
        let (size, body, heading) = if let Some(r) = trimmed.strip_prefix("### ") {
            (12.0, r, true)
        } else if let Some(r) = trimmed.strip_prefix("## ") {
            (13.0, r, true)
        } else if let Some(r) = trimmed.strip_prefix("# ") {
            (15.0, r, true)
        } else {
            (11.0, trimmed, false)
        };
        let galley = clip.layout(
            body.to_string(),
            egui::FontId::proportional(size),
            p.text,
            rect.width(),
        );
        let pos = egui::pos2(rect.left(), y);
        if heading {
            clip.galley(pos + egui::vec2(0.4, 0.0), galley.clone(), p.text);
        }
        clip.galley(pos, galley.clone(), p.text);
        y += galley.size().y + if heading { 4.0 } else { 2.0 };
    }
}

fn short_model(name: &str) -> String {
    if name.is_empty() {
        "—".into()
    } else {
        name.chars().take(26).collect()
    }
}

fn section(ui: &mut egui::Ui, pal: &Palette, label: &str) {
    // `.sect` in `style.css`: 18 px oben, 6 px unten, 13 px, gesperrt.
    ui.add_space(16.0);
    ui.label(
        RichText::new(label.to_uppercase())
            .size(12.0)
            .color(pal.muted),
    );
    ui.add_space(6.0);
}

/// Stil von `.tsel` und `.set select` aus `style.css`: runde Pille auf weichem
/// Grund, keine Umrandung, 13 px Schrift. Auch der gewaehlte Eintrag im
/// aufgeklappten Menue bleibt dunkel und bekommt nur einen Akzentrahmen —
/// sonst leuchtet die Auswahl im dunklen Modus zu hell.
fn tsel_scope(ui: &mut egui::Ui, pal: &Palette, add: impl FnOnce(&mut egui::Ui)) {
    ui.scope(|ui| {
        let st = ui.style_mut();
        st.spacing.button_padding = egui::vec2(10.0, 6.0);
        for w in [
            &mut st.visuals.widgets.inactive,
            &mut st.visuals.widgets.hovered,
            &mut st.visuals.widgets.active,
            &mut st.visuals.widgets.open,
        ] {
            w.bg_fill = pal.soft;
            w.weak_bg_fill = pal.soft;
            w.bg_stroke = Stroke::NONE;
            w.rounding = Rounding::same(999.0);
        }
        st.visuals.selection.bg_fill = pal.soft;
        st.visuals.selection.stroke = Stroke::new(1.0_f32, pal.accent);
        add(ui);
    });
}

/// Auswahlfeld im Stil von `.tsel`.
fn tsel_combo(
    ui: &mut egui::Ui,
    pal: &Palette,
    id: &str,
    text: &str,
    add: impl FnOnce(&mut egui::Ui),
) {
    tsel_scope(ui, pal, |ui| {
        egui::ComboBox::from_id_source(id)
            .selected_text(RichText::new(text).size(13.0).color(pal.text))
            .show_ui(ui, add);
    });
}

/// Zeile wie `.set.row-set`: Titel fett links, Auswahl rechts.
fn set_row(ui: &mut egui::Ui, pal: &Palette, label: &str, add: impl FnOnce(&mut egui::Ui)) {
    let breite = ui.available_width();
    ui.allocate_ui_with_layout(
        egui::vec2(breite, 36.0),
        egui::Layout::left_to_right(egui::Align::Center),
        |ui| {
            ui.label(RichText::new(label).size(14.0).strong().color(pal.text));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| add(ui));
        },
    );
    ui.add_space(2.0);
}

/// Einstellung mit Auswahlliste, gesetzt ueber `value`.
fn set_select<K: AsRef<str>>(
    ui: &mut egui::Ui,
    pal: &Palette,
    id: &str,
    label: &str,
    value: &mut String,
    options: &[(K, String)],
) {
    let current = options
        .iter()
        .find(|(v, _)| v.as_ref() == value.as_str())
        .map(|(_, l)| l.clone())
        .unwrap_or_default();
    set_row(ui, pal, label, |ui| {
        tsel_combo(ui, pal, id, &current, |ui| {
            for (v, l) in options {
                ui.selectable_value(value, v.as_ref().to_string(), RichText::new(l).size(13.0));
            }
        });
    });
}

/// Schalter wie in der Vorlage: kein Kaestchen, sondern „An“/„Aus“ als Liste.
fn set_bool(
    ui: &mut egui::Ui,
    pal: &Palette,
    id: &str,
    lang: Lang,
    label: &str,
    value: &mut bool,
) {
    let mut current = if *value { "on".to_string() } else { "off".to_string() };
    let options = [("on", t(lang, "on")), ("off", t(lang, "off"))];
    set_select(ui, pal, id, label, &mut current, &options);
    let neu = current == "on";
    if neu != *value {
        *value = neu;
    }
}

/// Zahleneinstellung wie `<input type=number>`: Titel links, Feld rechts.
fn set_number(ui: &mut egui::Ui, pal: &Palette, label: &str, add: impl FnOnce(&mut egui::Ui)) {
    set_row(ui, pal, label, |ui| {
        egui::Frame::none()
            .fill(pal.soft)
            .rounding(Rounding::same(12.0))
            .inner_margin(egui::Margin::symmetric(10.0, 5.0))
            .show(ui, |ui| add(ui));
    });
}

/// Textfeld wie `.set input`: Titel fett darueber, Feld auf weichem Grund.
fn set_field(ui: &mut egui::Ui, pal: &Palette, label: &str, add: impl FnOnce(&mut egui::Ui)) {
    ui.add_space(8.0);
    ui.label(RichText::new(label).size(14.0).strong().color(pal.text));
    ui.add_space(4.0);
    egui::Frame::none()
        .fill(pal.soft)
        .rounding(Rounding::same(12.0))
        .inner_margin(egui::Margin::symmetric(10.0, 6.0))
        .show(ui, |ui| add(ui));
    ui.add_space(4.0);
}

/// Wie `libDate` in `editor.js`: Datum und Uhrzeit in der Ortszeit.
fn format_datetime(secs: u64) -> String {
    let local = secs as i64 + local_offset_secs();
    let days = local.div_euclid(86_400);
    let sod = local.rem_euclid(86_400);
    let (d, m, y) = civil_from_days(days);
    format!(
        "{:02}.{:02}.{}, {:02}:{:02}",
        d,
        m,
        y,
        sod / 3600,
        (sod % 3600) / 60
    )
}

fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (d, m, y)
}

fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = if m > 2 { m - 3 } else { m + 9 };
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// Abstand der Ortszeit zu UTC in Sekunden. Unter Windows ueber
/// `GetLocalTime`, sonst 0 (UTC). Sommerzeit wird fuer das aktuelle Datum
/// bestimmt; alte Ablagedokumente koennen dadurch eine Stunde abweichen.
#[cfg(windows)]
fn local_offset_secs() -> i64 {
    #[repr(C)]
    struct SystemTime {
        year: u16,
        month: u16,
        _day_of_week: u16,
        day: u16,
        hour: u16,
        minute: u16,
        second: u16,
        _millis: u16,
    }
    extern "system" {
        fn GetLocalTime(t: *mut SystemTime);
    }
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let mut st = SystemTime {
        year: 0,
        month: 0,
        _day_of_week: 0,
        day: 0,
        hour: 0,
        minute: 0,
        second: 0,
        _millis: 0,
    };
    unsafe { GetLocalTime(&mut st) };
    let days = days_from_civil(st.year as i64, st.month as i64, st.day as i64);
    let local = days * 86_400
        + st.hour as i64 * 3600
        + st.minute as i64 * 60
        + st.second as i64;
    local - now
}

#[cfg(not(windows))]
fn local_offset_secs() -> i64 {
    0
}

#[cfg(test)]
mod app_tests {
    use super::chunk_text;

    #[test]
    fn kurzer_text_bleibt_ein_stueck() {
        let teile = chunk_text("Ein Absatz.
Noch einer.", 3500);
        assert_eq!(teile.len(), 1);
    }

    #[test]
    fn langer_text_wird_zerlegt() {
        let absatz = "Wort Wort Wort Wort Wort Wort Wort Wort.
";
        let text = absatz.repeat(200);
        let teile = chunk_text(&text, 500);
        assert!(teile.len() > 1);
        for teil in &teile {
            assert!(teil.chars().count() <= 560, "Teil zu gross: {}", teil.chars().count());
        }
        let zusammen: String = teile.concat();
        assert_eq!(zusammen.matches("Wort").count(), text.matches("Wort").count());
    }
}
