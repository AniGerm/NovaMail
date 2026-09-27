//! NovaMail application core — use-cases over ports (db, mail, secrets).

mod app;
mod contacts_store;
mod error;
mod folder_policies;
mod paths;
mod sanitize;
mod spam;
pub mod spellcheck;

pub use app::AppState;
pub use error::{CoreError, CoreResult};
pub use paths::AppPaths;
