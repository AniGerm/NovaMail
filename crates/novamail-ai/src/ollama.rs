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
            std::env::var("NOVAMAIL_OLLAMA_MODEL").unwrap_or_else(|_| "llama3.2".into()),
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
                    num_predict: 512,
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
            "Summarize the following email in at most 3 concise sentences. \
             Reply with the summary only.\n\nSubject: {}\n\n{}",
            request.subject,
            truncate(&request.body_text, 8_000)
        );
        Ok(SummarizeResponse {
            summary: self.generate(&prompt).await?,
            provider: self.name().into(),
        })
    }

    async fn suggest_reply(&self, request: SuggestReplyRequest) -> AiResult<SuggestReplyResponse> {
        let prompt = format!(
            "Draft a short professional reply to this email. \
             Reply with the email body only, no commentary.\n\nFrom: {}\nSubject: {}\n\n{}",
            request.from_email,
            request.subject,
            truncate(&request.body_text, 8_000)
        );
        Ok(SuggestReplyResponse {
            suggestion: self.generate(&prompt).await?,
            provider: self.name().into(),
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
