//! HTTP routes — `/healthz`, `/v1/info`, `/v1/records`, `/v1/query`.
//!
//! Generic over [`crate::IndexBackend`] so the same router serves the
//! embedded backend today and a managed graduation backend later. The
//! bin entry in `bin/ucfp.rs` instantiates it with `Arc<EmbeddedBackend>`.
//!
//! All handlers funnel through [`error::ApiError`] for a consistent
//! error envelope; HTTP status codes map per [`crate::Error`] variant.

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

/// Build the UCFP router. State is a single `Arc<I>` — clone-friendly
/// for axum's `with_state` and shareable across requests.
pub fn router<I>(index: Arc<I>) -> Router
where
    I: IndexBackend + 'static,
{
    let r = Router::new()
        .route("/healthz", get(handlers::healthz))
        .route("/v1/info", get(handlers::info))
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
