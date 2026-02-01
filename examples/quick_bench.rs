use std::path::PathBuf;
use std::time::Instant;
use ucfp::{
    canonicalize, process_record_with_perceptual, semanticize_document, CanonicalizeConfig,
    IngestMetadata, IngestPayload, IngestSource, PerceptualConfig, RawIngestRecord, SemanticConfig,
};

fn main() {
    let iterations = 100;

    // Test text - roughly 1000 words (will be truncated to 512 tokens for ONNX model)
    let text = "The quick brown fox jumps over the lazy dog. ".repeat(143);

    println!("Running {iterations} iterations with ONNX model (auto-truncation enabled)...\n");

    // Canonical benchmark
    let canonical_cfg = CanonicalizeConfig::default();
    let start = Instant::now();
    for _ in 0..iterations {
        let _ = canonicalize("bench", &text, &canonical_cfg).unwrap();
    }
    let canonical_us = start.elapsed().as_micros() as f64 / iterations as f64;

    // Perceptual benchmark
    let doc = canonicalize("bench", &text, &canonical_cfg).unwrap();
    let tokens: Vec<&str> = doc.tokens.iter().map(|t| t.text.as_str()).collect();
    let perceptual_cfg = PerceptualConfig::default();
    let start = Instant::now();
    for _ in 0..iterations {
        let _ = ucfp::perceptualize_tokens(&tokens, &perceptual_cfg).unwrap();
    }
    let perceptual_us = start.elapsed().as_micros() as f64 / iterations as f64;

    // Pipeline benchmark
    let record = RawIngestRecord {
        id: "bench".into(),
        source: IngestSource::RawText,
        metadata: IngestMetadata {
            tenant_id: None,
            doc_id: None,
            received_at: None,
            original_source: None,
            attributes: None,
        },
        payload: Some(IngestPayload::Text(text.clone())),
    };
    let start = Instant::now();
    for _ in 0..iterations {
        let _ = process_record_with_perceptual(record.clone(), &canonical_cfg, &perceptual_cfg)
            .unwrap();
    }
    let pipeline_us = start.elapsed().as_micros() as f64 / iterations as f64;

    // Semantic benchmark (uses real ONNX model with auto-truncation)
    let semantic_cfg = SemanticConfig {
        mode: "onnx".into(),
        tier: "balanced".into(),
        model_name: "bge-small-en-v1.5".into(),
        model_path: PathBuf::from("models/bge-small-en-v1.5/onnx/model.onnx"),
        tokenizer_path: Some(PathBuf::from("models/bge-small-en-v1.5/tokenizer.json")),
        ..Default::default()
    };
    let start = Instant::now();
    for _ in 0..iterations {
        let _ = semanticize_document(&doc, &semantic_cfg).unwrap();
    }
    let semantic_us = start.elapsed().as_micros() as f64 / iterations as f64;

    println!("Results (average per operation):");
    println!("  Canonical:   {canonical_us:.0} μs");
    println!("  Perceptual:  {perceptual_us:.0} μs");
    println!("  Semantic:    {semantic_us:.0} μs");
    println!("  Pipeline:    {pipeline_us:.0} μs");
    println!("\nPer 1,000 words: {:.1} ms", pipeline_us / 1000.0);
    println!("\nModel: bge-small-en-v1.5 (ONNX)");
    println!("Text: ~1000 words (auto-truncated to 512 tokens for semantic embedding)");
}
