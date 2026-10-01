use serde::{Deserialize, Serialize};

/// Status events emitted on `update://status` while checking / installing.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum UpdateStatusEvent {
    Checking,
    #[serde(rename_all = "camelCase")]
    UpdateAvailable { version: String },
    #[serde(rename_all = "camelCase")]
    UpdateNotAvailable { version: String },
    #[serde(rename_all = "camelCase")]
    DownloadProgress { percent: u32 },
    #[serde(rename_all = "camelCase")]
    UpdateDownloaded { version: String },
    #[serde(rename_all = "camelCase")]
    Installing { version: String },
    #[serde(rename_all = "camelCase")]
    Error { message: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AppVersionInfo {
    pub version: String,
    pub packaged: bool,
    pub channel: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct UpdateCheckResult {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub available_version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct UpdateActionResult {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}
