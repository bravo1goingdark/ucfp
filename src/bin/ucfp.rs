//! `ucfp` HTTP server binary.
//!
//! Single-binary deploy. Composes [`ucfp::server`] routes with the
//! production-hygiene middleware stack from ARCHITECTURE §8.3:
//! bearer-token auth (timing-safe), per-request Prometheus metrics,
//! body-size limit, concurrency cap, request timeout, structured trace.
//!
//! ## Environment
//! - `UCFP_BIND` — listen address (default `0.0.0.0:8080`)
//! - `UCFP_DATA_DIR` — directory for the redb file (default `./data`)
//! - `UCFP_TOKEN` — bearer token required on protected routes; the
//!   process refuses to start if unset or empty
//! - `UCFP_BODY_LIMIT_MB` — request body cap (default 16 MiB)
//!
//! ## Auth shape
//! `/healthz`, `/v1/info`, `/metrics` are public. Everything under
//! `/v1/records*`, `/v1/query`, `/v1/ingest/*` requires
//! `Authorization: Bearer $UCFP_TOKEN`. The token compare is constant
//! time via [`subtle::ConstantTimeEq`] to deny a timing oracle.

#![cfg(all(feature = "server", feature = "embedded"))]

use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::{
    Router,
    extract::{MatchedPath, Request, State},
    http::{HeaderMap, StatusCode, header},
    middleware::{self, Next},
    response::Response,
    routing::get,
};
use metrics_exporter_prometheus::{PrometheusBuilder, PrometheusHandle};
use subtle::ConstantTimeEq;
use tower::limit::ConcurrencyLimitLayer;
use tower_http::{
    limit::RequestBodyLimitLayer, timeout::TimeoutLayer, trace::TraceLayer,
};

use ucfp::EmbeddedBackend;
use ucfp::server::{protected_router, public_router};

#[derive(Clone)]
struct AuthState {
    /// Expected bearer-token bytes. Held as `Arc<Vec<u8>>` so each
    /// middleware invocation clones the pointer, not the secret.
    expected: Arc<Vec<u8>>,
}

/// Bearer-token middleware with timing-safe compare. Replaces the
/// deprecated `tower_http::validate_request::ValidateRequestHeaderLayer::bearer`,
/// which deliberately advertises itself as too coarse for production.
async fn require_bearer(
    State(auth): State<AuthState>,
    headers: HeaderMap,
    req: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let presented = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "))
        .ok_or(StatusCode::UNAUTHORIZED)?;
    let presented_b = presented.as_bytes();
    if presented_b.len() != auth.expected.len() {
        return Err(StatusCode::UNAUTHORIZED);
    }
    if !bool::from(presented_b.ct_eq(auth.expected.as_slice())) {
        return Err(StatusCode::UNAUTHORIZED);
    }
    Ok(next.run(req).await)
}

/// Per-request Prometheus metrics. Path label is the matched route
/// template (bounded cardinality, never the raw URI). `/metrics` is
/// excluded so a scrape doesn't bump its own counter.
async fn http_metrics(matched: Option<MatchedPath>, req: Request, next: Next) -> Response {
    let path = matched
        .map(|m| m.as_str().to_string())
        .unwrap_or_else(|| "unknown".into());
    if path == "/metrics" {
        return next.run(req).await;
    }
    let method = req.method().to_string();
    let start = Instant::now();
    let resp = next.run(req).await;
    let elapsed = start.elapsed().as_secs_f64();
    let status = resp.status().as_u16().to_string();
    metrics::counter!(
        "ucfp_http_requests_total",
        "method" => method.clone(),
        "path" => path.clone(),
        "status" => status,
    )
    .increment(1);
    metrics::histogram!(
        "ucfp_http_request_duration_seconds",
        "method" => method,
        "path" => path,
    )
    .record(elapsed);
    resp
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .json()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "ucfp=info,tower_http=info,axum=info".into()),
        )
        .init();

    // Install the Prometheus recorder *before* any `metrics!` macro fires.
    let prom: PrometheusHandle = PrometheusBuilder::new()
        .install_recorder()
        .map_err(|e| format!("install Prometheus recorder: {e}"))?;

    let token = std::env::var("UCFP_TOKEN").map_err(|_| {
        tracing::error!("UCFP_TOKEN env var is required");
        "UCFP_TOKEN env var is required"
    })?;
    if token.is_empty() {
        return Err("UCFP_TOKEN must not be empty".into());
    }

    let data_dir = std::env::var("UCFP_DATA_DIR").unwrap_or_else(|_| "./data".into());
    let db_path = std::path::PathBuf::from(&data_dir).join("ucfp.redb");
    let backend = Arc::new(EmbeddedBackend::open(&db_path)?);
    tracing::info!(path = %db_path.display(), "ucfp database open");

    let body_limit_mb: usize = std::env::var("UCFP_BODY_LIMIT_MB")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(16);

    let auth_state = AuthState {
        expected: Arc::new(token.into_bytes()),
    };
    let protected = protected_router(backend.clone())
        .layer(middleware::from_fn_with_state(auth_state, require_bearer));

    // /metrics: public, returns Prometheus text exposition format.
    // String → 200 with `content-type: text/plain; charset=utf-8`,
    // which is exactly what scrapers expect.
    let metrics_route = Router::new().route(
        "/metrics",
        get({
            let prom = prom.clone();
            move || {
                let prom = prom.clone();
                async move { prom.render() }
            }
        }),
    );

    // Layer order: each `.layer()` wraps the existing service, so the
    // last call is the *outermost*. http_metrics outermost → it sees
    // every final response, including 408/413/503 rejected by inner
    // layers. Body limit and concurrency are innermost so they reject
    // cheaply before the request reaches handlers.
    let app = Router::new()
        .merge(public_router(backend))
        .merge(metrics_route)
        .merge(protected)
        .layer(RequestBodyLimitLayer::new(body_limit_mb * 1024 * 1024))
        .layer(ConcurrencyLimitLayer::new(512))
        .layer(TimeoutLayer::with_status_code(
            StatusCode::REQUEST_TIMEOUT,
            Duration::from_secs(10),
        ))
        .layer(TraceLayer::new_for_http())
        .layer(middleware::from_fn(http_metrics));

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
