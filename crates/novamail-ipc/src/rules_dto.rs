use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RuleDto {
    pub id: Uuid,
    pub account_id: Option<Uuid>,
    pub name: String,
    pub enabled: bool,
    pub predicate_json: String,
    pub action_json: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct UpsertRuleRequest {
    pub id: Option<Uuid>,
    pub account_id: Option<Uuid>,
    pub name: String,
    pub enabled: bool,
    pub predicate_json: String,
    pub action_json: String,
}
