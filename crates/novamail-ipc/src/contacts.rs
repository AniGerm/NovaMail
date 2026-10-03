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

/// Suggestion for composer recipient autocomplete (contacts + mail history).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RecipientSuggestion {
    pub email: String,
    pub name: Option<String>,
    pub source: String,
    pub in_contacts: bool,
    pub contact_id: Option<Uuid>,
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

/// Role of this NovaMail install for the shared address book.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub enum ContactsShareMode {
    /// Contacts stay on this PC only.
    #[default]
    Local,
    /// This PC is the hub: LDAP + CardDAV servers are offered to the LAN.
    Server,
    /// This PC syncs contacts from a hub via LDAP.
    Client,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LdapServerStatus {
    pub running: bool,
    /// Primary advertised URL (usually port 1389).
    pub listen_url: String,
    /// All bound URLs (may include port 389 when available).
    #[serde(default)]
    pub listen_urls: Vec<String>,
    /// When true, empty DN + empty password is accepted for read-only search.
    #[serde(default)]
    pub allow_anonymous: bool,
    pub base_dn: String,
    pub bind_dn: String,
    pub username: String,
    pub password: String,
    pub contact_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ContactsShareStatus {
    pub mode: ContactsShareMode,
    pub carddav: CardDavServerStatus,
    pub ldap_server: LdapServerStatus,
    /// When mode is Client: last LDAP sync settings used against the hub.
    #[serde(default)]
    pub client: Option<LdapSyncSettings>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SetContactsShareModeRequest {
    pub mode: ContactsShareMode,
    /// Required when switching to Client: hub LDAP URL, e.g. `ldap://192.168.1.10:1389`.
    #[serde(default)]
    pub client_url: Option<String>,
    #[serde(default)]
    pub client_bind_dn: Option<String>,
    #[serde(default)]
    pub client_password: Option<String>,
    #[serde(default)]
    pub client_base_dn: Option<String>,
}

#[cfg(test)]
mod share_mode_tests {
    use super::*;
    #[test]
    fn share_mode_json_roundtrip() {
        for mode in [
            ContactsShareMode::Local,
            ContactsShareMode::Server,
            ContactsShareMode::Client,
        ] {
            let s = serde_json::to_string(&mode).unwrap();
            let back: ContactsShareMode = serde_json::from_str(&s).unwrap();
            assert_eq!(mode, back, "roundtrip {s}");
        }
        assert_eq!(
            serde_json::from_str::<ContactsShareMode>(r#""server""#).unwrap(),
            ContactsShareMode::Server
        );
        let req = serde_json::from_str::<SetContactsShareModeRequest>(
            r#"{"mode":"server","clientUrl":null}"#,
        )
        .unwrap();
        assert_eq!(req.mode, ContactsShareMode::Server);
    }
}
