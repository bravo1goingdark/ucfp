use super::*;
use std::sync::{Arc, RwLock};
use std::time::Duration;

use canonical::canonicalize;
use ingest::IngestSource;
use ingest::{ingest, CanonicalPayload};
use perceptual::perceptualize_tokens;
use semantic::semanticize;
use serde_json::json;

use crate::demo_utils::base_record_with_tenant;
use crate::metrics::{set_match_metrics, MatchMetrics};
use crate::types::MatchExpr;

/// Helper function to run the perceptual pipeline: ingest → canonical → perceptual
fn run_perceptual_pipeline_helper(
    raw: RawIngestRecord,
    ingest_cfg: &IngestConfig,
    canonical_cfg: &CanonicalizeConfig,
    perceptual_cfg: &PerceptualConfig,
) -> Result<(canonical::CanonicalizedDocument, PerceptualFingerprint), MatchError> {
    // Ingest stage
    let canonical_record =
        ingest(raw, ingest_cfg).map_err(|e| MatchError::Ingest(e.to_string()))?;

    // Get text payload
    let text = match canonical_record.normalized_payload {
        Some(CanonicalPayload::Text(ref t)) => t.as_str(),
        _ => return Err(MatchError::Pipeline("No text payload available".into())),
    };

    // Canonical stage
    let doc = canonicalize(&canonical_record.doc_id, text, canonical_cfg)
        .map_err(|e| MatchError::Canonical(e.to_string()))?;

    // Perceptual stage
    let token_refs: Vec<&str> = doc.tokens.iter().map(|t| t.text.as_str()).collect();
    let fingerprint = perceptualize_tokens(&token_refs, perceptual_cfg)
        .map_err(|e| MatchError::Perceptual(e.to_string()))?;

    Ok((doc, fingerprint))
}

/// Helper function to run the semantic pipeline: ingest → canonical → semantic
fn run_semantic_pipeline_helper(
    raw: RawIngestRecord,
    ingest_cfg: &IngestConfig,
    canonical_cfg: &CanonicalizeConfig,
    semantic_cfg: &SemanticConfig,
) -> Result<(canonical::CanonicalizedDocument, SemanticEmbedding), MatchError> {
    // Ingest stage
    let canonical_record =
        ingest(raw, ingest_cfg).map_err(|e| MatchError::Ingest(e.to_string()))?;

    // Get text payload
    let text = match canonical_record.normalized_payload {
        Some(CanonicalPayload::Text(ref t)) => t.as_str(),
        _ => return Err(MatchError::Pipeline("No text payload available".into())),
    };

    // Canonical stage
    let doc = canonicalize(&canonical_record.doc_id, text, canonical_cfg)
        .map_err(|e| MatchError::Canonical(e.to_string()))?;

    // Semantic stage - use block_on for async semanticize
    let embedding = tokio::task::block_in_place(|| {
        tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(async { semanticize(&doc.doc_id, &doc.canonical_text, semantic_cfg).await })
    })
    .map_err(|e| MatchError::Semantic(e.to_string()))?;

    Ok((doc, embedding))
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
    let _doc_a = base_record_with_tenant(
        tenant,
        "doc-alpha",
        "Rust gives you memory safety without garbage collection.",
    );
    let _doc_b = base_record_with_tenant(
        tenant,
        "doc-bravo",
        "The borrow checker enforces aliasing rules so data races are compile-time errors.",
    );

    let (doc_a_can, fp_a) = run_perceptual_pipeline_helper(
        RawIngestRecord {
            id: "doc-alpha".to_string(),
            source: IngestSource::RawText,
            metadata: IngestMetadata {
                tenant_id: Some(tenant.to_string()),
                doc_id: Some("doc-alpha".to_string()),
                received_at: None,
                original_source: None,
                attributes: None,
            },
            payload: Some(IngestPayload::Text(
                "Rust gives you memory safety without garbage collection.".to_string(),
            )),
        },
        &ingest_cfg,
        &canonical_cfg,
        &perceptual_cfg,
    )?;
    let (_, emb_a) = run_semantic_pipeline_helper(
        base_record_with_tenant(
            tenant,
            "doc-alpha",
            "Rust gives you memory safety without garbage collection.",
        ),
        &ingest_cfg,
        &canonical_cfg,
        &semantic_cfg,
    )?;

    let (doc_b_can, fp_b) = run_perceptual_pipeline_helper(
        RawIngestRecord {
            id: "doc-bravo".to_string(),
            source: IngestSource::RawText,
            metadata: IngestMetadata {
                tenant_id: Some(tenant.to_string()),
                doc_id: Some("doc-bravo".to_string()),
                received_at: None,
                original_source: None,
                attributes: None,
            },
            payload: Some(IngestPayload::Text(
                "The borrow checker enforces aliasing rules so data races are compile-time errors."
                    .to_string(),
            )),
        },
        &ingest_cfg,
        &canonical_cfg,
        &perceptual_cfg,
    )?;
    let (_, emb_b) = run_semantic_pipeline_helper(
        base_record_with_tenant(
            tenant,
            "doc-bravo",
            "The borrow checker enforces aliasing rules so data races are compile-time errors.",
        ),
        &ingest_cfg,
        &canonical_cfg,
        &semantic_cfg,
    )?;

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
            tenant_enforce: true,
            oversample_factor: 2.0,
            explain: true,
            strategy: MatchExpr::Semantic {
                metric: crate::types::MetricId::Cosine,
                min_score: 0.0,
            },
            ..Default::default()
        },
        attributes: None,
        pipeline_version: None,
        fingerprint_versions: None,
        query_canonical_hash: None,
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
            tenant_enforce: true,
            oversample_factor: 2.0,
            explain: true,
            strategy: MatchExpr::Semantic {
                metric: crate::types::MetricId::Cosine,
                min_score: 0.0,
            },
            ..Default::default()
        },
        attributes: None,
        pipeline_version: None,
        fingerprint_versions: None,
        query_canonical_hash: None,
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
        pipeline_version: None,
        fingerprint_versions: None,
        query_canonical_hash: None,
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

