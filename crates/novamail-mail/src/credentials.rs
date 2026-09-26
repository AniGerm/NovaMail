//! Ensure OAuth access tokens are fresh before IMAP/SMTP use.

use novamail_crypto::{AccountCredentials, SecretStore};
use novamail_db::AccountRecord;
use novamail_ipc::AuthType;

use crate::oauth::OAuthConfig;
use crate::{MailError, MailResult};

/// Load credentials and refresh OAuth tokens when near expiry.
pub async fn ensure_fresh_credentials(
    account: &AccountRecord,
    secrets: &SecretStore,
) -> MailResult<AccountCredentials> {
    let credentials = secrets
        .load_credentials(account.id)
        .map_err(|e| MailError::Auth(e.to_string()))?;

    match credentials {
        AccountCredentials::OAuth2 { tokens }
            if account.auth_type == AuthType::OAuth2 && token_needs_refresh(&tokens) =>
        {
            let refresh = tokens.refresh_token.as_deref().ok_or_else(|| {
                MailError::Auth(
                    "OAuth access token expired and no refresh_token is stored; sign in again"
                        .into(),
                )
            })?;
            let config = OAuthConfig::for_provider(&account.provider)?;
            let response = config.refresh_tokens(refresh).await?;
            let refreshed = AccountCredentials::OAuth2 {
                tokens: response.tokens,
            };
            secrets
                .store_credentials(account.id, &refreshed)
                .map_err(|e| MailError::Auth(e.to_string()))?;
            tracing::info!(account_id = %account.id, "refreshed OAuth access token");
            Ok(refreshed)
        }
        other => Ok(other),
    }
}

fn token_needs_refresh(tokens: &novamail_crypto::OAuthTokens) -> bool {
    match tokens.expires_at {
        Some(expires_at) => {
            // Refresh one minute early to avoid racing the IMAP login.
            expires_at <= chrono::Utc::now().timestamp() + 60
        }
        // Unknown expiry: keep using the access token until the server rejects it.
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use novamail_crypto::OAuthTokens;

    #[test]
    fn refreshes_when_expired() {
        let tokens = OAuthTokens {
            access_token: "a".into(),
            refresh_token: Some("r".into()),
            expires_at: Some(chrono::Utc::now().timestamp() - 10),
        };
        assert!(token_needs_refresh(&tokens));
    }

    #[test]
    fn skips_when_fresh() {
        let tokens = OAuthTokens {
            access_token: "a".into(),
            refresh_token: Some("r".into()),
            expires_at: Some(chrono::Utc::now().timestamp() + 3600),
        };
        assert!(!token_needs_refresh(&tokens));
    }

    #[test]
    fn skips_when_expiry_unknown() {
        let tokens = OAuthTokens {
            access_token: "a".into(),
            refresh_token: Some("r".into()),
            expires_at: None,
        };
        assert!(!token_needs_refresh(&tokens));
    }
}
