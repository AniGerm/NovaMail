//! Shared data transfer objects for Tauri IPC.
//!
//! Frontend and Rust core share these shapes via serde JSON serialization.
//! Keeping DTOs in an isolated crate avoids circular dependencies and
//! documents the public application boundary.

mod account;
mod ai;
mod attachments;
mod backup;
mod contacts;
mod error;
mod labels;
mod mail;
mod oauth;
mod rules_dto;
mod search;
mod signatures;
mod spellcheck;
mod sync;

pub use account::*;
pub use ai::*;
pub use attachments::*;
pub use backup::*;
pub use contacts::*;
pub use error::*;
pub use labels::*;
pub use mail::*;
pub use oauth::*;
pub use rules_dto::*;
pub use search::*;
pub use signatures::*;
pub use spellcheck::*;
pub use sync::*;
