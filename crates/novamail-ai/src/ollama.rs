use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::provider::{
    AiError, AiProvider, AiResult, PrioritizeRequest, PrioritizeResponse, SuggestReplyRequest,
    SuggestReplyResponse, SummarizeRequest, SummarizeResponse,
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
            std::env::var("NOVAMAIL_OLLAMA_URL").unwrap_or_else(|_| "http://127.0.0.1:11434".into()),
            // Small multilingual instruct model — CPU-friendly for background insights.
            std::env::var("NOVAMAIL_OLLAMA_MODEL")
                .unwrap_or_else(|_| "qwen2.5:1.5b".into()),
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
        let prompt = format!(
            "Draft a short professional reply (3-6 sentences). Match the email language \
             (German or English). Reply with the email body only.\n\nFrom: {}\nSubject: {}\n\n{}",
            request.from_email,
            request.subject,
            truncate(&request.body_text, 3_500)
        );
        Ok(SuggestReplyResponse {
            suggestion: self.generate_with_limit(&prompt, 200).await?,
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
}

fn truncate(input: &str, max_chars: usize) -> String {
    input.chars().take(max_chars).collect()
}

#[cfg(test)]
mod live_tests {
    use super::*;

    #[tokio::test]
    async fn qwen_summarizes_german_office_mail() {
        let provider = OllamaProvider::default();
        // Skip when Ollama isn't running in CI.
        if reqwest::Client::new()
            .get(format!("{}/api/tags", provider.base_url))
            .send()
            .await
            .is_err()
        {
            return;
        }
        let result = provider
            .summarize(SummarizeRequest {
                subject: "Termin verschieben".into(),
                body_text: "Hallo Team,\n\nkönnen wir das Meeting von Dienstag auf Donnerstag 14 Uhr verschieben? Bitte kurz rückmelden.\n\nViele Grüße\nAnna".into(),
            })
            .await
            .expect("qwen summarize");
        assert!(!result.summary.trim().is_empty());
        assert!(result.provider.contains("qwen") || result.provider.contains("ollama"));
        println!("summary={}", result.summary);

        let reply = provider
            .suggest_reply(SuggestReplyRequest {
                subject: "Termin verschieben".into(),
                body_text: "Hallo Team,\n\nkönnen wir das Meeting von Dienstag auf Donnerstag 14 Uhr verschieben?\n\nViele Grüße\nAnna".into(),
                from_email: "anna@example.com".into(),
            })
            .await
            .expect("qwen reply");
        assert!(!reply.suggestion.trim().is_empty());
        println!("reply={}", reply.suggestion);
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
