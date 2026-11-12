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

    syn_word: String,
    syn_out: Vec<String>,

    show_settings: bool,
    show_models: bool,
    show_dict: bool,
    show_style: bool,
    show_syn: bool,
    popup_lang: Option<&'static str>,
    lang_search: String,
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
            syn_word: String::new(),
            syn_out: Vec::new(),
            show_settings: false,
            show_models: false,
            show_dict: false,
            show_style: false,
            show_syn: false,
            popup_lang: None,
            lang_search: String::new(),
            confirm: None,
            set: set.clone(),
        };
        app.set = set;
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
            self.status = t(self.lang, "no_models");
        }
    }

    fn write_model(&self) -> Option<String> {
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
                    } else {
                        self.output = text;
                    }
                }
                Msg::Done(text) => {
                    self.busy = false;
                    if self.editor_job {
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
                    self.last_tr = self.output.clone();
                    self.status = t(self.lang, "stopped");
                }
                Msg::Error(e) => {
                    self.busy = false;
                    self.editor_job = false;
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
        let instr = match kind {
            "shorten" => "Shorten the following text by about one third. Keep the meaning, tone and key facts. Output only the shortened text, in the same language as the input, without explanations.",
            "lengthen" => "Expand the following text by about 50%: add useful detail, explanation and smoother transitions, but do not invent facts. Output only the expanded text, in the same language as the input.",
            "summarize" => "Summarize the following text concisely in one to three sentences. Output only the summary, in the same language as the input.",
            _ => "Proofread the following text. Correct only spelling, grammar, punctuation and obvious typos. Do not translate, do not change wording or style. Output only the corrected text.",
        };
        self.busy = true;
        self.editor_job = true;
        self.pending_diff = kind == "proofread";
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

fn pill(ui: &mut egui::Ui, p: &Palette, text: &str, min_w: f32) -> egui::Response {
    ui.add(
        egui::Button::new(RichText::new(text).color(p.text).size(14.0))
            .fill(p.card)
            .stroke(Stroke::new(1.0_f32, p.line))
            .rounding(Rounding::same(999.0))
            .min_size(egui::vec2(min_w, 32.0)),
    )
}

/// Knopf mit Vektorsymbol aus `icons_data.rs`. Ist kein Symbol vorhanden,
/// wird die Beschriftung als Text gezeichnet (etwa „H1“ oder „abc“).
fn icon_btn(ui: &mut egui::Ui, p: &Palette, name: &str, on: bool) -> egui::Response {
    let fill = if on { p.accent } else { p.soft };
    let ink = if on { p.on_accent } else { p.muted };
    if crate::icons::icon(name).is_none() {
        // Kein Symbol vorhanden (etwa „H1“): als Textknopf mit echter Breite.
        return ui.add(
            egui::Button::new(RichText::new(name).size(13.0).color(ink))
                .fill(fill)
                .stroke(Stroke::NONE)
                .rounding(Rounding::same(12.0))
                .min_size(egui::vec2(32.0, 32.0)),
        );
    }
    // Runde Knoepfe: bei quadratischer Groesse ergibt 999 einen Kreis.
    let resp = ui.add(
        egui::Button::new("")
            .fill(fill)
            .stroke(Stroke::NONE)
            .rounding(Rounding::same(999.0))
            .min_size(egui::vec2(32.0, 32.0)),
    );
    crate::icons::paint(ui.painter(), resp.rect.shrink(8.0), name, ink);
    resp
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
            .frame(egui::Frame::none().fill(pal.bg).inner_margin(egui::Margin::symmetric(10.0, 0.0)))
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
                            if icon_btn(ui, &pal, "x", false)
                                .on_hover_text(t(lang, "win_close"))
                                .clicked()
                            {
                                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                            }
                            if icon_btn(ui, &pal, "winMax", false)
                                .on_hover_text(t(lang, "win_max"))
                                .clicked()
                            {
                                let maximized = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
                                ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(!maximized));
                            }
                            if icon_btn(ui, &pal, "winMin", false)
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
            .exact_width(56.0)
            .resizable(false)
            .frame(egui::Frame::none().fill(pal.bg).inner_margin(egui::Margin::symmetric(10.0, 10.0)))
            .show(ctx, |ui| {
                ui.vertical(|ui| {
                    let both = self.set.workspace == "both";
                    if both || self.set.workspace == "tr" {
                        if icon_btn(ui, &pal, "swap", self.view == View::Translator)
                            .on_hover_text(t(lang, "nav_tr"))
                            .clicked()
                        {
                            self.set_view(View::Translator);
                        }
                    }
                    if both || self.set.workspace == "ed" {
                        if icon_btn(ui, &pal, "pencil", self.view == View::Editor)
                            .on_hover_text(t(lang, "nav_ed"))
                            .clicked()
                        {
                            self.set_view(View::Editor);
                        }
                        if icon_btn(ui, &pal, "library", self.view == View::Library)
                            .on_hover_text(t(lang, "nav_lib"))
                            .clicked()
                        {
                            self.set_view(View::Library);
                        }
                    }
                    ui.add_space(ui.available_height() - 40.0);
                    if icon_btn(ui, &pal, "sliders", false)
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
            .frame(egui::Frame::none().fill(pal.bg).inner_margin(egui::Margin::symmetric(10.0, 8.0)))
            .show(ctx, |ui| match self.view {
                View::Translator => self.translator(ui),
                View::Editor => self.editor(ui),
                View::Library => self.library(ui),
            });

        self.lang_popup(ctx);
        self.settings_window(ctx);
        self.models_window(ctx);
        self.dict_window(ctx);
        self.style_window(ctx);
        self.syn_window(ctx);
        self.confirm_window(ctx);
        self.diff_window(ctx);
        self.spell_window(ctx);
        self.spell_lang_window(ctx);
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
        ui.horizontal(|ui| {
            let src = if self.set.src == "auto" {
                t(lang, "detect")
            } else {
                lang_label(&self.set.src, &self.nllb_langs)
            };
            if pill(ui, &pal, &src, 150.0).clicked() {
                self.lang_search.clear();
                self.popup_lang = Some("src");
            }
            if icon_btn(ui, &pal, "swap", false)
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
            if pill(ui, &pal, &lang_label(&self.set.tgt, &self.nllb_langs), 150.0).clicked() {
                self.lang_search.clear();
                self.popup_lang = Some("tgt");
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if self
                    .models
                    .iter()
                    .find(|m| m.name == self.set.model)
                    .map(|m| m.vision || m.thinking)
                    .unwrap_or(false)
                {
                    ui.label(RichText::new("◉").size(13.0).color(pal.accent_ink));
                }
                if pill(ui, &pal, &format!("▤  {}", short_model(&self.set.model)), 210.0).clicked() {
                    self.show_models = true;
                }
                if icon_btn(ui, &pal, "refresh", false)
                    .on_hover_text(t(lang, "reload"))
                    .clicked()
                {
                    self.models_loading = true;
                    net::spawn_models(self.cfg(), self.tx.clone(), Vec::new());
                }
            });
        });
        ui.add_space(10.0);

        let gap = 12.0;
        let col_w = ((ui.available_width() - gap) / 2.0).max(160.0);
        let h = ui.available_height();
        let lim = is_mt(&self.set.model) || is_tg(&self.set.model);
        if lim {
            ui.label(RichText::new(t(lang, if is_mt(&self.set.model) { "note_nllb" } else { "note_tg" })).size(12.0).color(pal.muted));
        }

        ui.horizontal_top(|ui| {
            ui.allocate_ui(egui::vec2(col_w, h), |ui| {
                card_ui(ui, &pal, |ui| {
                    ui.vertical(|ui| {
                        egui::ScrollArea::vertical()
                            .id_source("in_scroll")
                            .max_height((h - 60.0).max(80.0))
                            .show(ui, |ui| {
                                ui.add(
                                    egui::TextEdit::multiline(&mut self.input)
                                        .frame(false)
                                        .desired_width(f32::INFINITY)
                                        .hint_text(t(lang, "input_ph"))
                                        .margin(egui::vec2(16.0, 14.0)),
                                );
                            });
                        ui.separator();
                        ui.horizontal(|ui| {
                            ui.add_space(8.0);
                            let attach = ui.add(
                                egui::Button::new(RichText::new("📎").color(pal.text))
                                    .fill(pal.soft)
                                    .rounding(Rounding::same(999.0)),
                            );
                            if attach.on_hover_text(t(lang, "attach")).clicked() {
                                if let Some(path) = rfd::FileDialog::new()
                                    .add_filter("Text", &["txt", "md", "csv", "srt", "json", "html"])
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
                                .size(12.0)
                                .color(pal.muted),
                            );
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                ui.add_space(8.0);
                                if ui
                                    .add_enabled(
                                        !self.busy,
                                        egui::Button::new(
                                            RichText::new(t(lang, "translate"))
                                                .color(pal.on_accent)
                                                .strong(),
                                        )
                                        .fill(pal.accent)
                                        .rounding(Rounding::same(999.0))
                                        .min_size(egui::vec2(110.0, 30.0)),
                                    )
                                    .clicked()
                                {
                                    self.start_translate();
                                }
                            });
                        });
                    });
                });
            });
            ui.add_space(gap);
            ui.allocate_ui(egui::vec2(col_w, h), |ui| {
                card_ui(ui, &pal, |ui| {
                    ui.vertical(|ui| {
                        egui::ScrollArea::vertical()
                            .id_source("out_scroll")
                            .max_height((h - 60.0).max(80.0))
                            .show(ui, |ui| {
                                ui.add_space(10.0);
                                ui.horizontal_wrapped(|ui| {
                                    ui.add_space(6.0);
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
                                            .size(16.0)
                                            .color(pal.text),
                                        )
                                        .wrap(true),
                                    );
                                });
                            });
                        ui.separator();
                        ui.horizontal(|ui| {
                            ui.add_space(8.0);
                            ui.label(RichText::new(&self.status).size(12.0).color(pal.muted));
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                ui.add_space(8.0);
                                if icon_btn(ui, &pal, "copy", false)
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
                                if self.busy && icon_btn(ui, &pal, "stop", false)
                                    .on_hover_text(t(lang, "stop"))
                                    .clicked()
                                {
                                    self.stop_translation();
                                }
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
            .frame(egui::Frame::none().fill(pal.bg).inner_margin(egui::Margin::symmetric(12.0, 14.0)))
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    ui.label(RichText::new(t(lang, "style")).size(15.0).strong());
                    ui.add_space(4.0);
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
                                    egui::Button::new(RichText::new(&option).color(pal.on_accent).size(13.0))
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

                    ui.add_space(6.0);
                    egui::Frame::none()
                        .fill(pal.soft)
                        .rounding(Rounding::same(18.0))
                        .inner_margin(egui::Margin::same(10.0))
                        .show(ui, |ui| {
                            ui.label(RichText::new(t(lang, "context")).strong());
                            ui.add(
                                egui::TextEdit::multiline(&mut self.set.ctxt)
                                    .frame(false)
                                    .desired_width(f32::INFINITY)
                                    .desired_rows(3)
                                    .hint_text(t(lang, "ctx_ph")),
                            );
                            if ui.input(|i| i.pointer.any_released()) {
                                self.save_set();
                            }
                        });
                    ui.add_space(8.0);
                    egui::Frame::none()
                        .fill(pal.soft)
                        .rounding(Rounding::same(18.0))
                        .inner_margin(egui::Margin::same(10.0))
                        .show(ui, |ui| {
                            ui.label(RichText::new(t(lang, "glossary")).strong());
                            let mut remove: Option<usize> = None;
                            for (i, g) in self.set.gloss.iter_mut().enumerate() {
                                ui.horizontal(|ui| {
                                    ui.add(
                                        egui::TextEdit::singleline(&mut g.s)
                                            .frame(false)
                                            .desired_width(100.0)
                                            .hint_text(t(lang, "term")),
                                    );
                                    ui.label("→");
                                    ui.add(
                                        egui::TextEdit::singleline(&mut g.t)
                                            .frame(false)
                                            .desired_width(100.0)
                                            .hint_text(t(lang, "term_t")),
                                    );
                                    if icon_btn(ui, &pal, "x", false).clicked() {
                                        remove = Some(i);
                                    }
                                });
                            }
                            if let Some(i) = remove {
                                self.set.gloss.remove(i);
                                self.save_set();
                            }
                            if ui
                                .add(egui::Button::new(RichText::new(t(lang, "add_entry")).size(12.5)).fill(pal.card))
                                .clicked()
                            {
                                self.set.gloss.push(crate::settings::GlossEntry {
                                    s: String::new(),
                                    t: String::new(),
                                });
                                self.save_set();
                            }
                        });
                });
            });
    }

    // ---- Sprachauswahl ----------------------------------------------------
    fn lang_popup(&mut self, ctx: &egui::Context) {
        let Some(target) = self.popup_lang else { return };
        let pal = self.pal;
        let lang = self.lang;
        let mut open = true;
        let langs = self.active_langs();
        let is_src = target == "src";
        let current = if is_src { self.set.src.clone() } else { self.set.tgt.clone() };
        let pins_key = if is_mt(&self.set.model) { "nllb" } else { "ollama" };
        let search = self.lang_search.to_lowercase();
        let title = t(lang, if is_src { "auto" } else { "translate" });
        egui::Window::new(title)
            .id(egui::Id::new("lang_pop"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .default_width(300.0)
            .anchor(egui::Align2::LEFT_TOP, [100.0, 90.0])
            .show(ctx, |ui| {
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
        if !open {
            self.popup_lang = None;
        }
    }

    // ---- Modellauswahl ----------------------------------------------------
    fn models_window(&mut self, ctx: &egui::Context) {
        if !self.show_models {
            return;
        }
        let pal = self.pal;
        let lang = self.lang;
        let mut open = true;
        let mut pick: Option<String> = None;
        egui::Window::new(t(lang, "nav_tr"))
            .id(egui::Id::new("models_win"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .default_width(420.0)
            .show(ctx, |ui| {
                if self.models.is_empty() {
                    ui.label(RichText::new(t(lang, "wait_ollama")).color(pal.muted));
                }
                egui::ScrollArea::vertical().max_height(420.0).show(ui, |ui| {
                    for m in &self.models {
                        let on = m.name == self.set.model;
                        let text = if on {
                            RichText::new(&m.name).color(pal.accent_ink)
                        } else {
                            RichText::new(&m.name).color(pal.text)
                        };
                        ui.horizontal(|ui| {
                            if ui.add(egui::Button::new(text).fill(Color32::TRANSPARENT)).clicked() {
                                pick = Some(m.name.clone());
                            }
                            if m.vision {
                                ui.label(RichText::new("◉").color(pal.muted));
                            }
                            if m.thinking {
                                ui.label(RichText::new("☀").color(pal.muted));
                            }
                            if m.mt {
                                ui.label(RichText::new("MT").size(11.0).color(pal.muted));
                            }
                        });
                    }
                });
            });
        if let Some(name) = pick {
            self.set.pref = name.clone();
            self.set.model = name;
            self.fix_langs();
            self.save_set();
            self.show_models = false;
        }
        if !open {
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
        egui::Window::new(t(lang, "settings"))
            .id(egui::Id::new("settings_win"))
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .default_width(560.0)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    section(ui, &pal, t(lang, "s_group_general").as_str());
                    combo(ui, "uilang", t(lang, "s_uilang").as_str(), &mut self.set.ui_lang, &[
                        ("auto", t(lang, "auto_sys")),
                        ("de", "Deutsch".into()),
                        ("en", "English".into()),
                        ("nl", "Nederlands".into()),
                        ("es", "Español".into()),
                        ("tr", "Türkçe".into()),
                        ("ar", "العربية".into()),
                    ]);
                    combo(ui, "theme", t(lang, "s_theme").as_str(), &mut self.set.theme, &[
                        ("auto", t(lang, "auto_sys")),
                        ("light", t(lang, "light")),
                        ("dark", t(lang, "dark")),
                    ]);
                    combo(ui, "start", t(lang, "s_start").as_str(), &mut self.set.start, &[
                        ("tr", t(lang, "start_tr")),
                        ("ed", t(lang, "start_ed")),
                        ("last", t(lang, "start_last")),
                    ]);
                    combo(ui, "workspace", t(lang, "s_workspace").as_str(), &mut self.set.workspace, &[
                        ("both", t(lang, "ws_both")),
                        ("tr", t(lang, "ws_tr")),
                        ("ed", t(lang, "ws_ed")),
                    ]);

                    section(ui, &pal, t(lang, "s_group_extra").as_str());
                    ui.checkbox(&mut self.set.boot_model, t(lang, "s_boot"));
                    ui.checkbox(&mut self.set.profanity_filter, t(lang, "s_prof"));
                    combo(ui, "profstyle", t(lang, "s_profstyle").as_str(), &mut self.set.profanity_style, &[
                        ("stars", t(lang, "o_stars")),
                        ("symbols", t(lang, "o_symbols")),
                    ]);
                    ui.checkbox(&mut self.set.ed_markdown, t(lang, "s_md"));
                    ui.checkbox(&mut self.set.auto_copy, t(lang, "s_autocopy"));
                    ui.checkbox(&mut self.set.auto_copy_typed, t(lang, "s_autocopy_t"));

                    section(ui, &pal, t(lang, "s_group_conn").as_str());
                    labeled(ui, t(lang, "s_url").as_str(), |ui| {
                        ui.add(egui::TextEdit::singleline(&mut self.set.url).desired_width(f32::INFINITY));
                    });
                    labeled(ui, t(lang, "s_nllb").as_str(), |ui| {
                        ui.add(egui::TextEdit::singleline(&mut self.set.nllb).desired_width(f32::INFINITY));
                    });

                    section(ui, &pal, t(lang, "s_group_trans").as_str());
                    labeled(ui, t(lang, "s_helper").as_str(), |ui| {
                        ui.add(egui::TextEdit::singleline(&mut self.set.helper).desired_width(f32::INFINITY));
                    });
                    labeled(ui, t(lang, "s_sys").as_str(), |ui| {
                        ui.add(egui::TextEdit::multiline(&mut self.set.sys).desired_width(f32::INFINITY).desired_rows(5));
                    });
                    ui.horizontal(|ui| {
                        ui.label(t(lang, "s_temp"));
                        ui.add(egui::DragValue::new(&mut self.set.temp).speed(0.05));
                        ui.add_space(12.0);
                        ui.label(t(lang, "s_ctx"));
                        ui.add(egui::DragValue::new(&mut self.set.numctx).speed(64.0));
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
        egui::Window::new(t(lang, "personal_dict"))
            .id(egui::Id::new("dict_win"))
            .open(&mut open)
            .collapsible(false)
            .default_width(420.0)
            .show(ctx, |ui| {
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
        let mut open = true;
        egui::Window::new(t(self.lang, "style"))
            .open(&mut open)
            .show(ctx, |ui| {
                ui.label(RichText::new(t(self.lang, "style")).color(self.pal.muted));
            });
        if !open {
            self.show_style = false;
        }
    }

    fn syn_window(&mut self, ctx: &egui::Context) {
        if !self.show_syn {
            return;
        }
        let lang = self.lang;
        let mut open = true;
        let mut ask = false;
        egui::Window::new(t(lang, "synonyms"))
            .id(egui::Id::new("syn_win"))
            .open(&mut open)
            .collapsible(false)
            .default_width(380.0)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut self.syn_word)
                            .hint_text(t(lang, "synonyms"))
                            .desired_width(240.0),
                    );
                    if ui.button(t(lang, "synonyms")).clicked() {
                        ask = true;
                    }
                });
                for s in &self.syn_out {
                    ui.label(s);
                }
            });
        if ask {
            self.request_synonyms();
        }
        if !open {
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
        egui::Window::new(t(lang, "settings"))
            .id(egui::Id::new("confirm_win"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .default_width(360.0)
            .show(ctx, |ui| {
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
            ui.separator();
            for (label, prefix) in [("H1", "# "), ("H2", "## "), ("H3", "### ")] {
                if icon_btn(ui, &pal, label, false)
                    .on_hover_text(t(lang, "heading"))
                    .clicked()
                {
                    self.format_line(prefix);
                }
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
            if ui.button(t(lang, "shorten")).clicked() {
                self.start_editor_op("shorten");
            }
            if ui.button(t(lang, "lengthen")).clicked() {
                self.start_editor_op("lengthen");
            }
            if ui.button(t(lang, "summarize")).clicked() {
                self.start_editor_op("summarize");
            }
            if ui.button(t(lang, "proofread")).clicked() {
                self.start_editor_op("proofread");
            }
            if ui.button(t(lang, "synonyms")).clicked() {
                self.show_syn = true;
            }
        });
        ui.add_space(6.0);

        let reserved = if self.show_sessions { 150.0 } else { 74.0 };
        let h = (ui.available_height() - reserved).max(120.0);
        egui::Frame::none()
            .fill(pal.card)
            .stroke(Stroke::new(1.0_f32, pal.line))
            .rounding(Rounding::same(22.0))
            .inner_margin(egui::Margin::same(18.0))
            .show(ui, |ui| {
                egui::ScrollArea::vertical()
                    .id_source("doc_scroll")
                    .max_height(h)
                    .show(ui, |ui| {
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
        self.context_bar(ui.ctx());

        ui.add_space(6.0);

        ui.horizontal(|ui| {
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
    fn context_bar(&mut self, ctx: &egui::Context) {
        let Some(sel_rect) = self.bar_rect else { return };
        if self.busy {
            return;
        }
        let pal = self.pal;
        let lang = self.lang;
        let ops: [(&str, &str); 5] = [
            ("shorten", "shorten"),
            ("lengthen", "lengthen"),
            ("summarize", "summarize"),
            ("proofread", "proofread"),
            ("synonyms", "synonyms"),
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
        egui::Area::new(egui::Id::new("ctx_bar"))
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
                            for (op, icon) in ops {
                                if icon_btn(ui, &pal, icon, false)
                                    .on_hover_text(t(lang, op))
                                    .clicked()
                                {
                                    action = Some(op);
                                }
                            }
                        });
                    });
            });
        self.bar_hover = over_bar;

        if let Some(op) = action {
            if op == "synonyms" {
                let (a, b) = self.selection();
                let src = &self.doc;
                let text: String = src
                    .chars()
                    .skip(a)
                    .take(b.saturating_sub(a))
                    .collect();
                self.syn_word = text.trim().to_string();
                self.syn_out.clear();
                self.show_syn = true;
            } else {
                self.start_editor_op(op);
            }
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
        egui::Window::new(t(lang, "diff_title"))
            .id(egui::Id::new("diff_win"))
            .open(&mut open)
            .collapsible(false)
            .default_width(620.0)
            .show(ctx, |ui| {
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
        egui::Window::new(t(lang, "spell_title"))
            .id(egui::Id::new("spell_win"))
            .open(&mut open)
            .collapsible(false)
            .default_width(520.0)
            .show(ctx, |ui| {
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
        let mut open = true;
        let mut pick: Option<String> = None;
        egui::Window::new(t(lang, "dict"))
            .id(egui::Id::new("spell_lang_win"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .default_width(260.0)
            .show(ctx, |ui| {
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
        ui.horizontal(|ui| {
            ui.label(RichText::new(t(lang, "nav_lib")).size(20.0).strong());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(RichText::new(format!("{}", self.lib.len())).size(12.0).color(pal.muted));
            });
        });
        ui.add_space(6.0);
        if self.lib.is_empty() {
            ui.vertical_centered(|ui| {
                ui.add_space(60.0);
                ui.label(RichText::new(t(lang, "lib_empty")).color(pal.muted));
                ui.label(RichText::new(t(lang, "lib_empty_hint")).size(12.0).color(pal.muted));
            });
            return;
        }
        let mut open_doc: Option<String> = None;
        let mut del: Option<(String, String)> = None;
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                for meta in &self.lib {
                    egui::Frame::none()
                        .fill(pal.card)
                        .stroke(Stroke::new(1.0_f32, pal.line))
                        .rounding(Rounding::same(18.0))
                        .inner_margin(egui::Margin::same(12.0))
                        .show(ui, |ui| {
                            ui.allocate_ui(egui::vec2(200.0, 120.0), |ui| {
                                ui.vertical(|ui| {
                                    ui.label(RichText::new(&meta.name).strong().size(14.0));
                                    ui.label(
                                        RichText::new(format_date(meta.updated))
                                            .size(11.0)
                                            .color(pal.muted),
                                    );
                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::Center),
                                        |ui| {
                                            if ui.button(t(lang, "open")).clicked() {
                                                open_doc = Some(meta.id.clone());
                                            }
                                            if ui.button(t(lang, "delete")).clicked() {
                                                del = Some((meta.id.clone(), meta.name.clone()));
                                            }
                                        },
                                    );
                                });
                            });
                        });
                }
            });
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
            let text = format!("{} – {}", t(lang, "lib_delete_q"), name);
            self.ask_confirm(text, Confirm::LibDelete(id));
        }
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
    ui.add_space(10.0);
    ui.label(RichText::new(label.to_uppercase()).size(11.0).color(pal.muted));
    ui.add_space(2.0);
}

fn labeled(ui: &mut egui::Ui, label: &str, add: impl FnOnce(&mut egui::Ui)) {
    ui.label(RichText::new(label).size(13.0));
    add(ui);
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

fn format_date(secs: u64) -> String {
    // Umrechnung Tage seit Epoche in ein Datum (ohne externe Datumsbibliothek).
    let days = (secs / 86_400) as i64;
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
    format!("{:02}.{:02}.{}", d, m, y)
}
