//! Event ingestion endpoint.

use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::Serialize;
use tracing::{debug, error, info, warn};

use crate::models::{ClientMessage, EventBatch, StoredEvent};
use crate::nats::NatsPublisher;

/// Shared application state
#[derive(Clone)]
pub struct AppState {
    pub nats: NatsPublisher,
}

#[derive(Serialize)]
pub struct IngestResponse {
    pub accepted: usize,
    pub failed: usize,
}

#[derive(Serialize)]
pub struct ErrorResponse {
    pub error: String,
}

/// POST /events - Ingest a batch of events
pub async fn ingest_batch(
    State(state): State<AppState>,
    Json(batch): Json<EventBatch>,
) -> impl IntoResponse {
    let total = batch.events.len();
    debug!("Received batch of {} events", total);
    
    if total == 0 {
        return (StatusCode::BAD_REQUEST, Json(ErrorResponse {
            error: "Empty event batch".to_string(),
        })).into_response();
    }
    
    // Convert to stored events
    let stored_events: Vec<StoredEvent> = batch.events
        .into_iter()
        .map(StoredEvent::from_client_message)
        .collect();
    
    // Publish to NATS
    match state.nats.publish_batch(&stored_events).await {
        Ok(accepted) => {
            let failed = total - accepted;
            if failed > 0 {
                warn!("Accepted {}/{} events ({} failed)", accepted, total, failed);
            } else {
                info!("Accepted {} events", accepted);
            }
            
            (StatusCode::ACCEPTED, Json(IngestResponse {
                accepted,
                failed,
            })).into_response()
        }
        Err(e) => {
            error!("Failed to publish events: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse {
                error: "Failed to process events".to_string(),
            })).into_response()
        }
    }
}

/// POST /event - Ingest a single event
pub async fn ingest_single(
    State(state): State<AppState>,
    Json(msg): Json<ClientMessage>,
) -> impl IntoResponse {
    debug!("Received single event: {:?}", msg.event.type_name());
    
    let stored_event = StoredEvent::from_client_message(msg);
    
    match state.nats.publish(&stored_event).await {
        Ok(_) => {
            debug!("Accepted event {}", stored_event.id);
            (StatusCode::ACCEPTED, Json(IngestResponse {
                accepted: 1,
                failed: 0,
            })).into_response()
        }
        Err(e) => {
            error!("Failed to publish event: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse {
                error: "Failed to process event".to_string(),
            })).into_response()
        }
    }
}
