//! Dokumentenablage der Rust-Fassung.
//!
//! Bewusst kompatibel zur Electron-Ablage: pro Dokument liegen `<id>.html`
//! (lesbarer Inhalt) und `<id>.json` (id, name, created, updated) im Ordner
//! `%APPDATA%\Uebersetzer\library`. Zusaetzlich speichert die Rust-Fassung den
//! Markdown-Quelltext im JSON-Feld `text`, das die Electron-App ignoriert.
//! Dadurch koennen beide Fassungen dieselben Dokumente oeffnen.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Meta {
    pub id: String,
    pub name: String,
    pub created: u64,
    pub updated: u64,
    #[serde(default)]
    pub text: Option<String>,
}

pub fn dir() -> PathBuf {
    let d = crate::settings::data_dir().join("library");
    let _ = std::fs::create_dir_all(&d);
    d
}

pub fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn safe_id(id: &str) -> bool {
    !id.is_empty()
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

pub fn list() -> Vec<Meta> {
    let mut out = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir()) {
        for e in entries.flatten() {
            let path = e.path();
            if path.extension().and_then(|x| x.to_str()) != Some("json") {
                continue;
            }
            if let Ok(text) = std::fs::read_to_string(&path) {
                if let Ok(meta) = serde_json::from_str::<Meta>(&text) {
                    if safe_id(&meta.id) {
                        out.push(meta);
                    }
                }
            }
        }
    }
    out.sort_by(|a, b| b.updated.cmp(&a.updated));
    out
}

/// Wandelt einfachen Text in absatzweises HTML (wie die Electron-Ablage).
fn text_to_html(text: &str) -> String {
    let mut out = String::new();
    for block in text.split("\n\n") {
        let mut para = String::new();
        for (i, line) in block.split('\n').enumerate() {
            if i > 0 {
                para.push_str("<br>");
            }
            para.push_str(&line.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;"));
        }
        if !para.is_empty() {
            out.push_str(&format!("<p>{}</p>", para));
        }
    }
    out
}

/// Grobe Rueckwandlung von HTML in Text, falls ein Dokument nur als HTML
/// vorliegt (etwa von der Electron-Fassung gespeichert).
fn html_to_text(html: &str) -> String {
    let mut s = html.to_string();
    for (from, to) in [
        ("</p>", "\n\n"),
        ("</div>", "\n\n"),
        ("<br>", "\n"),
        ("<br/>", "\n"),
        ("<br />", "\n"),
        ("</h1>", "\n\n"),
        ("</h2>", "\n\n"),
        ("</h3>", "\n\n"),
        ("</li>", "\n"),
    ] {
        s = s.replace(from, to);
    }
    let mut out = String::new();
    let mut in_tag = false;
    let mut entity = String::new();
    for c in s.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            '&' if !in_tag => entity.push('&'),
            ';' if !in_tag && !entity.is_empty() => {
                let e = std::mem::take(&mut entity);
                out.push_str(match e.as_str() {
                    "&amp" => "&",
                    "&lt" => "<",
                    "&gt" => ">",
                    "&nbsp" => " ",
                    _ => " ",
                });
            }
            _ if !entity.is_empty() && !in_tag => entity.push(c),
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    out.split('\n')
        .map(|l| l.trim_end())
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

pub fn read(id: &str) -> Option<(Meta, String)> {
    if !safe_id(id) {
        return None;
    }
    let json_path = dir().join(format!("{}.json", id));
    let text = std::fs::read_to_string(&json_path).ok()?;
    let meta: Meta = serde_json::from_str(&text).ok()?;
    let body = if let Some(t) = meta.text.clone() {
        t
    } else {
        let html = std::fs::read_to_string(dir().join(format!("{}.html", id))).unwrap_or_default();
        html_to_text(&html)
    };
    Some((meta, body))
}

pub fn save(id: Option<&str>, name: &str, text: &str) -> Option<String> {
    let id = match id {
        Some(i) if safe_id(i) => i.to_string(),
        _ => format!("d{}", now()),
    };
    let created = std::fs::read_to_string(dir().join(format!("{}.json", id)))
        .ok()
        .and_then(|t| serde_json::from_str::<Value>(&t).ok())
        .and_then(|v| v.get("created").and_then(|c| c.as_u64()))
        .unwrap_or_else(now);
    let meta = Meta {
        id: id.clone(),
        name: name.to_string(),
        created,
        updated: now(),
        text: Some(text.to_string()),
    };
    let _ = std::fs::write(
        dir().join(format!("{}.json", id)),
        serde_json::to_string_pretty(&meta).unwrap_or_default(),
    );
    let _ = std::fs::write(dir().join(format!("{}.html", id)), text_to_html(text));
    Some(id)
}

pub fn delete(id: &str) {
    if !safe_id(id) {
        return;
    }
    let _ = std::fs::remove_file(dir().join(format!("{}.json", id)));
    let _ = std::fs::remove_file(dir().join(format!("{}.html", id)));
}

pub fn format_date(path: &Path) -> String {
    let _ = path;
    String::new()
}

/// Ersatzname aus der ersten Ueberschrift, sonst "Dokument".
pub fn doc_name(text: &str, fallback: &str) -> String {
    for line in text.lines() {
        let trimmed = line.trim_start_matches('#').trim();
        if !trimmed.is_empty() {
            return trimmed.chars().take(60).collect();
        }
    }
    fallback.to_string()
}
