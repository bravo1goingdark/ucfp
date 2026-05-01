//! `ucfp` HTTP server binary.
//!
//! Single-binary deploy. Composes [`ucfp::server`] routes with the
//! production-hygiene middleware stack from ARCHITECTURE §8.3:
//! pluggable bearer-token auth, per-tenant rate limit, usage event sink,
//! per-request Prometheus metrics, body-size limit, concurrency cap,
//! request timeout, structured trace.
//!
//! ## Environment
//!
//! Auth source — at least ONE must be set (the binary refuses to start
//! otherwise):
//!
//! | Var | Effect |
//! |---|---|
//! | `UCFP_TOKEN` | `StaticSingleKey` (legacy single-token, `tenant_id=0`) |
//! | `UCFP_KEYS_FILE=/path/keys.toml` | `StaticMapKey` from a TOML file |
//! | `UCFP_KEY_LOOKUP_URL` | `WebhookKeyLookup` (requires `multi-tenant` feature) |
//!
//! Rate limit:
//!
//! | Var | Effect |
//! |---|---|
//! | `UCFP_RATELIMIT_URL` set | `WebhookRateLimiter` (requires `multi-tenant`) |
//! | `UCFP_RATELIMIT_URL` unset | `InMemoryTokenBucket::with_limits(100, 200)` |
//!
//! Usage sink:
//!
//! | Var | Effect |
//! |---|---|
//! | `UCFP_USAGE_WEBHOOK_URL` set | `WebhookUsageSink::spawn` (requires `multi-tenant`) |
//! | `UCFP_USAGE_LOG_PATH` set | `LogUsageSink::open` (NDJSON file) |
//! | neither set | `NoopUsageSink` |
//!
//! Other:
//! - `UCFP_BIND` — listen address (default `0.0.0.0:8080`)
//! - `UCFP_DATA_DIR` — directory for the redb file (default `./data`)
//! - `UCFP_BODY_LIMIT_MB` — request body cap (default 16 MiB)
//!
//! ## Auth shape
//! `/healthz`, `/v1/info`, `/metrics` are public. Everything under
//! `/v1/records*`, `/v1/query`, `/v1/ingest/*` requires
//! `Authorization: Bearer <token>` and is wired through the auth +
//! rate-limit + usage middleware in [`ucfp::server::router_with_state`].

#![cfg(all(feature = "server", feature = "embedded"))]

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::{
    Router,
    extract::{MatchedPath, Request},
    http::StatusCode,
    middleware::{self, Next},
    response::Response,
    routing::get,
};
use metrics_exporter_prometheus::{PrometheusBuilder, PrometheusHandle};
use tower::limit::ConcurrencyLimitLayer;
use tower_http::{limit::RequestBodyLimitLayer, timeout::TimeoutLayer, trace::TraceLayer};

use ucfp::EmbeddedBackend;
use ucfp::server::{
    ApiKeyLookup, InMemoryTokenBucket, LogUsageSink, NoopUsageSink, ServerState, StaticMapKey,
    StaticSingleKey, TenantRateLimiter, UsageSink, router_with_state,
};

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

/// Resolve the configured [`ApiKeyLookup`] from env vars. Returns the
/// trait-object Arc directly; the bin never names the concrete type
/// after this point.
fn resolve_api_keys() -> Result<Arc<dyn ApiKeyLookup>, Box<dyn std::error::Error>> {
    if let Ok(url) = std::env::var("UCFP_KEY_LOOKUP_URL") {
        #[cfg(feature = "multi-tenant")]
        {
            let parsed =
                reqwest::Url::parse(&url).map_err(|e| format!("UCFP_KEY_LOOKUP_URL parse: {e}"))?;
            let client = reqwest::Client::builder()
                .timeout(Duration::from_secs(5))
                .build()
                .map_err(|e| format!("reqwest client build: {e}"))?;
            return Ok(Arc::new(ucfp::server::WebhookKeyLookup::new(
                client, parsed,
            )));
        }
        #[cfg(not(feature = "multi-tenant"))]
        {
            let _ = url;
            return Err(
                "UCFP_KEY_LOOKUP_URL set but binary built without `multi-tenant` feature".into(),
            );
        }
    }

    if let Ok(path) = std::env::var("UCFP_KEYS_FILE") {
        let body = std::fs::read_to_string(&path)
            .map_err(|e| format!("read UCFP_KEYS_FILE={path}: {e}"))?;
        let map = StaticMapKey::from_toml(&body)
            .map_err(|e| format!("parse UCFP_KEYS_FILE={path}: {e}"))?;
        return Ok(Arc::new(map));
    }

    if let Ok(token) = std::env::var("UCFP_TOKEN") {
        if token.is_empty() {
            return Err("UCFP_TOKEN must not be empty".into());
        }
        return Ok(Arc::new(StaticSingleKey {
            expected: token.into_bytes(),
            tenant_id: 0,
        }));
    }

    Err("none of UCFP_TOKEN, UCFP_KEYS_FILE, UCFP_KEY_LOOKUP_URL is set; refusing to start".into())
}

