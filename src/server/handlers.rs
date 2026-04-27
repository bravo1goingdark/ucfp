//! Route handlers. Each one is `pub(super)` so the router builder in
//! `mod.rs` can register it without leaking the implementation.

use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Json,
};

use crate::core::{HitSource, Query, Record};
use crate::index::IndexBackend;
use crate::matcher::Matcher;

use super::dto::{
    HitOut, InfoResponse, QueryRequest, QueryResponse, RecordIn, UpsertRequest, UpsertResponse,
};
use super::error::ApiError;

// ── GET /healthz ───────────────────────────────────────────────────────

pub(super) async fn healthz() -> &'static str {
    "ok"
}

// ── GET /v1/info ───────────────────────────────────────────────────────

pub(super) async fn info() -> Json<InfoResponse> {
    Json(InfoResponse {
        format_version: crate::FORMAT_VERSION,
        crate_version: env!("CARGO_PKG_VERSION").to_string(),
    })
}

// ── POST /v1/records ───────────────────────────────────────────────────

pub(super) async fn upsert<I: IndexBackend>(
    State(index): State<Arc<I>>,
    Json(req): Json<UpsertRequest>,
) -> Result<Json<UpsertResponse>, ApiError> {
    let count = req.records.len();
    let records: Vec<Record> = req.records.into_iter().map(RecordIn::into).collect();
    index.upsert(&records).await?;
    Ok(Json(UpsertResponse { upserted: count }))
}

// ── DELETE /v1/records/{tenant_id}/{record_id} ─────────────────────────

pub(super) async fn delete_record<I: IndexBackend>(
    State(index): State<Arc<I>>,
    Path((tenant_id, record_id)): Path<(u32, u64)>,
) -> Result<StatusCode, ApiError> {
    index.delete(tenant_id, &[record_id]).await?;
    Ok(StatusCode::NO_CONTENT)
}

// ── POST /v1/query ─────────────────────────────────────────────────────

pub(super) async fn query<I: IndexBackend>(
    State(index): State<Arc<I>>,
    Json(req): Json<QueryRequest>,
) -> Result<Json<QueryResponse>, ApiError> {
    let q = Query {
        tenant_id: req.tenant_id,
        modality: req.modality,
        k: req.k.max(1),
        vector: Some(req.vector),
        terms: Vec::new(),
        filter: None,
        rrf_k: 60,
    };
    let matcher = Matcher::new(index.as_ref());
    let hits = matcher.search(&q).await?;

    let hits = hits
        .into_iter()
        .map(|h| HitOut {
            tenant_id: h.tenant_id,
            record_id: h.record_id,
            score: h.score,
            source: hit_source_str(h.source),
        })
        .collect();
    Ok(Json(QueryResponse { hits }))
}

fn hit_source_str(s: HitSource) -> &'static str {
    match s {
        HitSource::Vector => "vector",
        HitSource::Bm25 => "bm25",
        HitSource::Filter => "filter",
        HitSource::Reranker => "reranker",
        HitSource::Fused => "fused",
    }
}
