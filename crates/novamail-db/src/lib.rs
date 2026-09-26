//! SQLite persistence for NovaMail.
//!
//! ADR: rusqlite with bundled SQLite keeps packaging simple on Ubuntu and
//! avoids async overhead for a local single-writer desktop database.

mod error;
mod migrations;
pub mod models;
mod repository;

pub use error::{DbError, DbResult};
pub use models::*;
pub use repository::Database;