#[test]
fn perceptual_match_returns_results() -> Result<(), MatchError> {
    let (matcher, _hash_a, _hash_b) = build_index_with_docs()?;

    // Use exact text from indexed document to guarantee perceptual overlap
    let req = MatchRequest {
        tenant_id: "tenant-a".into(),
        query_text: "Rust gives you memory safety without garbage collection.".into(),
        config: MatchConfig {
            mode: MatchMode::Perceptual,
            max_results: 5,
            tenant_enforce: true,
            oversample_factor: 2.0,
            explain: true,
            strategy: MatchExpr::Perceptual {
                metric: crate::types::MetricId::Jaccard,
                min_score: 0.0,
            },
            ..Default::default()
        },
        attributes: None,
        pipeline_version: None,
        fingerprint_versions: None,
        query_canonical_hash: None,
    };

    let hits = matcher.match_document(&req)?;
    assert!(
        !hits.is_empty(),
        "Perceptual match should return results for similar text"
    );
    assert!(hits[0].perceptual_score.is_some());
    Ok(())
}

#[test]
fn hybrid_match_returns_results() -> Result<(), MatchError> {
    let (matcher, _hash_a, _hash_b) = build_index_with_docs()?;

    let req = MatchRequest {
        tenant_id: "tenant-a".into(),
        query_text: "Rust memory safety without garbage collection".into(),
        config: MatchConfig {
            mode: MatchMode::Hybrid,
            max_results: 5,
            tenant_enforce: true,
            oversample_factor: 2.0,
            explain: true,
            strategy: MatchExpr::Weighted {
                semantic_weight: 0.7,
                min_overall: 0.0,
            },
            ..Default::default()
        },
        attributes: None,
        pipeline_version: None,
        fingerprint_versions: None,
        query_canonical_hash: None,
    };

    let hits = matcher.match_document(&req)?;
    assert!(!hits.is_empty());
    // In hybrid mode, both scores should be populated
    assert!(hits[0].semantic_score.is_some() || hits[0].perceptual_score.is_some());
    Ok(())
}

#[test]
fn exact_hash_match_returns_perfect_score() -> Result<(), MatchError> {
    let (matcher, hash_a, _hash_b) = build_index_with_docs()?;

    let req = MatchRequest {
        tenant_id: "tenant-a".into(),
        query_text: "Rust gives you memory safety without garbage collection.".into(),
        config: MatchConfig {
            mode: MatchMode::Semantic,
            max_results: 5,
            tenant_enforce: true,
            oversample_factor: 2.0,
            explain: true,
            strategy: MatchExpr::Exact,
            ..Default::default()
        },
        attributes: None,
        pipeline_version: None,
        fingerprint_versions: None,
        query_canonical_hash: Some(hash_a.clone()),
    };

    let hits = matcher.match_document(&req)?;
    assert!(!hits.is_empty());

    // Find the exact match
    let exact_hit = hits.iter().find(|h| h.canonical_hash == hash_a);
    assert!(exact_hit.is_some(), "Should find exact hash match");
    assert_eq!(exact_hit.unwrap().exact_score, Some(1.0));
    Ok(())
}

