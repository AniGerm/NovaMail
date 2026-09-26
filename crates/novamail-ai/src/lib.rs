//! AI extension layer. MVP ships a null provider; Ollama is wired but optional.

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
}

#[async_trait]
pub trait AiProvider: Send + Sync {
    async fn summarize(&self, request: SummarizeRequest) -> AiResult<SummarizeResponse>;
}

#[derive(Debug, Default)]
pub struct NullAiProvider;

#[async_trait]
impl AiProvider for NullAiProvider {
    async fn summarize(&self, request: SummarizeRequest) -> AiResult<SummarizeResponse> {
        let snippet: String = request.body_text.chars().take(240).collect();
        Ok(SummarizeResponse {
            summary: format!("{} — {}", request.subject, snippet),
        })
    }
}

#[derive(Debug, Clone)]
pub struct OllamaProvider {
    pub base_url: String,
    pub model: String,
}

impl Default for OllamaProvider {
    fn default() -> Self {
        Self {
            base_url: "http://127.0.0.1:11434".into(),
            model: "llama3.2".into(),
        }
    }
}

#[async_trait]
impl AiProvider for OllamaProvider {
    async fn summarize(&self, _request: SummarizeRequest) -> AiResult<SummarizeResponse> {
        // Kept dependency-free for MVP compile surface; HTTP client lands with AI phase.
        Err(AiError::Unavailable(format!(
            "Ollama provider configured for {}/{} but HTTP client is enabled in production roadmap",
            self.base_url, self.model
        )))
    }
}
