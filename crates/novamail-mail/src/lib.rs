//! Mail transport: IMAP sync, SMTP send, OAuth helpers.
//!
//! ADR: async-imap + tokio-rustls for IMAP; lettre for SMTP. Provider presets
//! live in novamail-ipc so the UI can render them without round-trips.

mod error;
mod imap_client;
mod oauth;
mod oauth_callback;
mod parse;
mod scheduler;
mod smtp_client;
mod sync;
mod tls;

pub use error::{MailError, MailResult};
pub use imap_client::ImapSession;
pub use oauth::{OAuthConfig, OAuthFlow, OAuthTokenResponse};
pub use oauth_callback::{wait_for_oauth_callback, OAuthCallbackResult};
pub use scheduler::SyncScheduler;
pub use smtp_client::SmtpClient;
pub use sync::{SyncEngine, SyncReport};
