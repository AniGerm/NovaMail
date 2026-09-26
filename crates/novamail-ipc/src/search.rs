use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::MessageSummaryDto;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SearchRequest {
    pub query: String,
    pub account_id: Option<Uuid>,
    pub limit: u32,
    pub offset: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SearchResponse {
    pub messages: Vec<MessageSummaryDto>,
    pub total: u32,
}
