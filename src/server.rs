//! HTTP routes — `/healthz`, `/v1/info`, `/v1/records`, `/v1/query`.
//!
//! Generic over the [`crate::IndexBackend`] so the same router serves
//! the embedded backend today and a managed graduation backend later.
//! The bin entry [`crate::bin`-equivalent] in `bin/ucfp.rs` instantiates
//! it with `Arc<EmbeddedBackend>`.
//!
//! All handlers funnel through [`ApiError`] for a consistent error
//! envelope; HTTP status codes map per [`crate::Error`] variant.

#![cfg(feature = "server")]

use std::sync::Arc;

use axum::{
    Router,
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Json, Response},
    routing::{delete, get, post},
};
use bytes::Bytes;
use serde::{Deserialize, Serialize};

use crate::core::{HitSource, Modality, Query, Record};
use crate::error::Error;
use crate::index::IndexBackend;
use crate::matcher::Matcher;

/// Build the UCFP router. The state is a single `Arc<I>` — clone-friendly
/// for axum's `with_state` and shareable across requests.
pub fn router<I>(index: Arc<I>) -> Router
where
    I: IndexBackend + 'static,
{
    Router::new()
        .route("/healthz", get(healthz))
        .route("/v1/info", get(info))
        .route("/v1/records", post(upsert::<I>))
        .route(
            "/v1/records/{tenant_id}/{record_id}",
            delete(delete_record::<I>),
        )
        .route("/v1/query", post(query::<I>))
        .with_state(index)
}

// ── /healthz ───────────────────────────────────────────────────────────

async fn healthz() -> &'static str {
    "ok"
}

// ── /v1/info ───────────────────────────────────────────────────────────

#[derive(Serialize)]
struct InfoResponse {
    format_version: u32,
    crate_version: &'static str,
}

async fn info() -> Json<InfoResponse> {
    Json(InfoResponse {
        format_version: crate::FORMAT_VERSION,
        crate_version: env!("CARGO_PKG_VERSION"),
    })
}

// ── /v1/records (POST upsert) ──────────────────────────────────────────

#[derive(Deserialize)]
struct UpsertRequest {
    records: Vec<RecordIn>,
}

#[derive(Deserialize)]
struct RecordIn {
    tenant_id: u32,
    record_id: u64,
    modality: Modality,
    format_version: u32,
    algorithm: String,
    config_hash: u64,
    /// Raw fingerprint bytes — JSON array of u8. Not base64.
    fingerprint: Vec<u8>,
    #[serde(default)]
    embedding: Option<Vec<f32>>,
    #[serde(default)]
    model_id: Option<String>,
    #[serde(default)]
    metadata: Vec<u8>,
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
struct UpsertResponse {
    upserted: usize,
}

async fn upsert<I: IndexBackend>(
    State(index): State<Arc<I>>,
    Json(req): Json<UpsertRequest>,
) -> Result<Json<UpsertResponse>, ApiError> {
    let count = req.records.len();
    let records: Vec<Record> = req.records.into_iter().map(Into::into).collect();
    index.upsert(&records).await?;
    Ok(Json(UpsertResponse { upserted: count }))
}

// ── /v1/records/{tenant_id}/{record_id} (DELETE) ───────────────────────

async fn delete_record<I: IndexBackend>(
    State(index): State<Arc<I>>,
    Path((tenant_id, record_id)): Path<(u32, u64)>,
) -> Result<StatusCode, ApiError> {
    index.delete(tenant_id, &[record_id]).await?;
    Ok(StatusCode::NO_CONTENT)
}

// ── /v1/query (POST) ───────────────────────────────────────────────────

#[derive(Deserialize)]
struct QueryRequest {
    tenant_id: u32,
    modality: Modality,
    #[serde(default = "default_k")]
    k: usize,
    /// Dense query vector. BM25 path lands once `IndexBackend::bm25`
    /// is implemented; for now this is required.
    vector: Vec<f32>,
}

fn default_k() -> usize {
    10
}

#[derive(Serialize)]
struct QueryResponse {
    hits: Vec<HitOut>,
}

#[derive(Serialize)]
struct HitOut {
    tenant_id: u32,
    record_id: u64,
    score: f32,
    /// `"vector" | "bm25" | "filter" | "reranker" | "fused"`.
    source: &'static str,
}

async fn query<I: IndexBackend>(
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

// ── Error envelope ─────────────────────────────────────────────────────

/// Wraps [`crate::Error`] with an `IntoResponse` impl so handlers can
/// `?`-propagate without writing per-route error mapping.
struct ApiError(Error);

impl From<Error> for ApiError {
    fn from(e: Error) -> Self {
        Self(e)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, code) = match &self.0 {
            Error::Modality(_) => (StatusCode::BAD_REQUEST, "modality"),
            Error::Incompatible(_) => (StatusCode::CONFLICT, "incompatible"),
            Error::Index(_) => (StatusCode::INTERNAL_SERVER_ERROR, "index"),
            Error::Ingest(_) => (StatusCode::SERVICE_UNAVAILABLE, "ingest"),
            Error::Rerank(_) => (StatusCode::INTERNAL_SERVER_ERROR, "rerank"),
            Error::Io(_) => (StatusCode::INTERNAL_SERVER_ERROR, "io"),
        };
        let body = Json(serde_json::json!({
            "error": code,
            "message": self.0.to_string(),
        }));
        (status, body).into_response()
    }
}

// ── tests ──────────────────────────────────────────────────────────────

#[cfg(all(test, feature = "embedded"))]
mod tests {
    use super::*;
    use crate::index::embedded::EmbeddedBackend;
    use axum::body::Body;
    use axum::http::Request;
    use http_body_util::BodyExt;
    use tower::util::ServiceExt;

