use novamail_ipc::AppError;
use thiserror::Error;

pub type CoreResult<T> = Result<T, CoreError>;

#[derive(Debug, Error)]
pub enum CoreError {
    #[error(transparent)]
    Db(#[from] novamail_db::DbError),
    #[error(transparent)]
    Mail(#[from] novamail_mail::MailError),
    #[error(transparent)]
    Crypto(#[from] novamail_crypto::CryptoError),
    #[error(transparent)]
    Search(#[from] novamail_search::SearchError),
    #[error(transparent)]
    Ai(#[from] novamail_ai::AiError),
    #[error(transparent)]
    Contacts(#[from] novamail_contacts::ContactsError),
    #[error("{0}")]
    Message(String),
}

impl From<CoreError> for AppError {
    fn from(value: CoreError) -> Self {
        let code = match &value {
            CoreError::Db(novamail_db::DbError::NotFound(_)) => "not_found",
            CoreError::Crypto(novamail_crypto::CryptoError::BadPassphrase) => "bad_passphrase",
            CoreError::Crypto(_) => "crypto",
            CoreError::Mail(novamail_mail::MailError::Auth(_)) => "auth",
            CoreError::Mail(_) => "mail",
            CoreError::Search(_) => "search",
            CoreError::Ai(novamail_ai::AiError::Unavailable(_)) => "ai_unavailable",
            CoreError::Ai(_) => "ai",
            CoreError::Contacts(_) => "contacts",
            CoreError::Db(_) => "db",
            CoreError::Message(_) => "app",
        };
        AppError::new(code, value.to_string())
    }
}
