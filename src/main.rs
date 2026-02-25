use anyhow::Context;
use index::{BackendConfig, IndexConfig, IndexRecord, QueryMode, UfpIndex, INDEX_SCHEMA_VERSION};
use ingest::{IngestConfig, IngestMetadata, IngestPayload, IngestSource, RawIngestRecord};
use ndarray::Array1;
use serde_json::json;
use ucfp::{
    process_pipeline, CanonicalizeConfig, CanonicalizedDocument, PerceptualConfig,
    PerceptualFingerprint, PipelineStageConfig, SemanticConfig,
};

mod corpus;

const BATCH_SIZE: usize = 100;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let ingest_cfg = IngestConfig::default();
    let canonical_cfg = CanonicalizeConfig::default();

    let perceptual_cfg = PerceptualConfig {
        k: 3,
        use_parallel: true,
        ..Default::default()
    };

    let semantic_cfg = SemanticConfig {
        tier: "balanced".into(),
        mode: "api".into(),
        api_url: Some("https://router.huggingface.co/hf-inference/models/BAAI/bge-large-en-v1.5/pipeline/feature-extraction".into()),
        api_auth_header: std::env::var("HF_API_TOKEN").ok().map(|t| format!("Bearer {}", t)),
        api_provider: Some("hf".into()),
        api_timeout_secs: Some(30),
        normalize: true,
        ..Default::default()
    };

    let index_cfg = IndexConfig::new().with_backend(BackendConfig::redb("./data/ucfp.redb"));
    let index = UfpIndex::new(index_cfg.clone()).context("index init")?;

    let corpus = corpus::generate_large_corpus();
    let total_docs = corpus.len();

    println!(
        "Processing {} docs | k={} | {} docs/batch",
        total_docs, perceptual_cfg.k, BATCH_SIZE
    );

    let start_time = std::time::Instant::now();
    let mut processed_docs: Vec<(
        &'static str,
        String,
        String,
        PerceptualFingerprint,
        CanonicalizedDocument,
    )> = Vec::with_capacity(total_docs);

    println!("\nPhase 1: Ingest + Canonical + Perceptual...");
    let progress_mult = 100.0 / total_docs as f64;
    for (i, (tenant, doc_id, text)) in corpus.iter().enumerate() {
        let raw = build_raw_record(tenant, doc_id, text);
        let (doc, fingerprint, _) = process_pipeline(
            raw,
            PipelineStageConfig::Perceptual,
            &ingest_cfg,
            &canonical_cfg,
            Some(&perceptual_cfg),
            None,
        )?;
        let fingerprint = fingerprint.context("perceptual fingerprint missing")?;

        processed_docs.push((*tenant, doc_id.clone(), text.clone(), fingerprint, doc));

        let completed = i + 1;
        if completed % 100 == 0 || completed == total_docs {
            println!(
                "  Progress: {}/{} ({:.1}%)",
                completed,
                total_docs,
                completed as f64 * progress_mult
            );
        }
    }

    println!("\nPhase 2: Semantic embedding...");
    let total_batches = total_docs.div_ceil(BATCH_SIZE);
    let mut all_embeddings: Vec<semantic::SemanticEmbedding> = Vec::with_capacity(total_docs);

    for (batch_idx, batch) in processed_docs.chunks(BATCH_SIZE).enumerate() {
        let batch_input: Vec<(&str, &str)> = batch
            .iter()
            .map(|(_, doc_id, _, _, doc)| (doc_id.as_str(), doc.canonical_text.as_str()))
            .collect();

        println!(
            "  Batch {}/{}: {} docs",
            batch_idx + 1,
            total_batches,
            batch_input.len()
        );

        let embeddings = match semantic::semanticize_batch(&batch_input, &semantic_cfg).await {
            Ok(embs) => embs,
            Err(e) => {
                eprintln!("    API failed: {}, using stubs", e);
                batch_input
                    .into_iter()
                    .map(|(doc_id, _)| make_stub_embedding(doc_id, doc_id))
                    .collect()
            }
        };

        all_embeddings.extend(embeddings);
    }

    println!("\nPhase 3: Indexing...");
    for (i, ((tenant, doc_id, text, fingerprint, doc), semantic)) in processed_docs
        .into_iter()
        .zip(all_embeddings.into_iter())
        .enumerate()
    {
        let rec = IndexRecord {
            schema_version: INDEX_SCHEMA_VERSION,
            canonical_hash: doc.sha256_hex,
            perceptual: Some(fingerprint.minhash),
            embedding: Some(UfpIndex::quantize(&Array1::from(semantic.vector), 127.0)),
            metadata: {
                let mut source: String = doc.canonical_text.chars().take(80).collect();
                source.push_str("...");
                json!({
                    "tenant": tenant,
                    "doc_id": doc_id,
                    "full_text": text,
                    "source": source
                })
            },
        };

        index.upsert(&rec).context("index upsert")?;

        let completed = i + 1;
        if completed % 100 == 0 || completed == total_docs {
            println!(
                "  Progress: {}/{} ({:.1}%)",
                completed,
                total_docs,
                completed as f64 * progress_mult
            );
        }
    }

    let elapsed = start_time.elapsed();
    println!(
        "\nIndexed {} documents in {:.2}s ({:.1} docs/sec)",
        total_docs,
        elapsed.as_secs_f64(),
        total_docs as f64 / elapsed.as_secs_f64()
    );
    println!(
        "  Total API calls: {} ({} docs per call)",
        total_batches, BATCH_SIZE
    );

    let query_text = "Rust is a systems programming language with memory safety and ownership";
    println!("\nSearching for: '{}'", query_text);

    let query_raw = build_raw_record("query", "q1", query_text);
    let (query_doc, query_fingerprint, query_embedding) = process_pipeline(
        query_raw,
        PipelineStageConfig::Perceptual,
        &ingest_cfg,
        &canonical_cfg,
        Some(&perceptual_cfg),
        Some(&semantic_cfg),
    )?;

    let query_fingerprint = query_fingerprint.context("query perceptual fingerprint missing")?;
    println!(
        "  Query: {} minhash values",
        query_fingerprint.minhash.len()
    );

    let query_semantic =
        query_embedding.unwrap_or_else(|| make_stub_embedding(&query_doc.sha256_hex, "query"));

    let query = IndexRecord {
        schema_version: INDEX_SCHEMA_VERSION,
        canonical_hash: "query-1".into(),
        perceptual: Some(query_fingerprint.minhash),
        embedding: Some(UfpIndex::quantize(
            &Array1::from(query_semantic.vector),
            127.0,
        )),
        metadata: json!({}),
    };

    let semantic_hits = index
        .search(&query, QueryMode::Semantic, 10)
        .context("semantic search")?;
    let perceptual_hits = index
        .search(&query, QueryMode::Perceptual, 10)
        .context("perceptual search")?;

    print_search_results(&index, "Semantic", &semantic_hits);
    print_search_results(&index, "Perceptual", &perceptual_hits);

    println!("\nFull pipeline complete!");
    println!("Run `perf report --stdio` to view profiling results.");

    Ok(())
}

