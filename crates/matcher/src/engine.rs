use std::sync::Arc;
use std::time::Instant;

use canonical::{canonicalize, CanonicalizeConfig};
#[allow(unused_imports)]
use chrono::{NaiveDate, Utc};
use index::{
    BackendConfig, IndexConfig, IndexRecord, QueryMode, QueryResult, UfpIndex, INDEX_SCHEMA_VERSION,
};
use ingest::CanonicalPayload;
use ingest::{ingest, IngestConfig, IngestMetadata, IngestPayload, IngestSource, RawIngestRecord};
use perceptual::{perceptualize_tokens, PerceptualConfig, PerceptualFingerprint};
use semantic::{semanticize, SemanticConfig, SemanticEmbedding};

use crate::metrics::metrics_recorder;
use crate::types::{MatchConfig, MatchError, MatchExpr, MatchHit, MatchMode, MatchRequest};

#[cfg(test)]
mod tests;

/// Matcher for finding similar documents in the index.
pub struct Matcher {
    index: Arc<UfpIndex>,
    ingest_cfg: IngestConfig,
    canonical_cfg: CanonicalizeConfig,
    perceptual_cfg: PerceptualConfig,
    semantic_cfg: SemanticConfig,
}

impl Matcher {
    /// Construct a matcher from an existing index and explicit configs.
    pub fn new(
        index: UfpIndex,
        ingest_cfg: IngestConfig,
        canonical_cfg: CanonicalizeConfig,
        perceptual_cfg: PerceptualConfig,
        semantic_cfg: SemanticConfig,
    ) -> Self {
        Self::with_index_arc(
            Arc::new(index),
            ingest_cfg,
            canonical_cfg,
            perceptual_cfg,
            semantic_cfg,
        )
    }

    /// Construct a matcher from a shared index handle and explicit configs.
    pub fn with_index_arc(
        index: Arc<UfpIndex>,
        ingest_cfg: IngestConfig,
        canonical_cfg: CanonicalizeConfig,
        perceptual_cfg: PerceptualConfig,
        semantic_cfg: SemanticConfig,
    ) -> Self {
        Self {
            index,
            ingest_cfg,
            canonical_cfg,
            perceptual_cfg,
            semantic_cfg,
        }
    }

    /// Convenience helper to build an in-memory index for tests or ephemeral matching.
    pub fn in_memory_default(
        ingest_cfg: IngestConfig,
        canonical_cfg: CanonicalizeConfig,
        perceptual_cfg: PerceptualConfig,
        semantic_cfg: SemanticConfig,
    ) -> Result<Self, MatchError> {
        let cfg = IndexConfig::new().with_backend(BackendConfig::in_memory());
        let index = UfpIndex::new(cfg)?;
        Ok(Self::new(
            index,
            ingest_cfg,
            canonical_cfg,
            perceptual_cfg,
            semantic_cfg,
        ))
    }

    fn make_query_record_semantic(
        &self,
        tenant_id: &str,
        query_text: &str,
        _cfg: &MatchConfig,
    ) -> Result<(IndexRecord, Option<SemanticEmbedding>), MatchError> {
        let raw = self.build_raw_record(tenant_id, "query-semantic", query_text);
        let embedding = self.run_semantic_pipeline(&raw)?;

        let quantized = self.quantize_embedding(&embedding);
        let record = IndexRecord {
            schema_version: INDEX_SCHEMA_VERSION,
            canonical_hash: "query-semantic".into(),
            perceptual: None,
            embedding: Some(quantized),
            metadata: serde_json::json!({
                "tenant": tenant_id,
                "doc_id": "query-semantic",
                "kind": "query",
            }),
        };

        Ok((record, Some(embedding)))
    }

    fn make_query_record_perceptual(
        &self,
        tenant_id: &str,
        query_text: &str,
        _cfg: &MatchConfig,
    ) -> Result<(IndexRecord, Option<PerceptualFingerprint>), MatchError> {
        let raw = self.build_raw_record(tenant_id, "query-perceptual", query_text);
        let fingerprint = self.run_perceptual_pipeline(&raw)?;

        let record = IndexRecord {
            schema_version: INDEX_SCHEMA_VERSION,
            canonical_hash: "query-perceptual".into(),
            perceptual: Some(fingerprint.minhash.clone()),
            embedding: None,
            metadata: serde_json::json!({
                "tenant": tenant_id,
                "doc_id": "query-perceptual",
                "kind": "query",
            }),
        };

        Ok((record, Some(fingerprint)))
    }

