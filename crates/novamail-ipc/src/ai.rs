use serde::{Deserialize, Serialize};
use uuid::Uuid;

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
    pub provider: Option<String>,
}
