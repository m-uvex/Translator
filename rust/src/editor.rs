//! Bausteine des Editors: Wort-Diff, Formatierungshilfen und Undo-Snapshots.
//!
//! Der Editor arbeitet auf Markdown-Text. Formatierung heisst deshalb, Marker
//! um die Auswahl zu setzen oder zu entfernen; das Diff-Probelesen vergleicht
//! den alten und den vom Modell gelieferten Text wortweise und laesst jede
//! Aenderung einzeln annehmen oder verwerfen.

#[derive(Clone, Debug)]
pub struct Snapshot {
    pub text: String,
    pub sel: Option<(usize, usize)>,
    pub at: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Part {
    Same(String),
    Hunk { old: String, new: String },
}

/// Zustand des Probelesen-Fensters. `accepted[i]` gehoert zum i-ten Hunk.
#[derive(Clone, Debug)]
pub struct DiffState {
    pub parts: Vec<Part>,
    pub accepted: Vec<bool>,
    pub before: String,
    pub after: String,
}

impl DiffState {
    pub fn new(before: &str, after: &str) -> Self {
        let parts = diff_parts(before, after);
        let hunks = parts.iter().filter(|p| matches!(p, Part::Hunk { .. })).count();
        Self {
            parts,
            accepted: vec![true; hunks],
            before: before.to_string(),
            after: after.to_string(),
        }
    }

    pub fn hunk_count(&self) -> usize {
        self.accepted.len()
    }

    pub fn accepted_count(&self) -> usize {
        self.accepted.iter().filter(|a| **a).count()
    }

    /// Setzt den Text aus den getroffenen Entscheidungen zusammen.
    pub fn result(&self) -> String {
        let mut out = String::new();
        let mut i = 0usize;
        for part in &self.parts {
            match part {
                Part::Same(s) => out.push_str(s),
                Part::Hunk { old, new } => {
                    let take_new = self.accepted.get(i).copied().unwrap_or(true);
                    out.push_str(if take_new { new } else { old });
                    i += 1;
                }
            }
        }
        out
    }

    pub fn set_all(&mut self, value: bool) {
        for a in &mut self.accepted {
            *a = value;
        }
    }
}

/// Zerlegt Text in Woerter mit angehaengtem Leerraum. Die Zerlegung ist grob,
/// reicht aber fuer ein Wort-Diff und erhaelt alle Zeilenumbrueche.
fn tokens(text: &str) -> Vec<String> {
    text.split_inclusive(|c: char| c.is_whitespace())
        .map(|s| s.to_string())
        .collect()
}

/// Wort-Diff per LCS. Bei sehr grossen Texten wird auf einen gemeinsamen
/// Praefix/Suffix zurueckgefallen, damit der Speicherbedarf begrenzt bleibt.
fn diff_parts(before: &str, after: &str) -> Vec<Part> {
    let a = tokens(before);
    let b = tokens(after);
    let (n, m) = (a.len(), b.len());
    let mut segs: Vec<(u8, String)> = Vec::new(); // 0 = gleich, 1 = alt, 2 = neu

    if n.saturating_mul(m) > 4_000_000 {
        let mut pre = 0usize;
        while pre < n && pre < m && a[pre] == b[pre] {
            pre += 1;
        }
        let mut suf = 0usize;
        while suf < n - pre && suf < m - pre && a[n - 1 - suf] == b[m - 1 - suf] {
            suf += 1;
        }
        for t in &a[..pre] {
            segs.push((0, t.clone()));
        }
        for t in &a[pre..n - suf] {
            segs.push((1, t.clone()));
        }
        for t in &b[pre..m - suf] {
            segs.push((2, t.clone()));
        }
        for t in &a[n - suf..] {
            segs.push((0, t.clone()));
        }
    } else {
        // DP-Tabelle als u32, zeilenweise.
        let mut dp = vec![vec![0u32; m + 1]; n + 1];
        for i in (0..n).rev() {
            for j in (0..m).rev() {
                dp[i][j] = if a[i] == b[j] {
                    dp[i + 1][j + 1] + 1
                } else {
                    dp[i + 1][j].max(dp[i][j + 1])
                };
            }
        }
        let (mut i, mut j) = (0usize, 0usize);
        while i < n && j < m {
            if a[i] == b[j] {
                segs.push((0, a[i].clone()));
                i += 1;
                j += 1;
            } else if dp[i + 1][j] >= dp[i][j + 1] {
                segs.push((1, a[i].clone()));
                i += 1;
            } else {
                segs.push((2, b[j].clone()));
                j += 1;
            }
        }
        while i < n {
            segs.push((1, a[i].clone()));
            i += 1;
        }
        while j < m {
            segs.push((2, b[j].clone()));
            j += 1;
        }
    }

    // Aufeinanderfolgende Loeschungen und Einfuegungen zu einem Hunk buendeln.
    let mut parts: Vec<Part> = Vec::new();
    let mut same = String::new();
    let mut old = String::new();
    let mut new = String::new();
    let flush_same = |same: &mut String, parts: &mut Vec<Part>| {
        if !same.is_empty() {
            parts.push(Part::Same(std::mem::take(same)));
        }
    };
    let flush_hunk = |old: &mut String, new: &mut String, parts: &mut Vec<Part>| {
        if !old.is_empty() || !new.is_empty() {
            parts.push(Part::Hunk {
                old: std::mem::take(old),
                new: std::mem::take(new),
            });
        }
    };
    for (kind, text) in segs {
        match kind {
            0 => {
                flush_hunk(&mut old, &mut new, &mut parts);
                same.push_str(&text);
            }
            1 => {
                flush_same(&mut same, &mut parts);
                old.push_str(&text);
            }
            _ => {
                flush_same(&mut same, &mut parts);
                new.push_str(&text);
            }
        }
    }
    flush_same(&mut same, &mut parts);
    flush_hunk(&mut old, &mut new, &mut parts);
    parts
}

pub fn char_to_byte(s: &str, char_idx: usize) -> usize {
    s.char_indices()
        .nth(char_idx)
        .map(|(i, _)| i)
        .unwrap_or(s.len())
}

pub fn selected<'a>(text: &'a str, sel: (usize, usize)) -> &'a str {
    let a = char_to_byte(text, sel.0.min(sel.1));
    let b = char_to_byte(text, sel.0.max(sel.1));
    &text[a..b]
}

