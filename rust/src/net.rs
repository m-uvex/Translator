//! Netzwerkzugriffe auf die beiden lokalen Dienste.
//!
//! Wie in der Electron-Fassung laeuft alles ueber `127.0.0.1`/`localhost`:
//! Ollama an Port 11434 (Chat, Modellliste, Pull) und der Python-Server fuer
//! NLLB an Port 8765 (Sprachen, Modelle, Uebersetzung, Abbruch). Alle Aufrufe
//! laufen in eigenen Threads und melden ihr Ergebnis ueber einen Kanal, damit
//! die Oberflaeche waehrend des Streamens fluessig bleibt.

use serde_json::{json, Value};
use std::io::{BufRead, BufReader};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::Arc;
use std::time::Duration;

#[derive(Clone, Debug, PartialEq)]
pub struct Model {
    pub name: String,
    pub vision: bool,
    pub thinking: bool,
    pub mt: bool,
}

#[derive(Debug)]
pub enum Msg {
    Models(Vec<Model>),
    Meta {
        langs: Vec<(String, String)>,
        models: Vec<Value>,
    },
    Status(String),
    Token(String),
    Done(String),
    Stopped,
    Error(String),
    ModelsMeta(Vec<Value>),
    /// Adresse, unter der Ollama tatsaechlich antwortet (Rueckfall).
    Url(String),
    /// Verbindungsfehler beim Abruf der Modellliste.
    ModelsError(String),
}

#[derive(Clone, Debug)]
pub struct Cfg {
    pub url: String,
    pub nllb: String,
    pub temp: f32,
    pub numctx: u32,
}

fn base(url: &str) -> String {
    url.trim_end_matches('/').to_string()
}

/// Die jeweils andere Schreibweise des Rechnernamens. Windows loest
/// `localhost` zuerst auf `::1` auf, Ollama hoert aber standardmaessig nur auf
/// `127.0.0.1`; deshalb wird bei einem Fehlschlag die andere Form probiert.
fn alt_host(url: &str) -> Option<String> {
    if url.contains("://localhost") {
        Some(url.replace("://localhost", "://127.0.0.1"))
    } else if url.contains("://127.0.0.1") {
        Some(url.replace("://127.0.0.1", "://localhost"))
    } else {
        None
    }
}

/// Holt JSON von einem Pfad unterhalb der Adresse und gibt bei einem Fehler
/// dessen Text zurueck, damit die Oberflaeche ihn anzeigen kann.
fn get_json(url: &str, path: &str, secs: u64) -> Result<Value, String> {
    match ureq::get(&format!("{}{}", base(url), path))
        .timeout(Duration::from_secs(secs))
        .call()
    {
        Ok(resp) => resp.into_json::<Value>().map_err(|e| e.to_string()),
        Err(e) => Err(e.to_string()),
    }
}

pub fn spawn_models(cfg: Cfg, tx: Sender<Msg>, mt_models: Vec<Model>) {
    std::thread::spawn(move || {
        let mut url = cfg.url.clone();
        let mut tags = get_json(&url, "/api/tags", 2);
        if tags.is_err() {
            if let Some(alt) = alt_host(&url) {
                if let Ok(v) = get_json(&alt, "/api/tags", 2) {
                    url = alt;
                    tags = Ok(v);
                    let _ = tx.send(Msg::Url(url.clone()));
                }
            }
        }
        let mut list: Vec<Model> = Vec::new();
        let error = match tags {
            Ok(v) => {
                if let Some(models) = v.get("models").and_then(|m| m.as_array()) {
                    for m in models {
                        let name = m.get("name").and_then(|n| n.as_str()).unwrap_or("").to_string();
                        if name.is_empty() {
                            continue;
                        }
                        let mut vision = false;
                        let mut thinking = false;
                        if let Ok(show) = ureq::post(&format!("{}/api/show", base(&url)))
                            .timeout(Duration::from_secs(2))
                            .send_json(json!({ "model": name }))
                        {
                            if let Ok(sv) = show.into_json::<Value>() {
                                if let Some(caps) = sv.get("capabilities").and_then(|c| c.as_array()) {
                                    vision = caps.iter().any(|c| c.as_str() == Some("vision"));
                                    thinking = caps.iter().any(|c| c.as_str() == Some("thinking"));
                                }
                            }
                        }
                        let mt = name.to_lowercase().contains("translategemma");
                        list.push(Model { name, vision: vision && !mt, thinking, mt });
                    }
                }
                None
            }
            Err(e) => Some(e),
        };
        list.extend(mt_models);
        let _ = tx.send(Msg::Models(list));
        if let Some(e) = error {
            let _ = tx.send(Msg::ModelsError(e));
        }
    });
}

fn strip_think(s: &str) -> String {
    // Denk-Modelle liefern ` thinking…</think>`; das wird entfernt.
    let mut out = String::new();
    let mut rest = s;
    while let Some(start) = rest.find(" thinking") {
        out.push_str(&rest[..start]);
        match rest[start..].find("</think>") {
            Some(end) => rest = &rest[start + end + "</think>".len()..],
            None => return out.trim().to_string(),
        }
    }
    out.push_str(rest);
    out.trim().to_string()
}

