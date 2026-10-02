use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::provider::{
    heuristic_event_suggestions, strip_quoted_reply, AiError, AiProvider, AiResult, EventSuggestion,
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
        let lang = crate::resolve_output_language(
            &request.subject,
            &request.body_text,
            request.preferred_language.as_deref(),
        );
        let lang_name = crate::language_label(lang);
        let lang_native = crate::language_native_name(lang);
        let prompt = format!(
            "You are an email assistant. Summarize this work email in at most 2 short sentences.\n\
             CRITICAL: Write the summary ONLY in {lang_name} ({lang_native}). \
             Do not use any other language.\n\
             Reply with the summary text only — no labels, no markdown.\n\n\
             Subject: {}\n\n{}",
            request.subject,
            truncate(&request.body_text, 3_500)
        );
        Ok(SummarizeResponse {
            summary: self.generate_with_limit(&prompt, 128).await?,
            provider: format!("{}:{}", self.name(), self.model),
        })
    }

    async fn suggest_reply(&self, request: SuggestReplyRequest) -> AiResult<SuggestReplyResponse> {
        let lang = crate::resolve_output_language(
            &request.subject,
            &request.body_text,
            request.preferred_language.as_deref(),
        );
        let lang_name = crate::language_label(lang);
        let lang_native = crate::language_native_name(lang);
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
        let style = match (request.style.as_deref(), lang) {
            (Some("concise"), "de") => "knapp und klar",
            (Some("friendly"), "de") => "warm und freundlich",
            (_, "de") => "professionell",
            (Some("concise"), _) => "concise and direct",
            (Some("friendly"), _) => "warm and friendly",
            _ => "professional",
        };
        let reply_as = request
            .reply_as_email
            .as_deref()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or("the mailbox owner");
        let prompt = format!(
            "You write email REPLIES for {reply_as}.\n\
             ROLE: You are the RECIPIENT of the email below. Write a reply TO {from} \
             (the original sender). You are NOT the original sender.\n\
             Write a complete {style} reply body with greeting, 3-6 sentences, and sign-off.\n\
             CRITICAL RULES:\n\
             - Write ONLY in {lang_name} ({lang_native}). Do not mix languages.\n\
             - Answer their points; do NOT rewrite, paraphrase, or replace their email.\n\
             - Do NOT invent that you are {from}. Do NOT continue their message as them.\n\
             - Output ONLY the reply body — no Subject line, no markdown, no commentary.\
             {facts_block}\n\
             --- Incoming email ---\n\
             From: {from}\n\
             Subject: {subject}\n\n\
             {body}",
            from = request.from_email,
            subject = request.subject,
            body = truncate(&request.body_text, 3_500),
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
        // Never feed quoted reply history / "Am … schrieb" headers to the model.
        let clean_body = strip_quoted_reply(&request.body_text);
        let heuristics = heuristic_event_suggestions(
            &request.subject,
            &clean_body,
            request.reference_at,
        );
        let prompt = format!(
            "Extract meeting/appointment suggestions from the NEW message content only.\n\
             CRITICAL RULES:\n\
             - Ignore quoted replies, forwarded headers, and lines like \
             \"Am Donnerstag, … schrieb …\" / \"On … wrote:\".\n\
             - Do NOT treat the email Date header or send timestamp as an appointment.\n\
             - Resolve relative words against the reference datetime: \
             German morgen/heute/übermorgen and English tomorrow/today/day after tomorrow.\n\
             - Example: \"morgen 18:00\" with reference Thursday 2026-10-01 → Friday 18:00.\n\
             Reference datetime (UTC): {ref_iso}.\n\
             Reply with JSON ONLY: {{\"suggestions\":[{{\"label\":\"Fri 18:00\",\"startsAt\":1710000000,\"endsAt\":1710003600,\"location\":\"Bowling\",\"confidence\":0.85}}]}}.\n\
             Use unix seconds. Max 5 suggestions. Empty array if none.\n\n\
             Subject: {}\n\n{}",
            request.subject,
            truncate(&clean_body, 3_500)
        );
        let raw = self.generate_with_limit(&prompt, 400).await?;
        let mut llm = parse_event_suggestions_json(&raw);
        // Drop suggestions that are just the message send time (±3 min).
        llm.retain(|s| (s.starts_at - request.reference_at).abs() > 180);
        let mut suggestions = merge_event_suggestions(heuristics, llm);
        if suggestions.is_empty() {
            suggestions = heuristic_event_suggestions(
                &request.subject,
                &clean_body,
                request.reference_at,
            );
        }
        Ok(ExtractEventsResponse {
            suggestions,
            provider: format!("{}:{}", self.name(), self.model),
        })
    }

    async fn optimize_draft(
        &self,
        subject: &str,
        body_text: &str,
        preferred_language: Option<&str>,
    ) -> AiResult<String> {
        let lang = crate::resolve_output_language(subject, body_text, preferred_language);
        let lang_name = crate::language_label(lang);
        let lang_native = crate::language_native_name(lang);
        let prompt = format!(
            "You are an email writing assistant. Improve the user's draft below.\n\
             CRITICAL: Write ONLY in {lang_name} ({lang_native}).\n\
             Keep intent, facts, names, and commitments. Improve clarity, tone, and structure.\n\
             Output ONLY the improved email body — no Subject line, no commentary, no markdown fences.\n\n\
             Subject: {subject}\n\nDraft:\n{}",
            truncate(body_text, 3_500)
        );
        self.generate_with_limit(&prompt, 320).await
    }
}

fn merge_event_suggestions(
    heuristics: Vec<EventSuggestion>,
    llm: Vec<EventSuggestion>,
) -> Vec<EventSuggestion> {
    let mut combined = heuristics;
    for suggestion in llm {
        let near_existing = combined
            .iter()
            .any(|h| (h.starts_at - suggestion.starts_at).abs() < 120);
        if !near_existing {
            combined.push(suggestion);
        }
    }
    combined.sort_by(|a, b| {
        b.confidence
            .partial_cmp(&a.confidence)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.starts_at.cmp(&b.starts_at))
    });
    combined.dedup_by_key(|s| s.starts_at);
    combined.truncate(5);
    combined
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
                preferred_language: Some("de".into()),
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
                reply_as_email: Some("max@example.com".into()),
                facts: None,
                style: None,
                preferred_language: Some("de".into()),
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
                reply_as_email: Some("max@example.com".into()),
                facts: Some("Donnerstag 14 Uhr passt. Bitte Zoom-Link schicken.".into()),
                style: None,
                preferred_language: Some("de".into()),
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