/// Setzt Marker um die Auswahl oder entfernt sie wieder.
pub fn wrap_selection(
    text: &str,
    sel: (usize, usize),
    left: &str,
    right: &str,
) -> (String, (usize, usize)) {
    let chars: Vec<char> = text.chars().collect();
    let n = chars.len();
    let a = sel.0.min(sel.1).min(n);
    let b = sel.0.max(sel.1).min(n);
    let inner: String = chars[a..b].iter().collect();
    let lc = left.chars().count();
    let rc = right.chars().count();

    if b > a && inner.starts_with(left) && inner.ends_with(right) && inner.chars().count() >= lc + rc
    {
        let stripped: String = chars[a + lc..b - rc].iter().collect();
        let mut out: String = chars[..a].iter().collect();
        out.push_str(&stripped);
        out.extend(chars[b..].iter());
        let na = a;
        let nb = a + stripped.chars().count();
        (out, (na, nb))
    } else if b > a {
        let mut out: String = chars[..a].iter().collect();
        out.push_str(left);
        out.push_str(&inner);
        out.push_str(right);
        out.extend(chars[b..].iter());
        let na = a + lc;
        let nb = na + inner.chars().count();
        (out, (na, nb))
    } else {
        let mut out: String = chars[..a].iter().collect();
        out.push_str(left);
        out.push_str(right);
        out.extend(chars[a..].iter());
        (out, (a + lc, a + lc))
    }
}

/// Schaltet ein Zeilenpraefix (Ueberschrift, Liste, Zitat) fuer alle
/// beruehrten Zeilen ein oder aus.
pub fn toggle_line_prefix(
    text: &str,
    sel: (usize, usize),
    prefix: &str,
) -> (String, (usize, usize)) {
    let chars: Vec<char> = text.chars().collect();
    let n = chars.len();
    let a = sel.0.min(sel.1).min(n);
    let b = sel.0.max(sel.1).min(n);
    let start = chars[..a]
        .iter()
        .rposition(|c| *c == '\n')
        .map(|i| i + 1)
        .unwrap_or(0);
    let end = chars[b..]
        .iter()
        .position(|c| *c == '\n')
        .map(|i| b + i)
        .unwrap_or(n);
    let block: String = chars[start..end].iter().collect();
    let mut lines: Vec<String> = block.split('\n').map(|l| l.to_string()).collect();
    let relevant: Vec<&String> = lines.iter().filter(|l| !l.trim().is_empty()).collect();
    let all = !relevant.is_empty() && relevant.iter().all(|l| l.starts_with(prefix));
    if all {
        for line in lines.iter_mut() {
            if let Some(rest) = line.strip_prefix(prefix) {
                *line = rest.to_string();
            }
        }
    } else {
        for line in lines.iter_mut() {
            if !line.trim().is_empty() {
                line.insert_str(0, prefix);
            }
        }
    }
    let new_block = lines.join("\n");
    let mut out: String = chars[..start].iter().collect();
    out.push_str(&new_block);
    out.extend(chars[end..].iter());
    let delta = new_block.chars().count() as isize - block.chars().count() as isize;
    let nb = (end as isize + delta).max(0) as usize;
    (out, (start, nb))
}

