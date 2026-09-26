//! NovaMail application core — use-cases over ports (db, mail, secrets).

mod app;
mod error;
mod paths;

pub use app::AppState;
pub use error::{CoreError, CoreResult};
pub use paths::AppPaths;
