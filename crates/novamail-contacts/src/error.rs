use thiserror::Error;

#[derive(Debug, Error)]
pub enum ContactsError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("ldap error: {0}")]
    Ldap(String),
    #[error("carddav error: {0}")]
    CardDav(String),
    #[error("contact store error: {0}")]
    Store(String),
    #[error("not found: {0}")]
    NotFound(String),
}

pub type ContactsResult<T> = Result<T, ContactsError>;
