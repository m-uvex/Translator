//! Import von PDF und DOCX.
//!
//! Die Electron-Fassung liest beide Formate im Hauptprozess (`ipcMain.handle(
//! 'extract', ...)` in `main.js`) mit `mammoth` beziehungsweise `pdf-parse`.
//! Hier wird der Text ohne fremde Bibliotheken geholt: DOCX ist ein ZIP-Archiv
//! mit `word/document.xml`, PDF wird aus den entpackten Inhaltsstroemen gelesen.
//! Entpackt wird mit `flate2` (Deflate), ZIP und XML werden von Hand gelesen.

use std::io::Read;
use std::path::Path;

/// Liest den Text einer Datei. `txt`, `md` und `html` werden unveraendert
/// gelesen, `docx` und `pdf` werden ausgewertet.
pub fn extract(path: &Path) -> Result<String, String> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    match ext.as_str() {
        "docx" => {
            let doc = unzip(&bytes, "word/document.xml").ok_or("Kein Text in der Datei")?;
            let text = docx_text(&String::from_utf8_lossy(&doc));
            if text.trim().is_empty() {
                Err("Kein Text in der Datei".into())
            } else {
                Ok(text.trim().to_string())
            }
        }
        "pdf" => {
            let text = pdf_text(&bytes);
            if text.trim().is_empty() {
                Err("Kein Text in der Datei".into())
            } else {
                Ok(text.trim().to_string())
            }
        }
        _ => Ok(String::from_utf8_lossy(&bytes).to_string()),
    }
}

// ---- ZIP ------------------------------------------------------------------

fn u16_at(b: &[u8], at: usize) -> Option<u16> {
    b.get(at..at + 2).map(|s| u16::from_le_bytes([s[0], s[1]]))
}

fn u32_at(b: &[u8], at: usize) -> Option<u32> {
    b.get(at..at + 4)
        .map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
}

/// Holt einen Eintrag aus einem ZIP-Archiv (nur gespeichert oder deflate, das
/// genuegt fuer DOCX).
pub fn unzip(data: &[u8], wanted: &str) -> Option<Vec<u8>> {
    // Verzeichnis am Ende suchen (PK\x05\x06).
    let mut eocd = None;
    let start = data.len().saturating_sub(66_000);
    let mut i = data.len().saturating_sub(22);
    while i > start {
        if data[i..].starts_with(b"PK\x05\x06") {
            eocd = Some(i);
            break;
        }
        i -= 1;
    }
    let eocd = eocd?;
    let count = u16_at(data, eocd + 10)? as usize;
    let mut pos = u32_at(data, eocd + 16)? as usize;
    for _ in 0..count {
        if !data.get(pos..)?.starts_with(b"PK\x01\x02") {
            return None;
        }
        let method = u16_at(data, pos + 10)?;
        let csize = u32_at(data, pos + 20)? as usize;
        let nlen = u16_at(data, pos + 28)? as usize;
        let elen = u16_at(data, pos + 30)? as usize;
        let clen = u16_at(data, pos + 32)? as usize;
        let local = u32_at(data, pos + 42)? as usize;
        let name = data.get(pos + 46..pos + 46 + nlen)?;
        if String::from_utf8_lossy(name) == wanted {
            let lnlen = u16_at(data, local + 26)? as usize;
            let lelen = u16_at(data, local + 28)? as usize;
            let begin = local + 30 + lnlen + lelen;
            let raw = data.get(begin..begin + csize)?;
            if method == 0 {
                return Some(raw.to_vec());
            }
            if method == 8 {
                let mut out = Vec::new();
                let mut d = flate2::read::DeflateDecoder::new(raw);
                d.read_to_end(&mut out).ok()?;
                return Some(out);
            }
            return None;
        }
        pos += 46 + nlen + elen + clen;
    }
    None
}

// ---- DOCX -----------------------------------------------------------------

fn entschaerfen(s: &str) -> String {
    s.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
}

