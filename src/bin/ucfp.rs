//! `ucfp` HTTP server binary.
//!
//! Single-binary deploy — see ARCHITECTURE §1 + §8.3 for the production-
//! hygiene middleware stack. Stub today; real routes land alongside the
//! `EmbeddedBackend` impl.

#![cfg(all(feature = "server", feature = "embedded"))]

use std::time::Duration;

use axum::{Router, http::StatusCode, routing::get};
use tower::limit::ConcurrencyLimitLayer;
use tower_http::{limit::RequestBodyLimitLayer, timeout::TimeoutLayer, trace::TraceLayer};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt().json().init();

    let app = Router::new()
        .route("/healthz", get(|| async { "ok" }))
        .layer(RequestBodyLimitLayer::new(16 * 1024 * 1024)) // 16 MiB
        .layer(ConcurrencyLimitLayer::new(512))
        .layer(TimeoutLayer::with_status_code(
            StatusCode::REQUEST_TIMEOUT,
            Duration::from_secs(10),
        ))
        .layer(TraceLayer::new_for_http());

    let bind = std::env::var("UCFP_BIND").unwrap_or_else(|_| "0.0.0.0:8080".into());
    let listener = tokio::net::TcpListener::bind(&bind).await?;
    tracing::info!(%bind, "ucfp listening");

    axum::serve(listener, app)
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
            tracing::info!("ucfp shutting down");
        })
        .await?;

    Ok(())
}