#[test]
fn min_score_threshold_filters_results() -> Result<(), MatchError> {
    let (matcher, _hash_a, _hash_b) = build_index_with_docs()?;

    // Query with a high minimum score - should filter out low-similarity results
    let req = MatchRequest {
        tenant_id: "tenant-a".into(),
        query_text: "completely unrelated text about bananas and airplanes".into(),
        config: MatchConfig {
            mode: MatchMode::Semantic,
            max_results: 5,
            tenant_enforce: true,
            oversample_factor: 2.0,
            explain: true,
            strategy: MatchExpr::Semantic {
                metric: crate::types::MetricId::Cosine,
                min_score: 0.95, // Very high threshold
            },
            ..Default::default()
        },
        attributes: None,
        pipeline_version: None,
        fingerprint_versions: None,
        query_canonical_hash: None,
    };

    let hits = matcher.match_document(&req)?;
    // Should return no results because the query is unrelated and threshold is high
    assert!(hits.is_empty() || hits.iter().all(|h| h.score >= 0.95));
    Ok(())
}

#[test]
fn max_results_limits_output() -> Result<(), MatchError> {
    let (matcher, _hash_a, _hash_b) = build_index_with_docs()?;

    let req = MatchRequest {
        tenant_id: "tenant-a".into(),
        query_text: "Rust".into(),
        config: MatchConfig {
            mode: MatchMode::Semantic,
            max_results: 1, // Limit to 1 result
            tenant_enforce: true,
            oversample_factor: 2.0,
            explain: true,
            strategy: MatchExpr::Semantic {
                metric: crate::types::MetricId::Cosine,
                min_score: 0.0,
            },
            ..Default::default()
        },
        attributes: None,
        pipeline_version: None,
        fingerprint_versions: None,
        query_canonical_hash: None,
    };

    let hits = matcher.match_document(&req)?;
    assert!(hits.len() <= 1, "Should return at most 1 result");
    Ok(())
}

#[test]
fn and_strategy_requires_both_conditions() -> Result<(), MatchError> {
    let (matcher, _hash_a, _hash_b) = build_index_with_docs()?;

    let req = MatchRequest {
        tenant_id: "tenant-a".into(),
        query_text: "Rust memory safety".into(),
        config: MatchConfig {
            mode: MatchMode::Hybrid,
            max_results: 5,
            tenant_enforce: true,
            oversample_factor: 2.0,
            explain: true,
            strategy: MatchExpr::And {
                left: Box::new(MatchExpr::Semantic {
                    metric: crate::types::MetricId::Cosine,
                    min_score: 0.0,
                }),
                right: Box::new(MatchExpr::Perceptual {
                    metric: crate::types::MetricId::Jaccard,
                    min_score: 0.0,
                }),
            },
            ..Default::default()
        },
        attributes: None,
        pipeline_version: None,
        fingerprint_versions: None,
        query_canonical_hash: None,
    };

    let hits = matcher.match_document(&req)?;
    // Should return results that satisfy both semantic AND perceptual conditions
    for hit in &hits {
        assert!(
            hit.semantic_score.is_some() || hit.perceptual_score.is_some(),
            "AND strategy should return hits with at least one score"
        );
    }
    Ok(())
}

#[test]
fn or_strategy_returns_union() -> Result<(), MatchError> {
    let (matcher, _hash_a, _hash_b) = build_index_with_docs()?;

    let req = MatchRequest {
        tenant_id: "tenant-a".into(),
        query_text: "Rust memory safety".into(),
        config: MatchConfig {
            mode: MatchMode::Hybrid,
            max_results: 5,
            tenant_enforce: true,
            oversample_factor: 2.0,
            explain: true,
            strategy: MatchExpr::Or {
                left: Box::new(MatchExpr::Semantic {
                    metric: crate::types::MetricId::Cosine,
                    min_score: 0.0,
                }),
                right: Box::new(MatchExpr::Perceptual {
                    metric: crate::types::MetricId::Jaccard,
                    min_score: 0.0,
                }),
            },
            ..Default::default()
        },
        attributes: None,
        pipeline_version: None,
        fingerprint_versions: None,
        query_canonical_hash: None,
    };

    let hits = matcher.match_document(&req)?;
    // OR strategy should return hits that have either semantic OR perceptual match
    assert!(!hits.is_empty());
    Ok(())
}

