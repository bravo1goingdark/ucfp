use std::error::Error;

use serde_json::json;
use ucfp::{
    CanonicalizeConfig, IngestConfig, PerceptualConfig, RawIngestRecord, SemanticConfig,
    process_record_with_perceptual_configs, process_record_with_semantic_configs,
};
use ufp_index::{BackendConfig, INDEX_SCHEMA_VERSION, IndexConfig, IndexRecord, UfpIndex};
use ufp_match::{
    DefaultMatcher, MatchConfig, MatchExpr, MatchHit, MatchMode, MatchRequest, Matcher,
    demo_utils::{demo_base_record, quantize_with_scale},
};

fn base_record(doc_id: &str, text: &str) -> RawIngestRecord {
    demo_base_record(doc_id, text, "examples/match_demo.rs")
}

fn main() -> Result<(), Box<dyn Error>> {
    // Use deterministic configs: in-memory index, default perceptual seed, and
    // the "fast" semantic tier which produces stable stub embeddings.
    let ingest_cfg = IngestConfig::default();
    let canonical_cfg = CanonicalizeConfig::default();
    // Use a smaller k so that short demo texts still produce perceptual fingerprints.
    let perceptual_cfg = PerceptualConfig {
        k: 3,
        ..Default::default()
    };
    let semantic_cfg = SemanticConfig {
        mode: "fast".into(),
        tier: "fast".into(),
        ..Default::default()
    };

    // Two simple documents for the same tenant.
    let raw_a = base_record(
        "doc-alpha",
        "Rust gives you memory safety without garbage collection.",
    );
    let raw_b = base_record(
        "doc-bravo",
        "The borrow checker enforces aliasing rules so data races are compile-time errors.",
    );

    let (doc_a, fp_a) = process_record_with_perceptual_configs(
        raw_a.clone(),
        &ingest_cfg,
        &canonical_cfg,
        &perceptual_cfg,
    )?;
    let (_, emb_a) =
        process_record_with_semantic_configs(raw_a, &ingest_cfg, &canonical_cfg, &semantic_cfg)?;

    let (doc_b, fp_b) = process_record_with_perceptual_configs(
        raw_b.clone(),
        &ingest_cfg,
        &canonical_cfg,
        &perceptual_cfg,
    )?;
    let (_, emb_b) =
        process_record_with_semantic_configs(raw_b, &ingest_cfg, &canonical_cfg, &semantic_cfg)?;

    // Build an in-memory index and upsert deterministic records.
    let index_cfg = IndexConfig::new().with_backend(BackendConfig::in_memory());
    let index = UfpIndex::new(index_cfg.clone())?;
    let scale = index_cfg.quantization.scale();

    let rec_a = IndexRecord {
        schema_version: INDEX_SCHEMA_VERSION,
        canonical_hash: doc_a.sha256_hex.clone(),
        perceptual: Some(fp_a.minhash.clone()),
        embedding: Some(quantize_with_scale(&emb_a, scale)),
        metadata: json!({
            "tenant": "tenant-a",
            "doc_id": "doc-alpha",
        }),
    };
    let rec_b = IndexRecord {
        schema_version: INDEX_SCHEMA_VERSION,
        canonical_hash: doc_b.sha256_hex.clone(),
        perceptual: Some(fp_b.minhash.clone()),
        embedding: Some(quantize_with_scale(&emb_b, scale)),
        metadata: json!({
            "tenant": "tenant-a",
            "doc_id": "doc-bravo",
        }),
    };

    index.upsert(&rec_a)?;
    index.upsert(&rec_b)?;

    // Wire the index into a DefaultMatcher.
    let matcher = DefaultMatcher::new(
        index,
        ingest_cfg,
        canonical_cfg,
        perceptual_cfg,
        semantic_cfg,
    );

    // Issue a semantic query. The scoring model is deterministic for a given
    // config + index state, so repeated runs produce the same ordering.
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
                metric: ufp_match::types::MetricId::Cosine,
                min_score: 0.0,
            },
            ..Default::default()
        },
        attributes: None,
        pipeline_version: None,
        fingerprint_versions: None,
        query_canonical_hash: None,
    };

    let hits: Vec<MatchHit> = matcher.match_document(&req)?;

    println!("query: {}", req.query_text);
    for (idx, hit) in hits.iter().enumerate() {
        println!(
            "#{} hash={} score={:.4} semantic={:?} metadata={}",
            idx, hit.canonical_hash, hit.score, hit.semantic_score, hit.metadata,
        );
    }

    Ok(())
}