/// Zieht den Text aus `word/document.xml`: jeder Absatz (`w:p`) wird eine
/// Zeile, `w:br` und `w:tab` werden Zeilenumbruch und Tabulator.
pub fn docx_text(xml: &str) -> String {
    let mut absaetze: Vec<String> = Vec::new();
    let mut zeile = String::new();
    let mut im_text = false;
    let mut rest = xml;
    while let Some(lt) = rest.find('<') {
        if im_text {
            zeile.push_str(&entschaerfen(&rest[..lt]));
            im_text = false;
        }
        let Some(gt) = rest[lt..].find('>') else { break };
        let tag = &rest[lt + 1..lt + gt];
        let schliessend = tag.starts_with('/');
        let name = tag
            .trim_start_matches('/')
            .split(|c: char| c.is_whitespace() || c == '/')
            .next()
            .unwrap_or("");
        match name {
            "w:t" => im_text = !schliessend,
            "w:br" | "w:cr" => zeile.push('\n'),
            "w:tab" => zeile.push('\t'),
            "w:p" => {
                if schliessend {
                    absaetze.push(zeile.trim_end().to_string());
                    zeile.clear();
                }
            }
            _ => {}
        }
        rest = &rest[lt + gt + 1..];
    }
    if !zeile.trim().is_empty() {
        absaetze.push(zeile.trim_end().to_string());
    }
    let mut out = String::new();
    for (i, a) in absaetze.iter().enumerate() {
        if i > 0 {
            out.push_str("\n\n");
        }
        out.push_str(a);
    }
    out
}

// ---- PDF ------------------------------------------------------------------

fn entpacken(raw: &[u8]) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    flate2::read::ZlibDecoder::new(raw)
        .read_to_end(&mut out)
        .ok()?;
    if out.is_empty() {
        None
    } else {
        Some(out)
    }
}

/// Alle Inhaltsstroeme einer PDF-Datei, entpackt wo noetig.
fn stroeme(data: &[u8]) -> Vec<Vec<u8>> {
    let mut out = Vec::new();
    let mut i = 0;
    while let Some(p) = find(data, i, b"stream") {
        // Nach `stream` folgt ein Zeilenumbruch, danach die Daten.
        let mut begin = p + 6;
        if data.get(begin) == Some(&b'\r') {
            begin += 1;
        }
        if data.get(begin) == Some(&b'\n') {
            begin += 1;
        }
        let Some(ende) = find(data, begin, b"endstream") else {
            break;
        };
        let kopf = &data[p.saturating_sub(250)..p];
        let kopf = String::from_utf8_lossy(kopf);
        let roh = &data[begin..ende];
        // Bilder und eingebettete Schriften sind keine Inhaltsstroeme.
        let nutzlos = [
            "DCTDecode",
            "JPXDecode",
            "CCITTFaxDecode",
            "JBIG2Decode",
            "/Image",
            "FontFile",
            "Length1",
        ]
        .iter()
        .any(|k| kopf.contains(k));
        if nutzlos {
            i = ende + 9;
            continue;
        }
        if kopf.contains("FlateDecode") {
            if let Some(v) = entpacken(roh) {
                out.push(v);
            }
        } else {
            out.push(roh.to_vec());
        }
        i = ende + 9;
    }
    out
}

fn find(hay: &[u8], from: usize, needle: &[u8]) -> Option<usize> {
    if from >= hay.len() {
        return None;
    }
    hay[from..]
        .windows(needle.len())
        .position(|w| w == needle)
        .map(|p| p + from)
}

/// Wandelt einen PDF-Textstring in Zeichen. PDF kennt zwei Formen: literale
/// Strings in Klammern und Hex-Strings; UTF-16 mit Byte Order Mark kommt vor.
fn zeichen(bytes: &[u8]) -> String {
    if bytes.len() >= 2 && bytes[0] == 0xFE && bytes[1] == 0xFF {
        let mut s = String::new();
        for paar in bytes[2..].chunks(2) {
            if paar.len() == 2 {
                let c = u16::from_be_bytes([paar[0], paar[1]]);
                if let Some(ch) = char::from_u32(c as u32) {
                    s.push(ch);
                }
            }
        }
        return s;
    }
    bytes
        .iter()
        .map(|b| {
            let c = *b as u32;
            char::from_u32(c).unwrap_or(' ')
        })
        .collect()
}

fn literaler_string(d: &[u8], mut i: usize) -> (Vec<u8>, usize) {
    let mut out = Vec::new();
    let mut tiefe = 1;
    while i < d.len() {
        let c = d[i];
        i += 1;
        match c {
            b'\\' => {
                let Some(&n) = d.get(i) else { break };
                i += 1;
                match n {
                    b'n' => out.push(b'\n'),
                    b'r' => out.push(b'\r'),
                    b't' => out.push(b'\t'),
                    b'b' => out.push(8),
                    b'f' => out.push(12),
                    b'(' | b')' | b'\\' => out.push(n),
                    b'\r' => {
                        if d.get(i) == Some(&b'\n') {
                            i += 1;
                        }
                    }
                    b'\n' => {}
                    b'0'..=b'7' => {
                        let mut v = (n - b'0') as u32;
                        for _ in 0..2 {
                            match d.get(i) {
                                Some(c2 @ b'0'..=b'7') => {
                                    v = v * 8 + (*c2 - b'0') as u32;
                                    i += 1;
                                }
                                _ => break,
                            }
                        }
                        out.push(v as u8);
                    }
                    _ => out.push(n),
                }
            }
            b'(' => {
                tiefe += 1;
                out.push(c);
            }
            b')' => {
                tiefe -= 1;
                if tiefe == 0 {
                    break;
                }
                out.push(c);
            }
            _ => out.push(c),
        }
    }
    (out, i)
}

