use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ContactDto {
    pub id: Uuid,
    pub display_name: String,
    pub emails: Vec<String>,
    pub phones: Vec<String>,
    pub notes: String,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct UpsertContactRequest {
    pub id: Option<Uuid>,
    pub display_name: String,
    pub emails: Vec<String>,
    pub phones: Vec<String>,
    pub notes: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LdapSearchRequest {
    pub url: String,
    pub bind_dn: Option<String>,
    pub password: Option<String>,
    pub base_dn: String,
    pub filter: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CardDavServerStatus {
    pub running: bool,
    pub listen_url: String,
    pub addressbook_url: String,
    pub contact_count: u32,
}
