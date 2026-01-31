use crate::error::ServerError;
use crate::state::ServerState;
use axum::extract::Request;
use axum::http::header::AUTHORIZATION;
use axum::middleware::Next;
use axum::response::Response;
use std::sync::Arc;

/// API Key authentication middleware
pub async fn api_key_auth(
    state: axum::extract::State<Arc<ServerState>>,
    request: Request,
    next: Next,
) -> Result<Response, ServerError> {
    // Extract API key from header
    let api_key = request
        .headers()
        .get("x-api-key")
        .or_else(|| request.headers().get(AUTHORIZATION))
        .and_then(|v| v.to_str().ok())
        .map(|s| {
            // Handle "Bearer <token>" format
            s.strip_prefix("Bearer ").unwrap_or(s).to_string()
        });

    match api_key {
        Some(key) => {
            // Validate API key
            if !state.is_valid_api_key(&key) {
                return Err(ServerError::Authentication("Invalid API key".to_string()));
            }

            // Check rate limit
            if !state.check_rate_limit(&key) {
                return Err(ServerError::RateLimitExceeded);
            }

            // Continue to handler
            Ok(next.run(request).await)
        }
        None => Err(ServerError::Authentication(
            "API key required. Provide it in 'X-API-Key' or 'Authorization: Bearer <key>' header"
                .to_string(),
        )),
    }
}

/// Request ID injection middleware
pub async fn request_id(mut request: Request, next: Next) -> Response {
    // Generate or extract request ID
    let request_id = request
        .headers()
        .get("x-request-id")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

    // Add to request extensions for handlers to access
    request.extensions_mut().insert(request_id.clone());

    // Process request
    let mut response = next.run(request).await;

    // Add request ID to response headers
    response
        .headers_mut()
        .insert("x-request-id", request_id.parse().unwrap());

    response
}

/// Logging middleware
pub async fn log_requests(request: Request, next: Next) -> Response {
    let method = request.method().clone();
    let uri = request.uri().clone();
    let start = std::time::Instant::now();

    // Get request ID if available
    let request_id = request
        .extensions()
        .get::<String>()
        .cloned()
        .unwrap_or_default();

    tracing::info!(
        method = %method,
        uri = %uri,
        request_id = %request_id,
        "Request started"
    );

    let response = next.run(request).await;
    let duration = start.elapsed();
    let status = response.status();

    tracing::info!(
        method = %method,
        uri = %uri,
        status = %status,
        duration_ms = %duration.as_millis(),
        request_id = %request_id,
        "Request completed"
    );

    response
}
