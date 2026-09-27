use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AiError {
    #[error("provider unavailable: {0}")]
    Unavailable(String),
    #[error("inference failed: {0}")]
    Inference(String),
}

pub type AiResult<T> = Result<T, AiError>;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SummarizeRequest {
    pub subject: String,
    pub body_text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SummarizeResponse {
    pub summary: String,
    pub provider: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SuggestReplyRequest {
    pub subject: String,
    pub body_text: String,
    pub from_email: String,
    /// Optional facts / instructions the user wants included in the reply.
    #[serde(default)]
    pub facts: Option<String>,
    /// Optional style hint: "concise" | "friendly".
    #[serde(default)]
    pub style: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SuggestReplyResponse {
    pub suggestion: String,
    pub provider: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SuggestReplyVariantsResponse {
    pub variants: Vec<String>,
    pub provider: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrioritizeRequest {
    pub subject: String,
    pub snippet: String,
    pub from_email: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrioritizeResponse {
    /// 0.0..=1.0 higher means more urgent.
    pub score: f32,
    pub rationale: String,
    pub provider: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtractEventsRequest {
    pub subject: String,
    pub body_text: String,
    /// Unix seconds — usually the message Date header (local interpretation hint).
    pub reference_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct EventSuggestion {
    pub label: String,
    pub starts_at: i64,
    pub ends_at: Option<i64>,
    pub location: Option<String>,
    pub confidence: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtractEventsResponse {
    pub suggestions: Vec<EventSuggestion>,
    pub provider: String,
}

#[async_trait]
pub trait AiProvider: Send + Sync {
    fn name(&self) -> &'static str;

    async fn summarize(&self, request: SummarizeRequest) -> AiResult<SummarizeResponse>;

    async fn suggest_reply(&self, request: SuggestReplyRequest) -> AiResult<SuggestReplyResponse>;

    /// Two short reply alternatives (CPU-friendly single call when possible).
    async fn suggest_reply_variants(
        &self,
        request: SuggestReplyRequest,
    ) -> AiResult<SuggestReplyVariantsResponse>;

    async fn prioritize(&self, request: PrioritizeRequest) -> AiResult<PrioritizeResponse>;

    /// Detect meeting/appointment time suggestions in a mail body.
    async fn extract_event_suggestions(
        &self,
        request: ExtractEventsRequest,
    ) -> AiResult<ExtractEventsResponse>;
}

/// Deterministic offline fallback used when no local model is reachable.
#[derive(Debug, Default)]
pub struct NullAiProvider;

#[async_trait]
impl AiProvider for NullAiProvider {
    fn name(&self) -> &'static str {
        "null"
    }

    async fn summarize(&self, request: SummarizeRequest) -> AiResult<SummarizeResponse> {
        let body = collapse_whitespace(&request.body_text);
        let snippet: String = body.chars().take(280).collect();
        Ok(SummarizeResponse {
            summary: if snippet.is_empty() {
                request.subject
            } else {
                format!("{} — {}", request.subject, snippet)
            },
            provider: self.name().into(),
        })
    }

    async fn suggest_reply(&self, request: SuggestReplyRequest) -> AiResult<SuggestReplyResponse> {
        let facts = request
            .facts
            .as_deref()
            .filter(|s| !s.trim().is_empty())
            .map(|s| format!("\n\nPlease include these facts:\n{s}"))
            .unwrap_or_default();
        Ok(SuggestReplyResponse {
            suggestion: format!(
                "Hi {},\n\nThanks for your email regarding \"{}\".{}\n\nBest regards",
                guess_first_name(&request.from_email),
                request.subject,
                facts
            ),
            provider: self.name().into(),
        })
    }

    async fn suggest_reply_variants(
        &self,
        request: SuggestReplyRequest,
    ) -> AiResult<SuggestReplyVariantsResponse> {
        let name = guess_first_name(&request.from_email);
        let facts = request
            .facts
            .as_deref()
            .filter(|s| !s.trim().is_empty())
            .map(|s| format!("\n\nZu den Punkten: {s}"))
            .unwrap_or_default();
        Ok(SuggestReplyVariantsResponse {
            variants: vec![
                format!(
                    "Hallo {name},\n\nvielen Dank für Ihre Nachricht zu \"{}\".{} Wir melden uns zeitnah.\n\nFreundliche Grüße",
                    request.subject, facts
                ),
                format!(
                    "Hallo {name},\n\ndanke für die Mail.{} Gerne klären wir \"{}\" gemeinsam.\n\nViele Grüße",
                    facts, request.subject
                ),
            ],
            provider: self.name().into(),
        })
    }

    async fn prioritize(&self, request: PrioritizeRequest) -> AiResult<PrioritizeResponse> {
        let haystack = format!(
            "{} {} {}",
            request.subject, request.snippet, request.from_email
        )
        .to_ascii_lowercase();
        let mut score = 0.35_f32;
        for (needle, bump) in [
            ("urgent", 0.35),
            ("asap", 0.3),
            ("deadline", 0.25),
            ("invoice", 0.2),
            ("meeting", 0.15),
            ("fyi", -0.1),
            ("newsletter", -0.2),
        ] {
            if haystack.contains(needle) {
                score += bump;
            }
        }
        score = score.clamp(0.0, 1.0);
        Ok(PrioritizeResponse {
            score,
            rationale: format!("Heuristic score from subject/snippet keywords ({score:.2})"),
            provider: self.name().into(),
        })
    }

    async fn extract_event_suggestions(
        &self,
        request: ExtractEventsRequest,
    ) -> AiResult<ExtractEventsResponse> {
        Ok(ExtractEventsResponse {
            suggestions: heuristic_event_suggestions(
                &request.subject,
                &request.body_text,
                request.reference_at,
            ),
            provider: self.name().into(),
        })
    }
}

/// Lightweight offline / fallback extractor for ISO dates and HH:MM times.
pub fn heuristic_event_suggestions(
    subject: &str,
    body: &str,
    reference_at: i64,
) -> Vec<EventSuggestion> {
    use chrono::{Datelike, Duration, Local, TimeZone};

    let text = format!("{subject}\n{body}");
    let ref_dt = Local
        .timestamp_opt(reference_at, 0)
        .single()
        .unwrap_or_else(Local::now);
    let mut out = Vec::new();

    for (date, time) in regex_lite_iso(&text) {
        let (h, m) = time.unwrap_or((9, 0));
        let Some(naive) = date.and_hms_opt(h, m, 0) else {
            continue;
        };
        let Some(local) = Local.from_local_datetime(&naive).single() else {
            continue;
        };
        let label = if time.is_some() {
            format!("{} {h:02}:{m:02}", date.format("%a %d.%m."))
        } else {
            format!("{} (all day)", date.format("%a %d.%m."))
        };
        out.push(EventSuggestion {
            label,
            starts_at: local.timestamp(),
            ends_at: Some(local.timestamp() + 3600),
            location: None,
            confidence: 0.55,
        });
    }

    let weekdays = [
        ("montag", 1u32),
        ("dienstag", 2),
        ("mittwoch", 3),
        ("donnerstag", 4),
        ("freitag", 5),
        ("samstag", 6),
        ("sonntag", 7),
        ("monday", 1),
        ("tuesday", 2),
        ("wednesday", 3),
        ("thursday", 4),
        ("friday", 5),
        ("saturday", 6),
        ("sunday", 7),
    ];
    let lower = text.to_lowercase();
    for (name, target_iso) in weekdays {
        if !lower.contains(name) {
            continue;
        }
        let time = find_time_near(&lower, name).unwrap_or((10, 0));
        let mut days_ahead =
            (i32::try_from(target_iso).unwrap_or(1)
                - i32::try_from(ref_dt.weekday().number_from_monday()).unwrap_or(1)
                + 7)
                % 7;
        if days_ahead == 0 {
            if let Some(c) = ref_dt.date_naive().and_hms_opt(time.0, time.1, 0) {
                if Local
                    .from_local_datetime(&c)
                    .single()
                    .map(|dt| dt <= ref_dt)
                    .unwrap_or(true)
                {
                    days_ahead = 7;
                }
            }
        }
        let Some(naive) = (ref_dt.date_naive() + Duration::days(i64::from(days_ahead)))
            .and_hms_opt(time.0, time.1, 0)
        else {
            continue;
        };
        let Some(local) = Local.from_local_datetime(&naive).single() else {
            continue;
        };
        let mut label_name = name.to_string();
        if let Some(first) = label_name.get_mut(0..1) {
            first.make_ascii_uppercase();
        }
        out.push(EventSuggestion {
            label: format!("{label_name} {:02}:{:02}", time.0, time.1),
            starts_at: local.timestamp(),
            ends_at: Some(local.timestamp() + 3600),
            location: None,
            confidence: 0.45,
        });
    }

    out.sort_by_key(|s| s.starts_at);
    out.dedup_by_key(|s| s.starts_at);
    out.truncate(5);
    out
}

fn regex_lite_iso(text: &str) -> Vec<(chrono::NaiveDate, Option<(u32, u32)>)> {
    use chrono::NaiveDate;
    let mut out = Vec::new();
    let bytes = text.as_bytes();
    let mut i = 0;
    while i + 10 <= bytes.len() {
        if bytes[i].is_ascii_digit()
            && bytes[i + 1].is_ascii_digit()
            && bytes[i + 2].is_ascii_digit()
            && bytes[i + 3].is_ascii_digit()
            && bytes[i + 4] == b'-'
            && bytes[i + 5].is_ascii_digit()
            && bytes[i + 6].is_ascii_digit()
            && bytes[i + 7] == b'-'
            && bytes[i + 8].is_ascii_digit()
            && bytes[i + 9].is_ascii_digit()
        {
            let date_str = &text[i..i + 10];
            if let Ok(date) = NaiveDate::parse_from_str(date_str, "%Y-%m-%d") {
                let rest = &text[i + 10..];
                let time = parse_hhmm_prefix(rest.trim_start_matches(|c: char| {
                    c == 'T' || c == ' ' || c == ','
                }));
                out.push((date, time));
            }
            i += 10;
            continue;
        }
        i += 1;
    }
    out
}

fn parse_hhmm_prefix(s: &str) -> Option<(u32, u32)> {
    let s = s.trim_start();
    let digits: String = s
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == ':')
        .collect();
    if let Some((h, m)) = digits.split_once(':') {
        let h: u32 = h.parse().ok()?;
        let m: u32 = m.chars().take(2).collect::<String>().parse().ok()?;
        if h < 24 && m < 60 {
            return Some((h, m));
        }
    }
    // "14 Uhr"
    if let Some(pos) = s.to_lowercase().find(" uhr") {
        let before = s[..pos].trim();
        let num: String = before
            .chars()
            .rev()
            .take_while(|c| c.is_ascii_digit())
            .collect::<String>()
            .chars()
            .rev()
            .collect();
        if let Ok(h) = num.parse::<u32>() {
            if h < 24 {
                return Some((h, 0));
            }
        }
    }
    None
}

fn find_time_near(lower: &str, weekday: &str) -> Option<(u32, u32)> {
    let idx = lower.find(weekday)?;
    let window = &lower[idx..lower.len().min(idx + 48)];
    // HH:MM
    for (i, _) in window.char_indices() {
        let slice = &window[i..];
        if let Some(t) = parse_hhmm_prefix(slice) {
            return Some(t);
        }
    }
    None
}

fn collapse_whitespace(input: &str) -> String {
    input.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn guess_first_name(email: &str) -> String {
    email
        .split('@')
        .next()
        .unwrap_or("there")
        .split(|c: char| c == '.' || c == '_' || c == '-')
        .next()
        .map(|s| {
            let mut chars = s.chars();
            match chars.next() {
                Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
                None => "there".into(),
            }
        })
        .unwrap_or_else(|| "there".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn null_provider_prioritizes_urgent_mail() {
        let provider = NullAiProvider;
        let result = provider
            .prioritize(PrioritizeRequest {
                subject: "Urgent invoice".into(),
                snippet: "Please pay ASAP".into(),
                from_email: "billing@example.com".into(),
            })
            .await
            .unwrap();
        assert!(result.score > 0.7);
    }

    #[tokio::test]
    async fn null_provider_extracts_iso_date() {
        let provider = NullAiProvider;
        let result = provider
            .extract_event_suggestions(ExtractEventsRequest {
                subject: "Kickoff".into(),
                body_text: "Let's meet on 2026-10-05 14:30 in room A.".into(),
                reference_at: 1_725_000_000,
            })
            .await
            .unwrap();
        assert!(!result.suggestions.is_empty());
        assert!(result.suggestions.iter().any(|s| s.label.contains("14:30") || s.starts_at > 0));
    }
}