/// Startet einen Chat-Aufruf. Bei `stream` kommen Teiltexte ueber `Msg::Token`,
/// am Ende immer `Msg::Done` oder `Msg::Error`.
pub fn spawn_chat(
    cfg: Cfg,
    model: String,
    messages: Vec<Value>,
    temperature: Option<f32>,
    format: Option<&'static str>,
    stream: bool,
    tx: Sender<Msg>,
    stop: Arc<AtomicBool>,
) {
    std::thread::spawn(move || {
        let mut body = json!({
            "model": model,
            "messages": messages,
            "stream": stream,
            "options": {
                "temperature": temperature.unwrap_or(cfg.temp),
                "num_ctx": cfg.numctx,
            }
        });
        if let Some(f) = format {
            body["format"] = json!(f);
        }
        if model.starts_with("gpt-oss") {
            body["think"] = json!("low");
        } else if model.to_lowercase().contains("qwen3") || model.to_lowercase().contains("deepseek-r1") {
            body["think"] = json!(false);
        }
        let resp = ureq::post(&format!("{}/api/chat", base(&cfg.url)))
            .timeout(Duration::from_secs(300))
            .send_json(body);
        let resp = match resp {
            Ok(r) => r,
            Err(e) => {
                let _ = tx.send(Msg::Error(e.to_string()));
                return;
            }
        };
        if !stream {
            match resp.into_json::<Value>() {
                Ok(v) => {
                    let text = v
                        .get("message")
                        .and_then(|m| m.get("content"))
                        .and_then(|c| c.as_str())
                        .unwrap_or("")
                        .to_string();
                    let _ = tx.send(Msg::Done(strip_think(&text)));
                }
                Err(e) => {
                    let _ = tx.send(Msg::Error(e.to_string()));
                }
            }
            return;
        }
        let reader = BufReader::new(resp.into_reader());
        let mut acc = String::new();
        for line in reader.lines() {
            if stop.load(Ordering::SeqCst) {
                let _ = tx.send(Msg::Stopped);
                return;
            }
            let line = match line {
                Ok(l) => l,
                Err(e) => {
                    let _ = tx.send(Msg::Error(e.to_string()));
                    return;
                }
            };
            if line.trim().is_empty() {
                continue;
            }
            if let Ok(v) = serde_json::from_str::<Value>(&line) {
                if let Some(err) = v.get("error").and_then(|e| e.as_str()) {
                    let _ = tx.send(Msg::Error(err.to_string()));
                    return;
                }
                if let Some(part) = v.get("message").and_then(|m| m.get("content")).and_then(|c| c.as_str()) {
                    acc.push_str(part);
                    let _ = tx.send(Msg::Token(strip_think(&acc)));
                }
            }
        }
        let _ = tx.send(Msg::Done(strip_think(&acc)));
    });
}

/// Uebersetzt einen Textblock zeilenweise ueber den NLLB-Server.
pub fn spawn_nllb_translate(
    cfg: Cfg,
    model: String,
    text: String,
    src: String,
    tgt: String,
    req: String,
    tx: Sender<Msg>,
    stop: Arc<AtomicBool>,
) {
    std::thread::spawn(move || {
        let mut out: Vec<String> = Vec::new();
        for line in text.split('\n') {
            if stop.load(Ordering::SeqCst) {
                let _ = tx.send(Msg::Stopped);
                return;
            }
            if line.trim().is_empty() {
                out.push(String::new());
                continue;
            }
            let resp = ureq::post(&format!("{}/translate", base(&cfg.nllb)))
                .timeout(Duration::from_secs(300))
                .send_json(json!({
                    "text": line, "src": src, "tgt": tgt, "model": model, "req": req
                }));
            match resp {
                Ok(r) => match r.into_json::<Value>() {
                    Ok(v) => {
                        out.push(v.get("translation").and_then(|t| t.as_str()).unwrap_or("").to_string());
                        let _ = tx.send(Msg::Token(out.join("\n")));
                    }
                    Err(e) => {
                        let _ = tx.send(Msg::Error(e.to_string()));
                        return;
                    }
                },
                Err(e) => {
                    let _ = tx.send(Msg::Error(e.to_string()));
                    return;
                }
            }
        }
        let _ = tx.send(Msg::Done(out.join("\n")));
    });
}