/// Resolve the configured [`TenantRateLimiter`] from env vars.
fn resolve_rate_limit() -> Result<Arc<dyn TenantRateLimiter>, Box<dyn std::error::Error>> {
    if let Ok(url) = std::env::var("UCFP_RATELIMIT_URL") {
        #[cfg(feature = "multi-tenant")]
        {
            let parsed =
                reqwest::Url::parse(&url).map_err(|e| format!("UCFP_RATELIMIT_URL parse: {e}"))?;
            let client = reqwest::Client::builder()
                .timeout(Duration::from_secs(2))
                .build()
                .map_err(|e| format!("reqwest client build: {e}"))?;
            return Ok(Arc::new(ucfp::server::WebhookRateLimiter::new(
                client, parsed,
            )));
        }
        #[cfg(not(feature = "multi-tenant"))]
        {
            let _ = url;
            return Err(
                "UCFP_RATELIMIT_URL set but binary built without `multi-tenant` feature".into(),
            );
        }
    }
    Ok(Arc::new(InMemoryTokenBucket::with_limits(100, 200)))
}

/// Resolve the configured [`UsageSink`] from env vars.
fn resolve_usage() -> Result<Arc<dyn UsageSink>, Box<dyn std::error::Error>> {
    if let Ok(url) = std::env::var("UCFP_USAGE_WEBHOOK_URL") {
        #[cfg(feature = "multi-tenant")]
        {
            let parsed = reqwest::Url::parse(&url)
                .map_err(|e| format!("UCFP_USAGE_WEBHOOK_URL parse: {e}"))?;
            let client = reqwest::Client::builder()
                .timeout(Duration::from_secs(5))
                .build()
                .map_err(|e| format!("reqwest client build: {e}"))?;
            return Ok(Arc::new(ucfp::server::WebhookUsageSink::spawn(
                client, parsed,
            )));
        }
        #[cfg(not(feature = "multi-tenant"))]
        {
            let _ = url;
            return Err(
                "UCFP_USAGE_WEBHOOK_URL set but binary built without `multi-tenant` feature".into(),
            );
        }
    }
    if let Ok(path) = std::env::var("UCFP_USAGE_LOG_PATH") {
        let sink = LogUsageSink::open(std::path::Path::new(&path))
            .map_err(|e| format!("open UCFP_USAGE_LOG_PATH={path}: {e}"))?;
        return Ok(Arc::new(sink));
    }
    Ok(Arc::new(NoopUsageSink))
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

    let api_keys = resolve_api_keys()?;
    let rate_limit = resolve_rate_limit()?;
    let usage = resolve_usage()?;

    let data_dir = std::env::var("UCFP_DATA_DIR").unwrap_or_else(|_| "./data".into());
    let db_path = std::path::PathBuf::from(&data_dir).join("ucfp.redb");
    let backend = Arc::new(EmbeddedBackend::open(&db_path)?);
    tracing::info!(path = %db_path.display(), "ucfp database open");

    let body_limit_mb: usize = std::env::var("UCFP_BODY_LIMIT_MB")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(16);

    let state = ServerState {
        index: backend.clone(),
        api_keys,
        rate_limit,
        usage,
    };

    // /metrics: public, returns Prometheus text exposition format.
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

    // router_with_state already merges public + protected (with auth +
    // rate-limit + usage middleware on the protected half). Merge the
    // metrics route on top and apply outer hygiene layers.
    //
    // Layer order: each `.layer()` wraps the existing service, so the
    // last call is the *outermost*. http_metrics outermost → it sees
    // every final response, including 408/413/503 rejected by inner
    // layers. Body limit and concurrency are innermost so they reject
    // cheaply before the request reaches handlers.
    let app = router_with_state(state)
        .merge(metrics_route)
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
