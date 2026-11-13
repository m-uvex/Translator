//! Bilder im Uebersetzer: einlesen, entschluesseln, Groessen fuer den Stapel.
//!
//! Die Vorlage haelt Bilder als `{ b64, url }` in `images` (`renderer.js`),
//! zeigt sie als Vorschautapel ueber dem Eingabefeld und schickt `b64` an
//! Ollama. Hier steht dasselbe: `Bild` traegt die Base64-Fassung fuer die
//! Anfrage und eine Textur zum Zeichnen.

use base64::Engine;
use std::path::Path;

pub struct Bild {
    /// Base64 der Bilddatei, so wie Ollama sie erwartet.
    pub b64: String,
    /// Textur zum Zeichnen der Vorschau.
    pub tex: egui::TextureHandle,
    pub w: u32,
    pub h: u32,
}

/// Masse des Vorschautapels, wie `IMG_CFG` in `renderer.js`.
pub const GAP: f32 = 12.0;
pub const BTN: f32 = 6.0;
pub const PAD: f32 = 14.0;
pub const MIN: f32 = 48.0;
pub const MAX: f32 = 96.0;
pub const COLS: usize = 4;
pub const MAX_COLS: usize = 5;
pub const SHARE: f32 = 0.40;
pub const STACK: f32 = 92.0;
pub const STEP: f32 = 10.0;
pub const UP: f32 = 9.0;
/// Wie weit das erste Blatt des Stapels gedreht liegt.
pub const DREHUNG: f32 = 6.4;

pub fn ist_bild(pfad: &Path) -> bool {
    matches!(
        pfad.extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase()
            .as_str(),
        "png" | "jpg" | "jpeg" | "webp" | "gif" | "bmp"
    )
}

/// Groesste Vorschau, die fuer `n` Bilder in `w` x `h` passt — die Rechnung
/// aus `imgFit` in `renderer.js`, nur ohne die Zwischenschritte.
pub fn img_fit(n: usize, w: f32, h: f32) -> Option<(f32, usize)> {
    let n = n.max(1);
    let aw = (w - 2.0 * PAD).max(60.0);
    let ah = (h - 2.0 * PAD).max(60.0);
    let max_a = SHARE * w * h;
    let mut passes: Vec<(usize, usize)> = Vec::new();
    if n >= COLS {
        passes.push((COLS, MAX_COLS));
        passes.push((COLS - 1, 1));
    } else {
        passes.push((n, MAX_COLS));
        if n > 1 {
            passes.push((n - 1, 1));
        }
    }
    for (from, to) in passes {
        let mut s = MAX;
        while s >= MIN {
            let mut c = from;
            loop {
                let zeilen = (n as f32 / c as f32).ceil();
                let bw = c as f32 * s + (c as f32 - 1.0) * GAP + 2.0 * BTN;
                let bh = zeilen * s + (zeilen - 1.0) * GAP + 2.0 * BTN;
                if bw <= aw && bh <= ah && bw * bh <= max_a {
                    return Some((s, c));
                }
                if from <= to {
                    if c >= to {
                        break;
                    }
                    c += 1;
                } else {
                    if c <= to {
                        break;
                    }
                    c -= 1;
                }
            }
            s -= 2.0;
        }
    }
    None
}

/// Hoechstzahl an Bildern fuer die gegebene Flaeche (`imgLimit`).
pub fn img_limit(w: f32, h: f32) -> usize {
    let mut lim = 0;
    for n in 1..=64 {
        if img_fit(n, w, h).is_some() {
            lim = n;
        } else {
            break;
        }
    }
    lim.max(1)
}

fn textur(ctx: &egui::Context, bytes: &[u8]) -> Option<(egui::TextureHandle, u32, u32)> {
    let img = image::load_from_memory(bytes).ok()?;
    let rgba = img.to_rgba8();
    let (w, h) = (rgba.width(), rgba.height());
    let farben = egui::ColorImage::from_rgba_unmultiplied(
        [w as usize, h as usize],
        rgba.as_raw(),
    );
    let tex = ctx.load_texture(
        format!("bild_{}", crate::bilder::kennung(bytes)),
        farben,
        egui::TextureOptions::LINEAR,
    );
    Some((tex, w, h))
}

/// Kurze Kennung fuer den Texturnamen (Laenge und erste Bytes genuegen).
pub fn kennung(bytes: &[u8]) -> u64 {
    let mut h: u64 = 1469598103934665603;
    for b in bytes.iter().take(4096) {
        h ^= *b as u64;
        h = h.wrapping_mul(1099511628211);
    }
    h ^ bytes.len() as u64
}

/// Bild aus Rohdaten (Datei, Zwischenablage, Ablegen).
pub fn aus_bytes(bytes: &[u8], ctx: &egui::Context) -> Result<Bild, String> {
    let (tex, w, h) = textur(ctx, bytes).ok_or("Bild nicht lesbar")?;
    Ok(Bild {
        b64: base64::engine::general_purpose::STANDARD.encode(bytes),
        tex,
        w,
        h,
    })
}

pub fn aus_pfad(pfad: &Path, ctx: &egui::Context) -> Result<Bild, String> {
    let bytes = std::fs::read(pfad).map_err(|e| e.to_string())?;
    aus_bytes(&bytes, ctx)
}

/// Bild aus der Zwischenablage, falls dort eines liegt.
pub fn aus_zwischenablage(ctx: &egui::Context) -> Option<Bild> {
    let mut cb = arboard::Clipboard::new().ok()?;
    let img = cb.get_image().ok()?;
    let (w, h) = (img.width, img.height);
    let puffer = image::RgbaImage::from_raw(w as u32, h as u32, img.bytes.into_owned())?;
    let mut png = Vec::new();
    image::DynamicImage::ImageRgba8(puffer)
        .write_to(&mut std::io::Cursor::new(&mut png), image::ImageOutputFormat::Png)
        .ok()?;
    aus_bytes(&png, ctx).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vorschau_waechst_mit_der_flaeche() {
        // Ein einzelnes Bild darf gross werden, viele werden kleiner.
        let gross = img_fit(1, 600.0, 400.0).expect("passt nicht");
        let viele = img_fit(12, 600.0, 400.0).expect("passt nicht");
        assert!(gross.0 > viele.0, "{} <= {}", gross.0, viele.0);
    }

    #[test]
    fn bild_wird_entschluesselt_und_kodiert() {
        // Ein kleines PNG erzeugen, einlesen und pruefen, dass die Textur
        // entsteht und die Base64-Fassung zurueckkommt.
        let mut png = Vec::new();
        let bild = image::RgbaImage::from_pixel(2, 2, image::Rgba([200, 30, 40, 255]));
        image::DynamicImage::ImageRgba8(bild)
            .write_to(&mut std::io::Cursor::new(&mut png), image::ImageOutputFormat::Png)
            .unwrap();
        let ctx = egui::Context::default();
        let b = aus_bytes(&png, &ctx).expect("Bild nicht gelesen");
        assert_eq!((b.w, b.h), (2, 2));
        assert_eq!(
            base64::engine::general_purpose::STANDARD.decode(&b.b64).unwrap(),
            png
        );
    }

    #[test]
    fn grenze_ist_erreichbar_und_endlich() {
        let lim = img_limit(320.0, 320.0);
        assert!(lim >= 1 && lim <= 64, "Grenze {lim}");
        assert!(img_fit(lim, 320.0, 320.0).is_some());
    }
}
