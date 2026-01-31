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

    /// Optional perceptual fingerprint (MinHash values as strings)
    #[serde(default)]
    pub perceptual_fingerprint: Option<Vec<u64>>,

    /// Optional semantic embedding
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

/// Insert a record into the index
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
    State(_state): State<Arc<ServerState>>,
    Query(query): Query<IndexSearchQuery>,
) -> ServerResult<impl IntoResponse> {
    // For now, return mock results
    // In a full implementation, this would:
    // 1. Process the query text through the pipeline
    // 2. Generate fingerprint/embedding based on strategy
    // 3. Search the index using the appropriate method
    // 4. Return ranked results

    let hits = vec![];

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
