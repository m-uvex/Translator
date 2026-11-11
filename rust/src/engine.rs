//! Sprachlisten, Prompt-Aufbau und Schimpfwortfilter.
//!
//! Die Regeln sind aus der Electron-Fassung uebernommen: vier Modelltypen
//! (NLLB, OMT, TranslateGemma, allgemeines LLM), ein eigener Prompt fuer
//! TranslateGemma mit zwei Leerzeilen, und ein Filter, der nur die Listen der
//! Ausgangs- und Zielsprache plus Englisch verwendet.

use crate::settings::Style;

pub const OLLAMA_LANGS: &[(&str, &str, &str)] = &[
    ("de", "Deutsch", "German"),
    ("en", "English", "English"),
    ("fr", "Français", "French"),
    ("es", "Español", "Spanish"),
    ("it", "Italiano", "Italian"),
    ("pt", "Português", "Portuguese"),
    ("nl", "Nederlands", "Dutch"),
    ("pl", "Polski", "Polish"),
    ("cs", "Čeština", "Czech"),
    ("sv", "Svenska", "Swedish"),
    ("da", "Dansk", "Danish"),
    ("no", "Norsk", "Norwegian"),
    ("fi", "Suomi", "Finnish"),
    ("tr", "Türkçe", "Turkish"),
    ("ru", "Русский", "Russian"),
    ("uk", "Українська", "Ukrainian"),
    ("el", "Ελληνικά", "Greek"),
    ("ro", "Română", "Romanian"),
    ("hu", "Magyar", "Hungarian"),
    ("zh", "中文", "Chinese"),
    ("ja", "日本語", "Japanese"),
    ("ko", "한국어", "Korean"),
    ("ar", "العربية", "Arabic"),
    ("hi", "हिन्दी", "Hindi"),
    ("id", "Bahasa Indonesia", "Indonesian"),
    ("vi", "Tiếng Việt", "Vietnamese"),
];

pub fn is_mt(model: &str) -> bool {
    model.starts_with("nllb:") || model.starts_with("omt:")
}

pub fn is_tg(model: &str) -> bool {
    model.to_lowercase().contains("translategemma")
}

pub fn is_llm(model: &str) -> bool {
    !is_mt(model) && !is_tg(model)
}

/// Anzeigename einer Sprache; NLLB-Codes wie `deu_Latn` werden aufgeloest.
pub fn lang_label(code: &str, nllb: &[(String, String)]) -> String {
    if code == "auto" {
        return String::new();
    }
    let iso = code.split('_').next().unwrap_or(code);
    if let Some((_, native, _)) = OLLAMA_LANGS.iter().find(|(c, _, _)| *c == iso) {
        return native.to_string();
    }
    nllb.iter()
        .find(|(c, _)| c == code)
        .map(|(_, n)| n.clone())
        .unwrap_or_else(|| code.to_string())
}

pub fn eng_name(code: &str, nllb: &[(String, String)]) -> String {
    let iso = code.split('_').next().unwrap_or(code);
    if let Some((_, _, eng)) = OLLAMA_LANGS.iter().find(|(c, _, _)| *c == iso) {
        return eng.to_string();
    }
    nllb.iter()
        .find(|(c, _)| c == code)
        .map(|(_, n)| n.clone())
        .unwrap_or_else(|| code.to_string())
}

fn style_sentence(style: &Style) -> String {
    let mut parts: Vec<&str> = Vec::new();
    match style.kind.as_str() {
        "literal" => parts.push("Translate as literally as possible, staying close to the source wording and structure."),
        _ => parts.push("Translate naturally and idiomatically."),
    }
    match style.formal.as_str() {
        "formal" => parts.push("Use a formal register (e.g. formal address forms)."),
        "informal" => parts.push("Use an informal, casual register (e.g. informal address forms)."),
        _ => {}
    }
    match style.gender.as_str() {
        "neutral" => parts.push("Use gender-neutral wording for the addressed person."),
        "male" => parts.push("The addressed person is male; use matching grammatical forms."),
        "female" => parts.push("The addressed person is female; use matching grammatical forms."),
        _ => {}
    }
    parts.join(" ")
}

pub struct PromptCtx<'a> {
    pub src: &'a str,
    pub tgt: &'a str,
    pub style: &'a Style,
    pub ctxt: &'a str,
    pub gloss: Vec<(String, String)>,
    pub has_images: bool,
    pub nllb: &'a [(String, String)],
}

pub fn user_prompt_for(text: &str, p: &PromptCtx) -> String {
    let from = if p.src == "auto" {
        String::new()
    } else {
        format!("from {} ", eng_name(p.src, p.nllb))
    };
    let mut out = format!(
        "Translate {}into {}. {}",
        from,
        eng_name(p.tgt, p.nllb),
        style_sentence(p.style)
    );
    let writer = match p.style.writer.as_str() {
        "male" => Some("male"),
        "female" => Some("female"),
        "neutral" => Some("gender-neutral"),
        _ => None,
    };
    if let Some(w) = writer {
        out.push_str(&format!(
            " The writer is {}; use matching grammatical forms for first-person references.",
            w
        ));
    }
    let gl: Vec<&(String, String)> = p
        .gloss
        .iter()
        .filter(|(s, t)| !s.trim().is_empty() && !t.trim().is_empty())
        .collect();
    if !gl.is_empty() {
        out.push_str("\nGlossary (always use exactly these translations for the listed terms):\n");
        for (s, t) in gl {
            out.push_str(&format!("- \"{}\" → \"{}\"\n", s.trim(), t.trim()));
        }
    }
    if !p.ctxt.trim().is_empty() {
        out.push_str(&format!(
            "\nBackground context from the user (do not translate it): {}",
            p.ctxt.trim()
        ));
    }
    if p.has_images {
        out.push_str("\nAlso translate any text visible in the attached image(s).");
    }
    if !text.is_empty() {
        out.push_str(&format!("\n\n<text>\n{}\n</text>", text));
    }
    out
}

