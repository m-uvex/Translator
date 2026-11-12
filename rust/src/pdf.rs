//! Kleiner PDF-Schreiber fuer den Export.
//!
//! Die Vorlage druckt die Seite ueber `printToPDF`; hier entsteht ein PDF aus
//! reinem Text: A4, Helvetica, 60 px Rand, Wortumbruch, mehrere Seiten. Absaetze
//! bekommen einen kleinen Zwischenraum, damit der Ausdruck lesbar bleibt.

const SEITE_W: f32 = 595.0;
const SEITE_H: f32 = 842.0;
const RAND: f32 = 60.0;
const ZEILE: f32 = 15.0;
const SCHRIFT: f32 = 11.0;
/// Zeichen pro Zeile bei 11 px Helvetica auf 475 px Breite.
const SPALTE: usize = 92;

/// WinAnsi entspricht Latin-1; alles darueber wird zum Fragezeichen.
fn latin1(c: char) -> u8 {
    let n = c as u32;
    if n < 256 {
        n as u8
    } else {
        b'?'
    }
}

fn escape(s: &str, out: &mut Vec<u8>) {
    for c in s.chars() {
        match c {
            '(' | ')' | '\\' => {
                out.push(b'\\');
                out.push(latin1(c));
            }
            '\t' => out.push(b' '),
            _ => out.push(latin1(c)),
        }
    }
}

/// Bricht Text an Wortgrenzen um; sehr lange Woerter bleiben ganz.
fn umbrechen(text: &str) -> Vec<String> {
    let mut zeilen: Vec<String> = Vec::new();
    for absatz in text.split('\n') {
        let absatz = absatz.trim_end();
        if absatz.is_empty() {
            zeilen.push(String::new());
            continue;
        }
        let mut aktuell = String::new();
        for wort in absatz.split_whitespace() {
            let laenge = aktuell.chars().count();
            let zusatz = if laenge == 0 { 0 } else { 1 };
            if laenge + zusatz + wort.chars().count() > SPALTE && laenge > 0 {
                zeilen.push(std::mem::take(&mut aktuell));
            }
            if !aktuell.is_empty() {
                aktuell.push(' ');
            }
            aktuell.push_str(wort);
        }
        if !aktuell.is_empty() {
            zeilen.push(aktuell);
        }
    }
    if zeilen.is_empty() {
        zeilen.push(String::new());
    }
    zeilen
}

/// Baut ein PDF aus reinem Text.
pub fn text_pdf(text: &str) -> Vec<u8> {
    let zeilen = umbrechen(text);
    let pro_seite = (((SEITE_H - 2.0 * RAND) / ZEILE).floor() as usize).max(1);
    let mut seiten: Vec<Vec<String>> = Vec::new();
    let mut rest = zeilen;
    while !rest.is_empty() {
        let n = pro_seite.min(rest.len());
        seiten.push(rest.drain(..n).collect());
    }
    if seiten.is_empty() {
        seiten.push(vec![String::new()]);
    }

    // 1 Katalog, 2 Seitenbaum, 3 Schrift, danach je Seite Seite und Inhalt.
    let mut objekte: Vec<Vec<u8>> = Vec::new();
    objekte.push(b"<</Type/Catalog/Pages 2 0 R>>".to_vec());
    let kids = (0..seiten.len())
        .map(|i| format!("{} 0 R", 4 + i * 2))
        .collect::<Vec<_>>()
        .join(" ");
    objekte.push(format!("<</Type/Pages/Kids[{}]/Count {}>>", kids, seiten.len()).into_bytes());
    objekte.push(
        b"<</Type/Font/Subtype/Type1/BaseFont/Helvetica/Encoding/WinAnsiEncoding>>".to_vec(),
    );

    for (i, zeilen) in seiten.iter().enumerate() {
        let inhalt_id = 5 + i * 2;
        objekte.push(
            format!(
                "<</Type/Page/Parent 2 0 R/MediaBox[0 0 {} {}]/Resources<</Font<</F1 3 0 R>>>>/Contents {} 0 R>>",
                SEITE_W, SEITE_H, inhalt_id
            )
            .into_bytes(),
        );
        let mut strom: Vec<u8> = Vec::new();
        strom.extend_from_slice(format!("BT\n/F1 {} Tf\n{} TL\n", SCHRIFT, ZEILE).as_bytes());
        strom.extend_from_slice(
            format!("{} {} Td\n", RAND, SEITE_H - RAND - SCHRIFT).as_bytes(),
        );
        let mut leer = false;
        for zeile in zeilen {
            if zeile.is_empty() {
                if !leer {
                    strom.extend_from_slice(b"T*\n");
                    leer = true;
                }
                continue;
            }
            strom.push(b'(');
            escape(zeile, &mut strom);
            strom.extend_from_slice(b") Tj\nT*\n");
            leer = false;
        }
        strom.extend_from_slice(b"ET");
        let mut objekt = format!("<</Length {}>>\nstream\n", strom.len()).into_bytes();
        objekt.extend_from_slice(&strom);
        objekt.extend_from_slice(b"\nendstream");
        objekte.push(objekt);
    }

    let mut out: Vec<u8> = Vec::new();
    out.extend_from_slice(b"%PDF-1.4\n%\xe2\xe3\xcf\xd3\n");
    let mut offsets: Vec<usize> = vec![0; objekte.len() + 1];
    for (i, o) in objekte.iter().enumerate() {
        offsets[i + 1] = out.len();
        out.extend_from_slice(format!("{} 0 obj\n", i + 1).as_bytes());
        out.extend_from_slice(o);
        out.extend_from_slice(b"\nendobj\n");
    }
    let xref = out.len();
    out.extend_from_slice(format!("xref\n0 {}\n", objekte.len() + 1).as_bytes());
    out.extend_from_slice(b"0000000000 65535 f \n");
    for off in offsets.iter().skip(1) {
        out.extend_from_slice(format!("{:010} 00000 n \n", off).as_bytes());
    }
    out.extend_from_slice(
        format!(
            "trailer\n<</Size {}/Root 1 0 R>>\nstartxref\n{}\n%%EOF\n",
            objekte.len() + 1,
            xref
        )
        .as_bytes(),
    );
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pdf_hat_kopf_und_querverweis() {
        let datei = text_pdf("Hallo Welt\n\nZweiter Absatz mit Umlauten: äöü");
        let text = String::from_utf8_lossy(&datei);
        assert!(text.starts_with("%PDF-1.4"));
        assert!(text.contains("/Type/Page"));
        assert!(text.contains("startxref"));
        assert!(text.trim_end().ends_with("%%EOF"));
    }

    #[test]
    fn langer_text_ergibt_mehrere_seiten() {
        let lang = "Zeile\n".repeat(200);
        let datei = text_pdf(&lang);
        let text = String::from_utf8_lossy(&datei);
        assert!(text.matches("/Type/Page/").count() > 1);
    }

    #[test]
    fn klammern_werden_maskiert() {
        let datei = text_pdf("ein (Wort) mit \\ Zeichen");
        let text = String::from_utf8_lossy(&datei);
        assert!(text.contains("\\(Wort\\)"));
    }
}
