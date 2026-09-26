//! Shared data transfer objects for Tauri IPC.
//!
//! Frontend and Rust core share these shapes via serde JSON serialization.
//! Keeping DTOs in an isolated crate avoids circular dependencies and
//! documents the public application boundary.

mod account;
mod error;
mod mail;
mod search;
mod sync;

pub use account::*;
pub use error::*;
pub use mail::*;
pub use search::*;
pub use sync::*;
