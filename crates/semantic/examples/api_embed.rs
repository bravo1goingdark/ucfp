use std::{env, error::Error};

use semantic::{semanticize, SemanticConfig};

/// Demonstrates API-based embedding generation using Hugging Face Inference API.
///
/// This example shows how to generate embeddings via remote API instead of local ONNX runtime.
/// The API mode is useful when you want to:
/// - Offload embedding computation to external services
/// - Use models too large for local inference
/// - Scale embedding generation horizontally
///
/// ## Working API Endpoints
///
/// ### Hugging Face Inference API (Recommended)
/// ```bash
/// UFP_SEMANTIC_API_URL=https://router.huggingface.co/hf-inference/models/BAAI/bge-small-en-v1.5/pipeline/feature-extraction \
/// UFP_SEMANTIC_API_TOKEN=hf_xxx \
/// cargo run -p semantic --example api_embed -- "doc-1" "Some text"
/// ```
///
/// ### Alternative: Hugging Face API Inference
/// ```bash
/// UFP_SEMANTIC_API_URL=https://api-inference.huggingface.co/models/BAAI/bge-small-en-v1.5 \
/// UFP_SEMANTIC_API_TOKEN=hf_xxx \
/// cargo run -p semantic --example api_embed -- "doc-1" "Some text"
/// ```
///
/// ## API Response Format
///
/// The API returns a JSON array of embeddings:
/// ```json
/// [[0.023, -0.045, 0.067, ...]]  // 384 dimensions for bge-small-en-v1.5
/// ```
///
/// ## Environment Variables
///
/// - `UFP_SEMANTIC_API_URL`: The inference API endpoint URL
/// - `UFP_SEMANTIC_API_TOKEN`: Authentication token (Bearer token)
///
/// ## Supported Models
///
/// - BAAI/bge-small-en-v1.5 (384 dim, ~35MB)
/// - BAAI/bge-base-en-v1.5 (768 dim, ~110MB)
/// - BAAI/bge-large-en-v1.5 (1024 dim, ~330MB)
///
/// ## Testing the API
///
/// You can test the API directly with curl:
/// ```bash
/// curl -X POST https://router.huggingface.co/hf-inference/models/BAAI/bge-small-en-v1.5/pipeline/feature-extraction \
///   -H "Authorization: Bearer hf_xxx" \
///   -H "Content-Type: application/json" \
///   -d '{"inputs": "Hello world"}'
/// ```
#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let mut args = env::args().skip(1);
    let doc_id = args.next().unwrap_or_else(|| "api-doc".into());
    let text = args
        .next()
        .unwrap_or_else(|| "Text to embed via API".into());

    let api_url = env::var("UFP_SEMANTIC_API_URL").ok();
    let api_token = env::var("UFP_SEMANTIC_API_TOKEN").ok();

    let mut cfg = SemanticConfig {
        mode: "api".into(),
        api_url: api_url.clone(),
        api_auth_header: api_token.map(|token| format!("Bearer {token}")),
        api_provider: Some("hf".into()),
        api_timeout_secs: Some(60),
        ..SemanticConfig::default()
    };

    if let Some(url) = &api_url {
        println!("🚀 Using remote API for embedding generation");
        println!("   URL: {}", url);
        println!();
    } else {
        cfg.mode = "fast".into();
        cfg.tier = "fast".into();
        println!("⚠️  UFP_SEMANTIC_API_URL env var missing; falling back to deterministic stub.");
        println!("   To use the API:");
        println!();
        println!("   export UFP_SEMANTIC_API_URL=...");
        println!("   export UFP_SEMANTIC_API_TOKEN=...");
        println!();
    }

    let embedding = semanticize(&doc_id, &text, &cfg).await?;
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