/// Entfernt einfache Markdown-Marker innerhalb der Auswahl.
pub fn strip_markup(text: &str, sel: (usize, usize)) -> (String, (usize, usize)) {
    let a = char_to_byte(text, sel.0.min(sel.1));
    let b = char_to_byte(text, sel.0.max(sel.1));
    let mut inner = text[a..b].to_string();
    for marker in ["**", "__", "~~", "`", "<u>", "</u>", "*", "_"] {
        inner = inner.replace(marker, "");
    }
    let mut out = String::new();
    out.push_str(&text[..a]);
    out.push_str(&inner);
    out.push_str(&text[b..]);
    let na = sel.0.min(sel.1);
    let nb = na + inner.chars().count();
    (out, (na, nb))
}

/// Baut einen Markdown-Link; die Adresse wird zur Bearbeitung markiert.
pub fn make_link(text: &str, sel: (usize, usize)) -> (String, (usize, usize)) {
    let chars: Vec<char> = text.chars().collect();
    let n = chars.len();
    let a = sel.0.min(sel.1).min(n);
    let b = sel.0.max(sel.1).min(n);
    let label: String = if b > a {
        chars[a..b].iter().collect()
    } else {
        String::new()
    };
    let mut out: String = chars[..a].iter().collect();
    let head = format!("[{}](", label);
    out.push_str(&head);
    let url_pos = out.chars().count();
    out.push_str("https://");
    out.extend(chars[b..].iter());
    let url_end = url_pos + "https://".chars().count();
    // Die schliessende Klammer wird unmittelbar nach der Adresse eingefuegt.
    let mut chars_out: Vec<char> = out.chars().collect();
    chars_out.insert(url_end, ')');
    (chars_out.into_iter().collect(), (url_pos, url_end))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diff_erkennt_einfuegung() {
        let d = DiffState::new("Der Hund schläft.", "Der große Hund schläft.");
        assert_eq!(d.hunk_count(), 1);
        assert_eq!(d.result(), "Der große Hund schläft.");
        let mut abgelehnt = d.clone();
        abgelehnt.set_all(false);
        assert_eq!(abgelehnt.result(), "Der Hund schläft.");
    }

    #[test]
    fn diff_ohne_unterschied() {
        let d = DiffState::new("Gleich", "Gleich");
        assert_eq!(d.hunk_count(), 0);
        assert_eq!(d.result(), "Gleich");
    }

    #[test]
    fn diff_mehrere_stellen() {
        let d = DiffState::new("a b c d", "a x c y");
        assert_eq!(d.hunk_count(), 2);
        assert_eq!(d.result(), "a x c y");
    }

    #[test]
    fn formatierung_setzt_und_entfernt() {
        let (t, sel) = wrap_selection("Hallo Welt", (0, 5), "**", "**");
        assert_eq!(t, "**Hallo** Welt");
        let inner = &t[char_to_byte(&t, sel.0)..char_to_byte(&t, sel.1)];
        assert_eq!(inner, "Hallo");
        let (zurueck, _) = wrap_selection(&t, (0, 9), "**", "**");
        assert_eq!(zurueck, "Hallo Welt");
    }

    #[test]
    fn formatierung_ohne_auswahl() {
        let (t, sel) = wrap_selection("Text", (4, 4), "*", "*");
        assert_eq!(t, "Text**");
        assert_eq!(sel, (5, 5));
    }

    #[test]
    fn zeilenpraefix_schaltet_um() {
        let (t, _) = toggle_line_prefix("Zeile", (0, 0), "- ");
        assert_eq!(t, "- Zeile");
        let (zurueck, _) = toggle_line_prefix(&t, (0, 0), "- ");
        assert_eq!(zurueck, "Zeile");
    }

    #[test]
    fn markup_entfernen() {
        let (t, _) = strip_markup("**fett** und `Code`", (0, 19));
        assert_eq!(t, "fett und Code");
    }

    #[test]
    fn link_umschliesst_auswahl() {
        let (t, sel) = make_link("Text", (0, 4));
        assert_eq!(t, "[Text](https://)");
        let url = &t[char_to_byte(&t, sel.0)..char_to_byte(&t, sel.1)];
        assert_eq!(url, "https://");
    }
}
