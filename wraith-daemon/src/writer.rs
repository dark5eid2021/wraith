//! Event writer implementations.
//!
//! HTTP backend sends events to wraith-server.
//! File backend is a fallback for offline/debugging.

use std::path::PathBuf;
use tokio::fs::OpenOptions;
use tokio::io::AsyncWriteExt;
use tracing::{debug, info, warn, error};

use wraith_common::Event;

/// Trait for writing events to a backend
pub trait EventWriter: Send + Sync {
    /// Write a batch of events
    fn write_events(&mut self, events: &[Event]) -> impl std::future::Future<Output = Result<(), Box<dyn std::error::Error + Send + Sync>>> + Send;
}

/// File-based event writer (fallback backend)
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
pub struct HttpWriter {
    endpoint: String,
    client: reqwest::Client,
}

impl HttpWriter {
    /// Create a new HTTP writer
    pub fn new(endpoint: String) -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .expect("Failed to create HTTP client");
        
        Self { endpoint, client }
    }
}

impl EventWriter for HttpWriter {
    async fn write_events(&mut self, events: &[Event]) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if events.is_empty() {
            return Ok(());
        }

        let response = self.client
            .post(&self.endpoint)
            .json(&serde_json::json!({ "events": events }))
            .send()
            .await;
        
        match response {
            Ok(resp) => {
                if resp.status().is_success() {
                    info!("Sent {} events to {}", events.len(), self.endpoint);
                    Ok(())
                } else {
                    let status = resp.status();
                    let body = resp.text().await.unwrap_or_default();
                    warn!("Server returned {}: {}", status, body);
                    Err(format!("HTTP error: {} - {}", status, body).into())
                }
            }
            Err(e) => {
                error!("Failed to send events: {}", e);
                Err(Box::new(e))
            }
        }
    }
}

/// Combined writer that tries HTTP first, falls back to file
pub struct FallbackWriter {
    http: Option<HttpWriter>,
    file: FileWriter,
}

impl FallbackWriter {
    /// Create a new fallback writer
    pub fn new(endpoint: Option<String>, file_path: PathBuf) -> Self {
        Self {
            http: endpoint.map(HttpWriter::new),
            file: FileWriter::new(file_path),
        }
    }
}

impl EventWriter for FallbackWriter {
    async fn write_events(&mut self, events: &[Event]) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if let Some(ref mut http) = self.http {
            match http.write_events(events).await {
                Ok(()) => return Ok(()),
                Err(e) => {
                    warn!("HTTP backend failed ({}), falling back to file", e);
                }
            }
        }
        
        // Fall back to file
        self.file.write_events(events).await
    }
}
