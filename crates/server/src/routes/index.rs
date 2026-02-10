use crate::error::{ServerError, ServerResult};
use crate::state::ServerState;
use axum::extract::{Query, State};
use axum::response::IntoResponse;
use axum::Json;
use index::{IndexRecord, INDEX_SCHEMA_VERSION};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

/// Request to insert a record into the index
#[derive(Debug, Deserialize)]
pub struct IndexInsertRequest {
    /// Document ID
    pub doc_id: String,

    /// Tenant ID
    #[serde(default)]
    pub tenant_id: Option<String>,

    /// Canonical hash
    pub canonical_hash: String,

    /// Optional perceptual fingerprint for similarity search.
    ///
    /// This must be the **MinHash signature** (`Vec<u64>`) from `PerceptualFingerprint.minhash`,
    /// not the full fingerprint struct. The MinHash signature enables approximate Jaccard
    /// similarity search via Locality-Sensitive Hashing (LSH).
    ///
    /// Generate via: `perceptualize_tokens(&tokens, &config)?.minhash`
    #[serde(default)]
    pub perceptual_fingerprint: Option<Vec<u64>>,

    /// Optional semantic embedding for vector similarity search.
    ///
    /// Raw f32 embedding vector from the semantic layer. The server automatically
    /// quantizes this to i8 for storage. Use `semanticize()` or `semanticize_document()`
    /// from the semantic crate to generate embeddings.
    #[serde(default)]
    pub semantic_embedding: Option<Vec<f32>>,

    /// Optional metadata
    #[serde(default)]
    pub metadata: Option<HashMap<String, String>>,
}

/// Response from index insert
#[derive(Debug, Serialize)]
pub struct IndexInsertResponse {
    pub doc_id: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Query parameters for index search
#[derive(Debug, Deserialize)]
pub struct IndexSearchQuery {
    /// Search query text
    pub query: String,

    /// Search strategy: "perceptual" or "semantic"
    #[serde(default = "default_strategy")]
    pub strategy: String,

    /// Number of results to return
    #[serde(default = "default_top_k")]
    pub top_k: usize,

