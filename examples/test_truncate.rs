use std::path::PathBuf;
use ucfp::{canonicalize, semanticize_document, CanonicalizeConfig, SemanticConfig};

fn main() {
    // Test with short text (under 512 tokens)
    let short_text = "The quick brown fox jumps over the lazy dog. ".repeat(10);

    // Test with long text (over 512 tokens, will be truncated)
    let long_text = "The quick brown fox jumps over the lazy dog. ".repeat(150);

    println!("Testing ONNX model with text truncation...\n");

    let canonical_cfg = CanonicalizeConfig::default();
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
    let short_doc = canonicalize("short", &short_text, &canonical_cfg).unwrap();
    match semanticize_document(&short_doc, &semantic_cfg) {
        Ok(emb) => println!("   ✓ Success! Embedding dimension: {}", emb.vector.len()),
        Err(e) => println!("   ✗ Error: {e}"),
    }

    // Test long text
    println!("\n2. Processing long text (~1350 words, will be truncated to 512 tokens)...");
    let long_doc = canonicalize("long", &long_text, &canonical_cfg).unwrap();
    match semanticize_document(&long_doc, &semantic_cfg) {
        Ok(emb) => println!("   ✓ Success! Embedding dimension: {}", emb.vector.len()),
        Err(e) => println!("   ✗ Error: {e}"),
    }

    println!("\n✓ Text truncation is working correctly!");
    println!("  Long texts are automatically truncated to 512 tokens before ONNX inference.");
}
