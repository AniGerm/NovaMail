//! Retention policies for special folders (Spam, Trash, …).

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum RetentionMode {
    Keep,
    DeleteAfterDays,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct FolderPolicy {
    pub role: String,
    pub mode: RetentionMode,
    /// Used when `mode == DeleteAfterDays`.
    pub days: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct FolderPolicies {
    pub policies: Vec<FolderPolicy>,
}

impl Default for FolderPolicies {
    fn default() -> Self {
        Self {
            policies: vec![
                FolderPolicy {
                    role: "junk".into(),
                    mode: RetentionMode::DeleteAfterDays,
                    days: 30,
                },
                FolderPolicy {
                    role: "trash".into(),
                    mode: RetentionMode::DeleteAfterDays,
                    days: 30,
                },
            ],
        }
    }
}

pub const SETTINGS_KEY: &str = "folder.policies";
pub const SPAM_SETTINGS_KEY: &str = "spam.settings";
