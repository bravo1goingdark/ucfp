//! Server initialization and routing
//!
//! This module handles the Axum server setup including:
//! - Router configuration with all API endpoints
//! - Middleware stack (auth, logging, compression, etc.)
//! - Graceful shutdown handling
//! - Error handling middleware

use crate::config::ServerConfig;
use crate::middleware::{api_key_auth, log_requests, request_id};
use crate::routes::{api_info, not_found};
use crate::routes::{health, index, matching, process};
use crate::state::ServerState;
use axum::extract::DefaultBodyLimit;
use axum::http::StatusCode;
use axum::middleware::{from_fn, from_fn_with_state};
use axum::routing::{delete, get, post};
use axum::Router;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tower_http::compression::CompressionLayer;
use tower_http::cors::{Any, CorsLayer};
use tower_http::timeout::TimeoutLayer;
use tower_http::trace::TraceLayer;

/// Build the Axum router with all routes and middleware
///
/// Routes are divided into:
/// - Public routes: /, /health, /ready, /metrics (no auth required)
/// - Protected routes: All /api/v1/* endpoints (API key required)
///
/// Middleware stack (applied in reverse order):
/// 1. Request ID tracking
/// 2. Request logging
/// 3. Timeout handling
/// 4. Compression
/// 5. CORS
/// 6. Error handling
/// 7. API key authentication (protected routes only)
fn build_router(state: Arc<ServerState>) -> Router {
    // CORS layer
    let cors = if state.config.enable_cors {
        CorsLayer::new()
            .allow_origin(Any)
            .allow_methods(Any)
            .allow_headers(Any)
    } else {
        CorsLayer::new()
    };

    // Public routes (no auth required)
    let public_routes = Router::new()
        .route("/", get(api_info))
        .route("/health", get(health::health_check))
        .route("/ready", get(health::readiness_check))
        .route("/metrics", get(health::metrics));

    // Protected routes (require API key)
    let protected_routes = Router::new()
        // Processing
        .route("/api/v1/process", post(process::process_document))
        .route("/api/v1/batch", post(process::process_batch))
        // Index
        .route("/api/v1/index/insert", post(index::insert_record))
        .layer(DefaultBodyLimit::max(100 * 1024 * 1024))
        .route("/api/v1/index/search", get(index::search_index))
        .route("/api/v1/index/stats", get(index::index_stats))
        .route("/api/v1/index/documents", get(index::list_documents))
        .route("/api/v1/index/documents/{doc_id}", get(index::get_document))
        .route(
            "/api/v1/index/documents/{doc_id}",
            delete(index::delete_document),
        )
        // Matching
        .route("/api/v1/match", post(matching::match_documents))
        .route("/api/v1/compare", post(matching::compare_documents))
        // Pipeline
        .route("/api/v1/pipeline/status", get(health::pipeline_status))
        // Metadata
        .route("/api/v1/metadata", get(health::server_metadata))
        // Add auth middleware
        .layer(from_fn_with_state(state.clone(), api_key_auth));

    // Combine routes
    Router::new()
        .merge(public_routes)
        .merge(protected_routes)
        .fallback(not_found)
        // Global middleware - simplified to avoid type issues
        .layer(TimeoutLayer::with_status_code(
            StatusCode::REQUEST_TIMEOUT,
            Duration::from_secs(state.config.timeout_secs),
        ))
        .layer(CompressionLayer::new())
        .layer(cors)
        .layer(from_fn(request_id))
        .layer(from_fn(log_requests))
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

/// Start the UCFP HTTP server
///
/// Initializes the server with the provided configuration and starts listening
/// for incoming HTTP requests. This function will block until the server is
/// shut down via SIGTERM or Ctrl+C.
///
/// # Arguments
///
/// * `config` - Server configuration including bind address, port, timeouts, etc.
///
/// # Returns
///
/// Returns `Ok(())` on successful shutdown, or an error if the server fails
/// to start.
///
/// # Example
///
/// ```rust,no_run
/// use server::ServerConfig;
///
/// #[tokio::main]
/// async fn main() -> anyhow::Result<()> {
///     let config = ServerConfig::load()?;
///     server::start_server(config).await?;
///     Ok(())
/// }
/// ```
///
/// # Initialization
///
/// This function performs the following initialization steps:
/// 1. Sets up structured JSON logging with the configured log level
/// 2. Creates shared server state (index, matcher, rate limiter)
/// 3. Builds the Axum router with all routes and middleware
/// 4. Binds to the configured TCP address
/// 5. Starts the HTTP server with graceful shutdown support
///
/// # Shutdown
///
/// The server handles graceful shutdown on:
/// - SIGTERM (Unix/Linux)
/// - Ctrl+C (all platforms)
pub async fn start_server(config: ServerConfig) -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(&config.log_level)
        .with_target(false)
        .with_thread_ids(true)
        .with_thread_names(true)
        .json()
        .init();

    // Create server state
    let state = Arc::new(ServerState::new(config.clone())?);

    // Build router
    let app = build_router(state);

    // Parse bind address
    let addr: SocketAddr = config.socket_addr()?;

    tracing::info!(
        "Starting UCFP server on {} with {} API keys",
        addr,
        config.api_keys.len()
    );
    tracing::info!(
        "Timeout: {}s, Max body: {}MB",
        config.timeout_secs,
        config.max_body_size_mb
    );
    tracing::info!(
        "Rate limit: {} requests/minute",
        config.rate_limit_per_minute
    );
    tracing::info!(
        "CORS: {}, Metrics: {}",
        config.enable_cors,
        config.metrics_enabled
    );

    // Start server
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    tracing::info!("Server shutdown complete");
    Ok(())
}

/// Shutdown signal handler
async fn shutdown_signal() {
    use tokio::signal;

    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("Failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("Failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => tracing::info!("Received Ctrl+C, shutting down..."),
        _ = terminate => tracing::info!("Received SIGTERM, shutting down..."),
    }
}
