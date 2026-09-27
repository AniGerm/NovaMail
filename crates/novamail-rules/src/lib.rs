//! Declarative mail rules engine.
//!
//! Predicates and actions are JSON-serializable so they can be stored in SQLite
//! and edited from the UI without recompiling.

use novamail_ipc::{AddressDto, MessageSummaryDto};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum RuleError {
    #[error("invalid rule: {0}")]
    Invalid(String),
}

pub type RuleResult<T> = Result<T, RuleError>;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RuleDefinition {
    pub id: Uuid,
    pub account_id: Option<Uuid>,
    pub name: String,
    pub enabled: bool,
    pub predicate: Predicate,
    pub actions: Vec<Action>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Predicate {
    Always,
    FromContains { value: String },
    ToContains { value: String },
    SubjectContains { value: String },
    BodyContains { value: String },
    And { items: Vec<Predicate> },
    Or { items: Vec<Predicate> },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Action {
    MarkRead,
    MarkUnread,
    Star,
    Unstar,
    AddLabel { label: String },
    MoveToMailbox { mailbox: String },
    /// Move into the account's junk/spam folder (creates Spam if needed).
    MoveToSpam,
    Delete,
}

#[derive(Debug, Clone)]
pub struct RuleMatchContext<'a> {
    pub message: &'a MessageSummaryDto,
    pub body_text: &'a str,
}

pub fn matches(predicate: &Predicate, ctx: &RuleMatchContext<'_>) -> bool {
    match predicate {
        Predicate::Always => true,
        Predicate::FromContains { value } => contains_email_or_name(&ctx.message.from, value),
        Predicate::ToContains { value } => ctx
            .message
            .to
            .iter()
            .any(|addr| contains_email_or_name(addr, value)),
        Predicate::SubjectContains { value } => ctx
            .message
            .subject
            .to_ascii_lowercase()
            .contains(&value.to_ascii_lowercase()),
        Predicate::BodyContains { value } => ctx
            .body_text
            .to_ascii_lowercase()
            .contains(&value.to_ascii_lowercase()),
        Predicate::And { items } => items.iter().all(|item| matches(item, ctx)),
        Predicate::Or { items } => items.iter().any(|item| matches(item, ctx)),
    }
}

pub fn evaluate_rules(
    rules: &[RuleDefinition],
    ctx: &RuleMatchContext<'_>,
) -> Vec<(Uuid, Vec<Action>)> {
    rules
        .iter()
        .filter(|rule| rule.enabled)
        .filter(|rule| {
            rule.account_id
                .map(|id| id == ctx.message.account_id)
                .unwrap_or(true)
        })
        .filter(|rule| matches(&rule.predicate, ctx))
        .map(|rule| (rule.id, rule.actions.clone()))
        .collect()
}

fn contains_email_or_name(address: &AddressDto, value: &str) -> bool {
    let needle = value.to_ascii_lowercase();
    address.email.to_ascii_lowercase().contains(&needle)
        || address
            .name
            .as_ref()
            .map(|n| n.to_ascii_lowercase().contains(&needle))
            .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use novamail_ipc::AddressDto;

    fn sample_message() -> MessageSummaryDto {
        MessageSummaryDto {
            id: Uuid::new_v4(),
            account_id: Uuid::new_v4(),
            mailbox_id: Uuid::new_v4(),
            thread_id: Uuid::new_v4(),
            subject: "Invoice from Acme".into(),
            from: AddressDto {
                name: Some("Billing".into()),
                email: "billing@acme.example".into(),
            },
            to: vec![],
            date: 1,
            snippet: "Please pay".into(),
            unread: true,
            starred: false,
            has_attachments: false,
            account_email: "me@example.com".into(),
            local_only: false,
            snoozed_until: None,
        }
    }

    #[test]
    fn matches_from_and_subject() {
        let message = sample_message();
        let ctx = RuleMatchContext {
            message: &message,
            body_text: "Please pay the invoice",
        };
        let predicate = Predicate::And {
            items: vec![
                Predicate::FromContains {
                    value: "acme.example".into(),
                },
                Predicate::SubjectContains {
                    value: "invoice".into(),
                },
            ],
        };
        assert!(matches(&predicate, &ctx));
    }
}
