use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::provider::{
    heuristic_event_suggestions, AiError, AiProvider, AiResult, EventSuggestion,
    ExtractEventsRequest, ExtractEventsResponse, PrioritizeRequest, PrioritizeResponse,
    SuggestReplyRequest, SuggestReplyResponse, SuggestReplyVariantsResponse, SummarizeRequest,
    SummarizeResponse,
};

#[derive(Debug, Clone)]
pub struct OllamaProvider {
    pub base_url: String,
    pub model: String,
    client: reqwest::Client,
}

impl Default for OllamaProvider {
    fn default() -> Self {
        Self::new(
            crate::runtime::default_ollama_url(),
            // Small multilingual instruct model — CPU-friendly for background insights.
            crate::runtime::default_ollama_model(),
        )
    }
}

impl OllamaProvider {
    pub fn new(base_url: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into().trim_end_matches('/').to_string(),
            model: model.into(),
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(60))
                .build()
                .expect("reqwest client"),
        }
    }

    async fn generate(&self, prompt: &str) -> AiResult<String> {
        self.generate_with_limit(prompt, 192).await
    }

    async fn generate_with_limit(&self, prompt: &str, num_predict: u32) -> AiResult<String> {
        #[derive(Serialize)]
        struct RequestBody<'a> {
            model: &'a str,
            prompt: &'a str,
            stream: bool,
            options: Options,
        }

        #[derive(Serialize)]
        struct Options {
            temperature: f32,
            num_predict: u32,
            num_ctx: u32,
        }

        #[derive(Deserialize)]
        struct ResponseBody {
            response: String,
        }

        let url = format!("{}/api/generate", self.base_url);
        let response = self
            .client
            .post(&url)
            .json(&RequestBody {
                model: &self.model,
                prompt,
                stream: false,
                options: Options {
                    temperature: 0.2,
                    num_predict,
                    num_ctx: 4096,
                },
            })
            .send()
            .await
            .map_err(|e| {
                AiError::Unavailable(format!(
                    "Ollama unreachable at {} ({e}). Start Ollama or unset AI features.",
                    self.base_url
                ))
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(AiError::Inference(format!(
                "Ollama HTTP {status}: {body}"
            )));
        }

        let parsed = response
            .json::<ResponseBody>()
            .await
            .map_err(|e| AiError::Inference(e.to_string()))?;
        let text = parsed.response.trim().to_string();
        if text.is_empty() {
            return Err(AiError::Inference("Ollama returned an empty response".into()));
        }
        Ok(text)
    }
}

#[async_trait]
impl AiProvider for OllamaProvider {
    fn name(&self) -> &'static str {
        "ollama"
    }

    async fn summarize(&self, request: SummarizeRequest) -> AiResult<SummarizeResponse> {
        let prompt = format!(
            "Summarize this work email in at most 2 short sentences (German or English, \
             matching the email language). Reply with the summary only.\n\nSubject: {}\n\n{}",
            request.subject,
            truncate(&request.body_text, 3_500)
        );
        Ok(SummarizeResponse {
            summary: self.generate_with_limit(&prompt, 128).await?,
            provider: format!("{}:{}", self.name(), self.model),
        })
    }

    async fn suggest_reply(&self, request: SuggestReplyRequest) -> AiResult<SuggestReplyResponse> {
        let facts_block = request
            .facts
            .as_deref()
            .filter(|s| !s.trim().is_empty())
            .map(|s| {
                format!(
                    "\nImportant: write a FULL email reply that naturally includes these facts \
                     (never output the facts alone as the whole reply):\n- {}\n",
                    s.trim().replace('\n', "\n- ")
                )
            })
            .unwrap_or_default();
        let style = match request.style.as_deref() {
            Some("concise") => "concise and direct",
            Some("friendly") => "warm and friendly",
            _ => "professional",
        };
        let prompt = format!(
            "Write a complete {style} email reply body with greeting, 3-6 sentences, and sign-off. \
             Match German or English to the original mail. \
             Output ONLY the reply body — no Subject line, no markdown, no commentary.\
             {facts_block}\nOriginal From: {}\nOriginal Subject: {}\n\nOriginal body:\n{}",
            request.from_email,
            request.subject,
            truncate(&request.body_text, 3_500)
        );
        Ok(SuggestReplyResponse {
            suggestion: self.generate_with_limit(&prompt, 240).await?,
            provider: format!("{}:{}", self.name(), self.model),
        })
    }

    async fn suggest_reply_variants(
        &self,
        request: SuggestReplyRequest,
    ) -> AiResult<SuggestReplyVariantsResponse> {
        let mut a_req = request.clone();
        a_req.style = Some("concise".into());
        let mut b_req = request.clone();
        b_req.style = Some("friendly".into());
        let a = self.suggest_reply(a_req).await?;
        let b = self.suggest_reply(b_req).await?;
        Ok(SuggestReplyVariantsResponse {
            variants: vec![a.suggestion, b.suggestion],
            provider: format!("{}:{}", self.name(), self.model),
        })
    }

    async fn prioritize(&self, request: PrioritizeRequest) -> AiResult<PrioritizeResponse> {
        let prompt = format!(
            "Rate urgency of this email from 0.0 to 1.0. \
             Reply as JSON only: {{\"score\":0.0,\"rationale\":\"...\"}}\n\nFrom: {}\nSubject: {}\nSnippet: {}",
            request.from_email, request.subject, request.snippet
        );
        let raw = self.generate(&prompt).await?;
        let parsed = parse_priority_json(&raw).unwrap_or(PrioritizeResponse {
            score: 0.5,
            rationale: raw.chars().take(200).collect(),
            provider: self.name().into(),
        });
        Ok(PrioritizeResponse {
            score: parsed.score.clamp(0.0, 1.0),
            rationale: parsed.rationale,
            provider: self.name().into(),
        })
    }

    async fn extract_event_suggestions(
        &self,
        request: ExtractEventsRequest,
    ) -> AiResult<ExtractEventsResponse> {
        let ref_iso = chrono::DateTime::from_timestamp(request.reference_at, 0)
            .map(|dt| dt.format("%Y-%m-%dT%H:%M:%SZ").to_string())
            .unwrap_or_else(|| "unknown".into());
        let prompt = format!(
            "Extract meeting/appointment suggestions from this email. \
             Reference datetime (UTC): {ref_iso}. \
             Reply with JSON ONLY: {{\"suggestions\":[{{\"label\":\"Thu 14:00\",\"startsAt\":1710000000,\"endsAt\":1710003600,\"location\":\"Zoom\",\"confidence\":0.8}}]}}. \
             Use unix seconds. Max 5 suggestions. Empty array if none.\n\nSubject: {}\n\n{}",
            request.subject,
            truncate(&request.body_text, 3_500)
        );
        let raw = self.generate_with_limit(&prompt, 400).await?;
        let mut suggestions = parse_event_suggestions_json(&raw);
        if suggestions.is_empty() {
            suggestions = heuristic_event_suggestions(
                &request.subject,
                &request.body_text,
                request.reference_at,
            );
        }
        Ok(ExtractEventsResponse {
            suggestions,
            provider: format!("{}:{}", self.name(), self.model),
        })
    }
}