    /// Run the full pipeline: ingest → canonical → semantic
    fn run_semantic_pipeline(
        &self,
        raw: &RawIngestRecord,
    ) -> Result<SemanticEmbedding, MatchError> {
        // Ingest stage
        let canonical_record =
            ingest(raw.clone(), &self.ingest_cfg).map_err(|e| MatchError::Ingest(e.to_string()))?;

        // Get text payload
        let text = match canonical_record.normalized_payload {
            Some(CanonicalPayload::Text(ref t)) => t.as_str(),
            _ => return Err(MatchError::Pipeline("No text payload available".into())),
        };

        // Canonical stage
        let doc = canonicalize(&canonical_record.doc_id, text, &self.canonical_cfg)
            .map_err(|e| MatchError::Canonical(e.to_string()))?;

        // Semantic stage - use existing runtime if available, otherwise create one
        let embedding = if let Ok(handle) = tokio::runtime::Handle::try_current() {
            tokio::task::block_in_place(|| {
                handle.block_on(async {
                    semanticize(&doc.doc_id, &doc.canonical_text, &self.semantic_cfg).await
                })
            })
        } else {
            tokio::runtime::Runtime::new().unwrap().block_on(async {
                semanticize(&doc.doc_id, &doc.canonical_text, &self.semantic_cfg).await
            })
        }
        .map_err(|e| MatchError::Semantic(e.to_string()))?;

        Ok(embedding)
    }

    /// Run the full pipeline: ingest → canonical → perceptual
    fn run_perceptual_pipeline(
        &self,
        raw: &RawIngestRecord,
    ) -> Result<PerceptualFingerprint, MatchError> {
        // Ingest stage
        let canonical_record =
            ingest(raw.clone(), &self.ingest_cfg).map_err(|e| MatchError::Ingest(e.to_string()))?;

        // Get text payload
        let text = match canonical_record.normalized_payload {
            Some(CanonicalPayload::Text(ref t)) => t.as_str(),
            _ => return Err(MatchError::Pipeline("No text payload available".into())),
        };

        // Canonical stage
        let doc = canonicalize(&canonical_record.doc_id, text, &self.canonical_cfg)
            .map_err(|e| MatchError::Canonical(e.to_string()))?;

        // Perceptual stage
        let token_refs: Vec<&str> = doc.tokens.iter().map(|t| t.text.as_str()).collect();
        let fingerprint = perceptualize_tokens(&token_refs, &self.perceptual_cfg)
            .map_err(|e| MatchError::Perceptual(e.to_string()))?;

        Ok(fingerprint)
    }

    fn build_raw_record(&self, tenant_id: &str, doc_id: &str, text: &str) -> RawIngestRecord {
        RawIngestRecord {
            id: format!("match-{doc_id}"),
            source: IngestSource::RawText,
            metadata: IngestMetadata {
                tenant_id: Some(tenant_id.to_string()),
                doc_id: Some(doc_id.to_string()),
                received_at: None,
                original_source: None,
                attributes: None,
            },
            payload: Some(IngestPayload::Text(text.to_string())),
        }
    }

    fn quantize_embedding(&self, embedding: &SemanticEmbedding) -> Vec<i8> {
        // For now, reuse a default IndexConfig's quantization scale. This keeps the
        // API stable until UfpIndex exposes its runtime config.
        let cfg = IndexConfig::new();
        let scale = cfg.quantization.scale();
        embedding
            .vector
            .iter()
            .map(|v| (v * scale).clamp(-128.0, 127.0) as i8)
            .collect()
    }

    fn postprocess_hits(
        &self,
        req: &MatchRequest,
        mut hits_semantic: Option<Vec<QueryResult>>,
        mut hits_perceptual: Option<Vec<QueryResult>>,
    ) -> Vec<MatchHit> {
        let mut hits = Vec::new();
        let mut map: std::collections::HashMap<String, (Option<QueryResult>, Option<QueryResult>)> =
            std::collections::HashMap::new();

        let tenant_value = serde_json::Value::String(req.tenant_id.clone());

        if let Some(results) = hits_semantic.take() {
            for r in results {
                if req.config.tenant_enforce && r.metadata.get("tenant") != Some(&tenant_value) {
                    continue;
                }
                let hash = r.canonical_hash.clone();
                map.entry(hash).or_insert((None, None)).0 = Some(r);
            }
        }

        if let Some(results) = hits_perceptual.take() {
            for r in results {
                if req.config.tenant_enforce && r.metadata.get("tenant") != Some(&tenant_value) {
                    continue;
                }
                let hash = r.canonical_hash.clone();
                map.entry(hash).or_insert((None, None)).1 = Some(r);
            }
        }

        for (hash, (semantic_res, perceptual_res)) in map {
            let semantic_score = semantic_res.as_ref().map(|r| r.score);
            let perceptual_score = perceptual_res.as_ref().map(|r| r.score);
            let metadata = semantic_res
                .as_ref()
                .or(perceptual_res.as_ref())
                .unwrap()
                .metadata
                .clone();

            let (score, exact_score) = self.calculate_final_score(
                &req.config.strategy,
                semantic_score,
                perceptual_score,
                &hash,
                req.query_canonical_hash.as_deref(),
            );

            if self.should_include(&req.config.strategy, score) {
                hits.push(MatchHit {
                    canonical_hash: hash,
                    score,
                    semantic_score,
                    perceptual_score,
                    exact_score,
                    metadata,
                    match_version: "v1".to_string(),
                    policy_id: req.config.policy_id.clone(),
                    policy_version: req.config.policy_version.clone(),
                    explanation: None,
                });
            }
        }

        hits.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        if hits.len() > req.config.max_results {
            hits.truncate(req.config.max_results);
        }
        hits
    }

