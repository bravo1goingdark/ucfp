//! HTTP routes — `/healthz`, `/v1/info`, `/v1/records`, `/v1/query`,
//! and feature-gated modality ingest paths.
//!
//! Generic over [`crate::IndexBackend`] so the same router serves the
//! embedded backend today and a managed graduation backend later. The
//! bin entry in `bin/ucfp.rs` instantiates with `Arc<EmbeddedBackend>`.
//!
//! All handlers funnel through [`error::ApiError`] for a consistent
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

#![cfg(feature = "server")]

mod dto;
mod error;
mod handlers;

#[cfg(all(test, feature = "embedded"))]
mod tests;

use std::sync::Arc;

use axum::{
    Router,
    routing::{delete, get, post},
};

use crate::index::IndexBackend;

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
    let r = Router::new()
        .route("/v1/records", post(handlers::upsert::<I>))
        .route(
            "/v1/records/{tenant_id}/{record_id}",
            delete(handlers::delete_record::<I>),
        )
        .route("/v1/query", post(handlers::query::<I>));

    #[cfg(feature = "image")]
    let r = r.route(
        "/v1/ingest/image/{tenant_id}/{record_id}",
        post(handlers::ingest_image::<I>),
    );

    #[cfg(feature = "text")]
    let r = r.route(
        "/v1/ingest/text/{tenant_id}/{record_id}",
        post(handlers::ingest_text::<I>),
    );

    #[cfg(feature = "audio")]
    let r = r.route(
        "/v1/ingest/audio/{tenant_id}/{record_id}",
        post(handlers::ingest_audio::<I>),
    );

    r.with_state(index)
}

/// Merged router — public + protected, no auth applied. Convenient for
/// tests and library consumers that wire their own auth.
pub fn router<I>(index: Arc<I>) -> Router
where
    I: IndexBackend + 'static,
{
    public_router(index.clone()).merge(protected_router(index))
}
