//! Credential and secret management.
//!
//! ADR: OS keyring (Secret Service on Ubuntu) stores passwords and OAuth
//! tokens. SQLite never persists plaintext credentials.

mod error;
mod secrets;

pub use error::{CryptoError, CryptoResult};
pub use secrets::{AccountCredentials, OAuthTokens, SecretStore};
