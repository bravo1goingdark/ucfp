//! Request and response DTOs for the HTTP API.
//!
//! Bytes-typed fields (`fingerprint`, `metadata`) ride as JSON arrays of
//! u8 — verbose but no base64 dep, demo-friendly with `curl`.

use bytes::Bytes;
use serde::{Deserialize, Serialize};

use crate::core::{Modality, Record};

// ── /v1/info ───────────────────────────────────────────────────────────

#[derive(Serialize, Deserialize)]
pub(super) struct InfoResponse {
    pub format_version: u32,
    pub crate_version: String,
}

// ── /v1/records (POST upsert) ──────────────────────────────────────────

#[derive(Deserialize)]
pub(super) struct UpsertRequest {
    pub records: Vec<RecordIn>,
}

#[derive(Deserialize)]
pub(super) struct RecordIn {
    pub tenant_id: u32,
    pub record_id: u64,
    pub modality: Modality,
    pub format_version: u32,
    pub algorithm: String,
    pub config_hash: u64,
    /// Raw fingerprint bytes — JSON array of u8. Not base64.
    pub fingerprint: Vec<u8>,
    #[serde(default)]
    pub embedding: Option<Vec<f32>>,
    #[serde(default)]
    pub model_id: Option<String>,
    #[serde(default)]
    pub metadata: Vec<u8>,
}

impl From<RecordIn> for Record {
    fn from(r: RecordIn) -> Self {
        Record {
            tenant_id: r.tenant_id,
            record_id: r.record_id,
            modality: r.modality,
            format_version: r.format_version,
            algorithm: r.algorithm,
            config_hash: r.config_hash,
            fingerprint: Bytes::from(r.fingerprint),
            embedding: r.embedding,
            model_id: r.model_id,
            metadata: Bytes::from(r.metadata),
        }
    }
}

#[derive(Serialize)]
pub(super) struct UpsertResponse {
    pub upserted: usize,
}

// ── /v1/query (POST) ───────────────────────────────────────────────────

#[derive(Deserialize)]
pub(super) struct QueryRequest {
    pub tenant_id: u32,
    pub modality: Modality,
    #[serde(default = "default_k")]
    pub k: usize,
    /// Dense query vector. BM25 path lands once `IndexBackend::bm25`
    /// is implemented; for now this is required.
    pub vector: Vec<f32>,
}

pub(super) fn default_k() -> usize {
    10
}

#[derive(Serialize)]
pub(super) struct QueryResponse {
    pub hits: Vec<HitOut>,
}

#[derive(Serialize)]
pub(super) struct HitOut {
    pub tenant_id: u32,
    pub record_id: u64,
    pub score: f32,
    /// `"vector" | "bm25" | "filter" | "reranker" | "fused"`.
    pub source: &'static str,
}

// ── /v1/ingest/{modality}/{tid}/{rid} (POST) ───────────────────────────

/// Returned by the modality-specific ingest routes after a successful
/// upsert. Confirms what was stored so the client can reconcile.
#[cfg(any(feature = "audio", feature = "image", feature = "text"))]
#[derive(Serialize)]
pub(super) struct IngestResponse {
    pub tenant_id: u32,
    pub record_id: u64,
    pub modality: Modality,
    pub format_version: u32,
    pub algorithm: String,
    pub config_hash: u64,
    pub fingerprint_bytes: usize,
    pub has_embedding: bool,
}

/// Query parameters for `POST /v1/ingest/audio/...` — sample rate is
/// required since the body is raw f32 samples (no header carries it).
#[cfg(feature = "audio")]
#[derive(Deserialize)]
pub(super) struct AudioParams {
    pub sample_rate: u32,
}