fn hex_string(d: &[u8], i: usize) -> (Vec<u8>, usize) {
    let mut out = Vec::new();
    let mut i = i;
    let mut high: Option<u8> = None;
    while i < d.len() {
        let c = d[i];
        i += 1;
        if c == b'>' {
            break;
        }
        let v = match c {
            b'0'..=b'9' => c - b'0',
            b'a'..=b'f' => c - b'a' + 10,
            b'A'..=b'F' => c - b'A' + 10,
            _ => continue,
        };
        match high.take() {
            None => high = Some(v),
            Some(h) => out.push(h * 16 + v),
        }
    }
    if let Some(h) = high {
        out.push(h * 16);
    }
    (out, i)
}

/// Liest den Text eines Inhaltsstroms: `Tj`, `TJ`, `'` und `"` zeigen Text an,
/// `Td`, `TD`, `T*` und `ET` setzen einen Zeilenumbruch.
pub fn seiten_text(d: &[u8]) -> String {
    let mut out = String::new();
    let mut stueck: Vec<u8> = Vec::new();
    let mut sammlung: Vec<Vec<u8>> = Vec::new();
    let mut i = 0;
    while i < d.len() {
        match d[i] {
            b'(' => {
                let (s, ni) = literaler_string(d, i + 1);
                sammlung.push(s);
                i = ni;
            }
            b'<' if d.get(i + 1) != Some(&b'<') => {
                let (s, ni) = hex_string(d, i + 1);
                sammlung.push(s);
                i = ni;
            }
            b'%' => {
                while i < d.len() && d[i] != b'\n' && d[i] != b'\r' {
                    i += 1;
                }
            }
            b'[' | b']' => {
                i += 1;
            }
            c if c.is_ascii_alphabetic() || c == b'\'' || c == b'"' || c == b'*' => {
                let start = i;
                i += 1;
                while i < d.len()
                    && (d[i].is_ascii_alphanumeric() || d[i] == b'*' || d[i] == b'\'' || d[i] == b'"')
                {
                    i += 1;
                }
                let ganz = &d[start..i];
                match ganz {
                    b"Tj" | b"'" | b"\"" => {
                        for s in sammlung.drain(..) {
                            stueck.extend_from_slice(&s);
                        }
                        out.push_str(&zeichen(&stueck));
                        stueck.clear();
                        if ganz != b"Tj" {
                            out.push('\n');
                        }
                    }
                    b"TJ" => {
                        for s in sammlung.drain(..) {
                            stueck.extend_from_slice(&s);
                        }
                        out.push_str(&zeichen(&stueck));
                        stueck.clear();
                    }
                    b"Td" | b"TD" | b"T*" | b"ET" | b"BT" => {
                        out.push('\n');
                    }
                    _ => {}
                }
            }
            _ => i += 1,
        }
    }
    // Leerzeilen zusammenfassen und Zeilen ohne Inhalt entfernen.
    let mut zeilen: Vec<&str> = Vec::new();
    for z in out.split('\n') {
        let t = z.trim();
        if !t.is_empty() {
            zeilen.push(t);
        }
    }
    zeilen.join("\n")
}

fn saeubern(s: &str) -> String {
    s.chars()
        .filter(|c| !c.is_control() || *c == '\n' || *c == '\t')
        .collect()
}

