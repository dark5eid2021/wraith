//! Shared types for Wraith telemetry system.
//!
//! This crate contains the event types shared between:
//! - wraith-daemon (the client that collects events)
//! - wraith-server (the backend that stores events)
//!
//! Any changes to event format should be made here.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Log level / severity of an event
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum Level {
    Debug,
    Info,
    Warning,
    Error,
    Critical,
    Fatal,
}

impl Level {
    /// Returns true if this level should trigger immediate flush
    pub fn is_urgent(&self) -> bool {
        matches!(self, Level::Critical | Level::Fatal)
    }
    
    /// Convert to string for storage
    pub fn as_str(&self) -> &'static str {
        match self {
            Level::Debug => "DEBUG",
            Level::Info => "INFO",
            Level::Warning => "WARNING",
            Level::Error => "ERROR",
            Level::Critical => "CRITICAL",
            Level::Fatal => "FATAL",
        }
    }
}

/// Context sent with every event (anonymous)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventContext {
    /// UUID generated on first run, stored locally
    pub installation_id: String,
    
    /// InfraIQ version
    pub tool_version: String,
    
    /// Python version
    pub python_version: String,
    
    /// Operating system
    pub os: String,
    
    /// OS version
    #[serde(skip_serializing_if = "Option::is_none")]
    pub os_version: Option<String>,
}

/// Event types that Wraith tracks
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event_type", rename_all = "snake_case")]
pub enum EventType {
    /// Tool was invoked
    ToolInvoked {
        tool: String,
        command: String,
    },
    
    /// Tool completed successfully
    ToolSucceeded {
        tool: String,
        command: String,
        duration_ms: u64,
    },
    
    /// Tool failed (handled error)
    ToolFailed {
        tool: String,
        command: String,
        error_type: String,
        duration_ms: u64,
    },
    
    /// Unhandled exception
    ExceptionUnhandled {
        tool: String,
        exception_type: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        traceback: Option<String>,
    },
    
    /// Output validation failed (e.g., terraform validate)
    ValidationFailed {
        tool: String,
        validation_type: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        details: Option<String>,
    },
    
    /// Wraith daemon started
    DaemonStarted {
        parent_pid: u32,
    },
    
    /// Wraith daemon shutting down
    DaemonStopping {
        reason: String,
    },
}

impl EventType {
    /// Get the event type name as a string
    pub fn type_name(&self) -> &'static str {
        match self {
            EventType::ToolInvoked { .. } => "tool_invoked",
            EventType::ToolSucceeded { .. } => "tool_succeeded",
            EventType::ToolFailed { .. } => "tool_failed",
            EventType::ExceptionUnhandled { .. } => "exception_unhandled",
            EventType::ValidationFailed { .. } => "validation_failed",
            EventType::DaemonStarted { .. } => "daemon_started",
            EventType::DaemonStopping { .. } => "daemon_stopping",
        }
    }
    
    /// Extract tool name if present
    pub fn tool(&self) -> Option<&str> {
        match self {
            EventType::ToolInvoked { tool, .. } => Some(tool),
            EventType::ToolSucceeded { tool, .. } => Some(tool),
            EventType::ToolFailed { tool, .. } => Some(tool),
            EventType::ExceptionUnhandled { tool, .. } => Some(tool),
            EventType::ValidationFailed { tool, .. } => Some(tool),
            _ => None,
        }
    }
    
    /// Extract command name if present
    pub fn command(&self) -> Option<&str> {
        match self {
            EventType::ToolInvoked { command, .. } => Some(command),
            EventType::ToolSucceeded { command, .. } => Some(command),
            EventType::ToolFailed { command, .. } => Some(command),
            _ => None,
        }
    }
    
    /// Extract duration if present
    pub fn duration_ms(&self) -> Option<u64> {
        match self {
            EventType::ToolSucceeded { duration_ms, .. } => Some(*duration_ms),
            EventType::ToolFailed { duration_ms, .. } => Some(*duration_ms),
            _ => None,
        }
    }
    
    /// Extract error type if present
    pub fn error_type(&self) -> Option<&str> {
        match self {
            EventType::ToolFailed { error_type, .. } => Some(error_type),
            EventType::ExceptionUnhandled { exception_type, .. } => Some(exception_type),
            _ => None,
        }
    }
}

/// A complete telemetry event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    /// Unique event ID
    pub id: String,
    
    /// When the event occurred
    pub timestamp: DateTime<Utc>,
    
    /// Severity level
    pub level: Level,
    
    /// The event data
    #[serde(flatten)]
    pub event: EventType,
    
    /// Context about the environment
    pub context: EventContext,
}

impl Event {
    /// Create a new event with auto-generated ID and timestamp
    pub fn new(level: Level, event: EventType, context: EventContext) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            timestamp: Utc::now(),
            level,
            event,
            context,
        }
    }
    
    /// Returns true if this event should trigger immediate flush
    pub fn is_urgent(&self) -> bool {
        self.level.is_urgent()
    }
}

/// Message format received from clients over the socket (daemon) or HTTP (server)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientMessage {
    /// Log level
    pub level: Level,
    
    /// Event data (flattened in JSON)
    #[serde(flatten)]
    pub event: EventType,
    
    /// Context (client provides this)
    pub context: EventContext,
}

impl ClientMessage {
    /// Convert to a full Event
    pub fn into_event(self) -> Event {
        Event::new(self.level, self.event, self.context)
    }
}

/// Batch of events (used by HTTP API)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventBatch {
    pub events: Vec<ClientMessage>,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_event_serialization() {
        let context = EventContext {
            installation_id: "test-uuid".to_string(),
            tool_version: "0.1.0".to_string(),
            python_version: "3.11.0".to_string(),
            os: "linux".to_string(),
            os_version: Some("Ubuntu 22.04".to_string()),
        };
        
        let event = Event::new(
            Level::Info,
            EventType::ToolInvoked {
                tool: "migrateiq".to_string(),
                command: "scan".to_string(),
            },
            context,
        );
        
        let json = serde_json::to_string_pretty(&event).unwrap();
        println!("{}", json);
        
        // Should be able to deserialize back
        let _: Event = serde_json::from_str(&json).unwrap();
    }
    
    #[test]
    fn test_client_message_deserialization() {
        let json = r#"{
            "level": "INFO",
            "event_type": "tool_invoked",
            "tool": "migrateiq",
            "command": "scan",
            "context": {
                "installation_id": "test-uuid",
                "tool_version": "0.1.0",
                "python_version": "3.11.0",
                "os": "linux"
            }
        }"#;
        
        let msg: ClientMessage = serde_json::from_str(json).unwrap();
        assert_eq!(msg.event.tool(), Some("migrateiq"));
        assert_eq!(msg.event.command(), Some("scan"));
    }
    
    #[test]
    fn test_urgent_levels() {
        assert!(!Level::Debug.is_urgent());
        assert!(!Level::Info.is_urgent());
        assert!(!Level::Warning.is_urgent());
        assert!(!Level::Error.is_urgent());
        assert!(Level::Critical.is_urgent());
        assert!(Level::Fatal.is_urgent());
    }
}
