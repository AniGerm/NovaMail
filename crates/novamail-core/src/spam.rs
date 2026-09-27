//! Local spam detection: heuristics + trainable Bayesian filter.
//!
//! Offline libraries on crates.io include `bayespam` and `mailrs-bayes`
//! (Graham/Robinson token scoring). NovaMail keeps a small in-tree Bayesian
//! model plus content heuristics that large providers often run on the
//! server (DNSBL, Rspamd). Here we score what we can see without the network:
//! keywords, punctuation storms, script mixing, obfuscation, and URL noise.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::{CoreError, CoreResult};

const MODEL_FILE: &str = "spam-model.json";
const DEFAULT_THRESHOLD: f32 = 0.78;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpamSettings {
    pub enabled: bool,
    /// Auto-move scored spam into the Junk folder after sync.
    pub auto_move: bool,
    /// Probability threshold in `[0, 1]`.
    pub threshold: f32,
    /// Extra local signals: special chars, script mixing, obfuscation, URL density.
    #[serde(default = "default_true")]
    pub strict_heuristics: bool,
}

fn default_true() -> bool {
    true
}

impl Default for SpamSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            auto_move: true,
            threshold: DEFAULT_THRESHOLD,
            strict_heuristics: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct TokenStat {
    spam: u32,
    ham: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct SpamModel {
    spam_messages: u32,
    ham_messages: u32,
    tokens: HashMap<String, TokenStat>,
}

#[derive(Debug, Clone)]
pub struct SpamVerdict {
    pub score: f32,
    pub is_spam: bool,
    pub reasons: Vec<String>,
}

/// Punctuation / symbol characters that spam often overuses.
const SPAM_SYMBOLS: &[char] = &[
    '!', '?', '$', '€', '£', '¥', '%', '*', '#', '~', '|', '^', '`', '¡', '¿', '§', '¤', '†',
    '‡', '•', '…', '‼', '⁇', '⁈', '⁉', '★', '☆', '✦', '✧', '❖', '✔', '✖', '➤', '➔', '➜',
];

/// Zero-width / invisible characters used to break filters.
const INVISIBLE: &[char] = &[
    '\u{200B}', // zero width space
    '\u{200C}', // zero width non-joiner
    '\u{200D}', // zero width joiner
    '\u{2060}', // word joiner
    '\u{FEFF}', // BOM / ZWNBSP
    '\u{00AD}', // soft hyphen
    '\u{180E}', // mongolian vowel separator
    '\u{2062}', // invisible times
    '\u{2063}', // invisible separator
];

fn model_path(data_dir: &Path) -> PathBuf {
    data_dir.join(MODEL_FILE)
}

fn load_model(data_dir: &Path) -> SpamModel {
    let path = model_path(data_dir);
    match fs::read_to_string(&path) {
        Ok(raw) => serde_json::from_str(&raw).unwrap_or_default(),
        Err(_) => SpamModel::default(),
    }
}

fn save_model(data_dir: &Path, model: &SpamModel) -> CoreResult<()> {
    fs::create_dir_all(data_dir).map_err(|e| CoreError::Message(e.to_string()))?;
    let raw =
        serde_json::to_string_pretty(model).map_err(|e| CoreError::Message(e.to_string()))?;
    fs::write(model_path(data_dir), raw).map_err(|e| CoreError::Message(e.to_string()))?;
    Ok(())
}

fn tokenize(text: &str) -> Vec<String> {
    text.to_lowercase()
        .split(|c: char| !c.is_alphanumeric() && c != '@' && c != '.' && c != '-' && c != '_')
        .filter(|t| t.len() >= 3 && t.len() <= 48)
        .map(|t| t.to_string())
        .collect()
}

fn keyword_boost(subject: &str, body: &str) -> (f32, Vec<String>) {
    let mut boost = 0.0f32;
    let mut reasons = Vec::new();
    let blob = format!("{subject}\n{body}").to_ascii_lowercase();

    let spammy = [
        "viagra",
        "cialis",
        "crypto giveaway",
        "you have won",
        "lottery",
        "nigerian",
        "wire transfer",
        "act now",
        "limited time",
        "click here",
        "free money",
        "bitcoin",
        "xxx",
        "porn",
        "casino",
        "100% free",
        "dear customer",
        "verify your account",
        "urgent action required",
        "password expires",
        "konto gesperrt",
        "sofort handeln",
        "gewinnspiel",
        "gratis",
        "nur heute",
        "klicken sie hier",
        "ihre zahlung",
        "rechnung angehängt",
        "paket nicht zugestellt",
        "double your",
        "make money fast",
        "work from home",
        "congratulations you",
        "selected winner",
    ];
    for needle in spammy {
        if blob.contains(needle) {
            boost += 0.11;
            reasons.push(format!("keyword:{needle}"));
        }
    }
    (boost, reasons)
}

fn punctuation_storm(text: &str) -> (f32, Vec<String>) {
    let mut boost = 0.0f32;
    let mut reasons = Vec::new();
    let chars: Vec<char> = text.chars().collect();
    if chars.is_empty() {
        return (0.0, reasons);
    }
    let symbol_hits = chars.iter().filter(|c| SPAM_SYMBOLS.contains(c)).count();
    let ratio = symbol_hits as f32 / chars.len() as f32;
    if ratio >= 0.08 && symbol_hits >= 6 {
        boost += 0.14;
        reasons.push(format!("symbols:dense({symbol_hits})"));
    } else if symbol_hits >= 12 {
        boost += 0.1;
        reasons.push(format!("symbols:many({symbol_hits})"));
    }

    let bangs = text.matches('!').count();
    let questions = text.matches('?').count();
    if bangs >= 4 {
        boost += 0.1;
        reasons.push(format!("punct:exclamations({bangs})"));
    }
    if questions >= 4 {
        boost += 0.06;
        reasons.push(format!("punct:questions({questions})"));
    }
    if text.contains("!!!") || text.contains("???") {
        boost += 0.06;
        reasons.push("punct:triple".into());
    }
    (boost, reasons)
}

fn invisible_chars(text: &str) -> (f32, Vec<String>) {
    let mut count = 0u32;
    for ch in text.chars() {
        if INVISIBLE.contains(&ch) || (ch.is_control() && ch != '\n' && ch != '\r' && ch != '\t')
        {
            count += 1;
        }
    }
    if count >= 3 {
        (0.18, vec![format!("consistency:invisible({count})")])
    } else if count >= 1 {
        (0.08, vec![format!("consistency:invisible({count})")])
    } else {
        (0.0, Vec::new())
    }
}

/// Mixed Latin + Cyrillic/Greek lookalikes (common phishing obfuscation).
fn script_mixing(text: &str) -> (f32, Vec<String>) {
    let mut latin = 0u32;
    let mut cyrillic = 0u32;
    let mut greek = 0u32;
    let mut other_letter = 0u32;
    for ch in text.chars() {
        if !ch.is_alphabetic() {
            continue;
        }
        match ch {
            'A'..='Z' | 'a'..='z' | 'Ä' | 'Ö' | 'Ü' | 'ä' | 'ö' | 'ü' | 'ß' => latin += 1,
            '\u{0400}'..='\u{04FF}' => cyrillic += 1,
            '\u{0370}'..='\u{03FF}' => greek += 1,
            _ => other_letter += 1,
        }
    }
    let mut boost = 0.0f32;
    let mut reasons = Vec::new();
    // Even a single lookalike letter in an otherwise Latin string is phishing-typical.
    if latin >= 3 && cyrillic >= 1 {
        boost += 0.2;
        reasons.push("consistency:latin+cyrillic".into());
    }
    if latin >= 3 && greek >= 1 {
        boost += 0.12;
        reasons.push("consistency:latin+greek".into());
    }
    let letters = latin + cyrillic + greek + other_letter;
    if letters >= 10 {
        let foreign = cyrillic + greek + other_letter;
        if foreign as f32 / letters as f32 >= 0.25 && latin >= 3 {
            boost += 0.1;
            reasons.push("consistency:mixed-scripts".into());
        }
    }
    (boost.min(0.28), reasons)
}

/// Spaced / dotted letter obfuscation: "v i a g r a", "V.I.A.G.R.A".
fn obfuscation_patterns(text: &str) -> (f32, Vec<String>) {
    let mut boost = 0.0f32;
    let mut reasons = Vec::new();
    let lower = text.to_ascii_lowercase();

    let spaced = spaced_letter_runs(&lower);
    if spaced >= 1 {
        boost += 0.16;
        reasons.push(format!("obfuscation:spaced-letters({spaced})"));
    }

    if lower.contains("f.r.e.e")
        || lower.contains("v.i.a.g")
        || lower.contains("c.l.i.c.k")
        || lower.contains("w.i.n.n")
    {
        boost += 0.12;
        reasons.push("obfuscation:dotted-keyword".into());
    }

    (boost, reasons)
}

fn spaced_letter_runs(text: &str) -> u32 {
    let bytes = text.as_bytes();
    let mut hits = 0u32;
    let mut i = 0;
    while i + 8 < bytes.len() {
        if bytes[i].is_ascii_alphabetic() {
            let mut letters = 1u32;
            let mut j = i + 1;
            while j + 1 < bytes.len() {
                let sep = bytes[j];
                let next = bytes[j + 1];
                if (sep == b' ' || sep == b'.' || sep == b'-') && next.is_ascii_alphabetic() {
                    letters += 1;
                    j += 2;
                } else {
                    break;
                }
            }
            if letters >= 5 {
                hits += 1;
                i = j;
                continue;
            }
        }
        i += 1;
    }
    hits
}

fn url_and_noise(body: &str) -> (f32, Vec<String>) {
    let mut boost = 0.0f32;
    let mut reasons = Vec::new();
    let lower = body.to_ascii_lowercase();
    let http_count = lower.matches("http://").count() + lower.matches("https://").count();
    if http_count >= 5 {
        boost += 0.12;
        reasons.push(format!("urls:many({http_count})"));
    } else if http_count >= 3 {
        boost += 0.06;
        reasons.push(format!("urls:several({http_count})"));
    }

    let shorteners = [
        "bit.ly/",
        "tinyurl.com/",
        "t.co/",
        "goo.gl/",
        "ow.ly/",
        "is.gd/",
        "cutt.ly/",
    ];
    for s in shorteners {
        if lower.contains(s) {
            boost += 0.08;
            reasons.push(format!("urls:shortener:{s}"));
            break;
        }
    }

    let alnum: String = body.chars().filter(|c| c.is_alphanumeric()).collect();
    if http_count >= 1 && alnum.len() < 40 {
        boost += 0.1;
        reasons.push("consistency:link-only".into());
    }
    (boost, reasons)
}

fn caps_ratio(subject: &str) -> (f32, Vec<String>) {
    let caps = subject.chars().filter(|c| c.is_ascii_uppercase()).count();
    let letters = subject.chars().filter(|c| c.is_ascii_alphabetic()).count();
    if letters >= 8 && caps as f32 / letters as f32 > 0.7 {
        (0.15, vec!["subject:mostly-caps".into()])
    } else {
        (0.0, Vec::new())
    }
}

fn from_suspicious(from_email: &str) -> (f32, Vec<String>) {
    let from_l = from_email.to_ascii_lowercase();
    let mut boost = 0.0f32;
    let mut reasons = Vec::new();
    if from_l.ends_with(".zip")
        || from_l.ends_with(".ru.com")
        || from_l.contains("mailer-daemon@")
    {
        boost += 0.1;
        reasons.push("from:suspicious".into());
    }
    if let Some(local) = from_l.split('@').next() {
        let digits = local.chars().filter(|c| c.is_ascii_digit()).count();
        if local.len() >= 8 && digits as f32 / local.len() as f32 >= 0.4 {
            boost += 0.08;
            reasons.push("from:digit-heavy".into());
        }
    }
    (boost, reasons)
}

fn heuristic_boost(subject: &str, from_email: &str, body: &str, strict: bool) -> (f32, Vec<String>) {
    let mut boost = 0.0f32;
    let mut reasons = Vec::new();
    let combined = format!("{subject}\n{body}");

    for (b, r) in [
        keyword_boost(subject, body),
        caps_ratio(subject),
        from_suspicious(from_email),
    ] {
        boost += b;
        reasons.extend(r);
    }

    if strict {
        for (b, r) in [
            punctuation_storm(&combined),
            invisible_chars(&combined),
            script_mixing(subject),
            script_mixing(&combined),
            obfuscation_patterns(&combined),
            url_and_noise(body),
        ] {
            boost += b;
            reasons.extend(r);
        }
    } else {
        let (b, r) = punctuation_storm(subject);
        boost += b * 0.7;
        reasons.extend(r);
    }

    (boost.min(0.72), reasons)
}

fn bayes_score(model: &SpamModel, tokens: &[String]) -> Option<f32> {
    if model.spam_messages < 3 || model.ham_messages < 3 {
        return None;
    }
    let mut log_odds = 0.0f64;
    let mut used = 0u32;
    for token in tokens {
        let Some(stat) = model.tokens.get(token) else {
            continue;
        };
        let spam_f = (stat.spam as f64 + 1.0) / (model.spam_messages as f64 + 2.0);
        let ham_f = (stat.ham as f64 + 1.0) / (model.ham_messages as f64 + 2.0);
        let p = (spam_f / (spam_f + ham_f)).clamp(0.01, 0.99);
        log_odds += (p / (1.0 - p)).ln();
        used += 1;
    }
    if used == 0 {
        return None;
    }
    let odds = log_odds.exp();
    Some((odds / (1.0 + odds)) as f32)
}

pub fn score_message(
    data_dir: &Path,
    settings: &SpamSettings,
    subject: &str,
    from_email: &str,
    body: &str,
) -> SpamVerdict {
    if !settings.enabled {
        return SpamVerdict {
            score: 0.0,
            is_spam: false,
            reasons: vec!["disabled".into()],
        };
    }
    let model = load_model(data_dir);
    let blob = format!("{subject}\n{from_email}\n{body}");
    let tokens = tokenize(&blob);
    let (boost, mut reasons) =
        heuristic_boost(subject, from_email, body, settings.strict_heuristics);
    let bayes_opt = bayes_score(&model, &tokens);
    let bayes = bayes_opt.unwrap_or(0.35);
    if let Some(b) = bayes_opt {
        reasons.push(format!("bayes:{b:.2}"));
    } else {
        reasons.push("bayes:cold-start".into());
    }
    // Heuristics carry more weight so cold-start Bayes does not wash them out.
    let score = (bayes * 0.50 + boost * 0.75).clamp(0.0, 1.0);
    SpamVerdict {
        is_spam: score >= settings.threshold,
        score,
        reasons,
    }
}

pub fn train(
    data_dir: &Path,
    subject: &str,
    from_email: &str,
    body: &str,
    is_spam: bool,
) -> CoreResult<()> {
    let mut model = load_model(data_dir);
    let blob = format!("{subject}\n{from_email}\n{body}");
    let tokens = tokenize(&blob);
    if is_spam {
        model.spam_messages = model.spam_messages.saturating_add(1);
    } else {
        model.ham_messages = model.ham_messages.saturating_add(1);
    }
    for token in tokens {
        let entry = model.tokens.entry(token).or_default();
        if is_spam {
            entry.spam = entry.spam.saturating_add(1);
        } else {
            entry.ham = entry.ham.saturating_add(1);
        }
    }
    if model.tokens.len() > 50_000 {
        model.tokens.retain(|_, v| v.spam + v.ham >= 2);
    }
    save_model(data_dir, &model)
}

pub fn model_stats(data_dir: &Path) -> (u32, u32) {
    let model = load_model(data_dir);
    (model.spam_messages, model.ham_messages)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn heuristic_flags_obvious_spam() {
        let dir = tempdir().unwrap();
        let settings = SpamSettings {
            enabled: true,
            auto_move: true,
            threshold: 0.7,
            strict_heuristics: true,
        };
        let verdict = score_message(
            dir.path(),
            &settings,
            "URGENT ACTION REQUIRED!!!",
            "promo@casino.example",
            "Click here to claim your free money lottery win $$$$",
        );
        assert!(verdict.score > 0.5);
        assert!(!verdict.reasons.is_empty());
    }

    #[test]
    fn detects_script_mixing_and_spaced_obfuscation() {
        // Cyrillic capital ER (U+0420) looks like Latin P.
        let (b1, r1) = script_mixing("Pay\u{0420}al security alert");
        assert!(b1 > 0.0);
        assert!(r1.iter().any(|r| r.contains("cyrillic") || r.contains("mixed")));

        let (b2, r2) = obfuscation_patterns("Buy v i a g r a today");
        assert!(b2 > 0.0);
        assert!(r2.iter().any(|r| r.contains("spaced")));
    }

    #[test]
    fn detects_symbol_storm() {
        let (b, r) = punctuation_storm("WIN NOW!!!! $$$$$ ***** %%%%");
        assert!(b > 0.0);
        assert!(!r.is_empty());
    }

    #[test]
    fn detects_invisible_chars() {
        let sneaky = format!("Hello{}world", '\u{200B}');
        let (b, r) = invisible_chars(&sneaky);
        assert!(b > 0.0);
        assert!(r.iter().any(|x| x.contains("invisible")));
    }

    #[test]
    fn training_improves_stats() {
        let dir = tempdir().unwrap();
        train(dir.path(), "Cheap pills", "a@b.com", "buy viagra now", true).unwrap();
        train(
            dir.path(),
            "Meeting notes",
            "bob@acme.com",
            "see you at 4pm",
            false,
        )
        .unwrap();
        let (spam, ham) = model_stats(dir.path());
        assert_eq!(spam, 1);
        assert_eq!(ham, 1);
    }
}
