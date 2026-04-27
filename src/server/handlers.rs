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

// Imports only the ingest handlers need — feature-gated so a build
// with all three modality features off doesn't warn.
#[cfg(feature = "audio")]
use super::dto::AudioParams;
#[cfg(any(feature = "audio", feature = "image", feature = "text"))]
use super::dto::IngestResponse;
#[cfg(any(feature = "audio", feature = "text"))]
use crate::error::Error;
#[cfg(any(feature = "audio", feature = "image", feature = "text"))]
use axum::body::Bytes;
#[cfg(feature = "audio")]
use axum::extract::Query as Qs;

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

// ── POST /v1/ingest/* ──────────────────────────────────────────────────
//
// Each modality-specific ingest route takes the raw bytes, hands them to
// the right SDK adapter, and upserts a fully-formed Record. Clients no
// longer need to compute fingerprints themselves.

#[cfg(any(feature = "audio", feature = "image", feature = "text"))]
fn ingest_response(rec: &Record) -> IngestResponse {
    IngestResponse {
        tenant_id: rec.tenant_id,
        record_id: rec.record_id,
        modality: rec.modality,
        format_version: rec.format_version,
        algorithm: rec.algorithm.clone(),
        config_hash: rec.config_hash,
        fingerprint_bytes: rec.fingerprint.len(),
        has_embedding: rec.embedding.is_some(),
    }
}

#[cfg(feature = "image")]
pub(super) async fn ingest_image<I: IndexBackend>(
    State(index): State<Arc<I>>,
    Path((tenant_id, record_id)): Path<(u32, u64)>,
    body: Bytes,
) -> Result<(StatusCode, Json<IngestResponse>), ApiError> {
    let rec = crate::modality::image::fingerprint(&body, tenant_id, record_id)?;
    index.upsert(std::slice::from_ref(&rec)).await?;
    Ok((StatusCode::CREATED, Json(ingest_response(&rec))))
}

#[cfg(feature = "text")]
pub(super) async fn ingest_text<I: IndexBackend>(
    State(index): State<Arc<I>>,
    Path((tenant_id, record_id)): Path<(u32, u64)>,
    body: Bytes,
) -> Result<(StatusCode, Json<IngestResponse>), ApiError> {
    let text = std::str::from_utf8(&body)
        .map_err(|e| Error::Modality(format!("body is not valid UTF-8: {e}")))?;
    let rec = crate::modality::text::fingerprint_minhash(text, tenant_id, record_id)?;
    index.upsert(std::slice::from_ref(&rec)).await?;
    Ok((StatusCode::CREATED, Json(ingest_response(&rec))))
}

#[cfg(feature = "audio")]
pub(super) async fn ingest_audio<I: IndexBackend>(
    State(index): State<Arc<I>>,
    Path((tenant_id, record_id)): Path<(u32, u64)>,
    Qs(params): Qs<AudioParams>,
    body: Bytes,
) -> Result<(StatusCode, Json<IngestResponse>), ApiError> {
    if body.len() % 4 != 0 {
        return Err(Error::Modality(format!(
            "audio body must be a multiple of 4 bytes (raw f32 LE samples), got {}",
            body.len()
        ))
        .into());
    }
    // Decode body bytes → Vec<f32> via explicit little-endian conversion.
    // Avoids alignment concerns from a direct `bytemuck::cast_slice` on
    // arbitrary heap buffers across platforms.
    let samples: Vec<f32> = body
        .chunks_exact(4)
        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect();

    let rec = crate::modality::audio::fingerprint_wang(
        &samples,
        params.sample_rate,
        tenant_id,
        record_id,
    )?;
    index.upsert(std::slice::from_ref(&rec)).await?;
    Ok((StatusCode::CREATED, Json(ingest_response(&rec))))
}
