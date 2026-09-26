use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    AccountDto, AuthType, ContactAddress, ContactCustomField, ContactDto, ContactsBookSettings,
    LabelDto, LdapSyncSettings, MailProvider, RuleDto, SignatureDto,
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BackupAccountCredentials {
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub access_token: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refresh_token: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BackupAccount {
    pub id: Uuid,
    pub name: String,
    pub email: String,
    pub provider: MailProvider,
    pub auth_type: AuthType,
    pub imap_host: String,
    pub imap_port: u16,
    pub imap_tls: bool,
    pub smtp_host: String,
    pub smtp_port: u16,
    pub smtp_tls: bool,
    pub created_at: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub credentials: Option<BackupAccountCredentials>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BackupContact {
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
    #[serde(default)]
    pub photo_base64: Option<String>,
    #[serde(default)]
    pub ldap_dn: Option<String>,
    pub notes: String,
    pub updated_at: i64,
}

impl From<&ContactDto> for BackupContact {
    fn from(c: &ContactDto) -> Self {
        Self {
            id: c.id,
            display_name: c.display_name.clone(),
            given_name: c.given_name.clone(),
            family_name: c.family_name.clone(),
            emails: c.emails.clone(),
            phones: c.phones.clone(),
            faxes: c.faxes.clone(),
            organization: c.organization.clone(),
            job_title: c.job_title.clone(),
            addresses: c.addresses.clone(),
            custom_fields: c.custom_fields.clone(),
            photo_base64: c.photo_base64.clone(),
            ldap_dn: c.ldap_dn.clone(),
            notes: c.notes.clone(),
            updated_at: c.updated_at,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BackupPayload {
    pub version: u32,
    pub exported_at: i64,
    pub accounts: Vec<BackupAccount>,
    pub contacts: Vec<BackupContact>,
    pub labels: Vec<LabelDto>,
    pub rules: Vec<RuleDto>,
    pub signatures: Vec<SignatureDto>,
    #[serde(default)]
    pub contacts_book: Option<ContactsBookSettings>,
    #[serde(default)]
    pub ldap: Option<LdapSyncSettings>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ExportBackupRequest {
    pub passphrase: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ExportBackupResponse {
    pub filename: String,
    pub data_base64: String,
    pub accounts: u32,
    pub contacts: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ImportBackupRequest {
    pub passphrase: String,
    pub data_base64: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct ImportBackupResult {
    pub accounts_imported: u32,
    pub accounts_updated: u32,
    pub contacts_imported: u32,
    pub contacts_updated: u32,
    pub contacts_skipped: u32,
    pub labels_imported: u32,
    pub rules_imported: u32,
    pub signatures_imported: u32,
}

impl From<&AccountDto> for BackupAccount {
    fn from(a: &AccountDto) -> Self {
        Self {
            id: a.id,
            name: a.name.clone(),
            email: a.email.clone(),
            provider: a.provider.clone(),
            auth_type: a.auth_type.clone(),
            imap_host: a.imap_host.clone(),
            imap_port: a.imap_port,
            imap_tls: a.imap_tls,
            smtp_host: a.smtp_host.clone(),
            smtp_port: a.smtp_port,
            smtp_tls: a.smtp_tls,
            created_at: a.created_at,
            credentials: None,
        }
    }
}
