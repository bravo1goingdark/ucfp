//! `ucfp` HTTP server binary.
//!
//! Single-binary deploy. Composes [`ucfp::server::router`] with the
//! production-hygiene middleware stack from ARCHITECTURE §8.3.
//!
//! ## Environment
//! - `UCFP_BIND`     — listen address (default `0.0.0.0:8080`)
//! - `UCFP_DATA_DIR` — directory for the redb file (default `./data`)
//! - `UCFP_TOKEN`    — required bearer token; refuse to start if unset
//! - `UCFP_BODY_LIMIT_MB` — request body cap (default 16 MiB)

#![cfg(all(feature = "server", feature = "embedded"))]

use std::sync::Arc;
use std::time::Duration;

use axum::http::StatusCode;
use tower::limit::ConcurrencyLimitLayer;
use tower_http::{
    limit::RequestBodyLimitLayer, timeout::TimeoutLayer, trace::TraceLayer,
    validate_request::ValidateRequestHeaderLayer,
};

use ucfp::EmbeddedBackend;
use ucfp::server::router;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .json()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "ucfp=info,tower_http=info,axum=info".into()),
        )
        .init();

    let token = std::env::var("UCFP_TOKEN").map_err(|_| {
        tracing::error!("UCFP_TOKEN env var is required");
        "UCFP_TOKEN env var is required"
    })?;

    let data_dir = std::env::var("UCFP_DATA_DIR").unwrap_or_else(|_| "./data".into());
    let db_path = std::path::PathBuf::from(&data_dir).join("ucfp.redb");
    let backend = Arc::new(EmbeddedBackend::open(&db_path)?);
    tracing::info!(path = %db_path.display(), "ucfp database open");

    let body_limit_mb: usize = std::env::var("UCFP_BODY_LIMIT_MB")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(16);

    // tower-http 0.6 deprecates `::bearer` as too basic; for static-token
    // auth on a single-tenant deploy it's still the right shape. Replace
    // with a custom axum middleware once multi-tenant JWT lands (§6).
    #[allow(deprecated)]
    let auth_layer = ValidateRequestHeaderLayer::bearer(&token);

    let app = router(backend)
        .layer(auth_layer)
        .layer(RequestBodyLimitLayer::new(body_limit_mb * 1024 * 1024))
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
