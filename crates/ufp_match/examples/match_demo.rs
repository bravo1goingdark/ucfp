use std::error::Error;

use chrono::{NaiveDate, Utc};
use serde_json::json;
use ucfp::{
    CanonicalizeConfig, IngestConfig, IngestMetadata, IngestPayload, IngestSource,
    PerceptualConfig, RawIngestRecord, SemanticConfig, SemanticEmbedding,
    process_record_with_perceptual_configs, process_record_with_semantic_configs,
};
use ufp_index::{BackendConfig, INDEX_SCHEMA_VERSION, IndexConfig, IndexRecord, UfpIndex};
use ufp_match::{DefaultMatcher, MatchConfig, MatchHit, MatchMode, MatchRequest, Matcher};

fn demo_timestamp() -> chrono::DateTime<Utc> {
    // Fixed wall-clock timestamp so the example is fully deterministic.
    let Some(date) = NaiveDate::from_ymd_opt(2025, 1, 1) else {
        panic!("invalid demo date components");
    };
    let Some(date_time) = date.and_hms_opt(0, 0, 0) else {
        panic!("invalid demo time components");
    };
    chrono::DateTime::<Utc>::from_naive_utc_and_offset(date_time, Utc)
}

fn base_record(doc_id: &str, text: &str) -> RawIngestRecord {
    RawIngestRecord {
        id: format!("demo-{doc_id}"),
        source: IngestSource::RawText,
        metadata: IngestMetadata {
            tenant_id: Some("tenant-a".into()),
            doc_id: Some(doc_id.into()),
            received_at: Some(demo_timestamp()),
            original_source: Some("examples/match_demo.rs".into()),
            attributes: None,
        },
        payload: Some(IngestPayload::Text(text.into())),
    }
}

fn quantize(embedding: &SemanticEmbedding, scale: f32) -> Vec<i8> {
    embedding
        .vector
        .iter()
        .map(|v| (v * scale).clamp(-128.0, 127.0) as i8)
        .collect()
}

fn main() -> Result<(), Box<dyn Error>> {
    // Use deterministic configs: in-memory index, default perceptual seed, and
    // the "fast" semantic tier which produces stable stub embeddings.
    let ingest_cfg = IngestConfig::default();
    let canonical_cfg = CanonicalizeConfig::default();
    let perceptual_cfg = PerceptualConfig::default();
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
        embedding: Some(quantize(&emb_a, scale)),
        metadata: json!({
            "tenant": "tenant-a",
            "doc_id": "doc-alpha",
        }),
    };
    let rec_b = IndexRecord {
        schema_version: INDEX_SCHEMA_VERSION,
        canonical_hash: doc_b.sha256_hex.clone(),
        perceptual: Some(fp_b.minhash.clone()),
        embedding: Some(quantize(&emb_b, scale)),
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
            min_score: 0.0,
            tenant_enforce: true,
            oversample_factor: 2.0,
            explain: true,
        },
        attributes: None,
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
