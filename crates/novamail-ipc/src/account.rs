use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum AuthType {
    Password,
    OAuth2,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum MailProvider {
    Generic,
    Gmail,
    Microsoft365,
    Yahoo,
    ProtonBridge,
    Icloud,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AccountDto {
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
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AddAccountPasswordRequest {
    pub name: String,
    pub email: String,
    pub password: String,
    pub provider: MailProvider,
    pub imap_host: String,
    pub imap_port: u16,
    pub imap_tls: bool,
    pub smtp_host: String,
    pub smtp_port: u16,
    pub smtp_tls: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AddAccountOAuthRequest {
    pub name: String,
    pub email: String,
    pub provider: MailProvider,
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_at: Option<i64>,
    pub imap_host: String,
    pub imap_port: u16,
    pub imap_tls: bool,
    pub smtp_host: String,
    pub smtp_port: u16,
    pub smtp_tls: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProviderPreset {
    pub provider: MailProvider,
    pub label: String,
    pub imap_host: String,
    pub imap_port: u16,
    pub imap_tls: bool,
    pub smtp_host: String,
    pub smtp_port: u16,
    pub smtp_tls: bool,
    pub auth_type: AuthType,
    pub oauth_authorize_url: Option<String>,
}

impl ProviderPreset {
    pub fn all() -> Vec<Self> {
        vec![
            Self {
                provider: MailProvider::Gmail,
                label: "Gmail".into(),
                imap_host: "imap.gmail.com".into(),
                imap_port: 993,
                imap_tls: true,
                smtp_host: "smtp.gmail.com".into(),
                smtp_port: 465,
                smtp_tls: true,
                auth_type: AuthType::OAuth2,
                oauth_authorize_url: Some("https://accounts.google.com/o/oauth2/v2/auth".into()),
            },
            Self {
                provider: MailProvider::Microsoft365,
                label: "Microsoft 365".into(),
                imap_host: "outlook.office365.com".into(),
                imap_port: 993,
                imap_tls: true,
                smtp_host: "smtp.office365.com".into(),
                smtp_port: 587,
                smtp_tls: true,
                auth_type: AuthType::OAuth2,
                oauth_authorize_url: Some(
                    "https://login.microsoftonline.com/common/oauth2/v2.0/authorize".into(),
                ),
            },
            Self {
                provider: MailProvider::Yahoo,
                label: "Yahoo Mail".into(),
                imap_host: "imap.mail.yahoo.com".into(),
                imap_port: 993,
                imap_tls: true,
                smtp_host: "smtp.mail.yahoo.com".into(),
                smtp_port: 465,
                smtp_tls: true,
                auth_type: AuthType::OAuth2,
                oauth_authorize_url: Some("https://api.login.yahoo.com/oauth2/request_auth".into()),
            },
            Self {
                provider: MailProvider::ProtonBridge,
                label: "Proton Bridge".into(),
                imap_host: "127.0.0.1".into(),
                imap_port: 1143,
                imap_tls: false,
                smtp_host: "127.0.0.1".into(),
                smtp_port: 1025,
                smtp_tls: false,
                auth_type: AuthType::Password,
                oauth_authorize_url: None,
            },
            Self {
                provider: MailProvider::Icloud,
                label: "iCloud Mail".into(),
                imap_host: "imap.mail.me.com".into(),
                imap_port: 993,
                imap_tls: true,
                smtp_host: "smtp.mail.me.com".into(),
                smtp_port: 587,
                smtp_tls: true,
                auth_type: AuthType::Password,
                oauth_authorize_url: None,
            },
            Self {
                provider: MailProvider::Generic,
                label: "Other IMAP / SMTP".into(),
                imap_host: String::new(),
                imap_port: 993,
                imap_tls: true,
                smtp_host: String::new(),
                smtp_port: 465,
                smtp_tls: true,
                auth_type: AuthType::Password,
                oauth_authorize_url: None,
            },
        ]
    }
}
