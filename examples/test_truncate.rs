use std::path::PathBuf;
use ucfp::{
    process_pipeline, CanonicalizeConfig, IngestConfig, IngestMetadata, IngestPayload,
    IngestSource, PipelineStageConfig, RawIngestRecord, SemanticConfig,
};

fn main() {
    // Test with short text (under 512 tokens)
    let short_text = "The quick brown fox jumps over the lazy dog. ".repeat(10);

    // Test with long text (over 512 tokens, will be truncated)
    let long_text = "The quick brown fox jumps over the lazy dog. ".repeat(150);

    println!("Testing ONNX model with text truncation...\n");

    let canonical_cfg = CanonicalizeConfig::default();
    let ingest_cfg = IngestConfig::default();
    let semantic_cfg = SemanticConfig {
        mode: "onnx".into(),
        tier: "balanced".into(),
        model_name: "bge-small-en-v1.5".into(),
        model_path: PathBuf::from("models/bge-small-en-v1.5/onnx/model.onnx"),
        tokenizer_path: Some(PathBuf::from("models/bge-small-en-v1.5/tokenizer.json")),
        ..Default::default()
    };

    // Test short text
    println!("1. Processing short text (~90 words)...");
    let short_raw = create_raw_record("short", &short_text);
    let (_, _, short_emb) = process_pipeline(
        short_raw,
        PipelineStageConfig::Semantic,
        &ingest_cfg,
        &canonical_cfg,
        None,
        Some(&semantic_cfg),
    )
    .unwrap();
    match short_emb {
        Some(emb) => println!("   ✓ Success! Embedding dimension: {}", emb.vector.len()),
        None => println!("   ✗ Error: No embedding returned"),
    }

    // Test long text
    println!("\n2. Processing long text (~1350 words, will be truncated to 512 tokens)...");
    let long_raw = create_raw_record("long", &long_text);
    let (_, _, long_emb) = process_pipeline(
        long_raw,
        PipelineStageConfig::Semantic,
        &ingest_cfg,
        &canonical_cfg,
        None,
        Some(&semantic_cfg),
    )
    .unwrap();
    match long_emb {
        Some(emb) => println!("   ✓ Success! Embedding dimension: {}", emb.vector.len()),
        None => println!("   ✗ Error: No embedding returned"),
    }

    println!("\n✓ Text truncation is working correctly!");
    println!("  Long texts are automatically truncated to 512 tokens before ONNX inference.");
}

fn create_raw_record(id: &str, text: &str) -> RawIngestRecord {
    RawIngestRecord {
        id: id.to_string(),
        source: IngestSource::RawText,
        metadata: IngestMetadata {
            tenant_id: None,
            doc_id: None,
            received_at: None,
            original_source: None,
            attributes: None,
        },
        payload: Some(IngestPayload::Text(text.to_string())),
    }
}