fn build_raw_record(tenant: &str, doc_id: &str, text: &str) -> RawIngestRecord {
    use chrono::Utc;
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

fn make_stub_embedding(input: &str, doc_id: &str) -> semantic::SemanticEmbedding {
    let mut v = vec![0.0f32; 384];
    let h = input
        .bytes()
        .fold(0u64, |a, b| a.wrapping_mul(31).wrapping_add(b as u64));
    for (i, x) in v.iter_mut().enumerate() {
        *x = ((h + i as u64) % 1000) as f32 / 1000.0;
    }
    semantic::SemanticEmbedding {
        doc_id: doc_id.into(),
        vector: v,
        model_name: "stub".into(),
        tier: "fast".into(),
        embedding_dim: 384,
        normalized: false,
    }
}

fn print_search_results(index: &UfpIndex, label: &str, hits: &[index::QueryResult]) {
    println!("\n=== {} Results ===", label);
    for (rank, hit) in hits.iter().enumerate() {
        println!(
            "\n  {}. {} ({:.3})",
            rank + 1,
            &hit.canonical_hash[..16],
            hit.score
        );
        if let Ok(Some(rec)) = index.get(&hit.canonical_hash) {
            if let Some(t) = rec.metadata.get("full_text").and_then(|v| v.as_str()) {
                println!("     {}", &t[..t.len().min(120)]);
            }
        }
    }
}