fn parse_event_suggestions_json(raw: &str) -> Vec<EventSuggestion> {
    let trimmed = raw.trim();
    let json_slice = match (trimmed.find('{'), trimmed.rfind('}')) {
        (Some(start), Some(end)) if end > start => &trimmed[start..=end],
        _ => return Vec::new(),
    };
    #[derive(serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Wrap {
        suggestions: Vec<LooseSuggestion>,
    }
    #[derive(serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct LooseSuggestion {
        label: Option<String>,
        starts_at: Option<i64>,
        ends_at: Option<i64>,
        location: Option<String>,
        confidence: Option<f32>,
    }
    let Ok(wrap) = serde_json::from_str::<Wrap>(json_slice) else {
        return Vec::new();
    };
    wrap.suggestions
        .into_iter()
        .filter_map(|s| {
            let starts_at = s.starts_at?;
            Some(EventSuggestion {
                label: s.label.unwrap_or_else(|| "Meeting".into()),
                starts_at,
                ends_at: s.ends_at.or(Some(starts_at + 3600)),
                location: s.location,
                confidence: s.confidence.unwrap_or(0.7).clamp(0.0, 1.0),
            })
        })
        .take(5)
        .collect()
}

fn truncate(input: &str, max_chars: usize) -> String {
    input.chars().take(max_chars).collect()
}

#[cfg(test)]
mod live_tests {
    use super::*;

    #[tokio::test]
    async fn qwen_summarizes_german_office_mail() {
        // Prefer a model that is commonly present in CI/dev; fall back to default.
        let provider = match crate::runtime::list_ollama_models(&crate::runtime::default_ollama_url())
            .await
        {
            Ok(models) if !models.is_empty() => {
                let pick = models
                    .iter()
                    .find(|m| m.contains("qwen"))
                    .cloned()
                    .unwrap_or_else(|| models[0].clone());
                OllamaProvider::new(crate::runtime::default_ollama_url(), pick)
            }
            Ok(_) | Err(_) => return, // Ollama missing or empty — skip live test
        };
        let result = match provider
            .summarize(SummarizeRequest {
                subject: "Termin verschieben".into(),
                body_text: "Hallo Team,\n\nkönnen wir das Meeting von Dienstag auf Donnerstag 14 Uhr verschieben? Bitte kurz rückmelden.\n\nViele Grüße\nAnna".into(),
            })
            .await
        {
            Ok(r) => r,
            Err(_) => return,
        };
        assert!(!result.summary.trim().is_empty());
        assert!(result.provider.contains("qwen") || result.provider.contains("ollama"));
        println!("summary={}", result.summary);

        let reply = provider
            .suggest_reply(SuggestReplyRequest {
                subject: "Termin verschieben".into(),
                body_text: "Hallo Team,\n\nkönnen wir das Meeting von Dienstag auf Donnerstag 14 Uhr verschieben?\n\nViele Grüße\nAnna".into(),
                from_email: "anna@example.com".into(),
                facts: None,
                style: None,
            })
            .await
            .expect("qwen reply");
        assert!(!reply.suggestion.trim().is_empty());
        println!("reply={}", reply.suggestion);

        let variants = provider
            .suggest_reply_variants(SuggestReplyRequest {
                subject: "Termin verschieben".into(),
                body_text: "Hallo Team, Termin bitte verschieben.".into(),
                from_email: "anna@example.com".into(),
                facts: Some("Donnerstag 14 Uhr passt. Bitte Zoom-Link schicken.".into()),
                style: None,
            })
            .await
            .expect("qwen variants");
        assert!(variants.variants.len() >= 2);
        println!("variant_a={}", variants.variants[0]);
        println!("variant_b={}", variants.variants[1]);
    }
}

fn parse_priority_json(raw: &str) -> Option<PrioritizeResponse> {
    #[derive(Deserialize)]
    struct Partial {
        score: f32,
        rationale: String,
    }
    let start = raw.find('{')?;
    let end = raw.rfind('}')?;
    let slice = &raw[start..=end];
    let partial: Partial = serde_json::from_str(slice).ok()?;
    Some(PrioritizeResponse {
        score: partial.score,
        rationale: partial.rationale,
        provider: "ollama".into(),
    })
}
