//! End-to-end pipeline demo using real perceptual + semantic stages and a Redb backend.
//! Requires the `models/bge-small-en-v1.5` assets checked into the repo.

use anyhow::Context;
use chrono::Utc;
use index::{
    BackendConfig, IndexConfig, IndexRecord, QueryMode, QueryResult, UfpIndex, INDEX_SCHEMA_VERSION,
};
use ingest::{IngestConfig, IngestMetadata, IngestPayload, IngestSource, RawIngestRecord};
use ndarray::Array1;
use serde_json::json;
use std::path::PathBuf;
use ucfp::{
    process_pipeline, CanonicalizeConfig, PerceptualConfig, PipelineStageConfig, SemanticConfig,
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

    let db_path = PathBuf::from("target/full-pipeline.redb");
    if db_path.exists() {
        std::fs::remove_file(&db_path).context("removing previous Redb data")?;
    }
    let index_cfg =
        IndexConfig::new().with_backend(BackendConfig::redb(db_path.to_string_lossy().to_string()));
    let index = UfpIndex::new(index_cfg.clone()).context("index init")?;

    let corpus = [
        (
            "tenant-a",
            "doc-alpha",
            "Rust gives you memory safety without garbage collection. Ownership \
             and borrowing prevent data races at compile time.",
        ),
        (
            "tenant-a",
            "doc-beta",
            "Python offers rapid development with dynamic typing. \
             Great for prototyping but runtime errors are common.",
        ),
        (
            "tenant-b",
            "doc-gamma",
            "Memory safety in systems programming is critical. \
             Rust's borrow checker ensures safety without GC overhead.",
        ),
    ];

    for (tenant, doc_id, text) in corpus {
        let raw = build_raw_record(tenant, doc_id, text);
        let (doc, fingerprint, semantic) = process_pipeline(
            raw,
            PipelineStageConfig::Semantic,
            &ingest_cfg,
            &canonical_cfg,
            Some(&perceptual_cfg),
            Some(&semantic_cfg),
        )?;

        let semantic = semantic.context("semantic embedding")?;

        let rec = IndexRecord {
            schema_version: INDEX_SCHEMA_VERSION,
            canonical_hash: format!("{}-{}", tenant, doc_id),
            perceptual: Some(fingerprint.unwrap().minhash.clone()),
            embedding: Some(UfpIndex::quantize(
                &Array1::from(semantic.vector.clone()),
                127.0,
            )),
            metadata: json!({
                "tenant": tenant,
                "doc_id": doc_id,
                "source": doc.canonical_text.chars().take(80).collect::<String>() + "..."
            }),
        };

        index.upsert(&rec).context("index upsert")?;
        println!("Inserted: {} (tenant={})", rec.canonical_hash, tenant);
    }

    println!("\nSearching for documents similar to: 'Rust ownership model prevents data races...'");

    let query_raw = build_raw_record("query", "q1", "Rust ownership prevents data races");
    let (_query_doc, query_fingerprint, query_semantic) = process_pipeline(
        query_raw,
        PipelineStageConfig::Semantic,
        &ingest_cfg,
        &canonical_cfg,
        Some(&perceptual_cfg),
        Some(&semantic_cfg),
    )?;
    let query_semantic = query_semantic.context("query semantic")?;

    let query = IndexRecord {
        schema_version: INDEX_SCHEMA_VERSION,
        canonical_hash: "query-1".into(),
        perceptual: Some(query_fingerprint.unwrap().minhash.clone()),
        embedding: Some(UfpIndex::quantize(
            &Array1::from(query_semantic.vector.clone()),
            127.0,
        )),
        metadata: json!({}),
    };

    let semantic_hits: Vec<QueryResult> = index
        .search(&query, QueryMode::Semantic, 3)
        .context("semantic search")?;
    let perceptual_hits: Vec<QueryResult> = index
        .search(&query, QueryMode::Perceptual, 3)
        .context("perceptual search")?;

    println!("\n=== Semantic Search Results ===");
    for hit in &semantic_hits {
        println!("  {} (score={:.3})", hit.canonical_hash, hit.score);
    }

    println!("\n=== Perceptual Search Results ===");
    for hit in &perceptual_hits {
        println!("  {} (score={:.3})", hit.canonical_hash, hit.score);
    }

    if let Some(top) = semantic_hits.first() {
        if let Some(stored) = index.get(&top.canonical_hash).context("get stored")? {
            println!("\nTop match metadata: {:?}", stored.metadata);
        }
    }

    println!("\nPipeline complete!");
    Ok(())
}

fn build_raw_record(tenant: &str, doc_id: &str, text: &str) -> RawIngestRecord {
    RawIngestRecord {
        id: format!("{}-{}", tenant, doc_id),
        source: IngestSource::RawText,
        metadata: IngestMetadata {
            tenant_id: Some(tenant.to_string()),
            doc_id: Some(doc_id.to_string()),
            received_at: Some(Utc::now()),
            original_source: None,
            attributes: None,
        },
        payload: Some(IngestPayload::Text(text.to_string())),
    }
}
