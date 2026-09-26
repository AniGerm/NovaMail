use thiserror::Error;

pub type CryptoResult<T> = Result<T, CryptoError>;

#[derive(Debug, Error)]
pub enum CryptoError {
    #[error("keyring error: {0}")]
    Keyring(String),
    #[error("serialization error: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("backup encryption error: {0}")]
    Backup(String),
    #[error("wrong backup passphrase or corrupt file")]
    BadPassphrase,
}
