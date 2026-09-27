use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::SendMessageRequest;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum SnoozePreset {
    LaterToday,
    TomorrowMorning,
    NextMonday,
    Custom,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SnoozeRequest {
    pub message_id: Uuid,
    pub preset: SnoozePreset,
    /// Used when `preset == Custom` (unix seconds).
    #[serde(default)]
    pub wake_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SnoozedMessageDto {
    pub message_id: Uuid,
    pub account_id: Uuid,
    pub wake_at: i64,
    pub subject: String,
    pub from_email: String,
    pub account_email: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum OutboundStatus {
    Pending,
    Sending,
    Sent,
    Failed,
    Cancelled,
}

impl OutboundStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Sending => "sending",
            Self::Sent => "sent",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }

    pub fn parse(value: &str) -> Self {
        match value {
            "sending" => Self::Sending,
            "sent" => Self::Sent,
            "failed" => Self::Failed,
            "cancelled" => Self::Cancelled,
            _ => Self::Pending,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SendLaterRequest {
    pub send_at: i64,
    pub message: SendMessageRequest,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct OutboundQueueItemDto {
    pub id: Uuid,
    pub account_id: Uuid,
    pub account_email: String,
    pub subject: String,
    pub to_summary: String,
    pub send_at: i64,
    pub status: OutboundStatus,
    pub last_error: Option<String>,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct JobsTickReport {
    pub woke_snoozes: u32,
    pub sent_later: u32,
    pub failed_later: u32,
    #[serde(default)]
    pub calendar_reminders: u32,
    #[serde(default)]
    pub scanned_invites: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PlannedSummaryDto {
    pub snoozed_count: u32,
    pub outbound_pending_count: u32,
}
