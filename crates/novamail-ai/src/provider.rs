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
}
