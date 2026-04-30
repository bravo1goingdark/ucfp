//! HTTP routes — `/healthz`, `/v1/info`, `/v1/records`, `/v1/query`,
//! and feature-gated modality ingest paths.
//!
//! Generic over [`crate::IndexBackend`] so the same router serves the
//! embedded backend today and a managed graduation backend later. The
//! bin entry in `bin/ucfp.rs` instantiates with `Arc<EmbeddedBackend>`.
//!
//! All handlers funnel through `error::ApiError` for a consistent
//! error envelope; HTTP status codes map per [`crate::Error`] variant.
//!
//! ## Auth shape
//!
//! Routes split into two halves so bin/ucfp.rs can layer bearer-token
//! auth on the protected ones without a path-string allowlist:
//!
//! - [`public_router`] — `/healthz`, `/v1/info` (probe + version)
//! - [`protected_router`] — everything else (records + query + ingest)
//!
//! [`router`] returns the merged form (no auth) for tests and library
//! consumers that handle auth elsewhere.
//!
//! [`router_with_state`] is the new R3 constructor: it wires the
//! [`ApiKeyContext`] extractor (via the [`ApiKeyLookup`] in
//! [`ServerState`]), runs every protected request through the
//! [`TenantRateLimiter`], and emits a [`UsageEvent`] to the configured
//! [`UsageSink`] after each call. Self-hosters that don't need
//! per-tenant accounting can keep using [`router`].

#![cfg(feature = "server")]

mod apikey;
mod dto;
mod error;
mod extractors;
mod handlers;
mod ratelimit;
mod usage;

#[cfg(all(test, feature = "embedded"))]
mod tests;

use std::sync::Arc;
use std::time::Instant;

use axum::{
    Router,
    extract::FromRef,
    http::StatusCode,
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
};

use crate::index::IndexBackend;

pub use apikey::{ApiKeyContext, ApiKeyLookup, StaticMapKey, StaticSingleKey};
#[cfg(feature = "text")]
pub use dto::{CanonicalizerDto, WeightingDto};
pub use dto::{FingerprintDescription, WatermarkReport};
#[cfg(feature = "image")]
pub use dto::{MultiHashConfigDto, PreprocessConfigDto};
pub use ratelimit::{InMemoryTokenBucket, NoopRateLimiter, RateDecision, TenantRateLimiter};
pub use usage::{LogUsageSink, NoopUsageSink, UsageEvent, UsageOp, UsageSink};

#[cfg(feature = "multi-tenant")]
pub use apikey::WebhookKeyLookup;
#[cfg(feature = "multi-tenant")]
pub use ratelimit::WebhookRateLimiter;
#[cfg(feature = "multi-tenant")]
pub use usage::WebhookUsageSink;

/// Routes that are safe to expose without authentication — k8s probes
/// and version discovery. Composed under `with_state` so the healthz
/// handler can ping the backing index.
pub fn public_router<I>(index: Arc<I>) -> Router
where
    I: IndexBackend + 'static,
{
    Router::new()
        .route("/healthz", get(handlers::healthz::<I>))
        .route("/v1/info", get(handlers::info))
        .with_state(index)
}

/// Routes that read or mutate tenant data. Bin layers bearer-token auth
/// on this router before merging with [`public_router`].
pub fn protected_router<I>(index: Arc<I>) -> Router
where
    I: IndexBackend + 'static,
{
    protected_routes::<I, Arc<I>>().with_state(index)
}