    #[allow(clippy::only_used_in_recursion)]
    fn calculate_final_score(
        &self,
        strategy: &MatchExpr,
        semantic_score: Option<f32>,
        perceptual_score: Option<f32>,
        canonical_hash: &str,
        query_canonical_hash: Option<&str>,
    ) -> (f32, Option<f32>) {
        let exact_score =
            query_canonical_hash.map(|q_hash| if q_hash == canonical_hash { 1.0 } else { 0.0 });

        let score = match strategy {
            MatchExpr::Exact => exact_score.unwrap_or(0.0),
            MatchExpr::Semantic { .. } => semantic_score.unwrap_or(0.0),
            MatchExpr::Perceptual { .. } => perceptual_score.unwrap_or(0.0),
            MatchExpr::Weighted {
                semantic_weight, ..
            } => {
                let s = semantic_score.unwrap_or(0.0);
                let p = perceptual_score.unwrap_or(0.0);
                let alpha = semantic_weight.clamp(0.0, 1.0);
                alpha * s + (1.0 - alpha) * p
            }
            MatchExpr::And { left, right } => {
                let (left_score, _): (f32, _) = self.calculate_final_score(
                    left,
                    semantic_score,
                    perceptual_score,
                    canonical_hash,
                    query_canonical_hash,
                );
                let (right_score, _): (f32, _) = self.calculate_final_score(
                    right,
                    semantic_score,
                    perceptual_score,
                    canonical_hash,
                    query_canonical_hash,
                );
                left_score.min(right_score)
            }
            MatchExpr::Or { left, right } => {
                let (left_score, _): (f32, _) = self.calculate_final_score(
                    left,
                    semantic_score,
                    perceptual_score,
                    canonical_hash,
                    query_canonical_hash,
                );
                let (right_score, _): (f32, _) = self.calculate_final_score(
                    right,
                    semantic_score,
                    perceptual_score,
                    canonical_hash,
                    query_canonical_hash,
                );
                left_score.max(right_score)
            }
        };

        (score, exact_score)
    }

    #[allow(clippy::only_used_in_recursion)]
    fn should_include(&self, strategy: &MatchExpr, score: f32) -> bool {
        match strategy {
            MatchExpr::Exact => score >= 1.0,
            MatchExpr::Semantic { min_score, .. } => score >= *min_score,
            MatchExpr::Perceptual { min_score, .. } => score >= *min_score,
            MatchExpr::Weighted { min_overall, .. } => score >= *min_overall,
            MatchExpr::And { left, right } => {
                self.should_include(left, score) && self.should_include(right, score)
            }
            MatchExpr::Or { left, right } => {
                self.should_include(left, score) || self.should_include(right, score)
            }
        }
    }
}

impl Matcher {
    /// Run a single match request and return ordered hits.
    pub fn match_document(&self, req: &MatchRequest) -> Result<Vec<MatchHit>, MatchError> {
        if req.tenant_id.trim().is_empty() {
            return Err(MatchError::InvalidConfig(
                "tenant_id must not be empty".into(),
            ));
        }
        if req.query_text.trim().is_empty() {
            return Err(MatchError::InvalidConfig(
                "query_text must not be empty".into(),
            ));
        }
        req.config.validate()?;

        let start = Instant::now();
        let top_k =
            ((req.config.max_results as f32) * req.config.oversample_factor).ceil() as usize;

        let mut hits_semantic: Option<Vec<QueryResult>> = None;
        let mut hits_perceptual: Option<Vec<QueryResult>> = None;

        let needs_semantic =
            req.config.mode == MatchMode::Semantic || req.config.mode == MatchMode::Hybrid;
        let needs_perceptual =
            req.config.mode == MatchMode::Perceptual || req.config.mode == MatchMode::Hybrid;

        if needs_semantic {
            let (query_record, _emb) =
                self.make_query_record_semantic(&req.tenant_id, &req.query_text, &req.config)?;
            let results = self
                .index
                .search(&query_record, QueryMode::Semantic, top_k)?;
            hits_semantic = Some(results);
        }

        if needs_perceptual {
            let (query_record, _fp) =
                self.make_query_record_perceptual(&req.tenant_id, &req.query_text, &req.config)?;
            let results = self
                .index
                .search(&query_record, QueryMode::Perceptual, top_k)?;
            hits_perceptual = Some(results);
        }

        let hits = self.postprocess_hits(req, hits_semantic, hits_perceptual);
        let latency = start.elapsed();

        if let Some(recorder) = metrics_recorder() {
            recorder.record_match(&req.tenant_id, &req.config.mode, latency, hits.len());
        }

        Ok(hits)
    }
}
