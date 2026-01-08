use std::sync::Arc;
use std::time::Instant;

#[allow(unused_imports)]
use chrono::{NaiveDate, Utc};
use ucfp::{
    CanonicalizeConfig, IngestConfig, IngestMetadata, IngestPayload, IngestSource,
    PerceptualConfig, PerceptualFingerprint, RawIngestRecord, SemanticConfig, SemanticEmbedding,
    process_record_with_perceptual_configs, process_record_with_semantic_configs,
};
use ufp_index::{
    BackendConfig, INDEX_SCHEMA_VERSION, IndexConfig, IndexRecord, QueryMode, QueryResult, UfpIndex,
};

use crate::metrics::metrics_recorder;
use crate::types::{MatchConfig, MatchError, MatchHit, MatchMode, MatchRequest};

/// Trait for a matching engine.
pub trait Matcher: Send + Sync {
    /// Run a single match request and return ordered hits.
    fn match_document(&self, req: &MatchRequest) -> Result<Vec<MatchHit>, MatchError>;
}

/// Production-grade matcher implementation.
pub struct DefaultMatcher {
    index: Arc<UfpIndex>,
    ingest_cfg: IngestConfig,
    canonical_cfg: CanonicalizeConfig,
    perceptual_cfg: PerceptualConfig,
    semantic_cfg: SemanticConfig,
}

impl DefaultMatcher {
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
        let (_doc, embedding) = process_record_with_semantic_configs(
            raw,
            &self.ingest_cfg,
            &self.canonical_cfg,
            &self.semantic_cfg,
        )?;

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
        let (_doc, fingerprint) = process_record_with_perceptual_configs(
            raw,
            &self.ingest_cfg,
            &self.canonical_cfg,
            &self.perceptual_cfg,
        )?;

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

    fn combine_scores(mode: &MatchMode, semantic: Option<f32>, perceptual: Option<f32>) -> f32 {
        match mode {
            MatchMode::Semantic => semantic.unwrap_or(0.0),
            MatchMode::Perceptual => perceptual.unwrap_or(0.0),
            MatchMode::Hybrid { semantic_weight } => {
                let alpha = semantic_weight.clamp(0.0, 1.0);
                let s = semantic.unwrap_or(0.0);
                let p = perceptual.unwrap_or(0.0);
                alpha * s + (1.0 - alpha) * p
            }
        }
    }

