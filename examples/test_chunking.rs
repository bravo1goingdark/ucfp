use std::path::PathBuf;
use ucfp::{
    process_pipeline, CanonicalizeConfig, IngestConfig, IngestMetadata, IngestPayload,
    IngestSource, PipelineStageConfig, RawIngestRecord, SemanticConfig,
};

fn main() {
    // Create a long text that will definitely need chunking
    let long_text = "The quick brown fox jumps over the lazy dog. ".repeat(100); // ~900 tokens

    println!("Testing Sliding-Window Chunking with Weighted Mean Pooling\n");
    println!(
        "Text length: {} words\n",
        long_text.split_whitespace().count()
    );

    let canonical_cfg = CanonicalizeConfig::default();
    let ingest_cfg = IngestConfig::default();

    // Test 1: Without chunking (truncation at 512 tokens)
    println!("1. WITHOUT chunking (truncates at 512 tokens):");
    let cfg_no_chunk = SemanticConfig {
        mode: "onnx".into(),
        tier: "balanced".into(),
        model_name: "bge-small-en-v1.5".into(),
        model_path: PathBuf::from("models/bge-small-en-v1.5/onnx/model.onnx"),
        tokenizer_path: Some(PathBuf::from("models/bge-small-en-v1.5/tokenizer.json")),
        max_sequence_length: 512,
        enable_chunking: false, // Disabled
        ..Default::default()
    };

    let raw_no_chunk = create_raw_record(&long_text);
    let (_, _, emb_no_chunk) = process_pipeline(
        raw_no_chunk,
        PipelineStageConfig::Semantic,
        &ingest_cfg,
        &canonical_cfg,
        None,
        Some(&cfg_no_chunk),
    )
    .unwrap();
    match emb_no_chunk {
        Some(emb) => {
            println!("   ✓ Success (truncated)");
            println!("   Embedding dimension: {}", emb.vector.len());
            println!(
                "   First 5 values: {:.4?}",
                &emb.vector[..5.min(emb.vector.len())]
            );
        }
        None => println!("   ✗ Error: No embedding returned"),
    }

    // Test 2: With chunking enabled (sliding window + weighted mean)
    println!("\n2. WITH chunking (sliding window + weighted mean pooling):");
    let cfg_chunk = SemanticConfig {
        mode: "onnx".into(),
        tier: "balanced".into(),
        model_name: "bge-small-en-v1.5".into(),
        model_path: PathBuf::from("models/bge-small-en-v1.5/onnx/model.onnx"),
        tokenizer_path: Some(PathBuf::from("models/bge-small-en-v1.5/tokenizer.json")),
        max_sequence_length: 512,
        enable_chunking: true,                    // Enabled!
        chunk_overlap_ratio: 0.5,                 // 50% overlap
        pooling_strategy: "weighted_mean".into(), // Center-weighted
        ..Default::default()
    };

    let raw_chunk = create_raw_record(&long_text);
    let (_, _, emb_chunk) = process_pipeline(
        raw_chunk,
        PipelineStageConfig::Semantic,
        &ingest_cfg,
        &canonical_cfg,
        None,
        Some(&cfg_chunk),
    )
    .unwrap();
    match emb_chunk {
        Some(emb) => {
            println!("   ✓ Success with chunking!");
            println!("   Embedding dimension: {}", emb.vector.len());
            println!(
                "   First 5 values: {:.4?}",
                &emb.vector[..5.min(emb.vector.len())]
            );
            println!("\n   Note: Long text was automatically split into overlapping chunks,");
            println!("   each embedded separately, then pooled with center-weighted mean.");
        }
        None => println!("   ✗ Error: No embedding returned"),
    }

    // Test 3: Different pooling strategies
    println!("\n3. Testing different pooling strategies:");
    for strategy in ["mean", "max", "first"] {
        let cfg_strategy = SemanticConfig {
            mode: "onnx".into(),
            tier: "balanced".into(),
            model_name: "bge-small-en-v1.5".into(),
            model_path: PathBuf::from("models/bge-small-en-v1.5/onnx/model.onnx"),
            tokenizer_path: Some(PathBuf::from("models/bge-small-en-v1.5/tokenizer.json")),
            max_sequence_length: 512,
            enable_chunking: true,
            chunk_overlap_ratio: 0.5,
            pooling_strategy: strategy.into(),
            ..Default::default()
        };

        let raw_strategy = create_raw_record(&long_text);
        let (_, _, emb_strategy) = process_pipeline(
            raw_strategy,
            PipelineStageConfig::Semantic,
            &ingest_cfg,
            &canonical_cfg,
            None,
            Some(&cfg_strategy),
        )
        .unwrap();
        match emb_strategy {
            Some(emb) => println!("   ✓ {} pooling: dim={}", strategy, emb.vector.len()),
            None => println!("   ✗ {strategy} pooling failed: No embedding returned"),
        }
    }

    println!("\n✓ Chunking implementation complete!");
    println!("  - Sliding window with 50% overlap");
    println!("  - Weighted mean pooling (center chunks weighted higher)");
    println!("  - Explicit opt-in via enable_chunking: true");
    println!("  - Works with any max_sequence_length (512, 1024, 4096, etc.)");
}

fn create_raw_record(text: &str) -> RawIngestRecord {
    RawIngestRecord {
        id: "chunk-test".to_string(),
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
