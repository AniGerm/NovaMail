//! Detect reply/summary language from email text + optional UI preference.

/// BCP-47-ish language tag used in prompts (`de` / `en`).
pub type LangTag = &'static str;

/// Prefer the language of the mail body; fall back to UI locale (`de`/`en`).
pub fn resolve_output_language(
    subject: &str,
    body: &str,
    ui_locale: Option<&str>,
) -> LangTag {
    match detect_mail_language(subject, body) {
        Some(lang) => lang,
        None => match ui_locale.map(|s| s.to_ascii_lowercase()) {
            Some(ref l) if l.starts_with("de") => "de",
            Some(ref l) if l.starts_with("en") => "en",
            // App default locale is German.
            _ => "de",
        },
    }
}

pub fn language_label(lang: LangTag) -> &'static str {
    match lang {
        "de" => "German",
        "en" => "English",
        _ => "German",
    }
}

pub fn language_native_name(lang: LangTag) -> &'static str {
    match lang {
        "de" => "Deutsch",
        "en" => "English",
        _ => "Deutsch",
    }
}

/// Heuristic: score German vs English tokens / characters.
fn detect_mail_language(subject: &str, body: &str) -> Option<LangTag> {
    let text = format!("{subject}\n{body}").to_lowercase();
    if text.trim().is_empty() {
        return None;
    }

    let mut de = 0i32;
    let mut en = 0i32;

    // Strong German orthography signals.
    for ch in ['ä', 'ö', 'ü', 'ß'] {
        de += text.matches(ch).count() as i32 * 3;
    }
    for word in [
        "und", "der", "die", "das", "nicht", "ich", "sie", "wir", "für", "mit",
        "auch", "oder", "eine", "einem", "einer", "sind", "haben", "wird",
        "werden", "bitte", "danke", "freundliche", "grüße", "grüsse", "sehr",
        "geehrte", "anhang", "termin", "besprechung", "nachricht", "betreff",
    ] {
        de += count_word(&text, word) * 2;
    }
    for word in [
        "the", "and", "you", "for", "with", "this", "that", "have", "from",
        "please", "thanks", "thank", "regards", "meeting", "attached", "hello",
        "dear", "would", "could", "about", "your", "our",
    ] {
        en += count_word(&text, word) * 2;
    }

    if de == 0 && en == 0 {
        return None;
    }
    if de >= en + 2 {
        Some("de")
    } else if en >= de + 2 {
        Some("en")
    } else if de > en {
        Some("de")
    } else if en > de {
        Some("en")
    } else {
        None
    }
}

fn count_word(haystack: &str, word: &str) -> i32 {
    let mut count = 0i32;
    let mut rest = haystack;
    while let Some(idx) = rest.find(word) {
        let before_ok = idx == 0
            || !rest
                .as_bytes()
                .get(idx - 1)
                .map(|b| b.is_ascii_alphabetic())
                .unwrap_or(false);
        let after = idx + word.len();
        let after_ok = rest
            .as_bytes()
            .get(after)
            .map(|b| !b.is_ascii_alphabetic())
            .unwrap_or(true);
        if before_ok && after_ok {
            count += 1;
        }
        rest = &rest[idx + word.len()..];
    }
    count
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_german() {
        let lang = detect_mail_language(
            "Termin am Donnerstag",
            "Hallo zusammen, bitte bestätigt den Termin für die Besprechung. Viele Grüße",
        );
        assert_eq!(lang, Some("de"));
    }

    #[test]
    fn detects_english() {
        let lang = detect_mail_language(
            "Meeting tomorrow",
            "Hello team, please confirm the meeting and share your availability. Thanks and regards",
        );
        assert_eq!(lang, Some("en"));
    }

    #[test]
    fn ui_locale_fallback() {
        assert_eq!(resolve_output_language("", "", Some("de")), "de");
        assert_eq!(resolve_output_language("", "", Some("en-US")), "en");
    }
}