    fn postprocess_hits(
        &self,
        req: &MatchRequest,
        mut hits_semantic: Option<Vec<QueryResult>>,
        mut hits_perceptual: Option<Vec<QueryResult>>,
    ) -> Vec<MatchHit> {
        let mut hits = Vec::new();

        match req.config.mode {
            MatchMode::Semantic | MatchMode::Perceptual => {
                let is_semantic = matches!(req.config.mode, MatchMode::Semantic);
                let results = if is_semantic {
                    hits_semantic.take()
                } else {
                    hits_perceptual.take()
                };

                if let Some(results) = results {
                    for r in results.into_iter() {
                        if req.config.tenant_enforce
                            && r.metadata.get("tenant")
                                != Some(&serde_json::Value::String(req.tenant_id.clone()))
                        {
                            continue;
                        }

                        let (semantic_score, perceptual_score) = if is_semantic {
                            (Some(r.score), None)
                        } else {
                            (None, Some(r.score))
                        };

                        let score = Self::combine_scores(
                            &req.config.mode,
                            semantic_score,
                            perceptual_score,
                        );
                        if score < req.config.min_score {
                            continue;
                        }

                        hits.push(MatchHit {
                            canonical_hash: r.canonical_hash,
                            score,
                            semantic_score: if req.config.explain {
                                semantic_score
                            } else {
                                None
                            },
                            perceptual_score: if req.config.explain {
                                perceptual_score
                            } else {
                                None
                            },
                            metadata: r.metadata,
                        });
                    }
                }
            }
            MatchMode::Hybrid { .. } => {
                use std::collections::HashMap;

                let mut map: HashMap<String, (QueryResult, Option<QueryResult>)> = HashMap::new();

                if let Some(results) = hits_semantic.take() {
                    for r in results.into_iter() {
                        if req.config.tenant_enforce
                            && r.metadata.get("tenant")
                                != Some(&serde_json::Value::String(req.tenant_id.clone()))
                        {
                            continue;
                        }
                        map.entry(r.canonical_hash.clone())
                            .or_insert_with(|| (r, None));
                    }
                }

                if let Some(results) = hits_perceptual.take() {
                    for r in results.into_iter() {
                        if req.config.tenant_enforce
                            && r.metadata.get("tenant")
                                != Some(&serde_json::Value::String(req.tenant_id.clone()))
                        {
                            continue;
                        }
                        map.entry(r.canonical_hash.clone())
                            .and_modify(|entry| entry.1 = Some(r.clone()))
                            .or_insert_with(|| (r, None));
                    }
                }

                for (hash, (semantic_res, perceptual_res)) in map.into_iter() {
                    let (semantic_score, perceptual_score, metadata) =
                        match (semantic_res, perceptual_res) {
                            (s, None) => (Some(s.score), None, s.metadata),
                            (s, Some(p)) => (Some(s.score), Some(p.score), s.metadata),
                        };
                    let score =
                        Self::combine_scores(&req.config.mode, semantic_score, perceptual_score);
                    if score < req.config.min_score {
                        continue;
                    }
                    hits.push(MatchHit {
                        canonical_hash: hash,
                        score,
                        semantic_score: if req.config.explain {
                            semantic_score
                        } else {
                            None
                        },
                        perceptual_score: if req.config.explain {
                            perceptual_score
                        } else {
                            None
                        },
                        metadata,
                    });
                }
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
}

impl Matcher for DefaultMatcher {
    fn match_document(&self, req: &MatchRequest) -> Result<Vec<MatchHit>, MatchError> {
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

        match req.config.mode {
            MatchMode::Semantic => {
                let (query_record, _emb) =
                    self.make_query_record_semantic(&req.tenant_id, &req.query_text, &req.config)?;
                let results = self
                    .index
                    .search(&query_record, QueryMode::Semantic, top_k)?;
                hits_semantic = Some(results);
            }
            MatchMode::Perceptual => {
                let (query_record, _fp) = self.make_query_record_perceptual(
                    &req.tenant_id,
                    &req.query_text,
                    &req.config,
                )?;
                let results = self
                    .index
                    .search(&query_record, QueryMode::Perceptual, top_k)?;
                hits_perceptual = Some(results);
            }
            MatchMode::Hybrid { .. } => {
                let (query_record_sem, _emb) =
                    self.make_query_record_semantic(&req.tenant_id, &req.query_text, &req.config)?;
                let (query_record_perc, _fp) = self.make_query_record_perceptual(
                    &req.tenant_id,
                    &req.query_text,
                    &req.config,
                )?;
                let results_sem =
                    self.index
                        .search(&query_record_sem, QueryMode::Semantic, top_k)?;
                let results_perc =
                    self.index
                        .search(&query_record_perc, QueryMode::Perceptual, top_k)?;
                hits_semantic = Some(results_sem);
                hits_perceptual = Some(results_perc);
            }
        }

        let hits = self.postprocess_hits(req, hits_semantic, hits_perceptual);
        let latency = start.elapsed();

        if let Some(recorder) = metrics_recorder() {
            recorder.record_match(&req.tenant_id, &req.config.mode, latency, hits.len());
        }

        Ok(hits)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, RwLock};
    use std::time::Duration;

    use serde_json::json;

    use crate::metrics::MatchMetrics;
    use crate::metrics::set_match_metrics;
    fn demo_timestamp() -> chrono::DateTime<Utc> {
        let Some(date) = NaiveDate::from_ymd_opt(2025, 1, 1) else {
            panic!("invalid demo date components");
        };
        let Some(date_time) = date.and_hms_opt(0, 0, 0) else {
            panic!("invalid demo time components");
        };
        chrono::DateTime::<Utc>::from_naive_utc_and_offset(date_time, Utc)
    }

    fn base_record(tenant: &str, doc_id: &str, text: &str) -> RawIngestRecord {
        RawIngestRecord {
            id: format!("ingest-{doc_id}"),
            source: IngestSource::RawText,
            metadata: IngestMetadata {
                tenant_id: Some(tenant.to_string()),
                doc_id: Some(doc_id.to_string()),
                received_at: Some(demo_timestamp()),
                original_source: None,
                attributes: None,
            },
            payload: Some(IngestPayload::Text(text.to_string())),
        }
    }

    fn build_index_with_docs() -> Result<(DefaultMatcher, String, String), MatchError> {
        let ingest_cfg = IngestConfig::default();
        let canonical_cfg = CanonicalizeConfig::default();
        // Use a smaller k so that short demo texts can still produce perceptual fingerprints.
        let perceptual_cfg = PerceptualConfig {
            k: 3,
            ..Default::default()
        };
        let semantic_cfg = SemanticConfig {
            mode: "fast".into(),
            tier: "fast".into(),
            ..Default::default()
        };

        // Populate the index with two simple docs for a single tenant.
        let tenant = "tenant-a";
        let doc_a = base_record(
            tenant,
            "doc-alpha",
            "Rust gives you memory safety without garbage collection.",
        );
        let doc_b = base_record(
            tenant,
            "doc-bravo",
            "The borrow checker enforces aliasing rules so data races are compile-time errors.",
        );

        let (doc_a_can, fp_a) = process_record_with_perceptual_configs(
            doc_a,
            &ingest_cfg,
            &canonical_cfg,
            &perceptual_cfg,
        )?;
        let emb_a = process_record_with_semantic_configs(
            base_record(
                tenant,
                "doc-alpha",
                "Rust gives you memory safety without garbage collection.",
            ),
            &ingest_cfg,
            &canonical_cfg,
            &semantic_cfg,
        )?
        .1;

        let (doc_b_can, fp_b) = process_record_with_perceptual_configs(
            doc_b,
            &ingest_cfg,
            &canonical_cfg,
            &perceptual_cfg,
        )?;
        let emb_b = process_record_with_semantic_configs(
            base_record(
                tenant,
                "doc-bravo",
                "The borrow checker enforces aliasing rules so data races are compile-time errors.",
            ),
            &ingest_cfg,
            &canonical_cfg,
            &semantic_cfg,
        )?
        .1;

        let cfg = IndexConfig::new().with_backend(BackendConfig::in_memory());
        let index = UfpIndex::new(cfg.clone()).expect("in-memory index");

        let scale = cfg.quantization.scale();
        let qa: Vec<i8> = emb_a
            .vector
            .iter()
            .map(|v| (v * scale).clamp(-128.0, 127.0) as i8)
            .collect();
        let qb: Vec<i8> = emb_b
            .vector
            .iter()
            .map(|v| (v * scale).clamp(-128.0, 127.0) as i8)
            .collect();

        let rec_a = IndexRecord {
            schema_version: INDEX_SCHEMA_VERSION,
            canonical_hash: doc_a_can.sha256_hex.clone(),
            perceptual: Some(fp_a.minhash.clone()),
            embedding: Some(qa),
            metadata: json!({
                "tenant": tenant,
                "doc_id": "doc-alpha",
            }),
        };
        let rec_b = IndexRecord {
            schema_version: INDEX_SCHEMA_VERSION,
            canonical_hash: doc_b_can.sha256_hex.clone(),
            perceptual: Some(fp_b.minhash.clone()),
            embedding: Some(qb),
            metadata: json!({
                "tenant": tenant,
                "doc_id": "doc-bravo",
            }),
        };

        index.upsert(&rec_a).expect("upsert a");
        index.upsert(&rec_b).expect("upsert b");

        let matcher = DefaultMatcher::new(
            index,
            ingest_cfg,
            canonical_cfg,
            perceptual_cfg,
            semantic_cfg,
        );

        Ok((matcher, doc_a_can.sha256_hex, doc_b_can.sha256_hex))
    }

    #[test]
    fn semantic_match_returns_results() -> Result<(), MatchError> {
        let (matcher, _hash_a, _hash_b) = build_index_with_docs()?;

        let req = MatchRequest {
            tenant_id: "tenant-a".into(),
            query_text: "Rust and memory safety".into(),
            config: MatchConfig {
                mode: MatchMode::Semantic,
                max_results: 5,
                min_score: 0.0,
                tenant_enforce: true,
                oversample_factor: 2.0,
                explain: true,
            },
            attributes: None,
        };

        let hits = matcher.match_document(&req)?;
        assert!(!hits.is_empty());
        assert!(hits[0].semantic_score.is_some());
        Ok(())
    }

    #[test]
    fn tenant_isolation_enforced() -> Result<(), MatchError> {
        let (matcher, _hash_a, _hash_b) = build_index_with_docs()?;

        let req = MatchRequest {
            tenant_id: "other-tenant".into(),
            query_text: "Rust and memory safety".into(),
            config: MatchConfig {
                mode: MatchMode::Semantic,
                max_results: 5,
                min_score: 0.0,
                tenant_enforce: true,
                oversample_factor: 2.0,
                explain: true,
            },
            attributes: None,
        };

        let hits = matcher.match_document(&req)?;
        assert!(hits.is_empty());
        Ok(())
    }

    struct RecordingMetrics {
        events: Arc<RwLock<Vec<(String, MatchMode, usize)>>>,
    }

    impl RecordingMetrics {
        fn new() -> Self {
            Self {
                events: Arc::new(RwLock::new(Vec::new())),
            }
        }

        fn snapshot(&self) -> Vec<(String, MatchMode, usize)> {
            self.events.read().unwrap().clone()
        }
    }

    impl MatchMetrics for RecordingMetrics {
        fn record_match(
            &self,
            tenant_id: &str,
            mode: &MatchMode,
            _latency: Duration,
            hit_count: usize,
        ) {
            self.events
                .write()
                .unwrap()
                .push((tenant_id.to_string(), *mode, hit_count));
        }
    }

    #[test]
    fn metrics_recorder_observes_matches() -> Result<(), MatchError> {
        let (matcher, _hash_a, _hash_b) = build_index_with_docs()?;
        let metrics = Arc::new(RecordingMetrics::new());
        set_match_metrics(Some(metrics.clone()));

        let req = MatchRequest {
            tenant_id: "tenant-a".into(),
            query_text: "Rust and memory safety".into(),
            config: MatchConfig::default(),
            attributes: None,
        };

        let hits = matcher.match_document(&req)?;
        assert!(!hits.is_empty());

        let events = metrics.snapshot();
        // We expect at least one metrics event for the match; implementations
        // may emit additional observations, so assert on a lower bound.
        assert!(!events.is_empty());
        assert!(events.iter().any(|(tenant, _, _)| tenant == "tenant-a"));

        set_match_metrics(None);
        Ok(())
    }
}
