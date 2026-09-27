//! NovaMail application core — use-cases over ports (db, mail, secrets).

mod app;
mod caldav;
mod contacts_store;
mod error;
mod folder_policies;
mod offline_mailbox;
mod paths;
mod sanitize;
mod spam;
pub mod spellcheck;

pub use app::AppState;
pub use error::{CoreError, CoreResult};
pub use paths::AppPaths;
