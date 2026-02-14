use crate::error::ServerResult;
use crate::state::{ServerMetadata, ServerState};
use axum::extract::State;
use axum::response::IntoResponse;
use axum::Json;
use serde_json::json;
use std::sync::Arc;
use std::time::SystemTime;

/// Global server start time for uptime calculation
static SERVER_START_TIME: once_cell::sync::Lazy<SystemTime> =
    once_cell::sync::Lazy::new(SystemTime::now);

/// Health check endpoint (liveness)
/// Returns 200 if server is running
pub async fn health_check() -> impl IntoResponse {
    let uptime = SERVER_START_TIME
        .elapsed()
        .map(|d| d.as_secs())
        .unwrap_or(0);

    Json(json!({
        "status": "healthy",
        "service": "ucfp-server",
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "uptime_seconds": uptime,
    }))
}

/// Readiness check endpoint
/// Returns 200 if server is ready to accept requests
pub async fn readiness_check(
    State(_state): State<Arc<ServerState>>,
) -> ServerResult<impl IntoResponse> {
    // Check index is accessible (index is always ready for in-memory backend)
    let index_status = "ready";

    let uptime = SERVER_START_TIME
        .elapsed()
        .map(|d| d.as_secs())
        .unwrap_or(0);

    Ok(Json(json!({
        "status": "ready",
        "service": "ucfp-server",
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "uptime_seconds": uptime,
        "components": {
            "api": "ready",
            "index": index_status,
        }
    })))
}

/// Prometheus metrics endpoint
pub async fn metrics() -> ServerResult<impl IntoResponse> {
    // For now, return basic metrics
    // In production, integrate with the metrics system
    Ok(Json(json!({
        "uptime_seconds": SERVER_START_TIME
            .elapsed()
            .map(|d| d.as_secs())
            .unwrap_or(0),
    })))
}

/// Server metadata endpoint (authenticated)
pub async fn server_metadata(
    State(_state): State<Arc<ServerState>>,
) -> ServerResult<impl IntoResponse> {
    let uptime = SERVER_START_TIME
        .elapsed()
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let metadata = ServerMetadata {
        version: env!("CARGO_PKG_VERSION").to_string(),
        uptime_seconds: uptime,
    };

    Ok(Json(serde_json::to_value(metadata)?))
}

/// Pipeline status response
#[derive(serde::Serialize)]
pub struct PipelineStatusResponse {
    pub status: String,
    pub components: PipelineComponents,
}

/// Pipeline component status
#[derive(serde::Serialize)]
pub struct PipelineComponents {
    pub ingest: String,
    pub canonical: String,
    pub perceptual: String,
    pub semantic: String,
    pub index: String,
    pub matcher: String,
}

/// Get pipeline/component status
pub async fn pipeline_status(
    State(_state): State<Arc<ServerState>>,
) -> ServerResult<impl IntoResponse> {
    Ok(Json(PipelineStatusResponse {
        status: "ready".to_string(),
        components: PipelineComponents {
            ingest: "ready".to_string(),
            canonical: "ready".to_string(),
            perceptual: "ready".to_string(),
            semantic: "ready".to_string(),
            index: "ready".to_string(),
            matcher: "ready".to_string(),
        },
    }))
}
