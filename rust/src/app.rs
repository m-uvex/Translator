//! Fenster, Ansichten und Dialoge der Rust-Neufassung.

use crate::engine::{
    eng_name, filter_profanity, is_mt, is_tg, lang_label, strip_stars, tg_prompt, user_prompt_for,
    PromptCtx, OLLAMA_LANGS,
};
use crate::i18n::{t, Lang};
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

    lib: Vec<Meta>,
    doc: String,
    doc_id: Option<String>,
    saved: bool,

    dict_words: Vec<String>,
    dict_new: String,
    rename: Option<usize>,

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
            lib: library::list(),
            doc: set.doc.clone(),
            doc_id: set.doc_id.clone(),
            saved: true,
            dict_words,
            dict_new: String::new(),
            rename: None,
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
                        self.doc = text;
                        self.editor_job = false;
                        self.saved = false;
                        self.status = String::new();
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

fn icon_btn(ui: &mut egui::Ui, p: &Palette, label: &str, on: bool) -> egui::Response {
    let fill = if on { p.accent } else { p.soft };
    let ink = if on { p.on_accent } else { p.muted };
    ui.add(
        egui::Button::new(RichText::new(label).size(13.0).color(ink))
            .fill(fill)
            .stroke(Stroke::NONE)
            .rounding(Rounding::same(12.0))
            .min_size(egui::vec2(32.0, 32.0)),
    )
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
                            if icon_btn(ui, &pal, "✕", false)
                                .on_hover_text(t(lang, "win_close"))
                                .clicked()
                            {
                                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                            }
                            if icon_btn(ui, &pal, "▢", false)
                                .on_hover_text(t(lang, "win_max"))
                                .clicked()
                            {
                                let maximized = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
                                ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(!maximized));
                            }
                            if icon_btn(ui, &pal, "–", false)
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
                        if icon_btn(ui, &pal, "⇄", self.view == View::Translator)
                            .on_hover_text(t(lang, "nav_tr"))
                            .clicked()
                        {
                            self.set_view(View::Translator);
                        }
                    }
                    if both || self.set.workspace == "ed" {
                        if icon_btn(ui, &pal, "✎", self.view == View::Editor)
                            .on_hover_text(t(lang, "nav_ed"))
                            .clicked()
                        {
                            self.set_view(View::Editor);
                        }
                        if icon_btn(ui, &pal, "▤", self.view == View::Library)
                            .on_hover_text(t(lang, "nav_lib"))
                            .clicked()
                        {
                            self.set_view(View::Library);
                        }
                    }
                    ui.add_space(ui.available_height() - 40.0);
                    if icon_btn(ui, &pal, "⚙", false)
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
            if icon_btn(ui, &pal, "⇆", false)
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
                if icon_btn(ui, &pal, if self.models_loading { "◌" } else { "⟳" }, false)
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
                                if icon_btn(ui, &pal, "⧉", false)
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
                                if self.busy && icon_btn(ui, &pal, "■", false)
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
                                if icon_btn(ui, &pal, t(lang, text).as_str(), on).clicked() {
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
                                    if icon_btn(ui, &pal, "✕", false).clicked() {
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
                                if icon_btn(ui, &pal, "✕", false).clicked() {
                                    remove = Some(i);
                                }
                                if icon_btn(ui, &pal, "✎", false).clicked() {
                                    self.rename = Some(i);
                                }
                            });
                        });
                    }
                });
                if let Some(i) = remove {
                    self.dict_words.remove(i);
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
        let pal = self.pal;
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
        ui.horizontal_wrapped(|ui| {
            if ui.button(t(lang, "new_doc")).clicked() {
                if !self.doc.trim().is_empty() {
                    self.save_doc_to_tray(false);
                }
                self.doc.clear();
                self.doc_id = None;
                self.set.doc.clear();
                self.save_set();
                self.saved = true;
            }
            if ui.button(t(lang, "open")).clicked() {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("Dokument", &["txt", "md", "html"])
                    .pick_file()
                {
                    if let Ok(text) = std::fs::read_to_string(&path) {
                        self.doc = text;
                        self.doc_id = None;
                        self.saved = false;
                    }
                }
            }
            if ui.button(t(lang, "save")).clicked() {
                self.save_doc_to_tray(true);
            }
            if ui.button(t(lang, "save_to_tray")).clicked() {
                self.save_doc_to_tray(true);
            }
            if ui.button(t(lang, "export")).clicked() {
                if let Some(path) = rfd::FileDialog::new()
                    .set_file_name(format!("{}.md", library::doc_name(&self.doc, "Dokument")))
                    .save_file()
                {
                    let _ = std::fs::write(path, &self.doc);
                    self.status = t(lang, "saved");
                }
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
            ui.separator();
            if icon_btn(ui, &pal, "B", false).on_hover_text(t(lang, "bold")).clicked() {
                self.wrap_selection("**", "**");
            }
            if icon_btn(ui, &pal, "I", false).clicked() {
                self.wrap_selection("*", "*");
            }
        });
        ui.add_space(6.0);

        let h = ui.available_height() - 40.0;
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
                        }
                        if out.response.changed() {
                            self.saved = false;
                            self.set.doc = self.doc.clone();
                            self.save_set();
                        }
                    });
            });
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            let words = self.doc.split_whitespace().count();
            ui.label(
                RichText::new(format!("{} {}", words, t(lang, "words")))
                    .size(12.0)
                    .color(pal.muted),
            );
            ui.label(RichText::new(&self.status).size(12.0).color(pal.muted));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button(t(lang, "personal_dict")).clicked() {
                    self.show_dict = true;
                }
                ui.label(RichText::new(lang_label(&self.set.dict_lang, &self.nllb_langs)).size(12.0).color(pal.muted));
            });
        });
    }

    fn wrap_selection(&mut self, left: &str, right: &str) {
        let doc = self.doc.clone();
        if let Some((a, b)) = self.sel {
            if b > a {
                let ba = byte_idx(&doc, a);
                let bb = byte_idx(&doc, b);
                let mut out = String::new();
                out.push_str(&doc[..ba]);
                out.push_str(left);
                out.push_str(&doc[ba..bb]);
                out.push_str(right);
                out.push_str(&doc[bb..]);
                self.doc = out;
                self.saved = false;
                return;
            }
        }
        self.doc.push_str(left);
        self.doc.push_str(right);
        self.saved = false;
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
            if quiet {
                self.status = t(lang, "saved");
            }
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