/// Holt Sprachen und Modelle des NLLB-Servers (fuer die Einstellungen noetig).
pub fn spawn_nllb_meta(cfg: Cfg, tx: Sender<Msg>) {
    std::thread::spawn(move || {
        let mut langs: Vec<(String, String)> = Vec::new();
        let mut models: Vec<Value> = Vec::new();
        if let Ok(r) = ureq::get(&format!("{}/languages", base(&cfg.nllb)))
            .timeout(Duration::from_secs(2))
            .call()
        {
            if let Ok(v) = r.into_json::<Value>() {
                if let Some(arr) = v.as_array() {
                    for l in arr {
                        langs.push((
                            l.get("code").and_then(|c| c.as_str()).unwrap_or("").to_string(),
                            l.get("name").and_then(|c| c.as_str()).unwrap_or("").to_string(),
                        ));
                    }
                }
            }
        }
        if let Ok(r) = ureq::get(&format!("{}/models", base(&cfg.nllb)))
            .timeout(Duration::from_secs(2))
            .call()
        {
            if let Ok(v) = r.into_json::<Value>() {
                if let Some(arr) = v.as_array() {
                    for m in arr {
                        if m.get("installed").and_then(|i| i.as_bool()).unwrap_or(false) {
                            let id = m.get("id").and_then(|i| i.as_str()).unwrap_or("").to_string();
                            let family = m.get("family").and_then(|i| i.as_str()).unwrap_or("nllb").to_string();
                            let label = if family == "nllb" {
                                format!("NLLB-200 {}", id)
                            } else {
                                format!("Omnilingual MT {}", id)
                            };
                            models.push(json!({
                                "name": format!("{}:{}", family, id),
                                "label": label,
                                "vision": false,
                                "thinking": false,
                                "mt": true
                            }));
                        }
                    }
                }
            }
        }
        let _ = tx.send(Msg::Meta { langs, models });
    });
}

pub fn spawn_model_action(cfg: Cfg, id: String, action: &'static str, tx: Sender<Msg>) {
    std::thread::spawn(move || {
        let _ = ureq::post(&format!("{}/models/{}", base(&cfg.nllb), action))
            .timeout(Duration::from_secs(10))
            .send_json(json!({ "id": id }));
        if let Ok(r) = ureq::get(&format!("{}/models", base(&cfg.nllb)))
            .timeout(Duration::from_secs(5))
            .call()
        {
            if let Ok(Value::Array(arr)) = r.into_json::<Value>() {
                let _ = tx.send(Msg::ModelsMeta(arr));
            }
        }
    });
}

pub fn spawn_nllb_models(cfg: Cfg, tx: Sender<Msg>) {
    std::thread::spawn(move || {
        if let Ok(r) = ureq::get(&format!("{}/models", base(&cfg.nllb)))
            .timeout(Duration::from_secs(3))
            .call()
        {
            if let Ok(Value::Array(arr)) = r.into_json::<Value>() {
                let _ = tx.send(Msg::ModelsMeta(arr));
            }
        }
    });
}

pub fn spawn_warmup(cfg: Cfg, model: String) {
    std::thread::spawn(move || {
        if model.contains(':') && model.starts_with("nllb") {
            let _ = ureq::post(&format!("{}/load", base(&cfg.nllb)))
                .timeout(Duration::from_secs(60))
                .send_json(json!({ "id": model }));
        } else {
            let _ = ureq::post(&format!("{}/api/generate", base(&cfg.url)))
                .timeout(Duration::from_secs(60))
                .send_json(json!({ "model": model, "keep_alive": "60m" }));
        }
    });
}

pub fn cancel_nllb(cfg: &Cfg, req: &str) {
    let cfg = cfg.clone();
    let req = req.to_string();
    std::thread::spawn(move || {
        let _ = ureq::post(&format!("{}/cancel", base(&cfg.nllb)))
            .timeout(Duration::from_secs(3))
            .send_json(json!({ "req": req }));
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rueckfall_tauscht_nur_den_rechnernamen() {
        assert_eq!(
            alt_host("http://localhost:11434").as_deref(),
            Some("http://127.0.0.1:11434")
        );
        assert_eq!(
            alt_host("http://127.0.0.1:8765/").as_deref(),
            Some("http://localhost:8765/")
        );
        assert_eq!(alt_host("http://192.168.0.5:11434"), None);
    }

    /// Nur von Hand: braucht ein laufendes Ollama, das auf `127.0.0.1` hoert.
    #[test]
    #[ignore]
    fn modelle_ueber_rueckfall() {
        let (tx, rx) = std::sync::mpsc::channel();
        let cfg = Cfg {
            url: "http://localhost:11434".into(),
            nllb: String::new(),
            temp: 0.2,
            numctx: 2048,
        };
        spawn_models(cfg, tx, Vec::new());
        let mut models: Vec<Model> = Vec::new();
        let mut via: Option<String> = None;
        let mut err: Option<String> = None;
        while let Ok(msg) = rx.recv_timeout(Duration::from_secs(10)) {
            match msg {
                Msg::Models(list) => models = list,
                Msg::Url(u) => via = Some(u),
                Msg::ModelsError(e) => err = Some(e),
                _ => {}
            }
            if !models.is_empty() {
                break;
            }
        }
        assert!(err.is_none(), "Verbindungsfehler: {:?}", err);
        assert!(!models.is_empty(), "keine Modelle gefunden");
        assert_eq!(via.as_deref(), Some("http://127.0.0.1:11434"));
        println!("{} Modelle: {:?}", models.len(), models.iter().map(|m| &m.name).collect::<Vec<_>>());
    }
}
