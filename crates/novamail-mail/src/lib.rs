//! Mail transport: IMAP sync, SMTP send, OAuth helpers.
//!
//! ADR: async-imap + tokio-rustls for IMAP; lettre for SMTP. Provider presets
//! live in novamail-ipc so the UI can render them without round-trips.

mod credentials;
mod draft_mime;
mod error;
mod imap_client;
mod oauth;
mod oauth_callback;
mod parse;
mod pop3;
mod remote_actions;
mod scheduler;
mod smtp_client;
mod sync;
mod tls;

pub use credentials::ensure_fresh_credentials;
pub use error::{MailError, MailResult};
pub use imap_client::ImapSession;
pub use oauth::{OAuthConfig, OAuthFlow, OAuthTokenResponse};
pub use oauth_callback::{wait_for_oauth_callback, OAuthCallbackResult};
pub use pop3::{Pop3Client, Pop3Message};
pub use remote_actions::{
    append_sent_remote, archive_remote, delete_remote, move_remote, offload_message_remote,
    probe_account_quota, save_draft_remote, set_flags_remote,
};
pub use scheduler::SyncScheduler;
pub use smtp_client::SmtpClient;
pub use sync::{SyncEngine, SyncReport};
pub use tls::ensure_crypto_provider;
