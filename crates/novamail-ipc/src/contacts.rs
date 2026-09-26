use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct ContactAddress {
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub street: String,
    #[serde(default)]
    pub city: String,
    #[serde(default)]
    pub region: String,
    #[serde(default)]
    pub postal_code: String,
    #[serde(default)]
    pub country: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct ContactCustomField {
    pub label: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ContactDto {
    pub id: Uuid,
    pub display_name: String,
    #[serde(default)]
    pub given_name: String,
    #[serde(default)]
    pub family_name: String,
    pub emails: Vec<String>,
    pub phones: Vec<String>,
    #[serde(default)]
    pub faxes: Vec<String>,
    #[serde(default)]
    pub organization: String,
    #[serde(default)]
    pub job_title: String,
    #[serde(default)]
    pub addresses: Vec<ContactAddress>,
    #[serde(default)]
    pub custom_fields: Vec<ContactCustomField>,
    /// Optional JPEG/PNG as raw base64 (no data-URL prefix), capped by the UI.
    #[serde(default)]
    pub photo_base64: Option<String>,
    /// Stable LDAP distinguished name when the contact came from directory sync.
    #[serde(default)]
    pub ldap_dn: Option<String>,
    pub notes: String,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct UpsertContactRequest {
    pub id: Option<Uuid>,
    pub display_name: String,
    #[serde(default)]
    pub given_name: String,
    #[serde(default)]
    pub family_name: String,
    pub emails: Vec<String>,
    pub phones: Vec<String>,
    #[serde(default)]
    pub faxes: Vec<String>,
    #[serde(default)]
    pub organization: String,
    #[serde(default)]
    pub job_title: String,
    #[serde(default)]
    pub addresses: Vec<ContactAddress>,
    #[serde(default)]
    pub custom_fields: Vec<ContactCustomField>,
    #[serde(default)]
    pub photo_base64: Option<String>,
    #[serde(default)]
    pub ldap_dn: Option<String>,
    pub notes: String,
}

/// How contact names are shown in the address book list and editor.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub enum ContactNameOrder {
    /// "Max Mustermann"
    #[default]
    GivenFamily,
    /// "Mustermann, Max"
    FamilyGiven,
}

/// Primary key used when sorting the contact list.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub enum ContactSortBy {
    #[default]
    FamilyName,
    GivenName,
    DisplayName,
    Organization,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ContactsBookSettings {
    #[serde(default)]
    pub name_order: ContactNameOrder,
    #[serde(default)]
    pub sort_by: ContactSortBy,
    #[serde(default)]
    pub sort_ascending: bool,
}

impl Default for ContactsBookSettings {
    fn default() -> Self {
        Self {
            name_order: ContactNameOrder::GivenFamily,
            sort_by: ContactSortBy::FamilyName,
            sort_ascending: true,
        }
    }
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
pub struct LdapSyncRequest {
    pub url: String,
    pub bind_dn: Option<String>,
    pub password: Option<String>,
    pub base_dn: String,
    pub filter: String,
    /// Persist these settings for the next sync.
    #[serde(default = "default_true")]
    pub save_settings: bool,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct LdapSyncSettings {
    pub url: String,
    pub bind_dn: Option<String>,
    /// Stored locally for sync; cleared when returned via `ldap_get_settings`.
    #[serde(default)]
    pub password: Option<String>,
    pub base_dn: String,
    pub filter: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LdapSyncResult {
    pub imported: u32,
    pub updated: u32,
    pub total: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CardDavServerStatus {
    pub running: bool,
    pub listen_url: String,
    pub addressbook_url: String,
    pub contact_count: u32,
    /// Fixed CardDAV username for Basic auth (phones/printers).
    pub username: String,
    /// Shared password; shown in UI so devices can authenticate.
    pub password: String,
}
