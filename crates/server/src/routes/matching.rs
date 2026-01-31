use crate::error::ServerResult;
use crate::state::ServerState;
use axum::extract::State;
use axum::response::IntoResponse;
use axum::Json;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

/// Match request
#[derive(Debug, Deserialize)]
pub struct MatchRequest {
    /// Query text
    pub query: String,

    /// Tenant ID (optional)
    #[serde(default)]
    pub tenant_id: Option<String>,

    /// Match strategy: "perceptual", "semantic", or "hybrid"
    #[serde(default = "default_strategy")]
    pub strategy: String,

    /// Maximum results to return
    #[serde(default = "default_max_results")]
    pub max_results: usize,

    /// Oversample factor for semantic matching
    #[serde(default = "default_oversample")]
    pub oversample_factor: f32,

    /// Minimum score threshold (0.0 to 1.0)
    #[serde(default)]
    pub min_score: Option<f32>,
}

/// Match response
#[derive(Debug, Serialize)]
pub struct MatchResponse {
    pub query: String,
    pub strategy: String,
    pub total_matches: usize,
    pub matches: Vec<MatchHit>,
}

/// Single match result
#[derive(Debug, Serialize)]
pub struct MatchHit {
    pub doc_id: String,
    pub score: f32,
    pub rank: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tenant_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<HashMap<String, serde_json::Value>>,
}

fn default_strategy() -> String {
    "hybrid".to_string()
}

fn default_max_results() -> usize {
    10
}

fn default_oversample() -> f32 {
    1.5
}

/// Match documents against query
pub async fn match_documents(
    State(_state): State<Arc<ServerState>>,
    Json(request): Json<MatchRequest>,
) -> ServerResult<impl IntoResponse> {
    // For now, return mock results
    // In a full implementation, this would:
    // 1. Process query through canonical + perceptual/semantic pipeline
    // 2. Use matcher to find similar documents
    // 3. Return ranked results

    let matches = vec![];

    Ok(Json(MatchResponse {
        query: request.query,
        strategy: request.strategy,
        total_matches: matches.len(),
        matches,
    }))
}

/// Compare two documents directly
#[derive(Debug, Deserialize)]
pub struct CompareRequest {
    pub doc1: DocumentInput,
    pub doc2: DocumentInput,
}

#[derive(Debug, Deserialize)]
pub struct DocumentInput {
    pub text: String,
    #[serde(default)]
    pub doc_id: Option<String>,
}

/// Compare response
#[derive(Debug, Serialize)]
pub struct CompareResponse {
    pub similarity_score: f32,
    pub perceptual_similarity: Option<f32>,
    pub semantic_similarity: Option<f32>,
}

/// Compare two documents for similarity
pub async fn compare_documents(
    State(_state): State<Arc<ServerState>>,
    Json(_request): Json<CompareRequest>,
) -> ServerResult<impl IntoResponse> {
    // In a full implementation:
    // 1. Process both documents through pipeline
    // 2. Calculate perceptual similarity (Jaccard)
    // 3. Calculate semantic similarity (cosine)
    // 4. Return combined score

    Ok(Json(CompareResponse {
        similarity_score: 0.0,
        perceptual_similarity: None,
        semantic_similarity: None,
    }))
}
