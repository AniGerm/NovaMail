use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub enum OfflineMailboxMode {
    /// Do not offload. May prompt when quota nears threshold.
    #[default]
    Off,
    /// Offload oldest eligible mail when quota >= active threshold.
    Threshold,
    /// Keep server copies until overflow, then purge IMAP.
    Overflow,
    /// After sync, keep local copy and purge IMAP immediately for eligible mail.
    AlwaysPurge,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct OfflineMailboxAccountPolicy {
    pub account_id: Uuid,
    pub mode: OfflineMailboxMode,
    /// When mode is Off and usage reaches this %, show enable prompt once.
    pub prompt_threshold_percent: u8,
    /// When mode is Threshold/Overflow, act at this usage %.
    pub active_threshold_percent: u8,
    pub keep_starred_on_imap: bool,
    pub min_age_days: u32,
    pub batch_limit: u32,
    /// User dismissed the one-time enable prompt for this account.
    #[serde(default)]
    pub prompt_dismissed: bool,
}

impl OfflineMailboxAccountPolicy {
    pub fn default_for(account_id: Uuid) -> Self {
        Self {
            account_id,
            mode: OfflineMailboxMode::Off,
            prompt_threshold_percent: 90,
            active_threshold_percent: 85,
            keep_starred_on_imap: true,
            min_age_days: 30,
            batch_limit: 50,
            prompt_dismissed: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct OfflineMailboxSettingsDto {
    pub accounts: Vec<OfflineMailboxAccountPolicy>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum QuotaSource {
    Server,
    Estimate,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AccountQuotaDto {
    pub account_id: Uuid,
    pub used_bytes: u64,
    pub limit_bytes: Option<u64>,
    pub percent: Option<f32>,
    pub source: QuotaSource,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct OfflineOffloadReport {
    pub account_id: Uuid,
    pub candidates: u32,
    pub offloaded: u32,
    pub skipped_incomplete: u32,
    pub skipped_starred: u32,
    pub freed_bytes: u64,
    pub errors: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct OfflinePromptEvent {
    pub account_id: Uuid,
    pub account_email: String,
    pub percent: f32,
    pub used_bytes: u64,
    pub limit_bytes: Option<u64>,
}
