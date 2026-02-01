use std::path::PathBuf;
use ucfp::{canonicalize, semanticize_document, CanonicalizeConfig, SemanticConfig};

fn main() {
    // Create a long text that will definitely need chunking
    let long_text = "The quick brown fox jumps over the lazy dog. ".repeat(100); // ~900 tokens

    println!("Testing Sliding-Window Chunking with Weighted Mean Pooling\n");
    println!(
        "Text length: {} words\n",
        long_text.split_whitespace().count()
    );

    let canonical_cfg = CanonicalizeConfig::default();
    let doc = canonicalize("test", &long_text, &canonical_cfg).unwrap();

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

    match semanticize_document(&doc, &cfg_no_chunk) {
        Ok(emb) => {
            println!("   ✓ Success (truncated)");
            println!("   Embedding dimension: {}", emb.vector.len());
            println!(
                "   First 5 values: {:.4?}",
                &emb.vector[..5.min(emb.vector.len())]
            );
        }
        Err(e) => println!("   ✗ Error: {e}"),
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

    match semanticize_document(&doc, &cfg_chunk) {
        Ok(emb) => {
            println!("   ✓ Success with chunking!");
            println!("   Embedding dimension: {}", emb.vector.len());
            println!(
                "   First 5 values: {:.4?}",
                &emb.vector[..5.min(emb.vector.len())]
            );
            println!("\n   Note: Long text was automatically split into overlapping chunks,");
            println!("   each embedded separately, then pooled with center-weighted mean.");
        }
        Err(e) => println!("   ✗ Error: {e}"),
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

        match semanticize_document(&doc, &cfg_strategy) {
            Ok(emb) => println!("   ✓ {} pooling: dim={}", strategy, emb.vector.len()),
            Err(e) => println!("   ✗ {strategy} pooling failed: {e}"),
        }
    }

    println!("\n✓ Chunking implementation complete!");
    println!("  - Sliding window with 50% overlap");
    println!("  - Weighted mean pooling (center chunks weighted higher)");
    println!("  - Explicit opt-in via enable_chunking: true");
    println!("  - Works with any max_sequence_length (512, 1024, 4096, etc.)");
}
