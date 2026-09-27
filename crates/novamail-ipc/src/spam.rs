use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SpamSettingsDto {
    pub enabled: bool,
    pub auto_move: bool,
    pub threshold: f32,
    pub trained_spam: u32,
    pub trained_ham: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SpamScoreDto {
    pub message_id: Uuid,
    pub score: f32,
    pub is_spam: bool,
    pub reasons: Vec<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum RetentionModeDto {
    Keep,
    DeleteAfterDays,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct FolderPolicyDto {
    pub role: String,
    pub mode: RetentionModeDto,
    pub days: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct FolderPoliciesDto {
    pub policies: Vec<FolderPolicyDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MoveMessageRequest {
    pub message_id: Uuid,
    /// Target mailbox role (`junk`, `inbox`, `trash`, …) or explicit folder name.
    pub target: String,
}
