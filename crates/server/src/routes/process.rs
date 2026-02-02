use crate::error::{ServerError, ServerResult};
use crate::state::ServerState;
use axum::extract::State;
use axum::response::IntoResponse;
use axum::Json;
use chrono::Utc;
use futures::stream::{self, StreamExt};
use ingest::{IngestMetadata, IngestPayload, IngestSource, RawIngestRecord};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use ucfp::{CanonicalizeConfig, IngestConfig, PerceptualConfig, SemanticConfig};

/// Request to process a single document
#[derive(Debug, Deserialize)]
pub struct ProcessRequest {
    /// Document ID (optional, will be generated if not provided)
    #[serde(default)]
    pub doc_id: Option<String>,

    /// Tenant ID (optional, uses default if not provided)
    #[serde(default)]
    pub tenant_id: Option<String>,

    /// Document text content
    pub text: String,

    /// Enable perceptual fingerprinting
    #[serde(default = "default_true")]
    pub enable_perceptual: bool,

    /// Enable semantic embedding
    #[serde(default = "default_true")]
    pub enable_semantic: bool,

    /// Perceptual configuration overrides (optional)
    #[serde(default)]
    pub perceptual_config: Option<PerceptualConfig>,

    /// Semantic configuration overrides (optional)
    #[serde(default)]
    pub semantic_config: Option<SemanticConfig>,
}

/// Response from processing a single document
#[derive(Debug, Serialize)]
pub struct ProcessResponse {
    pub doc_id: String,
    pub tenant_id: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub canonical_hash: Option<String>,
    /// Perceptual fingerprint result containing the full `PerceptualFingerprint` struct.
    /// For indexing, extract the `minhash` field (`Vec<u64>`) which contains the LSH signature
    /// for approximate Jaccard similarity search.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub perceptual_fingerprint: Option<serde_json::Value>,
    /// Semantic embedding result containing the `SemanticEmbedding` struct.
    /// For indexing, the server API accepts the raw f32 vector which gets quantized to i8.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub semantic_embedding: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Batch processing request
#[derive(Debug, Deserialize)]
pub struct BatchProcessRequest {
    pub documents: Vec<BatchDocument>,

    #[serde(default = "default_true")]
    pub enable_perceptual: bool,

