//! Credential and secret management.
//!
//! ADR: OS keyring (Secret Service on Ubuntu) stores passwords and OAuth
//! tokens. SQLite never persists plaintext credentials.

mod error;
mod secrets;

pub use error::{CryptoError, CryptoResult};
pub mod backup;
pub mod pgp;

pub use backup::{
    decrypt_backup_payload, encrypt_backup_payload, EncryptedBackupFile, BACKUP_FORMAT,
};
pub use pgp::{
    decrypt_message, encrypt_message, generate_key, import_armored, looks_like_pgp, sign_message,
    verify_message, DecryptResult, PgpKeyInfo, VerifyResult,
};
pub use secrets::{AccountCredentials, OAuthTokens, SecretStore};
