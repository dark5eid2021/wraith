//! HTTP route handlers.

pub mod health;
pub mod ingest;

pub use health::{health, ready};
pub use ingest::{ingest_batch, ingest_single, AppState};