    #[serde(default = "default_true")]
    pub enable_semantic: bool,
}

/// Single document in a batch
#[derive(Debug, Deserialize)]
pub struct BatchDocument {
    #[serde(default)]
    pub doc_id: Option<String>,
    #[serde(default)]
    pub tenant_id: Option<String>,
    pub text: String,
}

/// Batch processing response
#[derive(Debug, Serialize)]
pub struct BatchProcessResponse {
    pub processed: usize,
    pub successful: usize,
    pub failed: usize,
    pub results: Vec<ProcessResponse>,
}

fn default_true() -> bool {
    true
}

/// Process a single document through the UCFP pipeline.
///
/// This endpoint runs text through the full pipeline: ingest → canonicalize →
/// perceptual fingerprinting and/or semantic embedding generation.
///
/// # Pipeline Stages
/// 1. **Ingest**: Validates and normalizes the input text
/// 2. **Canonicalize**: Tokenizes and normalizes text (lowercase, NFKC, etc.)
/// 3. **Perceptual** (optional): Generates MinHash LSH signature for similarity search
/// 4. **Semantic** (optional): Generates vector embedding for semantic similarity
///
/// # Response Fields
/// - `perceptual_fingerprint`: Full `PerceptualFingerprint` struct with `minhash` field
///   containing the LSH signature (use this field's value for the index insert API)
/// - `semantic_embedding`: `SemanticEmbedding` struct with the embedding vector
///   (extract the `vector` field for the index insert API)
///
/// # Example
/// ```json
/// // Request
/// {
///   "text": "Hello world",
///   "enable_perceptual": true,
///   "enable_semantic": true
/// }
///
/// // Response
/// {
///   "doc_id": "uuid",
///   "status": "success",
///   "perceptual_fingerprint": {
///     "minhash": [123456789, ...],
///     "meta": { "k": 9, "w": 4, ... }
///   },
///   "semantic_embedding": {
///     "vector": [0.1, 0.2, ...],
///     "doc_id": "uuid",
///     "model_name": "bge-small-en-v1.5"
///   }
/// }
/// ```
pub async fn process_document(
    State(state): State<Arc<ServerState>>,
    Json(request): Json<ProcessRequest>,
) -> ServerResult<impl IntoResponse> {
    let doc_id = request
        .doc_id
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

    let tenant_id = request.tenant_id.unwrap_or_else(|| {
        state
            .config
            .api_keys
            .iter()
            .next()
            .cloned()
            .unwrap_or_default()
    });

    // Create raw ingest record
    let raw = RawIngestRecord {
        id: doc_id.clone(),
        source: IngestSource::RawText,
        metadata: IngestMetadata {
            tenant_id: Some(tenant_id.clone()),
            doc_id: Some(doc_id.clone()),
            received_at: Some(Utc::now()),
            original_source: Some("api".to_string()),
            attributes: None,
        },
        payload: Some(IngestPayload::Text(request.text)),
    };

    let mut response = ProcessResponse {
        doc_id: doc_id.clone(),
        tenant_id: tenant_id.clone(),
        status: "success".to_string(),
        canonical_hash: None,
        perceptual_fingerprint: None,
        semantic_embedding: None,
        error: None,
    };

    let ingest_cfg = IngestConfig::default();
    let canonical_cfg = CanonicalizeConfig::default();

    // Process based on enabled features
    if request.enable_perceptual && request.enable_semantic {
        // Both perceptual and semantic - run separately
        let perceptual_cfg = request.perceptual_config.unwrap_or_default();
        let semantic_cfg = request.semantic_config.unwrap_or_default();

        // First process perceptual
        match ucfp::process_record_with_perceptual_configs(
            raw.clone(),
            &ingest_cfg,
            &canonical_cfg,
            &perceptual_cfg,
        ) {
            Ok((doc, fingerprint)) => {
                response.canonical_hash = Some(doc.sha256_hex.clone());
                response.perceptual_fingerprint = Some(
                    serde_json::to_value(fingerprint)
                        .map_err(|e| ServerError::Internal(e.to_string()))?,
                );
            }
            Err(e) => {
                response.status = "error".to_string();
                response.error = Some(e.to_string());
                return Ok(Json(response));
            }
        }

        // Then process semantic
        match ucfp::process_record_with_semantic_configs(
            raw,
            &ingest_cfg,
            &canonical_cfg,
            &semantic_cfg,
        ) {
            Ok((_, embedding)) => {
                response.semantic_embedding = Some(
                    serde_json::to_value(embedding)
                        .map_err(|e| ServerError::Internal(e.to_string()))?,
                );
            }
            Err(e) => {
                response.status = "error".to_string();
                response.error = Some(e.to_string());
            }
        }
    } else if request.enable_perceptual {
        // Perceptual only
        let perceptual_cfg = request.perceptual_config.unwrap_or_default();

        match ucfp::process_record_with_perceptual_configs(
            raw,
            &ingest_cfg,
            &canonical_cfg,
            &perceptual_cfg,
        ) {
            Ok((doc, fingerprint)) => {
                response.canonical_hash = Some(doc.sha256_hex.clone());
                response.perceptual_fingerprint = Some(
                    serde_json::to_value(fingerprint)
                        .map_err(|e| ServerError::Internal(e.to_string()))?,
                );
            }
            Err(e) => {
                response.status = "error".to_string();
                response.error = Some(e.to_string());
            }
        }
    } else if request.enable_semantic {
        // Semantic only
        let semantic_cfg = request.semantic_config.unwrap_or_default();

        match ucfp::process_record_with_semantic_configs(
            raw,
            &ingest_cfg,
            &canonical_cfg,
            &semantic_cfg,
        ) {
            Ok((doc, embedding)) => {
                response.canonical_hash = Some(doc.sha256_hex.clone());
                response.semantic_embedding = Some(
                    serde_json::to_value(embedding)
                        .map_err(|e| ServerError::Internal(e.to_string()))?,
                );
            }
            Err(e) => {
                response.status = "error".to_string();
                response.error = Some(e.to_string());
            }
        }
    } else {
        // Canonical only
        match ucfp::process_record_with_configs(raw, &ingest_cfg, &canonical_cfg) {
            Ok(doc) => {
                response.canonical_hash = Some(doc.sha256_hex.clone());
            }
            Err(e) => {
                response.status = "error".to_string();
                response.error = Some(e.to_string());
            }
        }
    }

    Ok(Json(response))
}

/// Process multiple documents in batch with parallel processing.
///
/// Documents are processed concurrently with a configurable concurrency limit (default: 10).
/// Results are returned in the same order as the input documents.
pub async fn process_batch(
    State(state): State<Arc<ServerState>>,
    Json(request): Json<BatchProcessRequest>,
) -> ServerResult<impl IntoResponse> {
    const CONCURRENCY: usize = 10;

    let ingest_cfg = IngestConfig::default();
    let canonical_cfg = CanonicalizeConfig::default();
    let enable_perceptual = request.enable_perceptual;
    let enable_semantic = request.enable_semantic;

    // Create a stream from documents with their indices to preserve order
    let results: Vec<(usize, ProcessResponse)> =
        stream::iter(request.documents.into_iter().enumerate().map(|(idx, doc)| {
            let state = state.clone();
            let ingest_cfg = ingest_cfg.clone();
            let canonical_cfg = canonical_cfg.clone();

            async move {
                let doc_id = doc
                    .doc_id
                    .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
                let tenant_id = doc.tenant_id.unwrap_or_else(|| {
                    state
                        .config
                        .api_keys
                        .iter()
                        .next()
                        .cloned()
                        .unwrap_or_default()
                });

                let raw = RawIngestRecord {
                    id: doc_id.clone(),
                    source: IngestSource::RawText,
                    metadata: IngestMetadata {
                        tenant_id: Some(tenant_id.clone()),
                        doc_id: Some(doc_id.clone()),
                        received_at: Some(Utc::now()),
                        original_source: Some("api".to_string()),
                        attributes: None,
                    },
                    payload: Some(IngestPayload::Text(doc.text)),
                };

                let mut response = ProcessResponse {
                    doc_id: doc_id.clone(),
                    tenant_id: tenant_id.clone(),
                    status: "success".to_string(),
                    canonical_hash: None,
                    perceptual_fingerprint: None,
                    semantic_embedding: None,
                    error: None,
                };

                // Process perceptual fingerprinting
                if enable_perceptual {
                    let perceptual_cfg = PerceptualConfig::default();
                    match ucfp::process_record_with_perceptual_configs(
                        raw.clone(),
                        &ingest_cfg,
                        &canonical_cfg,
                        &perceptual_cfg,
                    ) {
                        Ok((doc, fingerprint)) => {
                            response.canonical_hash = Some(doc.sha256_hex.clone());
                            response.perceptual_fingerprint = Some(
                                serde_json::to_value(fingerprint)
                                    .map_err(|e| ServerError::Internal(e.to_string()))
                                    .unwrap_or(serde_json::Value::Null),
                            );
                        }
                        Err(e) => {
                            response.status = "error".to_string();
                            response.error = Some(e.to_string());
                            return (idx, response);
                        }
                    }
                }

                // Process semantic embedding
                if enable_semantic {
                    let semantic_cfg = SemanticConfig::default();
                    match ucfp::process_record_with_semantic_configs(
                        raw.clone(),
                        &ingest_cfg,
                        &canonical_cfg,
                        &semantic_cfg,
                    ) {
                        Ok((doc, embedding)) => {
                            if response.canonical_hash.is_none() {
                                response.canonical_hash = Some(doc.sha256_hex.clone());
                            }
                            response.semantic_embedding = Some(
                                serde_json::to_value(embedding)
                                    .map_err(|e| ServerError::Internal(e.to_string()))
                                    .unwrap_or(serde_json::Value::Null),
                            );
                        }
                        Err(e) => {
                            response.status = "error".to_string();
                            response.error = Some(e.to_string());
                            return (idx, response);
                        }
                    }
                }

                (idx, response)
            }
        }))
        .buffer_unordered(CONCURRENCY)
        .collect()
        .await;

    // Sort results by index to preserve input order
    let mut results_with_indices = results;
    results_with_indices.sort_by_key(|(idx, _)| *idx);

    // Count successes and failures
    let successful = results_with_indices
        .iter()
        .filter(|(_, r)| r.status == "success")
        .count();
    let failed = results_with_indices.len() - successful;

    // Extract just the responses in order
    let results: Vec<ProcessResponse> = results_with_indices
        .into_iter()
        .map(|(_, response)| response)
        .collect();

    Ok(Json(BatchProcessResponse {
        processed: results.len(),
        successful,
        failed,
        results,
    }))
}
