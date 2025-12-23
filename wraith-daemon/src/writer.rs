//! Event writer trait and implementations.
//!
//! v1 uses a file-based stub backend. Future versions will POST to an HTTP endpoint.

use async_trait::async_trait;
use std::path::PathBuf;
use tokio::fs::OpenOptions;
use tokio::io::AsyncWriteExt;
use tracing::{debug, info};

use wraith_common::Event;

/// Trait for writing events to a backend
#[async_trait]
pub trait EventWriter: Send + Sync {
    /// Write a batch of events
    async fn write_events(&mut self, events: &[Event]) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
}

/// File-based event writer (stub backend for v1)
pub struct FileWriter {
    path: PathBuf,
}

impl FileWriter {
    /// Create a new file writer
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }
    
    /// Ensure the parent directory exists
    pub async fn ensure_dir(&self) -> Result<(), std::io::Error> {
        if let Some(parent) = self.path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        Ok(())
    }
}

#[async_trait]
impl EventWriter for FileWriter {
    async fn write_events(&mut self, events: &[Event]) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.ensure_dir().await?;
        
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .await?;
        
        for event in events {
            let json = serde_json::to_string(event)?;
            file.write_all(json.as_bytes()).await?;
            file.write_all(b"\n").await?;
        }
        
        file.flush().await?;
        
        debug!("Wrote {} events to {}", events.len(), self.path.display());
        Ok(())
    }
}

/// HTTP-based event writer (for wraith-server)
#[cfg(feature = "http-backend")]
pub struct HttpWriter {
    endpoint: String,
    client: reqwest::Client,
}

#[cfg(feature = "http-backend")]
impl HttpWriter {
    /// Create a new HTTP writer
    pub fn new(endpoint: String) -> Self {
        Self {
            endpoint,
            client: reqwest::Client::new(),
        }
    }
}

#[cfg(feature = "http-backend")]
#[async_trait]
impl EventWriter for HttpWriter {
    async fn write_events(&mut self, events: &[Event]) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let response = self.client
            .post(&self.endpoint)
            .json(&serde_json::json!({ "events": events }))
            .send()
            .await?;
        
        if response.status().is_success() {
            info!("Sent {} events to {}", events.len(), self.endpoint);
            Ok(())
        } else {
            Err(format!("HTTP error: {}", response.status()).into())
        }
    }
}

// Stub for when HTTP backend is disabled
#[cfg(not(feature = "http-backend"))]
pub struct HttpWriter {
    endpoint: String,
}

#[cfg(not(feature = "http-backend"))]
impl HttpWriter {
    pub fn new(endpoint: String) -> Self {
        Self { endpoint }
    }
}

#[cfg(not(feature = "http-backend"))]
#[async_trait]
impl EventWriter for HttpWriter {
    async fn write_events(&mut self, events: &[Event]) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        info!("HttpWriter (stub): Would send {} events to {}", events.len(), self.endpoint);
        Ok(())
    }
}

/// Async trait needs this
#[async_trait]
pub trait AsyncTrait {}
