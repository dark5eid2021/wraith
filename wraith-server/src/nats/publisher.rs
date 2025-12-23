//! NATS message publishing.

use async_nats::Client;
use tracing::{debug, error, info};

use crate::models::StoredEvent;

/// NATS publisher for events
#[derive(Clone)]
pub struct NatsPublisher {
    client: Client,
    subject: String,
}

impl NatsPublisher {
    /// Connect to NATS and create a publisher
    pub async fn connect(url: &str, subject: String) -> Result<Self, async_nats::Error> {
        info!("Connecting to NATS at {}", url);
        let client = async_nats::connect(url).await?;
        info!("Connected to NATS");
        
        Ok(Self { client, subject })
    }
    
    /// Publish an event to NATS
    pub async fn publish(&self, event: &StoredEvent) -> Result<(), async_nats::Error> {
        let payload = serde_json::to_vec(event)
            .map_err(|e| async_nats::Error::from(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                e,
            )))?;
        
        self.client.publish(self.subject.clone(), payload.into()).await?;
        debug!("Published event {} to NATS", event.id);
        
        Ok(())
    }
    
    /// Publish multiple events to NATS
    pub async fn publish_batch(&self, events: &[StoredEvent]) -> Result<usize, async_nats::Error> {
        let mut published = 0;
        
        for event in events {
            match self.publish(event).await {
                Ok(_) => published += 1,
                Err(e) => {
                    error!("Failed to publish event {}: {}", event.id, e);
                }
            }
        }
        
        debug!("Published {}/{} events to NATS", published, events.len());
        Ok(published)
    }
}