/// TranslateGemma ist auf genau zwei Leerzeilen vor dem Text trainiert.
pub fn tg_prompt(text: &str, src: &str, tgt: &str, nllb: &[(String, String)]) -> String {
    let a = eng_name(src, nllb);
    let b = eng_name(tgt, nllb);
    format!(
        "You are a professional {} ({}) to {} ({}) translator. Your goal is to accurately convey the meaning and nuances of the original {} text while adhering to {} grammar, vocabulary, and cultural sensitivities.\nProduce only the {} translation, without any additional explanations or commentary. Please translate the following {} text into {}:\n\n\n{}",
        a, src, b, tgt, a, b, b, a, b, text
    )
}

pub fn strip_stars(out: &str, src: &str) -> String {
    if src.contains("**") {
        out.to_string()
    } else {
        out.replace("**", "")
    }
}

const PROF_EN: &[&str] = &[
    "fuck", "fucking", "fucked", "fucker", "motherfucker", "shit", "shitty", "bullshit", "bitch",
    "bitches", "asshole", "assholes", "bastard", "cunt", "dickhead", "pussy", "slut", "whore",
    "wanker", "twat", "retard", "faggot", "nigger", "bollocks", "crap", "piss",
];
const PROF_DE: &[&str] = &[
    "scheiße", "scheisse", "scheiss", "scheiß", "arschloch", "arsch", "hurensohn", "hure",
    "schlampe", "wichser", "fotze", "missgeburt", "spast", "verdammt", "ficken", "gefickt",
    "schwuchtel", "nutte", "kacke", "pissen", "fresse", "drecksau", "miststück", "trottel",
    "vollidiot", "vollpfosten",
];
const PROF_NL: &[&str] = &["kut", "kanker", "tering", "tyfus", "klootzak", "lul", "godverdomme", "neuken", "mongool", "schijt"];
const PROF_ES: &[&str] = &["mierda", "puta", "puto", "coño", "cabrón", "joder", "pendejo", "chingar", "gilipollas", "maricón", "verga", "zorra", "boludo", "pelotudo"];
const PROF_TR: &[&str] = &["siktir", "sikeyim", "orospu", "yarrak", "ibne", "pezevenk", "sürtük", "gerizekalı", "bok", "şerefsiz"];
const PROF_AR: &[&str] = &["شرموطة", "قحبة", "كس", "طيز", "عرص", "خول", "زب", "نيك", "حقير", "سافل"];

fn prof_words(lang: &str) -> &'static [&'static str] {
    match lang {
        "de" => PROF_DE,
        "nl" => PROF_NL,
        "es" => PROF_ES,
        "tr" => PROF_TR,
        "ar" => PROF_AR,
        _ => PROF_EN,
    }
}

fn lang_key(code: &str) -> &str {
    let iso = code.split('_').next().unwrap_or(code);
    match iso {
        "deu" => "de",
        "eng" => "en",
        "nld" => "nl",
        "spa" => "es",
        "tur" => "tr",
        "ara" => "ar",
        "fra" => "fr",
        "ita" => "it",
        "por" => "pt",
        other => other,
    }
}

fn mask(word: &str, style: &str) -> String {
    const SYMBOLS: [char; 6] = ['@', '#', '$', '%', '&', '!'];
    let mut i = 0usize;
    word.chars()
        .map(|c| {
            if c.is_alphanumeric() {
                let r = if style == "symbols" {
                    SYMBOLS[i % SYMBOLS.len()]
                } else {
                    '*'
                };
                i += 1;
                r
            } else {
                c
            }
        })
        .collect()
}

/// Ersetzt Schimpfwoerter durch Sternchen oder Sonderzeichen. Kurze Woerter
/// (< 3 Zeichen) und Technikzeichenketten (URLs, E-Mails, Pfade) bleiben
/// unangetastet; das verhindert Treffer in `class` oder `Scunthorpe`.
pub fn filter_profanity(text: &str, tgt: &str, src: &str, style: &str) -> String {
    let langs = [
        lang_key(tgt).to_string(),
        lang_key(src).to_string(),
        "en".to_string(),
    ];
    let mut out = String::with_capacity(text.len());
    let mut word = String::new();
    let flush = |word: &mut String, out: &mut String| {
        if word.is_empty() {
            return;
        }
        let low = word.to_lowercase();
        let hit = langs
            .iter()
            .any(|l| prof_words(l).iter().any(|w| *w == low.as_str()));
        if hit && low.chars().count() >= 3 && !word.contains('@') && !word.contains('/') && !word.contains('\\') {
            out.push_str(&mask(word, style));
        } else {
            out.push_str(word);
        }
        word.clear();
    };
    for c in text.chars() {
        if c.is_alphanumeric() || c == '\'' || c == '’' {
            word.push(c);
        } else {
            flush(&mut word, &mut out);
            out.push(c);
        }
    }
    flush(&mut word, &mut out);
    out
}