/// Build the protected route table generic over the surrounding state
/// `S`. Both [`protected_router`] (state = `Arc<I>`) and
/// [`router_with_state`] (state = [`ServerState<I>`]) consume this and
/// then apply their state via `with_state` — axum's [`FromRef`]
/// machinery threads `Arc<I>` through to handler `State` extractors
/// regardless of which surrounding state was chosen.
fn protected_routes<I, S>() -> Router<S>
where
    I: IndexBackend + 'static,
    S: Clone + Send + Sync + 'static,
    Arc<I>: FromRef<S>,
{
    let r: Router<S> = Router::new()
        .route("/v1/records", post(handlers::upsert::<I>))
        .route(
            "/v1/records/{tenant_id}/{record_id}",
            // axum 0.8 panics if the same path is registered twice;
            // chain GET + DELETE on a single `.route()` call.
            get(handlers::describe_record::<I>).delete(handlers::delete_record::<I>),
        )
        .route("/v1/query", post(handlers::query::<I>));

    #[cfg(feature = "image")]
    let r = r.route(
        "/v1/ingest/image/{tenant_id}/{record_id}",
        post(handlers::ingest_image::<I>),
    );

    #[cfg(feature = "image-semantic")]
    let r = r.route(
        "/v1/ingest/image/{tenant_id}/{record_id}/semantic",
        post(handlers::ingest_image_semantic::<I>),
    );

    #[cfg(feature = "text")]
    let r = r.route(
        "/v1/ingest/text/{tenant_id}/{record_id}",
        post(handlers::ingest_text::<I>),
    );

    #[cfg(feature = "text-streaming")]
    let r = r.route(
        "/v1/ingest/text/{tenant_id}/{record_id}/stream",
        post(handlers::ingest_text_stream::<I>),
    );

    #[cfg(any(feature = "text-markup", feature = "text-pdf"))]
    let r = r.route(
        "/v1/ingest/text/{tenant_id}/{record_id}/preprocess/{kind}",
        post(handlers::ingest_text_preprocess::<I>),
    );

    #[cfg(feature = "audio")]
    let r = r.route(
        "/v1/ingest/audio/{tenant_id}/{record_id}",
        post(handlers::ingest_audio::<I>),
    );

    #[cfg(feature = "audio-watermark")]
    let r = r.route(
        "/v1/ingest/audio/{tenant_id}/{record_id}/watermark",
        post(handlers::ingest_audio_watermark::<I>),
    );

    #[cfg(all(feature = "audio-streaming", feature = "multipart"))]
    let r = r.route(
        "/v1/ingest/audio/{tenant_id}/{record_id}/stream",
        post(handlers::ingest_audio_stream::<I>),
    );

    r
}

/// Merged router — public + protected, no auth applied. Convenient for
/// tests and library consumers that wire their own auth.
pub fn router<I>(index: Arc<I>) -> Router
where
    I: IndexBackend + 'static,
{
    public_router(index.clone()).merge(protected_router(index))
}

// ── ServerState + router_with_state ────────────────────────────────────

/// Composite application state shared by every handler in
/// [`router_with_state`].
///
/// `Clone` is `O(1)` — every component is an [`Arc`]-wrapped trait
/// object. Implements [`FromRef`] for each component so axum extractors
/// (notably the [`ApiKeyContext`] extractor) can pull what they need
/// without seeing the rest.
pub struct ServerState<I: IndexBackend + 'static> {
    /// Backing storage + ANN.
    pub index: Arc<I>,
    /// Resolves bearer tokens into [`ApiKeyContext`].
    pub api_keys: Arc<dyn ApiKeyLookup>,
    /// Per-tenant rate limit / quota check.
    pub rate_limit: Arc<dyn TenantRateLimiter>,
    /// Where successful requests' [`UsageEvent`]s are recorded.
    pub usage: Arc<dyn UsageSink>,
}

impl<I: IndexBackend + 'static> Clone for ServerState<I> {
    fn clone(&self) -> Self {
        Self {
            index: self.index.clone(),
            api_keys: self.api_keys.clone(),
            rate_limit: self.rate_limit.clone(),
            usage: self.usage.clone(),
        }
    }
}

impl<I: IndexBackend + 'static> FromRef<ServerState<I>> for Arc<I> {
    fn from_ref(s: &ServerState<I>) -> Self {
        s.index.clone()
    }
}

impl<I: IndexBackend + 'static> FromRef<ServerState<I>> for Arc<dyn ApiKeyLookup> {
    fn from_ref(s: &ServerState<I>) -> Self {
        s.api_keys.clone()
    }
}

impl<I: IndexBackend + 'static> FromRef<ServerState<I>> for Arc<dyn TenantRateLimiter> {
    fn from_ref(s: &ServerState<I>) -> Self {
        s.rate_limit.clone()
    }
}

