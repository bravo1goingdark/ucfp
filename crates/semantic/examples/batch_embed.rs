use std::{env, error::Error};

use semantic::{semanticize_batch, SemanticConfig};

/// Demonstrates batch embedding generation with automatic stub fallback.
///
/// Usage:
/// ```bash
/// cargo run -p semantic --example batch_embed -- "doc-1" "text one" "doc-2" "text two"
/// ```
#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let mut args = env::args().skip(1);
    let mut docs = Vec::new();

    while let (Some(doc_id), Some(text)) = (args.next(), args.next()) {
        docs.push((doc_id, text));
    }

    if docs.is_empty() {
        docs.push(("doc-a".into(), "Hello from batch embeddings".into()));
        docs.push(("doc-b".into(), "More text to embed".into()));
    }

    let cfg = SemanticConfig {
        tier: "balanced".into(),
        ..SemanticConfig::default()
    };

    let embeddings = semanticize_batch(&docs, &cfg).await?;
    println!("generated {} embeddings", embeddings.len());

    for embedding in embeddings {
        println!(
            "{} => dim={}, normalized={}, head={:?}",
            embedding.doc_id,
            embedding.embedding_dim,
            embedding.normalized,
            &embedding.vector[..embedding.vector.len().min(5)]
        );
    }

    Ok(())
}
