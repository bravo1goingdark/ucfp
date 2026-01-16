use super::*;
use std::sync::{Arc, RwLock};
use std::time::Duration;

use serde_json::json;
use ucfp::IngestSource;

use crate::demo_utils::base_record_with_tenant;
use crate::metrics::{MatchMetrics, set_match_metrics};
use crate::types::MatchExpr;

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

    let (doc_a_can, fp_a) = process_record_with_perceptual_configs(
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
    let emb_a = process_record_with_semantic_configs(
        base_record_with_tenant(
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
    let emb_b = process_record_with_semantic_configs(
        base_record_with_tenant(
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
