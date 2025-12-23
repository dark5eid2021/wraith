//! Data models for Wraith server.

pub mod stored;

pub use stored::StoredEvent;

// Re-export common types for convenience
pub use wraith_common::{ClientMessage, Event, EventBatch, EventContext, EventType, Level};