/// Der ganze Text einer PDF-Datei.
pub fn pdf_text(data: &[u8]) -> String {
    let mut teile: Vec<String> = Vec::new();
    for s in stroeme(data) {
        let t = seiten_text(&s);
        if !t.trim().is_empty() {
            teile.push(t);
        }
    }
    saeubern(&teile.join("\n\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Baut ein ZIP mit einem Eintrag; `packen` waehlt Deflate statt Speichern
    /// (Word schreibt DOCX mit Deflate).
    fn zip_bauen(name: &str, inhalt: &[u8], packen: bool) -> Vec<u8> {
        use flate2::write::DeflateEncoder;
        use flate2::Compression;
        use std::io::Write;
        let daten = if packen {
            let mut e = DeflateEncoder::new(Vec::new(), Compression::default());
            e.write_all(inhalt).unwrap();
            e.finish().unwrap()
        } else {
            inhalt.to_vec()
        };
        let methode: u16 = if packen { 8 } else { 0 };
        let crc = {
            // CRC32 wie im ZIP-Standard (polynomial 0xEDB88320).
            let mut c: u32 = 0xFFFF_FFFF;
            for b in inhalt {
                c ^= *b as u32;
                for _ in 0..8 {
                    c = if c & 1 != 0 { (c >> 1) ^ 0xEDB8_8320 } else { c >> 1 };
                }
            }
            !c
        };
        let mut out = Vec::new();
        out.extend_from_slice(b"PK");
        out.extend_from_slice(&20u16.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&methode.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&crc.to_le_bytes());
        out.extend_from_slice(&(daten.len() as u32).to_le_bytes());
        out.extend_from_slice(&(inhalt.len() as u32).to_le_bytes());
        out.extend_from_slice(&(name.len() as u16).to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(name.as_bytes());
        out.extend_from_slice(&daten);
        let cd = out.len();
        out.extend_from_slice(b"PK");
        out.extend_from_slice(&20u16.to_le_bytes());
        out.extend_from_slice(&20u16.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&methode.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&crc.to_le_bytes());
        out.extend_from_slice(&(daten.len() as u32).to_le_bytes());
        out.extend_from_slice(&(inhalt.len() as u32).to_le_bytes());
        out.extend_from_slice(&(name.len() as u16).to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&0u32.to_le_bytes());
        out.extend_from_slice(&0u32.to_le_bytes());
        out.extend_from_slice(name.as_bytes());
        let cd_len = out.len() - cd;
        out.extend_from_slice(b"PK");
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&1u16.to_le_bytes());
        out.extend_from_slice(&1u16.to_le_bytes());
        out.extend_from_slice(&(cd_len as u32).to_le_bytes());
        out.extend_from_slice(&(cd as u32).to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out
    }

    #[test]
    fn docx_wird_gelesen() {
        let xml = r#"<w:document><w:body>
            <w:p><w:r><w:t>Erster</w:t></w:r><w:r><w:t xml:space="preserve"> Absatz</w:t></w:r></w:p>
            <w:p><w:r><w:t>Zweiter &amp; letzter</w:t></w:r><w:r><w:br/><w:t>nach Umbruch</w:t></w:r></w:p>
            </w:body></w:document>"#;
        let zip = zip_bauen("word/document.xml", xml.as_bytes(), false);
        let roh = unzip(&zip, "word/document.xml").expect("Eintrag fehlt");
        let text = docx_text(&String::from_utf8_lossy(&roh));
        assert_eq!(text, "Erster Absatz

Zweiter & letzter
nach Umbruch");
    }

    #[test]
    fn gepacktes_docx_wird_gelesen() {
        let xml = "<w:body><w:p><w:r><w:t>Gepackt</w:t></w:r></w:p></w:body>";
        let zip = zip_bauen("word/document.xml", xml.as_bytes(), true);
        let roh = unzip(&zip, "word/document.xml").expect("Eintrag fehlt");
        assert_eq!(docx_text(&String::from_utf8_lossy(&roh)), "Gepackt");
    }

    /// Rundlauf: Der eigene PDF-Schreiber erzeugt eine Datei, die der Leser
    /// wieder in Text verwandelt.
    #[test]
    fn eigenes_pdf_wird_wieder_gelesen() {
        let pdf = crate::pdf::text_pdf("Erste Zeile
Zweite Zeile");
        let text = pdf_text(&pdf);
        assert!(text.contains("Erste Zeile"), "gelesen: {text:?}");
        assert!(text.contains("Zweite Zeile"), "gelesen: {text:?}");
    }

    #[test]
    fn pdf_text_wird_gelesen() {
        let strom = b"BT /F1 12 Tf 72 700 Td (Hallo Welt) Tj 0 -14 Td [(Zweite) -300 (Zeile)] TJ ET";
        let mut pdf = Vec::new();
        pdf.extend_from_slice(b"%PDF-1.4
1 0 obj
<< /Length 99 >>
stream
");
        pdf.extend_from_slice(strom);
        pdf.extend_from_slice(b"
endstream
endobj
");
        let text = pdf_text(&pdf);
        assert_eq!(text, "Hallo Welt
ZweiteZeile");
    }

    #[test]
    fn pdf_ohne_flatedaten_stuerzt_nicht_ab() {
        let mut pdf = Vec::new();
        pdf.extend_from_slice(b"<< /Filter /FlateDecode >>
stream
");
        pdf.extend_from_slice(b"BT (kaputt) Tj ET");
        pdf.extend_from_slice(b"
endstream
");
        let text = pdf_text(&pdf);
        assert!(text.is_empty() || text.contains("kaputt"));
    }
}
