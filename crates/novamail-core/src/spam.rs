//! Local spam detection: heuristics + trainable Bayesian filter.
//!
//! Server-side filters (Rspamd, SpamAssassin) need a mail server. For a
//! local-first desktop client we combine cheap heuristics with a Graham-style
//! Bayesian model that learns from “Spam” / “Kein Spam” actions.

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
}

impl Default for SpamSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            auto_move: true,
            threshold: DEFAULT_THRESHOLD,
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
    let raw = serde_json::to_string_pretty(model)
        .map_err(|e| CoreError::Message(e.to_string()))?;
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

fn heuristic_boost(subject: &str, from_email: &str, body: &str) -> (f32, Vec<String>) {
    let mut boost = 0.0f32;
    let mut reasons = Vec::new();
    let subj = subject.to_ascii_lowercase();
    let body_l = body.to_ascii_lowercase();
    let from_l = from_email.to_ascii_lowercase();

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
        "unsubscribe",
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
    ];
    for needle in spammy {
        if subj.contains(needle) || body_l.contains(needle) {
            boost += 0.12;
            reasons.push(format!("keyword:{needle}"));
        }
    }

    let caps = subject.chars().filter(|c| c.is_ascii_uppercase()).count();
    let letters = subject.chars().filter(|c| c.is_ascii_alphabetic()).count();
    if letters >= 8 && caps as f32 / letters as f32 > 0.7 {
        boost += 0.15;
        reasons.push("subject:mostly-caps".into());
    }
    if subject.matches('!').count() >= 3 {
        boost += 0.1;
        reasons.push("subject:many-exclamations".into());
    }
    if from_l.ends_with(".zip")
        || from_l.ends_with(".ru.com")
        || from_l.contains("mailer-daemon@")
    {
        boost += 0.1;
        reasons.push("from:suspicious".into());
    }

    (boost.min(0.55), reasons)
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
    let (boost, mut reasons) = heuristic_boost(subject, from_email, body);
    let bayes = bayes_score(&model, &tokens).unwrap_or(0.35);
    if bayes_score(&model, &tokens).is_some() {
        reasons.push(format!("bayes:{bayes:.2}"));
    } else {
        reasons.push("bayes:cold-start".into());
    }
    let score = (bayes * 0.65 + boost).clamp(0.0, 1.0);
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
    // Cap vocabulary growth.
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
        };
        let verdict = score_message(
            dir.path(),
            &settings,
            "URGENT ACTION REQUIRED!!!",
            "promo@casino.example",
            "Click here to claim your free money lottery win",
        );
        assert!(verdict.score > 0.5);
        assert!(!verdict.reasons.is_empty());
    }

    #[test]
    fn training_improves_stats() {
        let dir = tempdir().unwrap();
        train(dir.path(), "Cheap pills", "a@b.com", "buy viagra now", true).unwrap();
        train(dir.path(), "Meeting notes", "bob@acme.com", "see you at 4pm", false).unwrap();
        let (spam, ham) = model_stats(dir.path());
        assert_eq!(spam, 1);
        assert_eq!(ham, 1);
    }
}