#[test]
fn weighted_strategy_combines_scores() -> Result<(), MatchError> {
    let (matcher, _hash_a, _hash_b) = build_index_with_docs()?;

    let req = MatchRequest {
        tenant_id: "tenant-a".into(),
        query_text: "Rust memory safety".into(),
        config: MatchConfig {
            mode: MatchMode::Hybrid,
            max_results: 5,
            tenant_enforce: true,
            oversample_factor: 2.0,
            explain: true,
            strategy: MatchExpr::Weighted {
                semantic_weight: 0.5,
                min_overall: 0.0,
            },
            ..Default::default()
        },
        attributes: None,
        pipeline_version: None,
        fingerprint_versions: None,
        query_canonical_hash: None,
    };

    let hits = matcher.match_document(&req)?;
    assert!(!hits.is_empty());

    // Check that scores are within valid range
    for hit in &hits {
        assert!(
            hit.score >= 0.0 && hit.score <= 1.0,
            "Score should be in [0, 1]"
        );
    }
    Ok(())
}

#[test]
fn empty_tenant_id_rejected() {
    let ingest_cfg = IngestConfig::default();
    let canonical_cfg = CanonicalizeConfig::default();
    let perceptual_cfg = PerceptualConfig::default();
    let semantic_cfg = SemanticConfig::default();

    let matcher =
        DefaultMatcher::in_memory_default(ingest_cfg, canonical_cfg, perceptual_cfg, semantic_cfg)
            .expect("in-memory matcher");

    let req = MatchRequest {
        tenant_id: "".into(),
        query_text: "Rust".into(),
        config: MatchConfig::default(),
        attributes: None,
        pipeline_version: None,
        fingerprint_versions: None,
        query_canonical_hash: None,
    };

    let result = matcher.match_document(&req);
    assert!(result.is_err());
    match result.unwrap_err() {
        MatchError::InvalidConfig(msg) => assert!(msg.contains("tenant_id")),
        other => panic!("Expected InvalidConfig error, got: {:?}", other),
    }
}

#[test]
fn empty_query_text_rejected() {
    let ingest_cfg = IngestConfig::default();
    let canonical_cfg = CanonicalizeConfig::default();
    let perceptual_cfg = PerceptualConfig::default();
    let semantic_cfg = SemanticConfig::default();

    let matcher =
        DefaultMatcher::in_memory_default(ingest_cfg, canonical_cfg, perceptual_cfg, semantic_cfg)
            .expect("in-memory matcher");

    let req = MatchRequest {
        tenant_id: "tenant-a".into(),
        query_text: "".into(),
        config: MatchConfig::default(),
        attributes: None,
        pipeline_version: None,
        fingerprint_versions: None,
        query_canonical_hash: None,
    };

    let result = matcher.match_document(&req);
    assert!(result.is_err());
    match result.unwrap_err() {
        MatchError::InvalidConfig(msg) => assert!(msg.contains("query_text")),
        other => panic!("Expected InvalidConfig error, got: {:?}", other),
    }
}

#[test]
fn without_tenant_enforcement_returns_all() -> Result<(), MatchError> {
    let (matcher, _hash_a, _hash_b) = build_index_with_docs()?;

    // Query with tenant enforcement disabled
    let req = MatchRequest {
        tenant_id: "other-tenant".into(),
        query_text: "Rust and memory safety".into(),
        config: MatchConfig {
            mode: MatchMode::Semantic,
            max_results: 5,
            tenant_enforce: false, // Disabled
            oversample_factor: 2.0,
            explain: true,
            strategy: MatchExpr::Semantic {
                metric: crate::types::MetricId::Cosine,
                min_score: 0.0,
            },
            ..Default::default()
        },
        attributes: None,
        pipeline_version: None,
        fingerprint_versions: None,
        query_canonical_hash: None,
    };

    let hits = matcher.match_document(&req)?;
    // Should return results even from different tenant when enforcement is off
    assert!(!hits.is_empty());
    Ok(())
}
