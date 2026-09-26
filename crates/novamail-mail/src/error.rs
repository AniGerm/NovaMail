use thiserror::Error;

pub type MailResult<T> = Result<T, MailError>;

#[derive(Debug, Error)]
pub enum MailError {
    #[error("imap error: {0}")]
    Imap(String),
    #[error("smtp error: {0}")]
    Smtp(String),
    #[error("pop3 error: {0}")]
    Pop3(String),
    #[error("tls error: {0}")]
    Tls(String),
    #[error("auth error: {0}")]
    Auth(String),
    #[error("parse error: {0}")]
    Parse(String),
    #[error("oauth error: {0}")]
    OAuth(String),
    #[error("database error: {0}")]
    Db(#[from] novamail_db::DbError),
    #[error("crypto error: {0}")]
    Crypto(#[from] novamail_crypto::CryptoError),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Other(String),
}
