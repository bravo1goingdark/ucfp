use crate::error::{ServerError, ServerResult};
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

    /// Match strategy for similarity search:
    /// - `"perceptual"`: Jaccard similarity via MinHash LSH (exact/shingle-level similarity)
    /// - `"semantic"`: Cosine similarity via quantized embeddings (meaning/vector similarity)
    /// - `"hybrid"`: Combines both strategies (default)
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

/// Match documents against query using perceptual and/or semantic similarity.
///
/// This endpoint searches the index for documents similar to the query text.
/// The similarity strategy determines which fingerprint type is used:
///
/// - **Perceptual**: Uses MinHash LSH signatures for Jaccard similarity.
///   Best for detecting near-duplicates, plagiarism, or verbatim reuse.
///   Documents are similar if they share many shingles (exact text chunks).
///
/// - **Semantic**: Uses quantized embeddings for cosine similarity.
///   Best for finding conceptually related content.
///   Documents are similar if their meanings are close in vector space.
///
/// - **Hybrid**: Combines both perceptual and semantic signals.
///
/// # Query Processing
/// The query text is automatically processed through the same pipeline (canonicalize →
/// perceptualize and/or embed) before matching against indexed documents.
pub async fn match_documents(
    State(state): State<Arc<ServerState>>,
    Json(request): Json<MatchRequest>,
) -> ServerResult<impl IntoResponse> {
    use matcher::types::MetricId;
    use matcher::{MatchConfig, MatchExpr, MatchMode, MatchRequest as MatcherRequest};

    // Parse strategy
    let (mode, strategy) = match request.strategy.as_str() {
        "perceptual" => (
            MatchMode::Perceptual,
            MatchExpr::Perceptual {
                metric: MetricId::Jaccard,
                min_score: request.min_score.unwrap_or(0.0),
            },
        ),
        "semantic" => (
            MatchMode::Semantic,
            MatchExpr::Semantic {
                metric: MetricId::Cosine,
                min_score: request.min_score.unwrap_or(0.0),
            },
        ),
        _ => (
            MatchMode::Hybrid,
            MatchExpr::Weighted {
                semantic_weight: 0.7,
                min_overall: request.min_score.unwrap_or(0.0),
            },
        ),
    };

    // Get tenant ID from request or use default
    let tenant_id = request.tenant_id.unwrap_or_else(|| {
        state
            .config
            .api_keys
            .iter()
            .next()
            .cloned()
            .unwrap_or_default()
    });

    // Build match request
    let query_text = request.query.clone();
    let match_req = MatcherRequest {
        tenant_id,
        query_text,
        config: MatchConfig {
            version: "v1".to_string(),
            policy_id: "api-policy".to_string(),
            policy_version: "v1".to_string(),
            mode,
            strategy,
            max_results: request.max_results,
            tenant_enforce: true,
            oversample_factor: request.oversample_factor,
            explain: false,
        },
        attributes: None,
        pipeline_version: None,
        fingerprint_versions: None,
        query_canonical_hash: None,
    };

    // Execute match using the shared matcher
    let hits = state
        .matcher
        .match_document(&match_req)
        .map_err(ServerError::Match)?;

    // Convert matcher::MatchHit to route::MatchHit response format
    let mut matches = Vec::new();
    for (rank, hit) in hits.into_iter().enumerate() {
        let doc_id = hit.canonical_hash.clone();
        let score = hit.score;
        let tenant_id = hit
            .metadata
            .get("tenant")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let metadata = hit.metadata.as_object().map(|obj| {
            obj.iter()
                .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), serde_json::json!(s))))
                .collect::<HashMap<String, serde_json::Value>>()
        });

        matches.push(MatchHit {
            doc_id,
            score,
            rank: rank + 1,
            tenant_id,
            metadata,
        });
    }

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
    Json(request): Json<CompareRequest>,
) -> ServerResult<impl IntoResponse> {
    use chrono::Utc;
    use ingest::{IngestMetadata, IngestPayload, IngestSource, RawIngestRecord};
    use ucfp::{CanonicalizeConfig, IngestConfig, PerceptualConfig, SemanticConfig};

    let ingest_cfg = IngestConfig::default();
    let canonical_cfg = CanonicalizeConfig::default();
    let perceptual_cfg = PerceptualConfig::default();
    let semantic_cfg = SemanticConfig::default();

    // Helper to process a document and get its fingerprints/embeddings
    async fn process_doc(
        doc: &DocumentInput,
        ingest_cfg: &IngestConfig,
        canonical_cfg: &CanonicalizeConfig,
        perceptual_cfg: &PerceptualConfig,
        semantic_cfg: &SemanticConfig,
    ) -> Result<(Option<serde_json::Value>, Option<serde_json::Value>), ServerError> {
        let raw = RawIngestRecord {
            id: doc
                .doc_id
                .clone()
                .unwrap_or_else(|| uuid::Uuid::new_v4().to_string()),
            source: IngestSource::RawText,
            metadata: IngestMetadata {
                tenant_id: None,
                doc_id: doc.doc_id.clone(),
                received_at: Some(Utc::now()),
                original_source: Some("compare_api".to_string()),
                attributes: None,
            },
            payload: Some(IngestPayload::Text(doc.text.clone())),
        };

        // Try to get both perceptual and semantic
        let mut perceptual = None;
        let mut semantic = None;

        // Process perceptual
        if let Ok((_, fingerprint)) = ucfp::process_record_with_perceptual_configs(
            raw.clone(),
            ingest_cfg,
            canonical_cfg,
            perceptual_cfg,
        ) {
            perceptual = Some(
                serde_json::to_value(fingerprint)
                    .map_err(|e| ServerError::Internal(e.to_string()))?,
            );
        }

        // Process semantic
        if let Ok((_, embedding)) =
            ucfp::process_record_with_semantic_configs(raw, ingest_cfg, canonical_cfg, semantic_cfg)
        {
            semantic = Some(
                serde_json::to_value(embedding)
                    .map_err(|e| ServerError::Internal(e.to_string()))?,
            );
        }

        Ok((perceptual, semantic))
    }

    // Process both documents
    let (doc1_perceptual, doc1_semantic) = process_doc(
        &request.doc1,
        &ingest_cfg,
        &canonical_cfg,
        &perceptual_cfg,
        &semantic_cfg,
    )
    .await?;

    let (doc2_perceptual, doc2_semantic) = process_doc(
        &request.doc2,
        &ingest_cfg,
        &canonical_cfg,
        &perceptual_cfg,
        &semantic_cfg,
    )
    .await?;

    // Calculate perceptual similarity (Jaccard)
    let perceptual_similarity = if let (Some(p1), Some(p2)) = (&doc1_perceptual, &doc2_perceptual) {
        let fp1: Vec<u64> =
            serde_json::from_value(p1.get("minhash").cloned().unwrap_or(p1.clone()))
                .unwrap_or_default();
        let fp2: Vec<u64> =
            serde_json::from_value(p2.get("minhash").cloned().unwrap_or(p2.clone()))
                .unwrap_or_default();

        if !fp1.is_empty() && !fp2.is_empty() {
            let set1: std::collections::HashSet<_> = fp1.iter().cloned().collect();
            let set2: std::collections::HashSet<_> = fp2.iter().cloned().collect();

            let intersection = set1.intersection(&set2).count();
            let union = set1.union(&set2).count();

            if union > 0 {
                Some(intersection as f32 / union as f32)
            } else {
                Some(0.0)
            }
        } else {
            Some(0.0)
        }
    } else {
        None
    };

    // Calculate semantic similarity (cosine)
    let semantic_similarity = if let (Some(s1), Some(s2)) = (&doc1_semantic, &doc2_semantic) {
        let emb1: Vec<f32> =
            serde_json::from_value(s1.get("vector").cloned().unwrap_or(s1.clone()))
                .unwrap_or_default();
        let emb2: Vec<f32> =
            serde_json::from_value(s2.get("vector").cloned().unwrap_or(s2.clone()))
                .unwrap_or_default();

        if !emb1.is_empty() && !emb2.is_empty() && emb1.len() == emb2.len() {
            let dot: f32 = emb1.iter().zip(&emb2).map(|(a, b)| a * b).sum();
            let norm1: f32 = emb1.iter().map(|v| v * v).sum::<f32>().sqrt();
            let norm2: f32 = emb2.iter().map(|v| v * v).sum::<f32>().sqrt();

            if norm1 > 0.0 && norm2 > 0.0 {
                Some(dot / (norm1 * norm2))
            } else {
                Some(0.0)
            }
        } else {
            Some(0.0)
        }
    } else {
        None
    };

    // Calculate combined score (weighted average if both available)
    let similarity_score = match (perceptual_similarity, semantic_similarity) {
        (Some(p), Some(s)) => 0.3 * p + 0.7 * s, // Weight semantic higher
        (Some(p), None) => p,
        (None, Some(s)) => s,
        (None, None) => 0.0,
    };

    Ok(Json(CompareResponse {
        similarity_score,
        perceptual_similarity,
        semantic_similarity,
    }))
}