impl<I: IndexBackend + 'static> FromRef<ServerState<I>> for Arc<dyn UsageSink> {
    fn from_ref(s: &ServerState<I>) -> Self {
        s.usage.clone()
    }
}

/// Build the auth-wired router used by `bin/ucfp.rs` in production.
///
/// Layers, in order of execution:
///
/// 1. [`ApiKeyContext`] extraction on every protected route — handlers
///    that don't take the extractor still flow through it because it's
///    invoked by the middleware below before the handler runs.
/// 2. [`TenantRateLimiter`] check — denies with `429` + `Retry-After`.
/// 3. Handler.
/// 4. [`UsageSink`] receives a [`UsageEvent`] for every request that
///    made it past the rate limiter (success or handler error).
///
/// `public_router` is merged in unauthenticated.
pub fn router_with_state<I>(state: ServerState<I>) -> Router
where
    I: IndexBackend + 'static,
{
    let public = public_router(state.index.clone());

    // Build the protected route table against `ServerState<I>` directly
    // (handler `State<Arc<I>>` extractors are satisfied by the
    // `FromRef<ServerState<I>> for Arc<I>` impl above), layer auth +
    // rate-limit + usage on top, then commit the state.
    let protected = protected_routes::<I, ServerState<I>>()
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth_rate_usage_layer::<I>,
        ))
        .with_state(state);

    public.merge(protected)
}

/// Tower middleware that performs auth + rate-limit pre-check and
/// fires-and-forgets a usage event after the handler runs. Errors at
/// either pre-check short-circuit with the matching status code; the
/// handler is never invoked.
async fn auth_rate_usage_layer<I>(
    axum::extract::State(state): axum::extract::State<ServerState<I>>,
    mut req: axum::http::Request<axum::body::Body>,
    next: Next,
) -> Response
where
    I: IndexBackend + 'static,
{
    // Mirror the extractor's bearer parse so we can both resolve the
    // context up-front and stash it in extensions for downstream reads.
    let token = match req
        .headers()
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|h| h.to_str().ok())
        .and_then(|h| h.strip_prefix("Bearer "))
        .map(str::trim)
        .filter(|t| !t.is_empty())
    {
        Some(t) => t.to_string(),
        None => {
            return (StatusCode::UNAUTHORIZED, "missing or empty bearer token").into_response();
        }
    };

    let ctx = match state.api_keys.lookup(&token).await {
        Ok(Some(c)) => c,
        Ok(None) => return (StatusCode::UNAUTHORIZED, "unknown api key").into_response(),
        Err(_) => {
            return (StatusCode::INTERNAL_SERVER_ERROR, "key lookup failed").into_response();
        }
    };

    match state.rate_limit.check(&ctx, 1).await {
        Ok(RateDecision::Allow { .. }) => {}
        Ok(RateDecision::Deny { retry_after_ms }) => {
            let retry_secs = retry_after_ms.div_ceil(1000);
            let mut resp = (StatusCode::TOO_MANY_REQUESTS, "rate limited").into_response();
            if let Ok(v) = axum::http::HeaderValue::from_str(&retry_secs.to_string()) {
                resp.headers_mut().insert("retry-after", v);
            }
            return resp;
        }
        Err(_) => {
            return (StatusCode::INTERNAL_SERVER_ERROR, "rate limit failed").into_response();
        }
    }

    // Stash the context in request extensions so handlers (and any
    // downstream extractors) can read it without re-resolving the token.
    req.extensions_mut().insert(ctx.clone());

    let started = Instant::now();
    let bytes_in = req
        .headers()
        .get(axum::http::header::CONTENT_LENGTH)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(0);

    let resp = next.run(req).await;
    let elapsed_ms = started.elapsed().as_millis() as u64;
    let status = resp.status().as_u16();

    let event = UsageEvent {
        tenant_id: ctx.tenant_id,
        key_id: ctx.key_id.clone(),
        op: UsageOp::Ingest, // routing-aware classification lands in R4
        modality: None,
        algorithm: None,
        bytes_in,
        units: 1,
        elapsed_ms,
        status,
        ts: std::time::SystemTime::now(),
    };
    let sink = state.usage.clone();
    tokio::spawn(async move { sink.record(&event).await });

    resp
}