    async fn fixture() -> (Router, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let backend = EmbeddedBackend::open(dir.path().join("ucfp.redb")).unwrap();
        let app = router(Arc::new(backend));
        (app, dir)
    }

    fn json_body(v: serde_json::Value) -> Body {
        Body::from(serde_json::to_vec(&v).unwrap())
    }

    async fn read_json<T: for<'de> Deserialize<'de>>(resp: Response) -> T {
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        serde_json::from_slice(&body).unwrap()
    }

    #[tokio::test]
    async fn healthz_returns_ok() {
        let (app, _dir) = fixture().await;
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/healthz")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn upsert_then_query_round_trips() {
        let (app, _dir) = fixture().await;

        // Upsert two records.
        let upsert_req = serde_json::json!({
            "records": [
                {
                    "tenant_id": 1, "record_id": 100,
                    "modality": "Image",
                    "format_version": 1, "algorithm": "test", "config_hash": 0,
                    "fingerprint": [1, 2, 3],
                    "embedding": [1.0, 0.0, 0.0]
                },
                {
                    "tenant_id": 1, "record_id": 200,
                    "modality": "Image",
                    "format_version": 1, "algorithm": "test", "config_hash": 0,
                    "fingerprint": [4, 5, 6],
                    "embedding": [0.7, 0.7, 0.0]
                }
            ]
        });
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/records")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&upsert_req).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body: serde_json::Value = read_json(resp).await;
        assert_eq!(body["upserted"], 2);

        // Query — record 200 has the higher cosine.
        let query_req = serde_json::json!({
            "tenant_id": 1,
            "modality": "Image",
            "k": 5,
            "vector": [0.6, 0.6, 0.0],
        });
        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/query")
                    .header("content-type", "application/json")
                    .body(json_body(query_req))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body: serde_json::Value = read_json(resp).await;
        let hits = body["hits"].as_array().unwrap();
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0]["record_id"], 200);
        assert_eq!(hits[0]["source"], "vector");
    }

    #[tokio::test]
    async fn delete_returns_204_and_removes_record() {
        let (app, _dir) = fixture().await;

        let upsert_req = serde_json::json!({
            "records": [{
                "tenant_id": 7, "record_id": 42,
                "modality": "Text",
                "format_version": 1, "algorithm": "minhash-h128", "config_hash": 0,
                "fingerprint": [9],
                "embedding": [1.0]
            }]
        });
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/records")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&upsert_req).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("DELETE")
                    .uri("/v1/records/7/42")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::NO_CONTENT);

        // Confirm the record is gone via query.
        let query_req = serde_json::json!({
            "tenant_id": 7,
            "modality": "Text",
            "k": 5,
            "vector": [1.0],
        });
        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/query")
                    .header("content-type", "application/json")
                    .body(json_body(query_req))
                    .unwrap(),
            )
            .await
            .unwrap();
        let body: serde_json::Value = read_json(resp).await;
        assert_eq!(body["hits"].as_array().unwrap().len(), 0);
    }

    #[tokio::test]
    async fn info_returns_format_version() {
        let (app, _dir) = fixture().await;
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/v1/info")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body: InfoResponse = read_json(resp).await;
        assert_eq!(body.format_version, crate::FORMAT_VERSION);
    }

    impl<'de> Deserialize<'de> for InfoResponse {
        fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
            #[derive(Deserialize)]
            struct Helper {
                format_version: u32,
                crate_version: String,
            }
            let h = Helper::deserialize(d)?;
            // NOTE: we leak the test-side string into a 'static so the
            // shared response struct stays trivially Serialize. Tests only.
            let leaked: &'static str = Box::leak(h.crate_version.into_boxed_str());
            Ok(InfoResponse {
                format_version: h.format_version,
                crate_version: leaked,
            })
        }
    }
}
