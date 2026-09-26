//! OAuth2 helpers for Gmail / Microsoft / Yahoo.
//!
//! Client IDs are injected at runtime via environment so the open-source tree
//! never ships secrets. Authorize URLs open in the system browser; token
//! exchange uses the authorization code from the localhost redirect.

use novamail_crypto::OAuthTokens;
use novamail_ipc::MailProvider;
use serde::{Deserialize, Serialize};

use crate::{MailError, MailResult};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OAuthConfig {
    pub client_id: String,
    pub client_secret: Option<String>,
    pub redirect_uri: String,
    pub scopes: Vec<String>,
    pub authorize_url: String,
    pub token_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OAuthTokenResponse {
    pub tokens: OAuthTokens,
    pub token_type: Option<String>,
    pub scope: Option<String>,
}

impl OAuthConfig {
    pub fn for_provider(provider: &MailProvider) -> MailResult<Self> {
        match provider {
            MailProvider::Gmail => Ok(Self {
                client_id: std::env::var("NOVAMAIL_GOOGLE_CLIENT_ID").unwrap_or_default(),
                client_secret: std::env::var("NOVAMAIL_GOOGLE_CLIENT_SECRET").ok(),
                redirect_uri: "http://127.0.0.1:17832/oauth/callback".into(),
                scopes: vec![
                    "https://mail.google.com/".into(),
                    "email".into(),
                    "profile".into(),
                ],
                authorize_url: "https://accounts.google.com/o/oauth2/v2/auth".into(),
                token_url: "https://oauth2.googleapis.com/token".into(),
            }),
            MailProvider::Microsoft365 => Ok(Self {
                client_id: std::env::var("NOVAMAIL_MS_CLIENT_ID").unwrap_or_default(),
                client_secret: std::env::var("NOVAMAIL_MS_CLIENT_SECRET").ok(),
                redirect_uri: "http://127.0.0.1:17832/oauth/callback".into(),
                scopes: vec![
                    "offline_access".into(),
                    "https://outlook.office.com/IMAP.AccessAsUser.All".into(),
                    "https://outlook.office.com/SMTP.Send".into(),
                ],
                authorize_url: "https://login.microsoftonline.com/common/oauth2/v2.0/authorize"
                    .into(),
                token_url: "https://login.microsoftonline.com/common/oauth2/v2.0/token".into(),
            }),
            MailProvider::Yahoo => Ok(Self {
                client_id: std::env::var("NOVAMAIL_YAHOO_CLIENT_ID").unwrap_or_default(),
                client_secret: std::env::var("NOVAMAIL_YAHOO_CLIENT_SECRET").ok(),
                redirect_uri: "http://127.0.0.1:17832/oauth/callback".into(),
                scopes: vec!["mail-r".into(), "mail-w".into()],
                authorize_url: "https://api.login.yahoo.com/oauth2/request_auth".into(),
                token_url: "https://api.login.yahoo.com/oauth2/get_token".into(),
            }),
            _ => Err(MailError::OAuth(
                "provider does not support OAuth2 in NovaMail".into(),
            )),
        }
    }

    pub fn authorize_url_with_state(&self, state: &str) -> MailResult<String> {
        if self.client_id.is_empty() {
            return Err(MailError::OAuth(
                "OAuth client_id is not configured. Set NOVAMAIL_*_CLIENT_ID.".into(),
            ));
        }
        let mut url =
            url::Url::parse(&self.authorize_url).map_err(|e| MailError::OAuth(e.to_string()))?;
        {
            let mut qp = url.query_pairs_mut();
            qp.append_pair("client_id", &self.client_id);
            qp.append_pair("redirect_uri", &self.redirect_uri);
            qp.append_pair("response_type", "code");
            qp.append_pair("state", state);
            qp.append_pair("access_type", "offline");
            qp.append_pair("prompt", "consent");
            qp.append_pair("scope", &self.scopes.join(" "));
        }
        Ok(url.into())
    }

    pub async fn exchange_code(&self, code: &str) -> MailResult<OAuthTokenResponse> {
        if self.client_id.is_empty() {
            return Err(MailError::OAuth(
                "OAuth client_id is not configured. Set NOVAMAIL_*_CLIENT_ID.".into(),
            ));
        }
        if code.trim().is_empty() {
            return Err(MailError::OAuth("authorization code is empty".into()));
        }

        let client = reqwest::Client::new();
        let mut form = vec![
            ("grant_type", "authorization_code".to_string()),
            ("code", code.to_string()),
            ("redirect_uri", self.redirect_uri.clone()),
            ("client_id", self.client_id.clone()),
        ];
        if let Some(secret) = &self.client_secret {
            form.push(("client_secret", secret.clone()));
        }

        let response = client
            .post(&self.token_url)
            .form(&form)
            .send()
            .await
            .map_err(|e| MailError::OAuth(format!("token request failed: {e}")))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(MailError::OAuth(format!(
                "token endpoint returned {status}: {body}"
            )));
        }

        #[derive(Deserialize)]
        struct TokenJson {
            access_token: String,
            refresh_token: Option<String>,
            expires_in: Option<i64>,
            token_type: Option<String>,
            scope: Option<String>,
        }

        let parsed = response
            .json::<TokenJson>()
            .await
            .map_err(|e| MailError::OAuth(format!("invalid token JSON: {e}")))?;

        let expires_at = parsed
            .expires_in
            .map(|seconds| chrono::Utc::now().timestamp() + seconds);

        Ok(OAuthTokenResponse {
            tokens: OAuthTokens {
                access_token: parsed.access_token,
                refresh_token: parsed.refresh_token,
                expires_at,
            },
            token_type: parsed.token_type,
            scope: parsed.scope,
        })
    }

    pub async fn refresh_tokens(&self, refresh_token: &str) -> MailResult<OAuthTokenResponse> {
        if refresh_token.trim().is_empty() {
            return Err(MailError::OAuth("refresh_token is empty".into()));
        }

        let client = reqwest::Client::new();
        let mut form = vec![
            ("grant_type", "refresh_token".to_string()),
            ("refresh_token", refresh_token.to_string()),
            ("client_id", self.client_id.clone()),
        ];
        if let Some(secret) = &self.client_secret {
            form.push(("client_secret", secret.clone()));
        }

        let response = client
            .post(&self.token_url)
            .form(&form)
            .send()
            .await
            .map_err(|e| MailError::OAuth(format!("refresh request failed: {e}")))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(MailError::OAuth(format!(
                "refresh endpoint returned {status}: {body}"
            )));
        }

        #[derive(Deserialize)]
        struct TokenJson {
            access_token: String,
            refresh_token: Option<String>,
            expires_in: Option<i64>,
            token_type: Option<String>,
            scope: Option<String>,
        }

        let parsed = response
            .json::<TokenJson>()
            .await
            .map_err(|e| MailError::OAuth(format!("invalid refresh JSON: {e}")))?;

        let expires_at = parsed
            .expires_in
            .map(|seconds| chrono::Utc::now().timestamp() + seconds);

        Ok(OAuthTokenResponse {
            tokens: OAuthTokens {
                access_token: parsed.access_token,
                refresh_token: parsed.refresh_token.or_else(|| Some(refresh_token.to_string())),
                expires_at,
            },
            token_type: parsed.token_type,
            scope: parsed.scope,
        })
    }
}

#[derive(Debug, Clone)]
pub struct OAuthFlow;

impl OAuthFlow {
    pub fn new_state() -> String {
        uuid::Uuid::new_v4().to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gmail_authorize_requires_client_id() {
        std::env::remove_var("NOVAMAIL_GOOGLE_CLIENT_ID");
        let cfg = OAuthConfig::for_provider(&MailProvider::Gmail).unwrap();
        assert!(cfg.authorize_url_with_state("abc").is_err());
    }

    #[tokio::test]
    async fn exchange_rejects_empty_code() {
        std::env::set_var("NOVAMAIL_GOOGLE_CLIENT_ID", "test-client");
        let cfg = OAuthConfig::for_provider(&MailProvider::Gmail).unwrap();
        let err = cfg.exchange_code("").await.unwrap_err();
        assert!(err.to_string().contains("empty"));
        std::env::remove_var("NOVAMAIL_GOOGLE_CLIENT_ID");
    }
}
