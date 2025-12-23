//! Storage models for ClickHouse.
//!
//! These types are used for persisting events to ClickHouse.

use chrono::{DateTime, Utc};
use clickhouse::Row;
use serde::{Deserialize, Serialize};

use wraith_common::{ClientMessage, Event};

/// Flattened event for ClickHouse storage
#[derive(Debug, Clone, Row, Serialize, Deserialize)]
pub struct StoredEvent {
    /// Unique event ID
    pub id: String,
    
    /// When the event was received
    pub received_at: DateTime<Utc>,
    
    /// Installation ID
    pub installation_id: String,
    
    /// Log level
    pub level: String,
    
    /// Event type name
    pub event_type: String,
    
    /// Tool name (if applicable)
    pub tool: String,
    
    /// Command name (if applicable)
    pub command: String,
    
    /// Duration in milliseconds (if applicable)
    pub duration_ms: u64,
    
    /// Error type (if applicable)
    pub error_type: String,
    
    /// InfraIQ version
    pub tool_version: String,
    
    /// Python version
    pub python_version: String,
    
    /// Operating system
    pub os: String,
    
    /// Full event JSON for additional fields
    pub raw_json: String,
}

impl StoredEvent {
    /// Convert an Event to a StoredEvent
    pub fn from_event(event: &Event) -> Self {
        let raw_json = serde_json::to_string(event).unwrap_or_default();
        
        StoredEvent {
            id: event.id.clone(),
            received_at: Utc::now(),
            installation_id: event.context.installation_id.clone(),
            level: event.level.as_str().to_string(),
            event_type: event.event.type_name().to_string(),
            tool: event.event.tool().unwrap_or("").to_string(),
            command: event.event.command().unwrap_or("").to_string(),
            duration_ms: event.event.duration_ms().unwrap_or(0),
            error_type: event.event.error_type().unwrap_or("").to_string(),
            tool_version: event.context.tool_version.clone(),
            python_version: event.context.python_version.clone(),
            os: event.context.os.clone(),
            raw_json,
        }
    }
    
    /// Convert a ClientMessage to a StoredEvent
    pub fn from_client_message(msg: ClientMessage) -> Self {
        let event = msg.into_event();
        Self::from_event(&event)
    }
}
