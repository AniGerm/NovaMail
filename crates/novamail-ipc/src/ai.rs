use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Persisted local-AI preferences (`settings` key `ai.settings`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AiSettings {
    pub enabled: bool,
    pub model: String,
    pub base_url: String,
    /// First-run wizard completed (yes/no + optional model pick).
    pub onboarding_completed: bool,
}

impl Default for AiSettings {
    fn default() -> Self {
        Self {
            // Until onboarding runs, keep AI off so we do not surprise new installs.
            enabled: false,
            model: "qwen3:4b-instruct".into(),
            base_url: "http://127.0.0.1:11434".into(),
            onboarding_completed: false,
        }
    }
}

/// Probe result for setup / settings UI.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AiRuntimeStatus {
    pub ollama_reachable: bool,
    /// Binary found on PATH or known locations (may still be stopped).
    pub ollama_installed: bool,
    pub ollama_binary: Option<String>,
    pub can_install_user: bool,
    pub can_install_system: bool,
    pub nvidia_gpu: bool,
    pub models: Vec<String>,
    pub recommended_model: String,
    /// True when GPU is present or Ollama already has installed models.
    pub allow_model_pick: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AiInstallOllamaRequest {
    /// `user` (no sudo, ~/.local) or `system` (pkexec → official install.sh).
    pub mode: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AiInstallOllamaResponse {
    pub binary_path: Option<String>,
    pub reachable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AiInstallProgressEvent {
    pub status: String,
    pub done: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AiPullModelRequest {
    /// Ollama tag, `hf.co/…` ref, or huggingface.co / ollama.com URL.
    pub model: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AiPullModelResponse {
    pub model: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AiPullProgressEvent {
    pub model: String,
    pub status: String,
    pub digest: Option<String>,
    pub total: Option<u64>,
    pub completed: Option<u64>,
    pub done: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SummarizeMessageRequest {
    pub message_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SummarizeMessageResponse {
    pub message_id: Uuid,
    pub summary: String,
    pub provider: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SuggestReplyMessageRequest {
    pub message_id: Uuid,
    #[serde(default)]
    pub facts: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SuggestReplyMessageResponse {
    pub message_id: Uuid,
    pub suggestion: String,
    pub provider: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SuggestRepliesMessageRequest {
    pub message_id: Uuid,
    /// Optional facts / instructions to weave into both variants.
    #[serde(default)]
    pub facts: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SuggestRepliesMessageResponse {
    pub message_id: Uuid,
    pub variants: Vec<String>,
    pub provider: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct EventSuggestionDto {
    pub label: String,
    pub starts_at: i64,
    pub ends_at: Option<i64>,
    pub location: Option<String>,
    pub confidence: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct MessageAiInsights {
    pub message_id: Uuid,
    pub summary: Option<String>,
    /// Concise reply variant.
    pub reply_a: Option<String>,
    /// Warmer reply variant.
    pub reply_b: Option<String>,
    /// Legacy single suggestion (same as reply_a when present).
    pub reply_suggestion: Option<String>,
    /// Meeting/appointment times detected in the mail.
    #[serde(default)]
    pub event_suggestions: Vec<EventSuggestionDto>,
    pub provider: Option<String>,
}
