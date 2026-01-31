//! API route handlers
//!
//! This module contains all HTTP endpoint implementations for the UCFP server.
//! Routes are organized by functionality:
//!
//! - `health`: Health checks, readiness, and metrics
//! - `process`: Document processing (single and batch)
//! - `index`: Index management (insert, search, delete)
//! - `matching`: Document matching and comparison

pub mod health;
pub mod index;
pub mod matching;
pub mod process;

use crate::error::{ServerError, ServerResult};
use axum::response::IntoResponse;
use axum::Json;
use serde_json::json;

/// API version and base info
///
/// Returns server information including version and available endpoints.
/// This is the root endpoint (GET /) and requires no authentication.
///
/// # Response
///
/// ```json
/// {
///   "name": "UCFP Server",
///   "version": "0.1.0",
///   "api_version": "v1",
///   "endpoints": ["..."]
/// }
/// ```
pub async fn api_info() -> ServerResult<impl IntoResponse> {
    Ok(Json(json!({
        "name": "UCFP Server",
        "version": env!("CARGO_PKG_VERSION"),
        "api_version": "v1",
        "endpoints": [
            "/api/v1/process",
            "/api/v1/batch",
            "/api/v1/index/insert",
            "/api/v1/index/search",
            "/api/v1/match",
            "/health",
            "/ready",
            "/metrics"
        ]
    })))
}

/// 404 Not Found handler
///
/// Returns a standardized error response for undefined routes.
pub async fn not_found() -> ServerError {
    ServerError::NotFound
}