    /// Tenant ID to filter by
    #[serde(default)]
    pub tenant_id: Option<String>,
}

/// Search response
#[derive(Debug, Serialize)]
pub struct IndexSearchResponse {
    pub query: String,
    pub strategy: String,
    pub total_hits: usize,
    pub hits: Vec<SearchHit>,
}

/// Single search hit
#[derive(Debug, Serialize)]
pub struct SearchHit {
    pub doc_id: String,
    pub score: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tenant_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<HashMap<String, String>>,
}

fn default_strategy() -> String {
    "perceptual".to_string()
}

fn default_top_k() -> usize {
    10
}

/// Insert a record into the index.
///
/// This endpoint accepts document data with optional perceptual and semantic fingerprints
/// for similarity search. The perceptual fingerprint enables Jaccard similarity via MinHash LSH,
/// while the semantic embedding enables cosine similarity via vector search.
///
/// # Field Processing
/// - `perceptual_fingerprint`: Stored directly as MinHash signature for LSH-based search
/// - `semantic_embedding`: Automatically quantized from f32 to i8 (scale=100.0) for storage
/// - `metadata`: Merged with doc_id and tenant_id fields
///
/// # Example Request Body
/// ```json
/// {
///   "doc_id": "doc-123",
///   "canonical_hash": "sha256_hex...",
///   "perceptual_fingerprint": [123456789, 987654321, ...],
///   "semantic_embedding": [0.1, 0.2, 0.3, ...],
///   "tenant_id": "tenant-a",
///   "metadata": {"title": "Example Doc"}
/// }
/// ```
pub async fn insert_record(
    State(state): State<Arc<ServerState>>,
    Json(request): Json<IndexInsertRequest>,
) -> ServerResult<impl IntoResponse> {
    // Build metadata with doc_id and tenant_id
    let mut metadata = serde_json::Map::new();
    metadata.insert("doc_id".to_string(), serde_json::json!(request.doc_id));
    if let Some(tenant_id) = &request.tenant_id {
        metadata.insert("tenant_id".to_string(), serde_json::json!(tenant_id));
    }
    if let Some(extra_meta) = &request.metadata {
        for (key, value) in extra_meta {
            metadata.insert(key.clone(), serde_json::json!(value));
        }
    }

    // Quantize f32 embeddings to i8
    let embedding = request.semantic_embedding.map(|s| {
        s.iter()
            .map(|&v| (v * 100.0).clamp(-128.0, 127.0) as i8)
            .collect()
    });

    // Build the index record
    let record = IndexRecord {
        schema_version: INDEX_SCHEMA_VERSION,
        canonical_hash: request.canonical_hash,
        perceptual: request.perceptual_fingerprint,
        embedding,
        metadata: serde_json::Value::Object(metadata),
    };

    // Insert into index
    match state.index.upsert(&record) {
        Ok(_) => Ok(Json(IndexInsertResponse {
            doc_id: request.doc_id,
            status: "inserted".to_string(),
            error: None,
        })),
        Err(e) => Err(ServerError::Index(e)),
    }
}

/// Search the index
pub async fn search_index(
    State(state): State<Arc<ServerState>>,
    Query(query): Query<IndexSearchQuery>,
) -> ServerResult<impl IntoResponse> {
    use matcher::types::MetricId;
    use matcher::{MatchConfig, MatchExpr, MatchMode, MatchRequest};

    // Determine strategy and mode
    let (mode, strategy) = match query.strategy.as_str() {
        "semantic" => (
            MatchMode::Semantic,
            MatchExpr::Semantic {
                metric: MetricId::Cosine,
                min_score: 0.0,
            },
        ),
        _ => (
            MatchMode::Perceptual,
            MatchExpr::Perceptual {
                metric: MetricId::Jaccard,
                min_score: 0.0,
            },
        ),
    };

    // Get tenant ID from query or use default from config
    let tenant_id = query.tenant_id.unwrap_or_else(|| {
        state
            .config
            .api_keys
            .iter()
            .next()
            .cloned()
            .unwrap_or_default()
    });

    // Build match request for searching
    let match_req = MatchRequest {
        tenant_id: tenant_id.clone(),
        query_text: query.query.clone(),
        config: MatchConfig {
            version: "v1".to_string(),
            policy_id: "search-policy".to_string(),
            policy_version: "v1".to_string(),
            mode,
            strategy,
            max_results: query.top_k,
            tenant_enforce: true,
            oversample_factor: 2.0,
            explain: false,
        },
        attributes: None,
        pipeline_version: None,
        fingerprint_versions: None,
        query_canonical_hash: None,
    };

    // Execute search using the matcher
    let match_hits = state
        .matcher
        .match_document(&match_req)
        .map_err(ServerError::Match)?;

    // Convert hits to SearchHit format
    let hits: Vec<SearchHit> = match_hits
        .into_iter()
        .map(|hit| {
            // Extract doc_id and tenant_id from metadata
            let doc_id = hit
                .metadata
                .get("doc_id")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .unwrap_or_else(|| hit.canonical_hash.clone());

            let hit_tenant_id = hit
                .metadata
                .get("tenant")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .or_else(|| {
                    hit.metadata
                        .get("tenant_id")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string())
                });

            // Convert metadata to HashMap<String, String>
            let metadata: HashMap<String, String> = hit
                .metadata
                .as_object()
                .map(|obj| {
                    obj.iter()
                        .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                        .collect()
                })
                .unwrap_or_default();

            SearchHit {
                doc_id,
                score: hit.score as f64,
                tenant_id: hit_tenant_id,
                metadata: if metadata.is_empty() {
                    None
                } else {
                    Some(metadata)
                },
            }
        })
        .collect();

    Ok(Json(IndexSearchResponse {
        query: query.query,
        strategy: query.strategy,
        total_hits: hits.len(),
        hits,
    }))
}

/// List all documents in index (admin only)
pub async fn list_documents(
    State(state): State<Arc<ServerState>>,
) -> ServerResult<impl IntoResponse> {
    let mut documents = Vec::new();

    state
        .index
        .scan(&mut |record: &IndexRecord| {
            // Extract doc_id from metadata
            let doc_id = record
                .metadata
                .get("doc_id")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .unwrap_or_else(|| record.canonical_hash.clone());

            // Extract tenant_id from metadata if present
            let tenant_id = record
                .metadata
                .get("tenant_id")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());

            // Convert metadata to HashMap<String, String> for response
            let metadata: HashMap<String, String> = record
                .metadata
                .as_object()
                .map(|obj| {
                    obj.iter()
                        .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                        .collect()
                })
                .unwrap_or_default();

            documents.push(serde_json::json!({
                "doc_id": doc_id,
                "canonical_hash": record.canonical_hash,
                "tenant_id": tenant_id,
                "metadata": metadata,
            }));

            Ok(())
        })
        .map_err(ServerError::Index)?;

    let total = documents.len();
    Ok(Json(serde_json::json!({
        "documents": documents,
        "total": total,
    })))
}

/// Delete a document from the index
pub async fn delete_document(
    State(state): State<Arc<ServerState>>,
    axum::extract::Path(doc_id): axum::extract::Path<String>,
) -> ServerResult<impl IntoResponse> {
    match state.index.delete(&doc_id) {
        Ok(_) => Ok(Json(serde_json::json!({
            "doc_id": doc_id,
            "status": "deleted"
        }))),
        Err(e) => Err(ServerError::Index(e)),
    }
}
