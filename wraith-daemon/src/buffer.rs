//! Event buffer with flush logic for Wraith.
//!
//! Handles batching events and deciding when to flush:
//! - Every 30-60 seconds
//! - When buffer reaches 25 events
//! - Immediately on CRITICAL or FATAL

use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
use tokio::time::interval;
use tracing::{debug, info, warn};

use wraith_common::Event;

use crate::config;
use crate::writer::EventWriter;

/// Commands that can be sent to the buffer manager
#[derive(Debug)]
pub enum BufferCommand {
    /// Add an event to the buffer
    Push(Event),
    
    /// Force flush all events
    Flush,
    
    /// Shutdown the buffer manager
    Shutdown,
}

/// Manages the event buffer and flush logic
pub struct EventBuffer {
    /// The actual buffer of events
    events: Vec<Event>,
    
    /// Maximum events before forced flush
    max_events: usize,
    
    /// Writer for persisting events
    writer: Arc<Mutex<dyn EventWriter + Send>>,
}

impl EventBuffer {
    /// Create a new event buffer
    pub fn new(writer: Arc<Mutex<dyn EventWriter + Send>>) -> Self {
        Self {
            events: Vec::with_capacity(config::BUFFER_MAX_EVENTS),
            max_events: config::BUFFER_MAX_EVENTS,
            writer,
        }
    }
    
    /// Add an event to the buffer
    /// Returns true if the event triggers an immediate flush
    pub fn push(&mut self, event: Event) -> bool {
        let urgent = event.is_urgent();
        self.events.push(event);
        
        urgent || self.events.len() >= self.max_events
    }
    
    /// Flush all events to the writer
    pub async fn flush(&mut self) {
        if self.events.is_empty() {
            debug!("Flush called but buffer is empty");
            return;
        }
        
        let event_count = self.events.len();
        debug!("Flushing {} events", event_count);
        
        // Take ownership of events
        let events = std::mem::take(&mut self.events);
        
        // Write to backend
        let mut writer = self.writer.lock().await;
        if let Err(e) = writer.write_events(&events).await {
            warn!("Failed to write events: {}", e);
            // Put events back in buffer to retry later
            self.events = events;
        } else {
            info!("Successfully flushed {} events", event_count);
        }
    }
    
    /// Get current buffer size
    pub fn len(&self) -> usize {
        self.events.len()
    }
    
    /// Check if buffer is empty
    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }
}

/// Runs the buffer manager loop
pub async fn run_buffer_manager(
    mut cmd_rx: mpsc::Receiver<BufferCommand>,
    writer: Arc<Mutex<dyn EventWriter + Send>>,
) {
    let mut buffer = EventBuffer::new(writer);
    let mut flush_interval = interval(config::get_flush_interval());
    
    // Skip the first immediate tick
    flush_interval.tick().await;
    
    loop {
        tokio::select! {
            // Handle commands
            Some(cmd) = cmd_rx.recv() => {
                match cmd {
                    BufferCommand::Push(event) => {
                        let should_flush = buffer.push(event);
                        if should_flush {
                            debug!("Immediate flush triggered");
                            buffer.flush().await;
                            // Reset the interval after an urgent flush
                            flush_interval.reset();
                        }
                    }
                    BufferCommand::Flush => {
                        buffer.flush().await;
                        flush_interval.reset();
                    }
                    BufferCommand::Shutdown => {
                        info!("Buffer manager shutting down, final flush");
                        buffer.flush().await;
                        break;
                    }
                }
            }
            
            // Periodic flush
            _ = flush_interval.tick() => {
                if !buffer.is_empty() {
                    debug!("Periodic flush triggered");
                    buffer.flush().await;
                }
            }
        }
    }
    
    info!("Buffer manager stopped");
}
