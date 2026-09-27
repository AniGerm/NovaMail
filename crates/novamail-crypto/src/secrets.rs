use keyring::Entry;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{CryptoError, CryptoResult};

const SERVICE: &str = "app.novamail.desktop";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct OAuthTokens {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum AccountCredentials {
    Password { password: String },
    OAuth2 { tokens: OAuthTokens },
}

#[derive(Debug, Clone, Default)]
pub struct SecretStore {
    /// When true, secrets stay in-process only (tests / headless CI without keyring).
    memory_fallback: bool,
    memory: std::sync::Arc<memory_map::MemoryMap>,
}

mod memory_map {
    use std::collections::HashMap;
    use std::sync::Mutex;

    #[derive(Debug, Default)]
    pub struct MemoryMap {
        inner: Mutex<HashMap<String, String>>,
    }

    impl MemoryMap {
        pub fn set(&self, key: &str, value: &str) {
            self.inner
                .lock()
                .unwrap()
                .insert(key.to_string(), value.to_string());
        }

        pub fn get(&self, key: &str) -> Option<String> {
            self.inner.lock().unwrap().get(key).cloned()
        }

        pub fn delete(&self, key: &str) {
            self.inner.lock().unwrap().remove(key);
        }
    }
}

impl SecretStore {
    pub fn new() -> Self {
        Self::default()
    }

    /// Prefer keyring; fall back to memory if the daemon is unavailable.
    pub fn with_memory_fallback(memory_fallback: bool) -> Self {
        Self {
            memory_fallback,
            ..Self::default()
        }
    }

    fn account_key(account_id: Uuid) -> String {
        format!("account:{account_id}")
    }

    pub fn store_credentials(
        &self,
        account_id: Uuid,
        credentials: &AccountCredentials,
    ) -> CryptoResult<()> {
        let payload = serde_json::to_string(credentials)?;
        let key = Self::account_key(account_id);
        match Entry::new(SERVICE, &key) {
            Ok(entry) => match entry.set_password(&payload) {
                Ok(()) => Ok(()),
                Err(err) if self.memory_fallback => {
                    tracing::warn!(error = %err, "keyring unavailable; using memory secret store");
                    self.memory.set(&key, &payload);
                    Ok(())
                }
                Err(err) => Err(CryptoError::Keyring(err.to_string())),
            },
            Err(err) if self.memory_fallback => {
                tracing::warn!(error = %err, "keyring entry failed; using memory secret store");
                self.memory.set(&key, &payload);
                Ok(())
            }
            Err(err) => Err(CryptoError::Keyring(err.to_string())),
        }
    }

    pub fn load_credentials(&self, account_id: Uuid) -> CryptoResult<AccountCredentials> {
        let key = Self::account_key(account_id);
        let payload = match Entry::new(SERVICE, &key) {
            Ok(entry) => match entry.get_password() {
                Ok(password) => password,
                Err(err) => {
                    if let Some(value) = self.memory.get(&key) {
                        value
                    } else {
                        return Err(CryptoError::Keyring(err.to_string()));
                    }
                }
            },
            Err(err) => {
                if let Some(value) = self.memory.get(&key) {
                    value
                } else {
                    return Err(CryptoError::Keyring(err.to_string()));
                }
            }
        };
        Ok(serde_json::from_str(&payload)?)
    }

    pub fn delete_credentials(&self, account_id: Uuid) -> CryptoResult<()> {
        self.delete_raw(&Self::account_key(account_id))
    }

    fn calendar_key(calendar_id: Uuid) -> String {
        format!("calendar:{calendar_id}")
    }

    pub fn store_calendar_password(&self, calendar_id: Uuid, password: &str) -> CryptoResult<()> {
        self.store_raw(&Self::calendar_key(calendar_id), password)
    }

    pub fn load_calendar_password(&self, calendar_id: Uuid) -> CryptoResult<Option<String>> {
        match self.load_raw(&Self::calendar_key(calendar_id)) {
            Ok(v) => Ok(Some(v)),
            Err(CryptoError::NotFound(_)) => Ok(None),
            Err(CryptoError::Keyring(_)) => Ok(None),
            Err(err) => Err(err),
        }
    }

    pub fn delete_calendar_password(&self, calendar_id: Uuid) -> CryptoResult<()> {
        self.delete_raw(&Self::calendar_key(calendar_id))
    }

    fn store_raw(&self, key: &str, payload: &str) -> CryptoResult<()> {
        match Entry::new(SERVICE, key) {
            Ok(entry) => match entry.set_password(payload) {
                Ok(()) => Ok(()),
                Err(err) if self.memory_fallback => {
                    tracing::warn!(error = %err, "keyring unavailable; using memory secret store");
                    self.memory.set(key, payload);
                    Ok(())
                }
                Err(err) => Err(CryptoError::Keyring(err.to_string())),
            },
            Err(err) if self.memory_fallback => {
                tracing::warn!(error = %err, "keyring entry failed; using memory secret store");
                self.memory.set(key, payload);
                Ok(())
            }
            Err(err) => Err(CryptoError::Keyring(err.to_string())),
        }
    }

    fn load_raw(&self, key: &str) -> CryptoResult<String> {
        match Entry::new(SERVICE, key) {
            Ok(entry) => match entry.get_password() {
                Ok(password) => Ok(password),
                Err(err) => {
                    if let Some(value) = self.memory.get(key) {
                        Ok(value)
                    } else {
                        Err(CryptoError::Keyring(err.to_string()))
                    }
                }
            },
            Err(err) => {
                if let Some(value) = self.memory.get(key) {
                    Ok(value)
                } else {
                    Err(CryptoError::Keyring(err.to_string()))
                }
            }
        }
    }

    fn delete_raw(&self, key: &str) -> CryptoResult<()> {
        self.memory.delete(key);
        if let Ok(entry) = Entry::new(SERVICE, key) {
            match entry.delete_credential() {
                Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
                Err(err) if self.memory_fallback => {
                    tracing::warn!(error = %err, "keyring delete failed; memory entry cleared");
                    Ok(())
                }
                Err(err) => Err(CryptoError::Keyring(err.to_string())),
            }
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_password_in_memory_fallback() {
        let store = SecretStore::with_memory_fallback(true);
        let id = Uuid::new_v4();
        let creds = AccountCredentials::Password {
            password: "s3cret".into(),
        };
        store.store_credentials(id, &creds).unwrap();
        let loaded = store.load_credentials(id).unwrap();
        assert_eq!(loaded, creds);
        store.delete_credentials(id).unwrap();
    }
}
