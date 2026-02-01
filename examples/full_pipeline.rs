//! End-to-end pipeline demo using real perceptual + semantic stages and a RocksDB backend.
//! Requires the `models/bge-small-en-v1.5` assets checked into the repo.

use anyhow::Context;
use index::{
    BackendConfig, IndexConfig, IndexRecord, QueryMode, QueryResult, UfpIndex, INDEX_SCHEMA_VERSION,
};
use serde_json::json;
use std::fs;
use std::path::PathBuf;
use ucfp::{
    process_record_with_perceptual_configs, semanticize_document, CanonicalizeConfig, IngestConfig,
    IngestMetadata, IngestPayload, IngestSource, PerceptualConfig, RawIngestRecord, SemanticConfig,
};

fn main() -> anyhow::Result<()> {
    let ingest_cfg = IngestConfig::default();
    let canonical_cfg = CanonicalizeConfig::default();
    let perceptual_cfg = PerceptualConfig {
        k: 6,
        ..Default::default()
    };
    // Use the bundled ONNX model/tokenizer for actual embeddings.
    // For long documents (>512 tokens), enable chunking:
    //   enable_chunking: true,
    //   chunk_overlap_ratio: 0.5,
    //   pooling_strategy: "weighted_mean".into(),
    let semantic_cfg = SemanticConfig {
        tier: "balanced".into(),
        mode: "onnx".into(),
        model_name: "bge-small-en-v1.5".into(),
        model_path: PathBuf::from("models/bge-small-en-v1.5/onnx/model.onnx"),
        tokenizer_path: Some(PathBuf::from("models/bge-small-en-v1.5/tokenizer.json")),
        ..Default::default()
    };

    let db_path = PathBuf::from("target/full-pipeline-rocksdb");
    if db_path.exists() {
        fs::remove_dir_all(&db_path).context("removing previous RocksDB data")?;
    }
    let index_cfg = IndexConfig::new().with_backend(BackendConfig::rocksdb(
        db_path.to_string_lossy().to_string(),
    ));
    let index = UfpIndex::new(index_cfg.clone()).context("index init")?;

    let corpus = [
        (
            "tenant-a",
            "doc-alpha",
            "Rust gives you memory safety without garbage collection. Ownership \
             and borrowing feel strict at first, but they eliminate entire bug classes.",
        ),
        (
            "tenant-a",
            "doc-bravo",
            "The borrow checker enforces aliasing rules so data races are compile-time errors. \
             That enables fearless concurrency in services that need predictability.",
        ),
    ];

    for (tenant, doc_id, text) in corpus {
        let raw = build_raw_record(tenant, doc_id, text);
        upsert_record(
            &index,
            &index_cfg,
            raw,
            &ingest_cfg,
            &canonical_cfg,
            &perceptual_cfg,
            &semantic_cfg,
        )?;
    }

    let query_raw = build_raw_record(
        "tenant-a",
        "doc-query",
        "Rust's ownership model provides deterministic performance and eliminates \
         most memory safety bugs. It is ideal for low-latency services.",
    );
    let query_record = build_query_record(
        &index_cfg,
        query_raw,
        &ingest_cfg,
        &canonical_cfg,
        &perceptual_cfg,
        &semantic_cfg,
    )?;

    print_hits(
        "Semantic",
        index
            .search(&query_record, QueryMode::Semantic, 2)
            .context("semantic search")?,
    );
    print_hits(
        "Perceptual",
        index
            .search(&query_record, QueryMode::Perceptual, 2)
            .context("perceptual search")?,
    );
    index.flush().ok();

    Ok(())
}

fn build_raw_record(tenant: &str, doc_id: &str, text: &str) -> RawIngestRecord {
    RawIngestRecord {
        id: format!("ingest-{doc_id}"),
        source: IngestSource::RawText,
        metadata: IngestMetadata {
            tenant_id: Some(tenant.to_string()),
            doc_id: Some(doc_id.to_string()),
            received_at: None,
            original_source: None,
            attributes: None,
        },
        payload: Some(IngestPayload::Text(text.to_string())),
    }
}

fn upsert_record(
    index: &UfpIndex,
    index_cfg: &IndexConfig,
    raw: RawIngestRecord,
    ingest_cfg: &IngestConfig,
    canonical_cfg: &CanonicalizeConfig,
    perceptual_cfg: &PerceptualConfig,
    semantic_cfg: &SemanticConfig,
) -> anyhow::Result<()> {
    let tenant = raw
        .metadata
        .tenant_id
        .clone()
        .unwrap_or_else(|| "tenant-a".into());
    let doc_id = raw.metadata.doc_id.clone().unwrap_or_else(|| "doc".into());
    let (doc, fingerprint) =
        process_record_with_perceptual_configs(raw, ingest_cfg, canonical_cfg, perceptual_cfg)
            .context("perceptual pipeline")?;
    let embedding =
        semanticize_document(&doc, semantic_cfg).context("semantic embedding generation")?;

    let quantized = quantize_embedding(&embedding.vector, index_cfg.quantization.scale());
    let record = IndexRecord {
        schema_version: INDEX_SCHEMA_VERSION,
        canonical_hash: doc.sha256_hex.clone(),
        perceptual: Some(fingerprint.minhash.clone()),
        embedding: Some(quantized),
        metadata: json!({
            "tenant": tenant,
            "doc_id": doc_id,
            "model": embedding.model_name,
            "tier": embedding.tier,
        }),
    };
    index.upsert(&record).context("index upsert")
}

fn build_query_record(
    index_cfg: &IndexConfig,
    raw: RawIngestRecord,
    ingest_cfg: &IngestConfig,
    canonical_cfg: &CanonicalizeConfig,
    perceptual_cfg: &PerceptualConfig,
    semantic_cfg: &SemanticConfig,
) -> anyhow::Result<IndexRecord> {
    let tenant = raw
        .metadata
        .tenant_id
        .clone()
        .unwrap_or_else(|| "tenant-a".into());
    let doc_id = raw
        .metadata
        .doc_id
        .clone()
        .unwrap_or_else(|| "query".into());
    let (doc, fingerprint) =
        process_record_with_perceptual_configs(raw, ingest_cfg, canonical_cfg, perceptual_cfg)
            .context("query perceptual pipeline")?;
    let embedding = semanticize_document(&doc, semantic_cfg).context("query semantic embedding")?;
    let quantized = quantize_embedding(&embedding.vector, index_cfg.quantization.scale());
    Ok(IndexRecord {
        schema_version: INDEX_SCHEMA_VERSION,
        canonical_hash: "query".into(),
        perceptual: Some(fingerprint.minhash),
        embedding: Some(quantized),
        metadata: json!({
            "tenant": tenant,
            "doc_id": doc_id,
            "kind": "query"
        }),
    })
}

fn quantize_embedding(values: &[f32], scale: f32) -> Vec<i8> {
    values
        .iter()
        .map(|v| (v * scale).clamp(-128.0, 127.0) as i8)
        .collect()
}

fn print_hits(label: &str, hits: Vec<QueryResult>) {
    println!("\n{label} matches:");
    if hits.is_empty() {
        println!("  (no matches)");
        return;
    }
    for hit in hits {
        println!(
            "  • {} score={:.3} metadata={}",
            hit.canonical_hash, hit.score, hit.metadata
        );
    }
}
