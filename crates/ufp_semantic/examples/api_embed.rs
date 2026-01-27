use std::{env, error::Error};

use ufp_semantic::{semanticize, SemanticConfig};

/// Demonstrates API-based embedding generation.
///
/// Usage:
/// ```bash
/// UFP_SEMANTIC_API_URL=https://api-inference.huggingface.co/models/BAAI/bge-small-en-v1.5 \
/// UFP_SEMANTIC_API_TOKEN=hf_xxx \
/// cargo run -p ufp_semantic --example api_embed -- "doc-1" "Some text"
/// ```
fn main() -> Result<(), Box<dyn Error>> {
    let mut args = env::args().skip(1);
    let doc_id = args.next().unwrap_or_else(|| "api-doc".into());
    let text = args
        .next()
        .unwrap_or_else(|| "Text to embed via API".into());

    let api_url = env::var("UFP_SEMANTIC_API_URL").ok();
    let api_token = env::var("UFP_SEMANTIC_API_TOKEN").ok();

    let mut cfg = SemanticConfig {
        mode: "api".into(),
        api_url,
        api_auth_header: api_token.map(|token| format!("Bearer {token}")),
        api_provider: Some("hf".into()),
        ..SemanticConfig::default()
    };

    if cfg.api_url.is_none() {
        cfg.mode = "fast".into();
        cfg.tier = "fast".into();
        println!("API env vars missing; falling back to deterministic stub.");
    }

    let embedding = semanticize(&doc_id, &text, &cfg)?;
    println!("doc_id: {}", embedding.doc_id);
    println!("model: {}", embedding.model_name);
    println!("tier: {}", embedding.tier);
    println!("dim: {}", embedding.embedding_dim);
    println!("normalized: {}", embedding.normalized);
    println!(
        "first values: {:?}",
        &embedding.vector[..embedding.vector.len().min(8)]
    );

    Ok(())
}
