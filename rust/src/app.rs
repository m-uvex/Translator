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
/// Offenes Rechtsklick-Menue zu einem falsch geschriebenen Wort.
#[derive(Clone)]
pub struct SpellMenu {
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
    spell: crate::spell::Spell,
    spell_hits: Vec<crate::spell::Misspelled>,
    spell_error: Option<String>,
    spell_scanned_doc: String,
    spell_scan_at: f64,
    show_spell: bool,
    show_spell_lang: bool,
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
            spell: crate::spell::Spell::new(),
            spell_hits: Vec::new(),
            spell_error: None,
            spell_scanned_doc: String::new(),
            spell_scan_at: 0.0,
            show_spell: false,
            show_spell_lang: false,
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
                Msg::Done(text) => {
                    self.busy = false;
                    if self.syn_job {
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
                        let clean = text.trim_end().to_string();
                        if self.pending_diff {
                            self.pending_diff = false;
                            self.diff = Some(DiffState::new(&self.doc, &clean));
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
                    self.last_tr = self.output.clone();
                    self.status = t(self.lang, "stopped");
                }
                Msg::Error(e) => {
                    self.busy = false;
                    self.editor_job = false;
                    self.mini_job = false;
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
        let instr = match kind {
            "shorten" => "Shorten the following text by about one third. Keep the meaning, tone and key facts. Output only the shortened text, in the same language as the input, without explanations.",
            "lengthen" => "Expand the following text by about 50%: add useful detail, explanation and smoother transitions, but do not invent facts. Output only the expanded text, in the same language as the input.",
            "summarize" => "Summarize the following text concisely in one to three sentences. Output only the summary, in the same language as the input.",
            _ => "Proofread the following text. Correct only spelling, grammar, punctuation and obvious typos. Do not translate, do not change wording or style. Output only the corrected text.",
        };
        self.start_editor_instr(instr, kind == "proofread");
    }

    /// Wie `start_editor_op`, aber immer fuer das ganze Dokument. Die drei
    /// Knoepfe der Werkzeugleiste entsprechen `allOp` in der Vorlage.
    fn start_editor_all(&mut self, kind: &str) {
        let saved = self.sel;
        self.sel = None;
        self.start_editor_op(kind);
        self.sel = saved;
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
        self.status = t(self.lang, "translating");
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
            true,
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

/// Eine Zeile im Modellmenue: Name links, rechts Symbole fuer Bild- und
/// Denkfaehigkeit sowie die MT-Kennzeichnung. Entspricht `.row` in `style.css`.
fn menu_row(
    ui: &mut egui::Ui,
    p: &Palette,
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
        ui.painter().text(
            egui::pos2(x, rect.center().y),
            egui::Align2::RIGHT_CENTER,
            "MT",
            egui::FontId::proportional(11.0),
            p.muted,
        );
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

        self.lang_popup(ctx);
        self.settings_window(ctx);
        self.models_menu(ctx);
        self.dict_window(ctx);
        self.style_window(ctx);
        self.syn_window(ctx);
        self.confirm_window(ctx);
        self.diff_window(ctx);
        self.spell_window(ctx);
        self.spell_lang_window(ctx);
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
                                        .hint_text(t(lang, "input_ph"))
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
                                        ui.add(
                                            egui::Label::new(
                                                RichText::new(if self.output.is_empty() {
                                                    " ".to_string()
                                                } else {
                                                    filter_profanity(
                                                        &self.output,
                                                        &self.set.tgt,
                                                        &self.set.src,
                                                        &self.set.profanity_style,
                                                    )
                                                })
                                                .size(17.0)
                                                .color(pal.text),
                                            )
                                            .wrap(true),
                                        );
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
                                        .hint_text(t(lang, "ctx_ph")),
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
                                                        .hint_text(t(lang, "term")),
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
                                                        .hint_text(t(lang, "term_t")),
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
                ui.add(
                    egui::TextEdit::singleline(&mut self.lang_search)
                        .hint_text(t(lang, "search_lang")),
                );
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
                // Angepinnte zuerst, dann der Rest.
                rows.sort_by_key(|(_, _, pinned)| !*pinned);
                let mut shown = 0usize;
                egui::ScrollArea::vertical().max_height(360.0).show(ui, |ui| {
                    for (code, label, pinned) in rows {
                        shown += 1;
                        ui.horizontal(|ui| {
                            let on = code == current;
                            let text = if on {
                                RichText::new(format!("• {}", label)).color(pal.accent_ink)
                            } else {
                                RichText::new(&label).color(pal.text)
                            };
                            if ui.add(egui::Button::new(text).fill(Color32::TRANSPARENT)).clicked() {
                                pick = Some(code.clone());
                            }
                            if code != "auto" {
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    if ui
                                        .add(
                                            egui::Button::new(
                                                RichText::new(if pinned { "★" } else { "☆" })
                                                    .color(if pinned { pal.accent_ink } else { pal.muted }),
                                            )
                                            .fill(Color32::TRANSPARENT),
                                        )
                                        .on_hover_text(t(lang, "pin"))
                                        .clicked()
                                    {
                                        toggle_pin = Some(code.clone());
                                    }
                                });
                            }
                        });
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
                                        if menu_row(ui, &pal, name, sel, *vision, *thinking, false) {
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
                                        if menu_row(ui, &pal, name, sel, *vision, *thinking, *mt) {
                                            pick_trans = Some(name.clone());
                                        }
                                    }
                                } else {
                                    for (name, vision, thinking, mt) in &rows {
                                        let sel = self.set.model == *name;
                                        if menu_row(ui, &pal, name, sel, *vision, *thinking, *mt) {
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
        dialog(ctx, "settings_win", &t(lang, "settings"), 560.0, &pal, &mut open, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
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
                    set_field(ui, &pal, t(lang, "s_helper").as_str(), |ui| {
                        ui.add(
                            egui::TextEdit::singleline(&mut self.set.helper)
                                .frame(false)
                                .desired_width(f32::INFINITY),
                        );
                    });
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
    fn dict_window(&mut self, ctx: &egui::Context) {
        if !self.show_dict {
            return;
        }
        let pal = self.pal;
        let lang = self.lang;
        let mut open = true;
        let mut changed = false;
        dialog(ctx, "dict_win", &t(lang, "personal_dict"), 420.0, &pal, &mut open, |ui| {
                ui.label(RichText::new(t(lang, "dict_words")).color(pal.muted));
                if self.dict_words.is_empty() {
                    ui.label(RichText::new(t(lang, "no_dict_words")).color(pal.muted));
                }
                let mut remove: Option<usize> = None;
                egui::ScrollArea::vertical().max_height(300.0).show(ui, |ui| {
                    for (i, w) in self.dict_words.iter_mut().enumerate() {
                        ui.horizontal(|ui| {
                            if self.rename == Some(i) {
                                if ui.add(egui::TextEdit::singleline(w)).lost_focus() {
                                    self.rename = None;
                                    changed = true;
                                }
                            } else {
                                ui.label(&*w);
                            }
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if icon_btn(ui, &pal, "x", false).clicked() {
                                    remove = Some(i);
                                }
                                if icon_btn(ui, &pal, "pencil", false).clicked() {
                                    self.rename = Some(i);
                                }
                            });
                        });
                    }
                });
                if let Some(i) = remove {
                    self.dict_words.remove(i);
                    self.spell_scanned_doc.clear();
                    changed = true;
                }
                ui.separator();
                ui.horizontal(|ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut self.dict_new)
                            .hint_text(t(lang, "word_ph"))
                            .desired_width(240.0),
                    );
                    if ui.button(t(lang, "add_word")).clicked() && !self.dict_new.trim().is_empty() {
                        let word = self.dict_new.trim().to_string();
                        if self.dict_words.iter().any(|w| w.to_lowercase() == word.to_lowercase()) {
                            self.status = t(lang, "word_exists");
                        } else {
                            self.dict_words.push(word);
                            self.dict_new.clear();
                            self.spell_scanned_doc.clear();
                            changed = true;
                        }
                    }
                });
            });
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
                    ui.spinner();
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
            if icon_btn(ui, &pal, "save", false)
                .on_hover_text(t(lang, "save"))
                .clicked()
            {
                self.save_doc_to_tray(true);
            }
            if icon_btn(ui, &pal, "download", false)
                .on_hover_text(t(lang, "export"))
                .clicked()
            {
                self.export_document();
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
            if icon_btn(ui, &pal, "`", self.selection_wrapped("`", "`"))
                .on_hover_text("Code")
                .clicked()
            {
                self.format_wrap("`", "`");
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
        egui::Frame::none()
            .fill(pal.soft)
            .rounding(Rounding::same(20.0))
            .inner_margin(egui::Margin {
                left: 16.0,
                right: 16.0,
                top: 28.0,
                // Unten ein kleiner Rand, damit die Seite nicht ueber die
                // gerundete Kante der Flaeche haengt.
                bottom: 8.0,
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
                        let page_h = (h - 64.0).max(1040.0);
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
                            .hint_text("…")
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
                                        self.spell_menu = Some(SpellMenu { word, pos });
                                    }
                                }
                            }
                        } else {
                            self.spell_anchor = None;
                            self.spell_mark_rects.clear();
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
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if icon_btn(ui, &pal, "docs", self.show_sessions)
                    .on_hover_text(t(lang, "session_docs"))
                    .clicked()
                {
                    self.show_sessions = !self.show_sessions;
                }
                if icon_btn(ui, &pal, "book", false).on_hover_text(t(lang, "personal_dict")).clicked() {
                    self.show_dict = true;
                }
                if icon_btn(ui, &pal, "abc", self.show_spell && self.set.ed_spell)
                    .on_hover_text(t(lang, "spell_title"))
                    .clicked()
                {
                    self.show_spell = !self.show_spell;
                    self.spell_scanned_doc.clear();
                    if self.show_spell {
                        self.open_spell(ui.ctx().screen_rect());
                    }
                }
                if ui
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
                    .on_hover_text(t(lang, "dict"))
                    .clicked()
                {
                    self.show_spell_lang = true;
                }
                if self.set.ed_spell && !self.spell_hits.is_empty() {
                    ui.label(
                        RichText::new(format!("{}", self.spell_hits.len()))
                            .size(12.0)
                            .color(Color32::from_rgb(0xb9, 0x1c, 0x1c)),
                    )
                    .on_hover_text(t(lang, "spell_title"));
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
        egui::Frame::none()
            .fill(pal.soft)
            .stroke(Stroke::new(1.0_f32, pal.line))
            .rounding(Rounding::same(18.0))
            .inner_margin(egui::Margin::same(10.0))
            .show(ui, |ui| {
                egui::ScrollArea::horizontal()
                    .id_source("session_dock")
                    .max_height(120.0)
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            for i in 0..self.sessions.len() {
                                let active = i == self.active_session;
                                let name = self.sessions[i].name.clone();
                                let dirty = self.sessions[i].text != self.sessions[i].saved_text;
                                let fill = if active { pal.accent } else { pal.card };
                                let ink = if active { pal.on_accent } else { pal.text };
                                egui::Frame::none()
                                    .fill(fill)
                                    .stroke(Stroke::new(1.0_f32, pal.line))
                                    .rounding(Rounding::same(12.0))
                                    .inner_margin(egui::Margin::same(8.0))
                                    .show(ui, |ui| {
                                        ui.allocate_ui(egui::vec2(132.0, 74.0), |ui| {
                                            ui.vertical(|ui| {
                                                ui.horizontal(|ui| {
                                                    let text = RichText::new(name.chars().take(20).collect::<String>())
                                                        .size(12.0)
                                                        .color(ink);
                                                    if ui
                                                        .add(egui::Button::new(text).fill(Color32::TRANSPARENT))
                                                        .clicked()
                                                    {
                                                        switch = Some(i);
                                                    }
                                                    if icon_btn(ui, &pal, "x", false)
                                                        .on_hover_text(t(lang, "close_session"))
                                                        .clicked()
                                                    {
                                                        close = Some(i);
                                                    }
                                                });
                                                ui.label(
                                                    RichText::new(if dirty { "•" } else { " " })
                                                        .size(11.0)
                                                        .color(ink),
                                                );
                                            });
                                        });
                                    });
                                ui.add_space(4.0);
                            }
                        });
                    });
            });
        if let Some(i) = switch {
            self.switch_session(i);
        }
        if let Some(i) = close {
            let dirty = self
                .sessions
                .get(i)
                .map(|s| s.text != s.saved_text)
                .unwrap_or(false);
            if dirty && Some(i) != Some(self.active_session) {
                self.ask_confirm(t(lang, "discard_q"), Confirm::SessionClose(i));
            } else if dirty && i == self.active_session {
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
        if self.busy {
            return;
        }
        let pal = self.pal;
        let lang = self.lang;
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
                let dy = 1.4 * ((x - x0) * std::f32::consts::TAU / 6.0).sin();
                pts.push(egui::pos2(x, y + dy));
            }
            painter.add(egui::Shape::line(pts, Stroke::new(1.4_f32, color)));
        }
        self.spell_anchor = anchor;
    }

    /// Rechtsklick-Menue auf einem falsch geschriebenen Wort: Vorschlaege aus
    /// dem Woerterbuch, Ignorieren und Ins-Woerterbuch. Entspricht dem
    /// Kontextmenue der Electron-Fassung.
    fn spell_menu_window(&mut self, ctx: &egui::Context) {
        let Some(menu) = self.spell_menu.clone() else {
            return;
        };
        let pal = self.pal;
        let lang = self.lang;
        let sug: Vec<String> = self
            .spell_hits
            .iter()
            .find(|h| h.word == menu.word)
            .map(|h| h.suggestions.clone())
            .unwrap_or_default();
        let red = Color32::from_rgb(0xd0, 0x3a, 0x3a);
        let mut close = false;
        let mut pick: Option<String> = None;
        let mut ignore = false;
        let mut add = false;
        let screen = ctx.screen_rect();
        let pos = egui::pos2(
            menu.pos.x.clamp(8.0, (screen.right() - 220.0).max(8.0)),
            menu.pos.y.clamp(8.0, (screen.bottom() - 200.0).max(8.0)),
        );
        let area = egui::Area::new(egui::Id::new("spell_menu"))
            .order(egui::Order::Foreground)
            .fixed_pos(pos)
            .show(ctx, |ui| {
                egui::Frame::popup(&ctx.style()).show(ui, |ui| {
                    ui.set_min_width(190.0);
                    ui.label(RichText::new(&menu.word).size(13.0).color(red));
                    if sug.is_empty() {
                        ui.label(
                            RichText::new(t(lang, "suggestions"))
                                .size(12.0)
                                .color(pal.muted),
                        );
                    }
                    for s in &sug {
                        if ui.button(s).clicked() {
                            pick = Some(s.clone());
                        }
                    }
                    ui.separator();
                    if ui.button(t(lang, "ignore")).clicked() {
                        ignore = true;
                    }
                    if ui.button(t(lang, "add_to_dict")).clicked() {
                        add = true;
                    }
                });
            });
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            close = true;
        }
        if ctx.input(|i| i.pointer.button_clicked(egui::PointerButton::Primary))
            && !area.response.hovered()
        {
            close = true;
        }
        if let Some(s) = pick {
            self.replace_word_all(&menu.word, &s);
            self.rescan_spell();
            close = true;
        }
        if ignore {
            self.spell.ignore(&menu.word);
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

    fn export_document(&mut self) {
        let name = library::doc_name(&self.doc, &t(self.lang, "doc_name"));
        if let Some(path) = rfd::FileDialog::new()
            .set_file_name(format!("{}.md", name))
            .save_file()
        {
            let _ = std::fs::write(path, &self.doc);
            self.status = t(self.lang, "saved");
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
    fn diff_window(&mut self, ctx: &egui::Context) {
        let Some(diff) = self.diff.clone() else { return };
        let pal = self.pal;
        let lang = self.lang;
        let mut open = true;
        let mut accept_all = false;
        let mut discard_all = false;
        let mut toggle: Option<usize> = None;
        let mut apply = false;
        dialog(ctx, "diff_win", &t(lang, "diff_title"), 620.0, &pal, &mut open, |ui| {
                let hunks: Vec<(String, String)> = diff
                    .parts
                    .iter()
                    .filter_map(|p| match p {
                        Part::Hunk { old, new } => Some((old.clone(), new.clone())),
                        _ => None,
                    })
                    .collect();
                ui.label(
                    RichText::new(format!(
                        "{} {} · {} / {}",
                        hunks.len(),
                        t(lang, "diff_hunks"),
                        diff.accepted_count(),
                        hunks.len()
                    ))
                    .size(12.0)
                    .color(pal.muted),
                );
                ui.horizontal(|ui| {
                    if ui.button(t(lang, "accept_all")).clicked() {
                        accept_all = true;
                    }
                    if ui.button(t(lang, "discard_all")).clicked() {
                        discard_all = true;
                    }
                });
                ui.separator();
                if hunks.is_empty() {
                    ui.label(RichText::new(t(lang, "diff_empty")).color(pal.muted));
                }
                egui::ScrollArea::vertical()
                    .id_source("diff_list")
                    .max_height(300.0)
                    .show(ui, |ui| {
                        let green = Color32::from_rgb(0x1f, 0x7a, 0x3a);
                        let red = Color32::from_rgb(0xb9, 0x1c, 0x1c);
                        for (i, (old, new)) in hunks.iter().enumerate() {
                            let on = diff.accepted.get(i).copied().unwrap_or(true);
                            egui::Frame::none()
                                .fill(pal.soft)
                                .rounding(Rounding::same(10.0))
                                .inner_margin(egui::Margin::same(8.0))
                                .show(ui, |ui| {
                                    ui.horizontal_wrapped(|ui| {
                                        ui.label(
                                            RichText::new(old.trim())
                                                .size(13.0)
                                                .color(red)
                                                .strikethrough(),
                                        );
                                        ui.label(RichText::new("→").size(12.0).color(pal.muted));
                                        ui.label(RichText::new(new.trim()).size(13.0).color(green));
                                    });
                                    ui.horizontal(|ui| {
                                        let label = if on { t(lang, "discard") } else { t(lang, "accept") };
                                        if ui.button(label).clicked() {
                                            toggle = Some(i);
                                        }
                                        ui.label(
                                            RichText::new(if on {
                                                t(lang, "accept")
                                            } else {
                                                t(lang, "discard")
                                            })
                                            .size(11.0)
                                            .color(pal.muted),
                                        );
                                    });
                                });
                            ui.add_space(4.0);
                        }
                    });
                ui.separator();
                egui::ScrollArea::vertical()
                    .id_source("diff_preview")
                    .max_height(150.0)
                    .show(ui, |ui| {
                        ui.label(RichText::new(diff.result()).size(13.0).color(pal.text));
                    });
                ui.horizontal(|ui| {
                    if ui
                        .add(
                            egui::Button::new(
                                RichText::new(t(lang, "apply")).color(pal.on_accent),
                            )
                            .fill(pal.accent)
                            .rounding(Rounding::same(999.0)),
                        )
                        .clicked()
                    {
                        apply = true;
                    }
                    if ui.button(t(lang, "cancel")).clicked() {
                        self.diff = None;
                    }
                });
            });

        if let Some(d) = self.diff.as_mut() {
            if accept_all {
                d.set_all(true);
            }
            if discard_all {
                d.set_all(false);
            }
            if let Some(i) = toggle {
                if let Some(a) = d.accepted.get_mut(i) {
                    *a = !*a;
                }
            }
        }
        if apply {
            let result = self.diff.as_ref().map(|d| d.result()).unwrap_or_default();
            self.push_undo();
            self.doc = result;
            self.diff = None;
            self.saved = false;
            self.set.doc = self.doc.clone();
            self.save_set();
            self.sync_active_session();
        }
        if !open {
            self.diff = None;
        }
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
        let now = ctx.input(|i| i.time);
        if now - self.spell_scan_at < 0.5 {
            ctx.request_repaint_after(std::time::Duration::from_millis(250));
            return;
        }
        self.spell_scan_at = now;
        self.rescan_spell();
    }

    fn rescan_spell(&mut self) {
        let lang = self.set.dict_lang.clone();
        match self.spell.scan(&lang, &self.doc, &self.dict_words) {
            Ok(hits) => {
                self.spell_hits = hits;
                self.spell_error = None;
            }
            Err(code) => {
                self.spell_hits.clear();
                self.spell_error = Some(code);
            }
        }
        self.spell_scanned_doc = self.doc.clone();
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
        let closed = pop_at(ctx, "mini_tr_pop", anchor, 320.0, &pal, |ui| {
            ui.horizontal(|ui| {
                let r = pill(ui, &pal, &src, 90.0);
                if r.clicked() {
                    self.lang_search.clear();
                    self.popup_lang = Some("src");
                    self.lang_anchor = Some(r.rect);
                }
                let (icon_rect, _) =
                    ui.allocate_exact_size(egui::vec2(18.0, 18.0), egui::Sense::hover());
                crate::icons::paint(ui.painter(), icon_rect, "arrowR", pal.muted);
                let r = pill(ui, &pal, &tgt, 90.0);
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
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let btn = egui::Button::new(
                        RichText::new(t(lang, "translate")).color(pal.on_accent).size(13.0),
                    )
                    .fill(pal.accent)
                    .stroke(Stroke::NONE)
                    .rounding(Rounding::same(999.0))
                    .min_size(egui::vec2(120.0, 32.0));
                    if self.busy {
                        ui.spinner();
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
            self.spell.ignore(&word);
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

    fn spell_lang_window(&mut self, ctx: &egui::Context) {
        if !self.show_spell_lang {
            return;
        }
        let lang = self.lang;
        let pal = self.pal;
        let mut open = true;
        let mut pick: Option<String> = None;
        dialog(ctx, "spell_lang_win", &t(lang, "dict"), 260.0, &pal, &mut open, |ui| {
                ui.checkbox(&mut self.set.ed_spell, t(lang, "spell_on"));
                ui.separator();
                for (code, name) in DICTS {
                    let on = self.set.dict_lang == code;
                    if ui
                        .add(
                            egui::Button::new(
                                RichText::new(if on {
                                    format!("• {}", name)
                                } else {
                                    name.to_string()
                                })
                                .color(if on { self.pal.accent_ink } else { self.pal.text }),
                            )
                            .fill(Color32::TRANSPARENT),
                        )
                        .clicked()
                    {
                        pick = Some(code.to_string());
                    }
                }
                ui.separator();
                let missing: Vec<&str> = DICTS
                    .iter()
                    .filter(|(c, _)| !self.spell.has_dict(c))
                    .map(|(c, _)| *c)
                    .collect();
                if !missing.is_empty() {
                    ui.label(
                        RichText::new(format!(
                            "{}: {}",
                            t(lang, "spell_missing"),
                            missing.join(", ")
                        ))
                        .size(11.0)
                        .color(self.pal.muted),
                    );
                }
            });
        if let Some(code) = pick {
            self.set.dict_lang = code;
            self.spell_scanned_doc.clear();
            self.save_set();
        }
        self.save_set();
        if !open {
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

fn labeled(ui: &mut egui::Ui, label: &str, add: impl FnOnce(&mut egui::Ui)) {
    ui.label(RichText::new(label).size(13.0));
    add(ui);
    ui.add_space(4.0);
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
fn set_select(
    ui: &mut egui::Ui,
    pal: &Palette,
    id: &str,
    label: &str,
    value: &mut String,
    options: &[(&str, String)],
) {
    let current = options
        .iter()
        .find(|(v, _)| v == value)
        .map(|(_, l)| l.clone())
        .unwrap_or_default();
    set_row(ui, pal, label, |ui| {
        tsel_combo(ui, pal, id, &current, |ui| {
            for (v, l) in options {
                ui.selectable_value(value, (*v).to_string(), RichText::new(l).size(13.0));
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

fn combo(ui: &mut egui::Ui, id: &str, label: &str, value: &mut String, options: &[(&str, String)]) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(label).size(13.0));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let current = options
                .iter()
                .find(|(v, _)| v == value)
                .map(|(_, l)| l.clone())
                .unwrap_or_default();
            egui::ComboBox::from_id_source(id)
                .selected_text(current)
                .show_ui(ui, |ui| {
                    for (v, l) in options {
                        ui.selectable_value(value, (*v).to_string(), l.clone());
                    }
                });
        });
    });
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
