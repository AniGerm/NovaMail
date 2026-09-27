//! Per-account offline mailbox / IMAP quota offload policy.

use novamail_ipc::{OfflineMailboxAccountPolicy, OfflineMailboxMode, OfflineMailboxSettingsDto};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const SETTINGS_KEY: &str = "offline.mailbox";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct OfflineMailboxSettings {
    #[serde(default)]
    pub accounts: Vec<OfflineMailboxAccountPolicy>,
}

impl OfflineMailboxSettings {
    pub fn policy_for(&self, account_id: Uuid) -> OfflineMailboxAccountPolicy {
        self.accounts
            .iter()
            .find(|p| p.account_id == account_id)
            .cloned()
            .unwrap_or_else(|| OfflineMailboxAccountPolicy::default_for(account_id))
    }

    pub fn upsert(&mut self, policy: OfflineMailboxAccountPolicy) {
        if let Some(existing) = self
            .accounts
            .iter_mut()
            .find(|p| p.account_id == policy.account_id)
        {
            *existing = policy;
        } else {
            self.accounts.push(policy);
        }
    }

    pub fn to_dto(&self) -> OfflineMailboxSettingsDto {
        OfflineMailboxSettingsDto {
            accounts: self.accounts.clone(),
        }
    }

    pub fn from_dto(dto: OfflineMailboxSettingsDto) -> Self {
        Self {
            accounts: dto
                .accounts
                .into_iter()
                .map(sanitize_policy)
                .collect(),
        }
    }
}

pub fn sanitize_policy(mut policy: OfflineMailboxAccountPolicy) -> OfflineMailboxAccountPolicy {
    policy.prompt_threshold_percent = policy.prompt_threshold_percent.clamp(50, 99);
    policy.active_threshold_percent = policy.active_threshold_percent.clamp(10, 99);
    policy.min_age_days = policy.min_age_days.clamp(1, 3650);
    policy.batch_limit = policy.batch_limit.clamp(1, 500);
    policy
}

/// Whether the policy should actively offload messages given current quota %.
pub fn should_offload(policy: &OfflineMailboxAccountPolicy, percent: Option<f32>) -> bool {
    match policy.mode {
        OfflineMailboxMode::Off => false,
        OfflineMailboxMode::AlwaysPurge => true,
        OfflineMailboxMode::Threshold => percent
            .map(|p| p >= f32::from(policy.active_threshold_percent))
            .unwrap_or(false),
        OfflineMailboxMode::Overflow => {
            // Overflow acts near/at capacity; active_threshold is the trigger (often 95–99).
            percent
                .map(|p| p >= f32::from(policy.active_threshold_percent))
                .unwrap_or(false)
        }
    }
}

/// Whether to show the one-time enable prompt (mode Off, high usage, not dismissed).
pub fn should_prompt(policy: &OfflineMailboxAccountPolicy, percent: Option<f32>) -> bool {
    if policy.mode != OfflineMailboxMode::Off || policy.prompt_dismissed {
        return false;
    }
    percent
        .map(|p| p >= f32::from(policy.prompt_threshold_percent))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn threshold_requires_percent() {
        let policy = OfflineMailboxAccountPolicy {
            mode: OfflineMailboxMode::Threshold,
            active_threshold_percent: 80,
            ..OfflineMailboxAccountPolicy::default_for(Uuid::nil())
        };
        assert!(!should_offload(&policy, None));
        assert!(!should_offload(&policy, Some(79.0)));
        assert!(should_offload(&policy, Some(80.0)));
    }

    #[test]
    fn always_purge_without_quota() {
        let policy = OfflineMailboxAccountPolicy {
            mode: OfflineMailboxMode::AlwaysPurge,
            ..OfflineMailboxAccountPolicy::default_for(Uuid::nil())
        };
        assert!(should_offload(&policy, None));
    }

    #[test]
    fn prompt_once() {
        let mut policy = OfflineMailboxAccountPolicy::default_for(Uuid::nil());
        policy.prompt_threshold_percent = 90;
        assert!(should_prompt(&policy, Some(91.0)));
        policy.prompt_dismissed = true;
        assert!(!should_prompt(&policy, Some(99.0)));
        policy.prompt_dismissed = false;
        policy.mode = OfflineMailboxMode::Threshold;
        assert!(!should_prompt(&policy, Some(99.0)));
    }
}
