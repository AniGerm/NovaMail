use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SignatureDto {
    pub id: Uuid,
    pub account_id: Option<Uuid>,
    pub name: String,
    pub body_text: String,
    pub is_default: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct UpsertSignatureRequest {
    pub id: Option<Uuid>,
    pub account_id: Option<Uuid>,
    pub name: String,
    pub body_text: String,
    pub is_default: bool,
}
