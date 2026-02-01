use std::path::PathBuf;
use ucfp::{canonicalize, semanticize_document, CanonicalizeConfig, SemanticConfig};

fn main() {
    let text = "The quick brown fox jumps over the lazy dog. ".repeat(100); // ~500 words

    println!("Testing configurable max_sequence_length...\n");
    println!(
        "Text length: {} words (will test with different sequence limits)\n",
        text.split_whitespace().count()
    );

    let canonical_cfg = CanonicalizeConfig::default();
    let doc = canonicalize("test", &text, &canonical_cfg).unwrap();

    // Test 1: Default 512 tokens (BERT-based models)
    println!("1. Default max_sequence_length (512 tokens):");
    let cfg_512 = SemanticConfig {
        mode: "onnx".into(),
        tier: "balanced".into(),
        model_name: "bge-small-en-v1.5".into(),
        model_path: PathBuf::from("models/bge-small-en-v1.5/onnx/model.onnx"),
        tokenizer_path: Some(PathBuf::from("models/bge-small-en-v1.5/tokenizer.json")),
        max_sequence_length: 512, // Default for BERT
        ..Default::default()
    };
    match semanticize_document(&doc, &cfg_512) {
        Ok(emb) => println!(
            "   ✓ Success with 512 token limit. Embedding dim: {}",
            emb.vector.len()
        ),
        Err(e) => println!("   ✗ Error: {e}"),
    }

    // Test 2: Custom 256 tokens (smaller models)
    println!("\n2. Custom max_sequence_length (256 tokens):");
    let cfg_256 = SemanticConfig {
        mode: "onnx".into(),
        tier: "balanced".into(),
        model_name: "bge-small-en-v1.5".into(),
        model_path: PathBuf::from("models/bge-small-en-v1.5/onnx/model.onnx"),
        tokenizer_path: Some(PathBuf::from("models/bge-small-en-v1.5/tokenizer.json")),
        max_sequence_length: 256, // Custom smaller limit
        ..Default::default()
    };
    match semanticize_document(&doc, &cfg_256) {
        Ok(emb) => println!(
            "   ✓ Success with 256 token limit. Embedding dim: {}",
            emb.vector.len()
        ),
        Err(e) => println!("   ✗ Error: {e}"),
    }

    // Test 3: Custom 1024 tokens (if you had a model that supports it)
    // Note: bge-small-en-v1.5 only supports 512, but this demonstrates
    // how you would configure a different model
    println!("\n3. Example: How to configure 1024 tokens for Longformer:");
    println!("   let cfg_1024 = SemanticConfig {{");
    println!("       max_sequence_length: 1024,");
    println!("       model_name: \"longformer-base-4096\".into(),");
    println!("       model_path: PathBuf::from(\"models/longformer/onnx/model.onnx\"),");
    println!("       // ... other config");
    println!("   }};");

    println!("\n✓ max_sequence_length is now configurable!");
    println!("  - Default: 512 tokens (BERT-based models)");
    println!("  - Can be set to any value: 256, 512, 1024, 2048, 4096, etc.");
    println!("  - Allows using models with different context window sizes");
}
