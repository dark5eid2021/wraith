//! Health check endpoints.

use axum::{Json, response::IntoResponse};
use serde::Serialize;

#[derive(Serialize)]
pub struct HealthResponse {
    pub status: &'static str,
    pub version: &'static str,
}

/// GET /health - Basic health check
pub async fn health() -> impl IntoResponse {
    Json(HealthResponse {
        status: "ok",
        version: env!("CARGO_PKG_VERSION"),
    })
}

/// GET /ready - Readiness check (can add dependency checks)
pub async fn ready() -> impl IntoResponse {
    // Could add NATS/ClickHouse connectivity checks here
    Json(HealthResponse {
        status: "ready",
        version: env!("CARGO_PKG_VERSION"),
    })
}
